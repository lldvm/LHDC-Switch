// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! Nonblocking L2CAP gate sharing the producer's media-clock deadline.
//! Modified for LHDC Switch 2.4.3: do not append another per-packet wait
//! after USB/controller readiness delays. The producer already paces audio.
use std::time::{Duration,Instant};
use crate::media::MediaPacket;

pub(crate) const MAX_SEND_LATENESS: Duration = Duration::from_millis(120);

pub(crate) fn due(packet: &MediaPacket, now: Instant) -> Instant {
    packet.send_at.unwrap_or(now)
}

/// Preserve a short recoverable backlog, but never keep sending half-second
/// old audio while the full ring rejects fresh audio. Normal HCI credits still
/// bound submission; this does not override controller flow control.
pub(crate) fn expired(packet: &MediaPacket, now: Instant) -> bool {
    packet.send_at.is_some_and(|at| now.saturating_duration_since(at) > MAX_SEND_LATENESS)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controller_wait_does_not_shift_future_media_deadlines() {
        let t=Instant::now();
        let mut p=MediaPacket {sample_rate:48000,audio_us:5000,send_at:Some(t),..Default::default()};
        assert_eq!(due(&p,t),t);
        p.timestamp=240;p.send_at=Some(t+Duration::from_millis(5));
        assert_eq!(due(&p,t),t+Duration::from_millis(5));
        assert_eq!(due(&p,t+Duration::from_millis(25)),t+Duration::from_millis(5));
    }
    #[test]
    fn recovered_stall_drains_without_losing_spacing_or_accumulating_delay() {
        for audio_us in [5000,5442,10000,20000] {
            let t=Instant::now();
            let interval=Duration::from_micros(audio_us);
            let mut actual=t;
            let mut maximum_delay=Duration::ZERO;
            for n in 0..200u32 {
                let scheduled=t+interval*n;
                let p=MediaPacket {audio_us:audio_us as u32,send_at:Some(scheduled),..Default::default()};
                if n==10 {actual=scheduled+Duration::from_millis(80);}
                actual=actual.max(due(&p,actual));
                maximum_delay=maximum_delay.max(actual.saturating_duration_since(scheduled));
                // Model controller/USB readiness, rather than pretending
                // every can-send callback is instantaneous.
                actual+=Duration::from_millis(3);
                if n>100 {assert_eq!(actual,scheduled+Duration::from_millis(3),"{audio_us} us packets must recover after one stall");}
            }
            assert_eq!(maximum_delay,Duration::from_millis(80));
        }
    }
    #[test]
    fn all_packet_durations_avoid_accumulating_timer_and_controller_delays() {
        for rate in [44100,48000,96000,192000] {
          for frames in 1..=4 {
            let samples=frames*if rate<=48000 {240} else {rate/200};
            // Ready callbacks consume 80% of the packet interval: enough
            // transport capacity for this stream, at both low/high bitrates.
            let readiness=Duration::from_nanos(u64::from(samples)*1_000_000_000/u64::from(rate)*4/5);
            let t=Instant::now();
            let mut actual=t;
            let mut old_actual=t;
            let mut old_sent=None;
            let mut max_lag=Duration::ZERO;
            for n in 0..2000u64 {
                let scheduled=t+Duration::from_nanos(n*u64::from(samples)*1_000_000_000/u64::from(rate));
                let packet=MediaPacket {audio_us:(u64::from(samples)*1_000_000/u64::from(rate)) as u32,
                    send_at:Some(scheduled),sample_rate:rate,..Default::default()};
                // A millisecond timer may add another tick when an
                // artificial post-send wait is requested.
                let deadline=due(&packet,actual);
                if deadline>actual {actual=deadline;}
                actual+=readiness;
                max_lag=max_lag.max(actual.duration_since(scheduled));
                let mut old_due=scheduled;
                if let Some(sent)=old_sent {
                    old_due=old_due.max(sent+Duration::from_micros(u64::from(packet.audio_us/2).min(5000)));
                }
                if old_due>old_actual {old_actual=old_due+Duration::from_millis(1);}
                old_actual+=readiness;
                old_sent=Some(old_actual);
            }
            assert_eq!(max_lag,readiness,"{rate} Hz, {frames}-frame packets");
            assert!(old_actual.duration_since(actual)>Duration::from_secs(1),"the old extra gate must reproduce growing backlog");
          }
        }
    }
    #[test]
    fn lateness_budget_preserves_small_stalls_and_expires_only_stale_packets() {
        let t=Instant::now();
        let packet=MediaPacket {send_at:Some(t),..Default::default()};
        assert!(!expired(&packet,t-Duration::from_millis(20)));
        assert!(!expired(&packet,t+Duration::from_millis(80)));
        assert!(!expired(&packet,t+MAX_SEND_LATENESS));
        assert!(expired(&packet,t+MAX_SEND_LATENESS+Duration::from_micros(1)));
        assert!(!expired(&MediaPacket::default(),t+Duration::from_secs(1)));
    }
}
