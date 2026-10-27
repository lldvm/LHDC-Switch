//! Silence fill timing.
//!
//! Loopback capture of a device that plays nothing delivers nothing: no
//! packets, often not even the buffer event. The A2DP stream must keep
//! flowing all the same (a sink that hears nothing may drop the link or
//! re-buffer when audio returns), so with `fill_silence` the capture thread
//! asks this clock how much silence is due whenever it wakes. Time is passed
//! in (as the offset from the stream start) so the logic is testable.

use std::time::Duration;

#[derive(Debug)]
pub(crate) struct SilenceFill {
    sample_rate: u32,
    /// No packet for this long = the device is silent (~2 periods)
    threshold: Duration,
    /// When the last captured packet arrived (or the stream started)
    last_audio: Duration,
    /// Start of the current silent stretch; None while audio flows
    silent_since: Option<Duration>,
    /// Frames of silence delivered in the current stretch
    emitted: u64,
}

/// At most this much silence in one go. A larger gap means the thread did
/// not run at all (system sleep, debugger): delivering it in full would
/// flood the pipeline with seconds of stale silence, so the excess is
/// dropped and the clock moves on.
const MAX_BURST: Duration = Duration::from_millis(200);

impl SilenceFill {
    /// `period`: the stream's device period; silence starts after two of
    /// them without a packet. `now`: the stream start.
    pub fn new(sample_rate: u32, period: Duration, now: Duration) -> SilenceFill {
        SilenceFill { sample_rate, threshold: period * 2, last_audio: now, silent_since: None, emitted: 0 }
    }

    /// A captured packet arrived at `now`
    pub fn on_audio(&mut self, now: Duration) {
        self.last_audio = now;
        self.silent_since = None;
        self.emitted = 0;
    }

    /// Frames of silence due at `now`: nothing while packets keep arriving,
    /// then, once none has come for the threshold, the whole time since the
    /// last one (so the delivered stream keeps real-time length), and from
    /// then on whatever has elapsed since the previous call.
    pub fn poll(&mut self, now: Duration) -> usize {
        if now.saturating_sub(self.last_audio) < self.threshold {
            return 0;
        }
        let since = *self.silent_since.get_or_insert(self.last_audio);
        let due = frames(now.saturating_sub(since), self.sample_rate);
        let n = due.saturating_sub(self.emitted);
        let max = frames(MAX_BURST, self.sample_rate);
        if n > max {
            // Resynchronise: count everything up to `now` as delivered
            self.emitted = due;
            return max as usize;
        }
        self.emitted = due;
        n as usize
    }
}

fn frames(d: Duration, rate: u32) -> u64 {
    (d.as_nanos() * rate as u128 / 1_000_000_000) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);
    const PERIOD: Duration = Duration::from_millis(10);

    #[test]
    fn nothing_while_audio_flows() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        for i in 1..100u32 {
            let now = PERIOD * i;
            f.on_audio(now);
            assert_eq!(f.poll(now + 5 * MS), 0);
        }
    }

    #[test]
    fn nothing_before_two_periods() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        assert_eq!(f.poll(10 * MS), 0);
        assert_eq!(f.poll(19 * MS), 0);
    }

    #[test]
    fn fills_elapsed_time_from_last_packet() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        f.on_audio(100 * MS);
        assert_eq!(f.poll(110 * MS), 0);
        // 20 ms after the last packet: the whole 20 ms at once
        assert_eq!(f.poll(120 * MS), 960);
        // then whatever elapsed since the previous poll
        assert_eq!(f.poll(130 * MS), 480);
        assert_eq!(f.poll(135 * MS), 240);
        assert_eq!(f.poll(135 * MS), 0);
    }

    #[test]
    fn total_matches_real_time_without_drift() {
        // Wake-ups at odd intervals over 10 s of silence: the frames add up
        // to exactly the elapsed time (no rounding drift), less the part
        // before the threshold, which arrives in the first fill.
        let rate = 44_100;
        let mut f = SilenceFill::new(rate, PERIOD, Duration::ZERO);
        let mut total = 0u64;
        let mut now = Duration::ZERO;
        let mut step = 3u64;
        while now < Duration::from_secs(10) {
            step = step * 7 % 13 + 1; // 1..13 ms
            now += Duration::from_millis(step);
            total += f.poll(now) as u64;
        }
        assert_eq!(total, frames(now, rate));
    }

    #[test]
    fn audio_resets_the_stretch() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        assert_eq!(f.poll(30 * MS), 1440);
        f.on_audio(40 * MS);
        assert_eq!(f.poll(50 * MS), 0);
        assert_eq!(f.poll(60 * MS), 960);
    }

    #[test]
    fn long_gap_is_capped() {
        let mut f = SilenceFill::new(48_000, PERIOD, Duration::ZERO);
        // The thread did not run for 5 s
        assert_eq!(f.poll(Duration::from_secs(5)), 9600);
        // and resumes normally afterwards
        assert_eq!(f.poll(Duration::from_secs(5) + 10 * MS), 480);
    }
}
