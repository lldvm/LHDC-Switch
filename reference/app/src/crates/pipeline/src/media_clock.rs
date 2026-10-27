//! RTP sample clock scheduling, independent of bitrate and packet byte size.
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct MediaClock {
    origin: Option<Instant>,
    previous_ts: u32,
    samples: u64,
}
impl MediaClock {
    pub fn reset(&mut self) { *self = Self::default(); }

    /// Initial 20 ms cushion absorbs capture/encoding bursts. Recover from a
    /// long scheduling stall by shifting the clock instead of burst-sending
    /// all overdue packets. RTP timestamps themselves remain untouched.
    pub fn deadline(&mut self, ts: u32, rate: u32, now: Instant) -> (Instant, bool) {
        let Some(origin) = self.origin else {
            let due = now + Duration::from_millis(20);
            self.origin = Some(due);
            self.previous_ts = ts;
            return (due,false);
        };
        self.samples += u64::from(ts.wrapping_sub(self.previous_ts));
        self.previous_ts = ts;
        // Divide the accumulated sample count, not each packet's rounded
        // duration: 44.1 kHz and timestamp wrap never accumulate rounding drift.
        let offset = Duration::from_nanos((u128::from(self.samples) * 1_000_000_000 / u128::from(rate.max(1))) as u64);
        let due = origin + offset;
        if now.saturating_duration_since(due) > Duration::from_millis(30) {
            self.origin = Some(now);
            self.samples = 0;
            (now,true)
        } else { (due,false) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_clock_is_exact_at_44100_and_across_rtp_wrap() {
        let mut c=MediaClock::default(); let t=Instant::now();
        let start=u32::MAX-100;
        let (first,_)=c.deadline(start,44100,t);
        for n in 1..10000u32 {
            let expected=first+Duration::from_nanos(u64::from(n)*240*1_000_000_000/44100);
            assert_eq!(c.deadline(start.wrapping_add(n*240),44100,expected).0,expected);
        }
    }
    #[test]
    fn capture_bursts_are_spaced_and_long_stalls_rebase() {
        let mut c=MediaClock::default(); let t=Instant::now();
        let first=c.deadline(0,48000,t).0;
        assert_eq!(c.deadline(240,48000,t).0,first+Duration::from_millis(5));
        let late=t+Duration::from_millis(100);
        assert_eq!(c.deadline(480,48000,late),(late,true));
        assert_eq!(c.deadline(720,48000,late),(late+Duration::from_millis(5),false));
        c.reset(); assert_eq!(c.deadline(0,48000,late).0,late+Duration::from_millis(20));
    }
}
