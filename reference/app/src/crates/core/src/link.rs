//! Link arithmetic for the link quality view and the diagnostics — v1's
//! wx_link_quality_dialog.cpp: which ACL packet type the media packets take
//! on an EDR 3 Mbps link, the air time and bandwidth share that means, and
//! the rates of the "Audio sending" step.

use crate::diag::AudioRates;

/// Bluetooth BR/EDR baseband slot
pub const SLOT_SECONDS: f64 = 625e-6;
/// Each L2CAP payload gets a 4-byte L2CAP header in the ACL packet
pub const L2CAP_HEADER_BYTES: f64 = 4.0;

/// An EDR 3 Mbps ACL packet type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AclType {
    pub name: &'static str,
    pub max_bytes: u32,
    pub slots: u32,
}

pub const EDR3_TYPES: [AclType; 3] = [
    AclType { name: "3-DH1", max_bytes: 83, slots: 1 },
    AclType { name: "3-DH3", max_bytes: 552, slots: 3 },
    AclType { name: "3-DH5", max_bytes: 1021, slots: 5 },
];

/// 1021 bytes every 6 slots (3-DH5 and the reply slot): 2178 kbps
pub fn theoretical_max_kbps() -> f64 {
    let t = EDR3_TYPES[2];
    t.max_bytes as f64 * 8.0 / ((t.slots + 1) as f64 * SLOT_SECONDS) / 1000.0
}

/// Slots one ACL payload of `acl_bytes` takes on air, the receiver's reply
/// slot included, and the packet type used (several 3-DH5 packets when it
/// is larger than one carries)
pub fn acl_slots(acl_bytes: u32) -> (&'static str, u32) {
    for t in EDR3_TYPES {
        if acl_bytes <= t.max_bytes {
            return (t.name, t.slots + 1);
        }
    }
    let largest = EDR3_TYPES[2];
    let n = acl_bytes.div_ceil(largest.max_bytes);
    (largest.name, n * (largest.slots + 1))
}

/// What the media stream takes of the link
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Airtime {
    /// Media packet size (L2CAP payload)
    pub avg_packet_bytes: f64,
    pub packet: &'static str,
    pub slots: u32,
    /// Share of the air time (0..1, more when the link cannot carry it)
    pub airtime: f64,
    /// Share of the link's usable rate (0..1, L2CAP headers included)
    pub bandwidth_share: f64,
}

/// The air time of `packets_per_second` packets carrying
/// `bytes_per_second` (L2CAP payload); None while nothing is sent
pub fn airtime(packets_per_second: f64, bytes_per_second: f64) -> Option<Airtime> {
    if packets_per_second <= 0.0 || !packets_per_second.is_finite() || !bytes_per_second.is_finite() {
        return None;
    }
    let avg = bytes_per_second / packets_per_second;
    let acl_bytes = avg + L2CAP_HEADER_BYTES;
    let acl_kbps = packets_per_second * acl_bytes * 8.0 / 1000.0;
    let (packet, slots) = acl_slots((acl_bytes + 0.5) as u32);
    Some(Airtime {
        avg_packet_bytes: avg,
        packet,
        slots,
        airtime: packets_per_second * slots as f64 * SLOT_SECONDS,
        bandwidth_share: acl_kbps / theoretical_max_kbps(),
    })
}

/// The stream's rates from counter deltas over `seconds`
pub fn audio_rates(packets: u64, bytes: u64, capture_frames: u64, seconds: f64) -> AudioRates {
    if seconds <= 0.0 {
        return AudioRates::default();
    }
    AudioRates {
        packets_per_second: packets as f64 / seconds,
        kbps: bytes as f64 * 8.0 / seconds / 1000.0,
        capture_frames_per_second: capture_frames as f64 / seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_types_by_size() {
        assert_eq!(acl_slots(83), ("3-DH1", 2));
        assert_eq!(acl_slots(84), ("3-DH3", 4));
        assert_eq!(acl_slots(552), ("3-DH3", 4));
        assert_eq!(acl_slots(1021), ("3-DH5", 6));
        // Two 3-DH5 packets
        assert_eq!(acl_slots(1022), ("3-DH5", 12));
    }

    #[test]
    fn theoretical_rate_is_v1s() {
        assert!((theoretical_max_kbps() - 2178.13).abs() < 0.01);
    }

    #[test]
    fn ldac_990_airtime() {
        // LDAC 990 kbps at 96 kHz in 1005-byte packets: about 123 a second
        let pps = 123.0;
        let a = airtime(pps, pps * 1005.0).unwrap();
        assert_eq!(a.packet, "3-DH5");
        assert_eq!(a.slots, 6);
        assert!((a.airtime - pps * 6.0 * SLOT_SECONDS).abs() < 1e-9);
        assert!((a.avg_packet_bytes - 1005.0).abs() < 1e-9);
        assert!(a.bandwidth_share > 0.45 && a.bandwidth_share < 0.47, "{}", a.bandwidth_share);
        assert!(airtime(0.0, 0.0).is_none());
    }

    #[test]
    fn rates_over_an_interval() {
        let r = audio_rates(250, 125_000, 96_000, 2.0);
        assert_eq!(r.packets_per_second, 125.0);
        assert_eq!(r.kbps, 500.0);
        assert_eq!(r.capture_frames_per_second, 48_000.0);
        assert_eq!(audio_rates(1, 1, 1, 0.0), AudioRates::default());
    }
}
