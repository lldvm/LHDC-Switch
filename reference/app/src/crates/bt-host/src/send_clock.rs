//! Nonblocking L2CAP gate sharing the producer's media-clock deadline.
//! A second RTP clock here used to turn one stall into permanent queue delay.
use std::time::{Duration,Instant};
use crate::media::MediaPacket;
#[derive(Default)]
pub(crate) struct SendClock {
    last_sent: Option<(Instant,u32)>,
}
impl SendClock {
    pub fn due(&mut self, packet: &MediaPacket, now: Instant) -> Instant {
        let mut due=packet.send_at.unwrap_or(now);
        if let Some((sent,audio_us))=self.last_sent {
            // Old packets may catch up at up to 2x media speed (up to 4x
            // for 20 ms LHDC packets), without an instantaneous bulk burst.
            due=due.max(sent+Duration::from_micros(u64::from(audio_us/2).min(5000)));
        }
        due
    }
    pub fn sent(&mut self, packet: &MediaPacket, now: Instant) { self.last_sent=Some((now,packet.audio_us)); }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_wakes_keep_the_same_deadline() {
        let t=Instant::now(); let mut c=SendClock::default();
        let mut p=MediaPacket {sample_rate:48000,audio_us:5000,send_at:Some(t),..Default::default()};
        assert_eq!(c.due(&p,t),t);c.sent(&p,t);
        p.timestamp=240;p.send_at=Some(t+Duration::from_millis(5));
        assert_eq!(c.due(&p,t),t+Duration::from_millis(5));
        assert_eq!(c.due(&p,t),t+Duration::from_millis(5));
    }
    #[test]
    fn recovered_stall_drains_without_losing_spacing_or_accumulating_delay() {
        for audio_us in [5000,10000,20000] {
            let t=Instant::now();let mut c=SendClock::default();
            let interval=Duration::from_micros(audio_us);
            let mut actual=t;
            let mut maximum_delay=Duration::ZERO;
            for n in 0..200u32 {
                let scheduled=t+interval*n;
                let p=MediaPacket {audio_us:audio_us as u32,send_at:Some(scheduled),..Default::default()};
                if n==10 {actual=scheduled+Duration::from_millis(80);}
                actual=actual.max(c.due(&p,actual));
                maximum_delay=maximum_delay.max(actual.saturating_duration_since(scheduled));
                c.sent(&p,actual);
                if n>100 {assert_eq!(actual,scheduled,"{audio_us} us packets must recover after one stall");}
            }
            assert_eq!(maximum_delay,Duration::from_millis(80));
        }
    }
}