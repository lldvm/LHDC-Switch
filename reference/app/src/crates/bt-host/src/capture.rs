//! HCI capture in PacketLogger format (.pklg), for Wireshark and
//! tools/a2dp_decode.
//!
//! Can be started in the middle of a connection: the packets that set up the
//! current connections (Connection Complete, L2CAP signaling, AVDTP
//! signaling) are remembered all the time and written first, so Wireshark
//! still tells the AVDTP media channel from signaling.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use btstack_sys::consts::{
    HCI_ACL_DATA_PACKET, HCI_COMMAND_DATA_PACKET, HCI_EVENT_PACKET, HCI_ISO_DATA_PACKET, HCI_SCO_DATA_PACKET,
    LOG_MESSAGE_PACKET,
};

const PSM_AVDTP: u16 = 0x0019;
const AVDTP_SIGNAL_DELAYREPORT: u8 = 0x0D;
/// Upper bound on remembered setup packets (a few dozen per connection)
const MAX_SETUP_RECORDS: usize = 1000;

fn packetlogger_type(packet_type: u8, incoming: bool) -> Option<u8> {
    Some(match packet_type {
        HCI_COMMAND_DATA_PACKET => 0x00,
        HCI_EVENT_PACKET => 0x01,
        HCI_ACL_DATA_PACKET => {
            if incoming {
                0x03
            } else {
                0x02
            }
        }
        HCI_SCO_DATA_PACKET => {
            if incoming {
                0x09
            } else {
                0x08
            }
        }
        HCI_ISO_DATA_PACKET => {
            if incoming {
                0x0D
            } else {
                0x0C
            }
        }
        LOG_MESSAGE_PACKET => 0xFC,
        _ => return None,
    })
}

/// PacketLogger record: length (4, BE) = 9 + payload, seconds (4, BE),
/// microseconds (4, BE), type (1), payload
fn record(pl_type: u8, payload: &[u8]) -> Vec<u8> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let mut r = Vec::with_capacity(13 + payload.len());
    r.extend_from_slice(&(9 + payload.len() as u32).to_be_bytes());
    r.extend_from_slice(&(now.as_secs() as u32).to_be_bytes());
    r.extend_from_slice(&now.subsec_micros().to_be_bytes());
    r.push(pl_type);
    r.extend_from_slice(payload);
    r
}

fn le16(p: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([p[i], p[i + 1]])
}

#[derive(Clone, Copy)]
struct AvdtpChannel {
    local_cid: u16,
    remote_cid: u16,
}

#[derive(Default)]
struct State {
    file: Option<File>,
    path: PathBuf,
    /// Connection setup records per ACL handle
    setup: Vec<(u16, Vec<u8>)>,
    /// AVDTP channels per handle ([0] = signaling)
    avdtp: HashMap<u16, Vec<AvdtpChannel>>,
    /// L2CAP connection requests waiting for their response: (handle,
    /// direction, identifier) -> (PSM, source CID)
    pending: HashMap<(u16, bool, u8), (u16, u16)>,
}

impl State {
    fn forget_handle(&mut self, handle: u16) {
        self.avdtp.remove(&handle);
        self.pending.retain(|k, _| k.0 != handle);
        self.setup.retain(|(h, _)| *h != handle);
    }

    /// Whether this packet belongs to a connection's setup (and its handle)
    fn track(&mut self, packet_type: u8, incoming: bool, p: &[u8]) -> Option<u16> {
        if packet_type == HCI_EVENT_PACKET {
            // Disconnection Complete: status, handle, reason
            if p.len() >= 6 && p[0] == 0x05 && p[2] == 0x00 {
                self.forget_handle(le16(p, 3) & 0x0FFF);
            }
            // Connection Complete: Wireshark tracks L2CAP channels only
            // within a connection it has seen established
            if p.len() >= 13 && p[0] == 0x03 && p[2] == 0x00 {
                return Some(le16(p, 3) & 0x0FFF);
            }
            return None;
        }
        if packet_type != HCI_ACL_DATA_PACKET || p.len() < 8 {
            return None;
        }
        let handle = le16(p, 0) & 0x0FFF;
        let pb = (p[1] >> 4) & 0x03;
        let acl_len = le16(p, 2) as usize;
        if pb == 0x01 || 4 + acl_len > p.len() {
            return None; // continuation fragment
        }
        let (l2_len, cid) = (le16(p, 4) as usize, le16(p, 6));
        if 8 + l2_len > p.len() {
            return None; // fragmented L2CAP frame
        }
        let d = &p[8..8 + l2_len];
        if cid == 0x0001 {
            let mut off = 0;
            while off + 4 <= d.len() {
                let (code, id, clen) = (d[off], d[off + 1], le16(d, off + 2) as usize);
                if off + 4 + clen > d.len() {
                    break;
                }
                let c = &d[off + 4..off + 4 + clen];
                match code {
                    // Connection Request: PSM, SCID
                    0x02 if clen >= 4 => {
                        self.pending.insert((handle, incoming, id), (le16(c, 0), le16(c, 2)));
                    }
                    // Connection Response: DCID, SCID, result
                    0x03 if clen >= 8 => {
                        let result = le16(c, 4);
                        if result != 1
                            && let Some((psm, _)) = self.pending.remove(&(handle, !incoming, id))
                            && result == 0
                            && psm == PSM_AVDTP
                        {
                            // The request went out from us if the
                            // response comes in
                            let (dcid, scid) = (le16(c, 0), le16(c, 2));
                            let ch = if incoming {
                                AvdtpChannel { local_cid: scid, remote_cid: dcid }
                            } else {
                                AvdtpChannel { local_cid: dcid, remote_cid: scid }
                            };
                            self.avdtp.entry(handle).or_default().push(ch);
                        }
                    }
                    // Disconnection Request: DCID, SCID
                    0x06 if clen >= 4 => {
                        let local = if incoming { le16(c, 0) } else { le16(c, 2) };
                        if let Some(chans) = self.avdtp.get_mut(&handle)
                            && let Some(i) = chans.iter().position(|ch| ch.local_cid == local)
                        {
                            chans.remove(i);
                        }
                    }
                    _ => {}
                }
                off += 4 + clen;
            }
            return Some(handle);
        }
        let sig = *self.avdtp.get(&handle)?.first()?;
        if cid != if incoming { sig.local_cid } else { sig.remote_cid } {
            return None;
        }
        // AVDTP signaling, without the periodic DELAYREPORT
        if d.len() >= 2 && (d[0] >> 2) & 0x03 == 0 && d[1] & 0x3F == AVDTP_SIGNAL_DELAYREPORT {
            return None;
        }
        Some(handle)
    }
}

/// The capture; cheap to clone (shared)
#[derive(Clone, Default)]
pub struct HciCapture {
    state: Arc<Mutex<State>>,
    bytes: Arc<AtomicU64>,
}

impl HciCapture {
    pub fn new() -> HciCapture {
        HciCapture::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Every HCI packet (from the BTstack thread)
    pub fn log_packet(&self, packet_type: u8, incoming: bool, packet: &[u8]) {
        let Some(pl_type) = packetlogger_type(packet_type, incoming) else { return };
        let mut s = self.lock();
        let setup = s.track(packet_type, incoming, packet);
        if setup.is_none() && s.file.is_none() {
            return;
        }
        let rec = record(pl_type, packet);
        if let Some(f) = s.file.as_mut()
            && f.write_all(&rec).is_ok()
        {
            self.bytes.fetch_add(rec.len() as u64, Ordering::Relaxed);
        }
        if let Some(handle) = setup
            && s.setup.len() < MAX_SETUP_RECORDS
        {
            s.setup.push((handle, rec));
        }
    }

    /// A note in the capture (shown by Wireshark as a log message)
    pub fn note(&self, text: &str) {
        let mut s = self.lock();
        if let Some(f) = s.file.as_mut() {
            let rec = record(0xFC, text.as_bytes());
            if f.write_all(&rec).is_ok() {
                self.bytes.fetch_add(rec.len() as u64, Ordering::Relaxed);
            }
        }
    }

    /// Forgets the setup records (a new BTstack run)
    pub fn reset_tracking(&self) {
        let mut s = self.lock();
        s.setup.clear();
        s.avdtp.clear();
        s.pending.clear();
    }

    pub fn start(&self, path: &Path) -> std::io::Result<()> {
        let mut s = self.lock();
        if s.file.is_some() {
            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "a capture is running"));
        }
        let mut f = File::create(path)?;
        let mut written = 0u64;
        if !s.setup.is_empty() {
            let note = format!(
                "A2DPWB: capture started during a connection; {} connection setup packets recorded earlier follow",
                s.setup.len()
            );
            let rec = record(0xFC, note.as_bytes());
            f.write_all(&rec)?;
            written += rec.len() as u64;
            for (_, r) in &s.setup {
                f.write_all(r)?;
                written += r.len() as u64;
            }
        }
        let rec = record(0xFC, b"A2DPWB: live capture starts");
        f.write_all(&rec)?;
        written += rec.len() as u64;
        self.bytes.store(written, Ordering::Relaxed);
        s.file = Some(f);
        s.path = path.to_path_buf();
        tracing::info!("HCI capture: started -> {}", path.display());
        Ok(())
    }

    pub fn stop(&self) {
        let mut s = self.lock();
        let Some(mut f) = s.file.take() else { return };
        let _ = f.write_all(&record(0xFC, b"A2DPWB: capture stopped"));
        let _ = f.flush();
        tracing::info!("HCI capture: stopped ({} bytes) -> {}", self.bytes.load(Ordering::Relaxed), s.path.display());
    }

    pub fn active(&self) -> bool {
        self.lock().file.is_some()
    }

    pub fn path(&self) -> Option<PathBuf> {
        let s = self.lock();
        s.file.as_ref().map(|_| s.path.clone())
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_records_come_first() {
        let cap = HciCapture::new();
        // Connection Complete for handle 0x000B, before the capture starts
        cap.log_packet(HCI_EVENT_PACKET, true, &[0x03, 11, 0, 0x0B, 0x00, 1, 2, 3, 4, 5, 6, 1, 0]);
        // Some unrelated event: not remembered
        cap.log_packet(HCI_EVENT_PACKET, true, &[0x0E, 4, 1, 0x03, 0x0C, 0]);
        let path = std::env::temp_dir().join(format!("a2dpwb-cap-{}.pklg", std::process::id()));
        cap.start(&path).unwrap();
        cap.log_packet(HCI_COMMAND_DATA_PACKET, false, &[0x03, 0x0C, 0x00]);
        cap.stop();
        let data = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        // note, Connection Complete, note, command, note
        let mut types = Vec::new();
        let mut off = 0;
        while off + 13 <= data.len() {
            let len = u32::from_be_bytes(data[off..off + 4].try_into().unwrap()) as usize;
            types.push(data[off + 12]);
            off += 4 + len;
        }
        assert_eq!(types, vec![0xFC, 0x01, 0xFC, 0x00, 0xFC]);
        assert_eq!(off, data.len());
    }
}
