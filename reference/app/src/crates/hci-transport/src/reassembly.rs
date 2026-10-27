//! Splits a byte stream of HCI packets of one type into whole packets.
//!
//! USB delivers events on the interrupt pipe and ACL data on the bulk pipe in
//! transfers that may hold part of a packet (a long event spans several
//! interrupt transfers) or, for ACL, more than one. The packet's own length
//! field tells where it ends.

use btstack_sys::consts::{HCI_ACL_DATA_PACKET, HCI_EVENT_PACKET, HCI_ISO_DATA_PACKET, HCI_SCO_DATA_PACKET};

/// Header length and payload length of an HCI packet of `packet_type`, once
/// enough of the header is there
pub fn packet_length(packet_type: u8, data: &[u8]) -> Option<usize> {
    match packet_type {
        HCI_EVENT_PACKET if data.len() >= 2 => Some(2 + data[1] as usize),
        HCI_ACL_DATA_PACKET if data.len() >= 4 => Some(4 + u16::from_le_bytes([data[2], data[3]]) as usize),
        HCI_SCO_DATA_PACKET if data.len() >= 3 => Some(3 + data[2] as usize),
        HCI_ISO_DATA_PACKET if data.len() >= 4 => Some(4 + (u16::from_le_bytes([data[2], data[3]]) & 0x3FFF) as usize),
        _ => None,
    }
}

/// Collects one packet type's data and hands out whole packets
pub struct Reassembler {
    packet_type: u8,
    buffer: Vec<u8>,
}

impl Reassembler {
    pub fn new(packet_type: u8) -> Reassembler {
        Reassembler { packet_type, buffer: Vec::with_capacity(4096) }
    }

    /// Adds received bytes; calls `deliver` for each packet completed
    pub fn push(&mut self, data: &[u8], mut deliver: impl FnMut(&[u8])) {
        self.buffer.extend_from_slice(data);
        let mut start = 0;
        while let Some(len) = packet_length(self.packet_type, &self.buffer[start..]) {
            if self.buffer.len() - start < len {
                break;
            }
            deliver(&self.buffer[start..start + len]);
            start += len;
        }
        self.buffer.drain(..start);
    }

    /// Drops a partial packet (after a transfer error: what follows cannot
    /// continue it)
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    pub fn pending(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_split_over_transfers() {
        let mut r = Reassembler::new(HCI_EVENT_PACKET);
        let event: Vec<u8> = [0x0E, 20].into_iter().chain(0..20).collect();
        let mut out = Vec::new();
        r.push(&event[..16], |p| out.push(p.to_vec()));
        assert!(out.is_empty());
        r.push(&event[16..], |p| out.push(p.to_vec()));
        assert_eq!(out, vec![event]);
        assert_eq!(r.pending(), 0);
    }

    #[test]
    fn two_acl_packets_in_one_read() {
        let mut r = Reassembler::new(HCI_ACL_DATA_PACKET);
        let a = [0x01, 0x20, 0x02, 0x00, 0xAA, 0xBB];
        let b = [0x01, 0x20, 0x01, 0x00, 0xCC];
        let mut data = a.to_vec();
        data.extend_from_slice(&b);
        data.extend_from_slice(&[0x01, 0x20]); // start of a third
        let mut out = Vec::new();
        r.push(&data, |p| out.push(p.to_vec()));
        assert_eq!(out, vec![a.to_vec(), b.to_vec()]);
        assert_eq!(r.pending(), 2);
    }
}
