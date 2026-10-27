// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! Link statistics: cumulative counters written by the audio threads and the
//! BTstack thread without locks, read a few times a second by core (which
//! works with deltas, so nothing is ever reset).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Capacity of the media send queue, in packets (as v1)
pub const MEDIA_QUEUE_PACKETS: usize = 64;

#[derive(Debug, Default)]
pub struct LinkStats {
    /// Media packets handed to L2CAP and their L2CAP payload bytes (RTP
    /// header, media payload header and codec data)
    pub packets_sent: AtomicU64,
    pub bytes_sent: AtomicU64,
    /// Packets lost before the radio: send queue full / L2CAP refused
    pub queue_drops: AtomicU64,
    pub send_errors: AtomicU64,
    /// PCM frames captured from Windows (real audio, not silence fill), and
    /// those dropped because the encoder fell behind
    pub capture_frames: AtomicU64,
    pub capture_dropped_frames: AtomicU64,
    /// Silence frames inserted while nothing played
    pub silence_frames: AtomicU64,
    /// Instantaneous send queue fill (packets)
    pub queue_depth: AtomicU32,
    /// Live encoder target, updated by the encode thread (LHDC Switch).
    pub encoder_kbps: AtomicU32,
    pub queue_audio_us: AtomicU32,
    pub queue_wait_us: AtomicU32,
    /// Time from queue insertion to L2CAP submission (host only).
    pub queue_wait_max_us: AtomicU32,
    pub pacing_late_max_us: AtomicU32,
    pub pacing_rebases: AtomicU64,
    /// Largest host-side media send interval in this stream; not RF loss.
    pub send_gap_max_us: AtomicU32,
    pub send_gaps_30ms: AtomicU64,
    pub capture_discontinuities: AtomicU64,
    /// The counters above when the current stream started; stream_starts
    /// changes with every stream (written after them)
    pub start_packets: AtomicU64,
    pub start_bytes: AtomicU64,
    pub start_dropped: AtomicU64,
    pub start_capture_frames: AtomicU64,
    pub start_capture_dropped: AtomicU64,
    pub stream_starts: AtomicU32,
    /// Radio readings of the link (BTstack thread, every few seconds)
    pub radio: Mutex<LinkRadio>,
}

impl LinkStats {
    pub(crate) fn mark_stream_start(&self) {
        self.queue_audio_us.store(0,Ordering::Relaxed);
        self.queue_wait_us.store(0,Ordering::Relaxed);
        self.queue_wait_max_us.store(0,Ordering::Relaxed);
        self.pacing_late_max_us.store(0,Ordering::Relaxed);
        self.pacing_rebases.store(0,Ordering::Relaxed);
        self.send_gap_max_us.store(0,Ordering::Relaxed);
        self.send_gaps_30ms.store(0,Ordering::Relaxed);
        self.capture_discontinuities.store(0,Ordering::Relaxed);
        let r = Ordering::Relaxed;
        self.start_packets.store(self.packets_sent.load(r), r);
        self.start_bytes.store(self.bytes_sent.load(r), r);
        self.start_dropped.store(self.queue_drops.load(r) + self.send_errors.load(r), r);
        self.start_capture_frames.store(self.capture_frames.load(r), r);
        self.start_capture_dropped.store(self.capture_dropped_frames.load(r), r);
        self.stream_starts.fetch_add(1, Ordering::Release);
    }

    pub fn radio(&self) -> LinkRadio {
        self.radio.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub(crate) fn record_send_gap(&self, micros: u32) {
        self.send_gap_max_us.fetch_max(micros,Ordering::Relaxed);
        if micros >= 30_000 { self.send_gaps_30ms.fetch_add(1,Ordering::Relaxed); }
    }

    pub(crate) fn with_radio(&self, f: impl FnOnce(&mut LinkRadio)) {
        f(&mut self.radio.lock().unwrap_or_else(|e| e.into_inner()));
    }
}

/// Radio state of the current link
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkRadio {
    pub link_quality: Option<u8>,
    pub tx_power_dbm: Option<i8>,
    pub tx_power_max_dbm: Option<i8>,
    pub link_mode: Option<u8>,
    pub link_policy: Option<u16>,
    /// BR/EDR RSSI: relative to the golden receive range, not dBm
    pub rssi: Option<i8>,
    /// Channels the host classification leaves usable (None = none passed)
    pub afh_usable: Option<u32>,
    /// Wi-Fi channels passed as bad, "6, 11"
    pub afh_avoided: String,
    /// Channels the link hops over (Read AFH Channel Map)
    pub afh_in_use: Option<u32>,
}

impl LinkRadio {
    /// Standard HCI return parameters; a failed/unsupported read stays unknown.
    pub(crate) fn read_status(&mut self, opcode: u16, rp: &[u8], handle: u16, max_power: bool) {
        if rp.len() >= 3 && u16::from_le_bytes([rp[1],rp[2]]) != handle { return; }
        let ok = rp.first() == Some(&0) && rp.len() >= 4;
        match opcode {
            0x1403 => self.link_quality = ok.then(|| rp[3]),
            0x0c2d => {
                let power = (ok && rp[3] != 127).then(|| rp[3] as i8);
                if max_power { self.tx_power_max_dbm=power; } else { self.tx_power_dbm=power; }
            },
            0x080c => self.link_policy = (ok && rp.len() >= 5).then(|| u16::from_le_bytes([rp[3],rp[4]])),
            _ => {},
        }
    }
}

#[cfg(test)]
mod radio_tests {
    use super::*;
    #[test]
    fn send_timing_gaps_do_not_claim_packet_loss() {
        let s=LinkStats::default(); s.record_send_gap(10_000);s.record_send_gap(45_000);s.record_send_gap(12_000);
        assert_eq!(s.send_gap_max_us.load(Ordering::Relaxed),45_000);
        assert_eq!(s.send_gaps_30ms.load(Ordering::Relaxed),1);
        assert_eq!(s.queue_drops.load(Ordering::Relaxed),0);
        s.mark_stream_start();assert_eq!(s.send_gaps_30ms.load(Ordering::Relaxed),0);
    }
    #[test]
    fn reads_signed_power_and_quality_without_inventing_unknown_values() {
        let mut r=LinkRadio::default();
        r.read_status(0x0c2d,&[0,0,1,(-7i8) as u8],0x100,false);
        r.read_status(0x0c2d,&[0,0,1,10],0x100,true);
        assert_eq!((r.tx_power_dbm,r.tx_power_max_dbm),(Some(-7),Some(10)));
        r.read_status(0x1403,&[0,0,1,0],0x100,false); assert_eq!(r.link_quality,Some(0));
        r.read_status(0x1403,&[1],0x100,false); assert_eq!(r.link_quality,None);
        r.read_status(0x0c2d,&[0,0,1,127],0x100,false); assert_eq!(r.tx_power_dbm,None);
    }
    #[test]
    fn ignores_other_handles_and_truncated_policy() {
        let mut r=LinkRadio::default();
        r.read_status(0x1403,&[0,1,1,255],0x100,false); assert_eq!(r.link_quality,None);
        r.read_status(0x080c,&[0,0,1,1,0],0x100,false); assert_eq!(r.link_policy,Some(1));
        r.read_status(0x080c,&[0,0,1,1],0x100,false); assert_eq!(r.link_policy,None);
    }
}
