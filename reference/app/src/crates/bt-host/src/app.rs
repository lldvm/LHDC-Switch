// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! The application on the BTstack thread ([`btstack_sys::App`]): setup of
//! HCI and the profiles, event handling, the jobs the callers of
//! [`crate::BtHost`] send, and the media sender.
//!
//! Ported from v1's BtStackTransport (btstack_transport.cpp); the comments
//! keep v1's reasons.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use btstack_sys::consts::*;
use btstack_sys::{AclState, App, AvdtpState, HciBackend, HciState, Source, Stack};
use codec::CodecKind;
use codec::caps::{ConfigRequest, Configuration, SbcCapabilities, VendorCapability};
use codec::sbc::ChannelMode;

use crate::events::{addr_at, addr_str, event_type, hci_error_string, le16, le24, subevent, u8_at};
use crate::le::core::LeCore;
use crate::le::iso_send::TIMER_ISO;
use crate::link_keys::FileLinkKeyStore;
use crate::media::{MediaPacket, RTP_HEADER_SIZE};
use crate::shared::Shared;
use crate::stats::LinkStats;
use crate::{AfhClassification, ConnectFailure, Controls, DiscoveredDevice, HostEvent, InitFailure, MediaKey};

pub(crate) type Job = Box<dyn FnOnce(&mut HostApp, &Stack) + Send>;

fn confirmed_volume_reply(p: &[u8], cid: u16) -> Option<u8> {
    if le16(p,3)!=Some(cid) || u8_at(p,5)!=Some(9) { return None; }
    u8_at(p,6).filter(|v| *v<=127)
}

pub(crate) const TIMER_AFH: u32 = 1;
pub(crate) const TIMER_PLAYBACK: u32 = 2;
const TIMER_MEDIA: u32 = 5;

/// One radio reading per tick (RSSI and the AFH map in turn)
const AFH_TICK_MS: u32 = 1000;
/// Read cached Wi-Fi classification; its worker surveys every 30 s.
const AFH_CLASSIFY_EVERY_TICKS: u32 = 1;
const PLAYBACK_TICK_MS: u32 = 1000;
/// No audio for ~2 s = paused
const PLAYBACK_IDLE_TICKS: u32 = 2;
/// Let the player react to a button before the capture check takes over
const PLAYBACK_HOLD: Duration = Duration::from_millis(3000);
/// One volume command at a time; a lost response must not block for good
const VOLUME_RESPONSE_TIMEOUT: Duration = Duration::from_millis(1000);

/// What the BTstack thread needs to set up HCI
pub(crate) struct Setup {
    pub tcp: bool,
    pub link_key_dir: Option<PathBuf>,
    pub firmware_dir: PathBuf,
    pub realtek_pid: u16,
    /// Realtek firmware / config file stems (no extension) of the chip
    pub realtek_files: Option<(String, String)>,
    pub broadcom_ids: Vec<(u16, u16)>,
}

/// LMP subversions Realtek chips report while running their ROM code,
/// before a firmware download (BTstack's btstack_chipset_realtek.c)
/// AVDTP signal identifiers (AVDTP 8.5)
const AVDTP_SI_SET_CONFIGURATION: u8 = 0x03;
const AVDTP_SI_START: u8 = 0x07;

fn is_realtek_rom_lmp(lmp_subver: u16) -> bool {
    matches!(lmp_subver, 0x1200 | 0x8723 | 0x8821 | 0x8761 | 0x8822 | 0x8852 | 0x8851)
}

pub(crate) struct HostApp {
    pub shared: Arc<Shared>,
    pub controls: Arc<Controls>,
    pub stats: Arc<LinkStats>,
    pub jobs: mpsc::Receiver<Job>,
    pub events: mpsc::Sender<HostEvent>,
    pub media: rtrb::Consumer<MediaPacket>,
    /// The encode thread queues packets only while this is set
    pub media_streaming: crate::media::StreamingFlag,
    pub setup: Setup,
    pub afh: Option<Box<dyn crate::AfhSource>>,
    /// LE Audio (experimental): None while off
    pub le: Option<LeCore>,

    // ---- connection (BTstack thread only) ----
    pub a2dp_cid: u16,
    pub avrcp_cid: u16,
    pub local_seid: u8,
    pub remote_seid: u8,
    pub remote_addr: Option<[u8; 6]>,
    pub streaming: bool,
    /// Codec of the configured stream (RTP or not)
    pub stream_codec: Option<CodecKind>,
    pub endpoints: HashMap<CodecKind, u8>,
    pub can_send_requested: bool,
    /// SEP discovery in progress
    sep_sinks: u8,
    sep_sinks_in_use: u8,
    /// Capabilities of the discovery in progress
    caps: codec::caps::RemoteCaps,

    // ---- init / chipset ----
    init_started: Instant,
    rtk_version_seen: bool,
    rtk_lmp_first: u16,
    rtk_verify_pending: bool,
    bcm_checked: bool,

    // ---- AVRCP ----
    volume_in_flight: Option<Instant>,
    playback_status: u8,
    playback_prev_frames: u64,
    playback_idle_ticks: u32,
    playback_hold_until: Instant,

    // ---- AFH ----
    afh_sent_map: Option<[u8; 10]>,
    afh_policy_version_seen: Option<u32>,
    afh_ticks: u32,
    /// The next radio reading is RSSI (else the AFH map). Its own toggle: a
    /// classification sent on a tick must not always skip the same reading
    radio_read_next: u8,
    power_read_max: bool,
    last_media_sent: Option<Instant>,
    media_timer_due: Option<Instant>,
}

impl HostApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shared: Arc<Shared>,
        controls: Arc<Controls>,
        stats: Arc<LinkStats>,
        jobs: mpsc::Receiver<Job>,
        events: mpsc::Sender<HostEvent>,
        media: rtrb::Consumer<MediaPacket>,
        media_streaming: crate::media::StreamingFlag,
        setup: Setup,
        afh: Option<Box<dyn crate::AfhSource>>,
        le: Option<LeCore>,
    ) -> HostApp {
        HostApp {
            shared,
            controls,
            stats,
            jobs,
            events,
            media,
            media_streaming,
            setup,
            afh,
            le,
            a2dp_cid: 0,
            avrcp_cid: 0,
            local_seid: 0,
            remote_seid: 0,
            remote_addr: None,
            streaming: false,
            stream_codec: None,
            endpoints: HashMap::new(),
            can_send_requested: false,
            sep_sinks: 0,
            sep_sinks_in_use: 0,
            caps: Default::default(),
            init_started: Instant::now(),
            rtk_version_seen: false,
            rtk_lmp_first: 0,
            rtk_verify_pending: false,
            bcm_checked: false,
            volume_in_flight: None,
            playback_status: AVRCP_PLAYBACK_STATUS_STOPPED,
            playback_prev_frames: 0,
            playback_idle_ticks: 0,
            playback_hold_until: Instant::now(),
            afh_sent_map: None,
            afh_policy_version_seen: None,
            afh_ticks: 0,
            radio_read_next: 0,
            power_read_max: false,
            last_media_sent: None,
            media_timer_due: None,
        }
    }

    fn init_failed(&self, failure: InitFailure) {
        self.shared.update(|s| {
            if s.init_done.is_none() {
                s.init_done = Some(Err(failure));
            }
        });
    }

    // ======================================================================
    // Setup
    // ======================================================================

    /// hci_init() through hci_power_control(ON). Directly from the init
    /// closure, or once the Intel bootloader download has finished (then the
    /// transport is installed already: `backend` is None).
    pub fn setup_hci_and_power_on(&mut self, stack: &Stack, backend: Option<Box<dyn HciBackend>>) {
        match backend {
            Some(b) => stack.hci_init(b),
            None => stack.hci_init_installed(),
        }
        if self.setup.tcp && self.setup.realtek_pid != 0 {
            // The virtual controller is no Realtek chip, even when a Realtek
            // adapter is plugged in (it would never answer vendor commands)
            tracing::info!("H4 over TCP: ignoring Realtek PID 0x{:04X}", self.setup.realtek_pid);
            self.setup.realtek_pid = 0;
        }
        let pid = self.setup.realtek_pid;
        if pid != 0 {
            tracing::info!("Realtek adapter PID 0x{pid:04X}, firmware folder {}", self.setup.firmware_dir.display());
            // BTstack's Realtek driver builds "${folder}/${name}" without the
            // .bin extension the files have (linux-firmware, our download):
            // make extension-less aliases (hard link, else copy)
            if let Some((fw, cfg)) = self.setup.realtek_files.clone() {
                ensure_alias(&self.setup.firmware_dir, &fw);
                ensure_alias(&self.setup.firmware_dir, &cfg);
            }
            stack.realtek_setup(pid, &self.setup.firmware_dir.to_string_lossy());
            stack.set_chipset_realtek();
        } else {
            tracing::info!("no Realtek PID configured, skipping the Realtek chipset init");
        }

        match &self.setup.link_key_dir {
            Some(dir) => stack.set_link_key_store(Some(Box::new(FileLinkKeyStore::open(dir)))),
            None => stack.set_link_key_store(None),
        }
        // Audio (major 0x04), Hi-Fi audio device (minor 0x28), A2DP source;
        // page timeout ~20 s (0x8000 * 0.625 ms)
        stack.gap_setup(0x200428, 0x8000);
        tracing::info!("LHDC Switch: requested link policy {} (role switch allowed)",
            if std::env::var("LHDC_SWITCH_ACTIVE_LINK").as_deref() == Ok("1") { "0x0001, Sniff disabled" } else { "0x0005, Sniff allowed" });
        // L2CAP before any profile: it registers with HCI for Connection
        // Complete events
        stack.l2cap_init();
        stack.sdp_init();
        stack.add_hci_event_handler();
        stack.a2dp_source_init();
        stack.avrcp_init();
        // Controller category 2 (monitor / amplifier): our CT sends absolute
        // volume; target category 1 (player): the headphones' buttons
        stack.sdp_register_records(
            AVRCP_FEATURE_MASK_CATEGORY_MONITOR_OR_AMPLIFIER,
            AVRCP_FEATURE_MASK_CATEGORY_PLAYER_OR_RECORDER,
        );
        self.register_endpoints(stack);
        // LE Audio (experimental, off by default): SM, GATT client / server,
        // ISO. Nothing of it runs while the setting is off.
        if let Some(le) = self.le.as_mut() {
            le.stack_init(stack);
        }
        // The Realtek driver sends vendor commands before HCI Reset (reading
        // the LMP subversion); without custom pre-init that step runs in the
        // download's slot and the download never happens
        if pid != 0 {
            stack.enable_custom_pre_init();
        }
        self.rtk_version_seen = false;
        self.rtk_lmp_first = 0;
        self.rtk_verify_pending = false;
        self.init_started = Instant::now();
        self.shared.update(|s| s.power_on_at = Some(Instant::now()));
        tracing::info!("hci_init done, powering on");
        if stack.power_on().is_err() {
            tracing::error!("could not open the Bluetooth adapter");
            self.init_failed(InitFailure::Failed);
        }
    }

    fn register_endpoints(&mut self, stack: &Stack) {
        for (slot, ep) in codec::caps::local_endpoints().iter().enumerate() {
            match stack.create_stream_endpoint(slot, ep.codec_type, &ep.caps, &ep.config) {
                Some(seid) => {
                    tracing::debug!("registered the {} endpoint (SEID {seid})", ep.kind);
                    self.endpoints.insert(ep.kind, seid);
                }
                None => tracing::warn!("failed to register the {} endpoint", ep.kind),
            }
        }
    }

    // ======================================================================
    // Jobs (called through BtHost)
    // ======================================================================

    /// a2dp_source_establish_stream; resets the capability discovery
    pub fn establish(&mut self, stack: &Stack, addr: [u8; 6]) -> Result<(), u8> {
        self.remote_addr = Some(addr);
        self.caps = Default::default();
        self.sep_sinks = 0;
        self.sep_sinks_in_use = 0;
        self.stream_codec = None;
        // Packets of a previous stream must not go out on this one
        while self.media.pop().is_ok() {}
        self.can_send_requested = false;
        match stack.a2dp_establish_stream(&addr) {
            Ok(cid) => {
                self.a2dp_cid = cid;
                Ok(())
            }
            Err(status) => Err(status),
        }
    }

    /// Closes an AVDTP connection to `addr` BTstack still knows (left over
    /// when a stream was given up on), which would make
    /// a2dp_source_establish_stream fail with COMMAND_DISALLOWED. Returns
    /// whether a release event follows.
    pub fn release_leftover(&mut self, stack: &Stack, addr: [u8; 6]) -> Option<bool> {
        let (state, cid) = stack.avdtp_state_for_addr(&addr);
        if state == AvdtpState::None {
            return None;
        }
        tracing::info!("closing a leftover AVDTP connection (cid 0x{cid:04x}) first");
        stack.a2dp_disconnect(cid);
        Some(state == AvdtpState::Open)
    }

    /// Tears down a half-open or open AVDTP connection after a failed or
    /// cancelled connect, or for a disconnect. Returns whether a release
    /// event follows.
    pub fn abort(&mut self, stack: &Stack) -> bool {
        if self.a2dp_cid == 0 {
            return false;
        }
        match stack.avdtp_state_for_cid(self.a2dp_cid) {
            // Already finalized by BTstack (e.g. signaling connect failed)
            AvdtpState::None => false,
            // Signaling channel up: normal close, RELEASED follows
            AvdtpState::Open => {
                stack.a2dp_disconnect(self.a2dp_cid);
                true
            }
            // Not open yet (SDP query / waiting for L2CAP): the disconnect
            // emits a failed CONNECTION_ESTABLISHED and frees the connection
            // at once, but a pending outgoing L2CAP channel would be
            // orphaned: drop the ACL too if it is up (never one still
            // paging: no valid handle yet)
            AvdtpState::Pending => {
                stack.a2dp_disconnect(self.a2dp_cid);
                if let Some(addr) = self.remote_addr
                    && let AclState::Open(handle) = stack.acl_state(&addr)
                {
                    stack.disconnect(handle);
                }
                false
            }
        }
    }

    /// After an abort: this was never a live stream
    pub fn forget_connection(&mut self) {
        self.a2dp_cid = 0;
        self.set_streaming(false);
        self.can_send_requested = false;
    }

    /// HCI Disconnect of the ACL link to the remote. True if one is going down.
    pub fn drop_acl(&mut self, stack: &Stack) -> bool {
        let Some(addr) = self.remote_addr else { return false };
        match stack.acl_state(&addr) {
            AclState::Open(handle) => stack.disconnect(handle) == ERROR_CODE_SUCCESS,
            _ => false,
        }
    }

    pub fn set_config(&mut self, stack: &Stack, config: &Configuration) -> u8 {
        let Some(&local) = self.endpoints.get(&codec::caps::endpoint_kind(config.kind)) else {
            return ERROR_CODE_COMMAND_DISALLOWED;
        };
        self.local_seid = local;
        self.remote_seid = config.remote_seid;
        self.stream_codec = Some(config.kind);
        let cid = self.a2dp_cid;
        match &config.request {
            ConfigRequest::Sbc { sampling_frequency, channel_mode, min_bitpool, max_bitpool } => stack
                .a2dp_set_config_sbc(
                    cid,
                    local,
                    config.remote_seid,
                    *sampling_frequency,
                    match channel_mode {
                        ChannelMode::Mono => AVDTP_CHANNEL_MODE_MONO,
                        ChannelMode::DualChannel => AVDTP_CHANNEL_MODE_DUAL_CHANNEL,
                        ChannelMode::Stereo => AVDTP_CHANNEL_MODE_STEREO,
                        ChannelMode::JointStereo => AVDTP_CHANNEL_MODE_JOINT_STEREO,
                    },
                    AVDTP_SBC_BLOCK_LENGTH_16,
                    AVDTP_SBC_SUBBANDS_8,
                    AVDTP_SBC_ALLOCATION_METHOD_LOUDNESS,
                    *min_bitpool,
                    *max_bitpool,
                ),
            ConfigRequest::Aac { sampling_frequency, channels, bit_rate } => stack.a2dp_set_config_aac(
                cid,
                local,
                config.remote_seid,
                AVDTP_AAC_MPEG2_LC,
                *sampling_frequency,
                *channels,
                *bit_rate,
                false,
            ),
            ConfigRequest::Vendor(info) => stack.a2dp_set_config_other(cid, local, config.remote_seid, info),
        }
    }

    pub fn start_stream(&mut self, stack: &Stack) -> u8 {
        if self.a2dp_cid == 0 || self.local_seid == 0 {
            return ERROR_CODE_COMMAND_DISALLOWED;
        }
        stack.a2dp_start_stream(self.a2dp_cid, self.local_seid)
    }

    pub fn pause_stream(&mut self, stack: &Stack) {
        if self.a2dp_cid != 0 && self.local_seid != 0 {
            stack.a2dp_pause_stream(self.a2dp_cid, self.local_seid);
        }
        self.set_streaming(false);
    }

    /// Paging long after the page timeout: the controller lost the Create
    /// Connection, or BTstack never got to send it
    pub fn still_paging(&self, stack: &Stack) -> bool {
        self.remote_addr.is_some_and(|a| stack.acl_state(&a) == AclState::Paging)
    }

    /// Sends HCI Read BD_ADDR for a liveness probe. False if HCI cannot send
    /// a command now (one still waits for its answer).
    pub fn probe(&mut self, stack: &Stack) -> bool {
        if !stack.can_send_command_now() {
            return false;
        }
        stack.cmd_read_bd_addr() == ERROR_CODE_SUCCESS
    }

    /// Sends the queued volume request, if AVRCP is free
    pub fn send_pending_volume(&mut self, stack: &Stack) {
        if self.avrcp_cid == 0 {
            return;
        }
        if self.volume_in_flight.is_some_and(|t| t.elapsed() < VOLUME_RESPONSE_TIMEOUT) {
            return;
        }
        let v = self.controls.volume_request.swap(-1, Ordering::AcqRel);
        if v < 0 {
            return;
        }
        if stack.avrcp_set_absolute_volume(self.avrcp_cid, v as u8) != ERROR_CODE_SUCCESS {
            // Busy with another AVRCP command: keep it unless a newer one came
            let _ = self.controls.volume_request.compare_exchange(-1, v, Ordering::AcqRel, Ordering::Acquire);
            return;
        }
        self.volume_in_flight = Some(Instant::now());
        self.controls.volume.store(v, Ordering::Release);
    }

    // ======================================================================
    // Media
    // ======================================================================

    /// The stream started or stopped. Packets left from before a suspend or
    /// a lost link are stale: dropped before the encode thread may queue
    /// again (v1 flushed after a reconnect).
    fn set_streaming(&mut self, on: bool) {
        if on != self.streaming {
            self.last_media_sent = None;
            self.media_timer_due = None;
        }
        if on && !self.streaming {
            let stale = self.media.slots();
            if stale > 0 {
                if let Ok(chunk) = self.media.read_chunk(stale) {
                    chunk.commit_all();
                }
                tracing::info!("dropped {stale} media packet(s) queued before the stream started");
            }
            self.stats.queue_depth.store(0, Ordering::Relaxed);
        }
        self.streaming = on;
        self.media_streaming.store(on, Ordering::Release);
    }

    fn kick_media(&mut self, stack: &Stack) {
        if self.streaming && self.stream_codec == Some(CodecKind::LhdcV5) {
            crate::media::discard_expired(&mut self.media,&self.stats,Instant::now());
        }
        if self.streaming && !self.can_send_requested && self.a2dp_cid != 0 && !self.media.is_empty() {
            if !self.media_due(stack) { return; }
            self.can_send_requested = true;
            stack.a2dp_request_can_send_now(self.a2dp_cid, self.local_seid);
        }
    }

    fn media_due(&mut self, stack: &Stack) -> bool {
        if self.stream_codec != Some(CodecKind::LhdcV5) { return true; }
        let Ok(packet) = self.media.peek() else { return false; };
        let now=Instant::now();
        let due=crate::send_clock::due(packet,now);
        if due>now {
            // Do not reschedule the same deadline on every USB wake.
            if self.media_timer_due != Some(due) {
                self.media_timer_due=Some(due);
                stack.timer_start(TIMER_MEDIA,due.duration_since(now).as_micros().div_ceil(1000).max(1) as u32);
            }
            false
        } else {
            self.media_timer_due=None;
            stack.timer_stop(TIMER_MEDIA);
            self.stats.pacing_late_max_us.fetch_max(now.duration_since(due).as_micros().min(u32::MAX as u128) as u32,Ordering::Relaxed);
            true
        }
    }

    fn can_send_now(&mut self, stack: &Stack) {
        self.can_send_requested = false;
        if self.streaming && self.stream_codec == Some(CodecKind::LhdcV5) {
            crate::media::discard_expired(&mut self.media,&self.stats,Instant::now());
        }
        if !self.streaming || !self.media_due(stack) { return; }
        if let Ok(packet) = self.media.pop() {
            if let Some(at) = packet.queued_at {
                self.stats.queue_wait_max_us.fetch_max(at.elapsed().as_micros().min(u32::MAX as u128) as u32, Ordering::Relaxed);
            }
            let depth = self.media.slots() as u32;
            if depth == 0 {
                self.stats.queue_audio_us.store(0,Ordering::Relaxed);
                self.stats.queue_wait_us.store(0,Ordering::Relaxed);
            }
            self.stats.queue_depth.store(depth, Ordering::Relaxed);
            // Remote L2CAP MTU minus the RTP header; aptX / aptX LL go
            // without one
            let max = stack
                .a2dp_max_media_payload_size(self.a2dp_cid, self.local_seid)
                .map(|m| if packet.rtp { m as usize } else { m as usize + RTP_HEADER_SIZE });
            let payload = packet.payload();
            if max.is_some_and(|m| payload.len() <= m) {
                let status = if packet.rtp {
                    stack.a2dp_send_media_payload_rtp(self.a2dp_cid, self.local_seid, 0, packet.timestamp, payload)
                } else {
                    stack.a2dp_send_media_packet(self.a2dp_cid, self.local_seid, payload)
                };
                if status == ERROR_CODE_SUCCESS {
                    let now = Instant::now();
                    if let Some(previous) = self.last_media_sent {
                        self.stats.record_send_gap(now.duration_since(previous).as_micros().min(u32::MAX as u128) as u32);
                    }
                    self.last_media_sent = Some(now);
                    self.stats.packets_sent.fetch_add(1, Ordering::Relaxed);
                    let bytes = payload.len() + if packet.rtp { RTP_HEADER_SIZE } else { 0 };
                    self.stats.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
                } else {
                    self.stats.send_errors.fetch_add(1, Ordering::Relaxed);
                }
            } else {
                self.stats.send_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
        // Chain the next one; when empty, the next queued packet wakes us
        self.kick_media(stack);
    }

    // ======================================================================
    // HCI events
    // ======================================================================

    fn handle_hci(&mut self, stack: &Stack, p: &[u8]) {
        let Some(ev) = event_type(p) else { return };
        // Events from the controller (not BTstack's own 0x60..0xFE)
        if ev < 0x60 || ev == HCI_EVENT_VENDOR_SPECIFIC {
            self.shared.update(|s| s.controller_events += 1);
        }
        match ev {
            BTSTACK_EVENT_STATE => match u8_at(p, 2) {
                Some(2) => self.on_working(stack),
                Some(0) => {
                    tracing::info!("HCI state OFF");
                    stack.timer_stop(TIMER_AFH);
                    stack.timer_stop(TIMER_PLAYBACK);
                    self.stats.with_radio(|r| *r = Default::default());
                    self.shared.update(|s| {
                        s.hci_ready = false;
                        s.hci_off = true;
                    });
                }
                _ => {}
            },
            HCI_EVENT_COMMAND_COMPLETE => {
                let opcode = le16(p, 3).unwrap_or(0);
                if opcode == HCI_OPCODE_READ_LOCAL_VERSION_INFORMATION && p.len() >= 14 {
                    self.on_local_version(stack, p);
                }
                if opcode == HCI_OPCODE_READ_BD_ADDR {
                    self.shared.update(|s| {
                        if s.probe_pending {
                            s.probe_pending = false;
                            s.probe_answered = true;
                        }
                    });
                }
                self.on_radio_command_complete(opcode, p);
            }
            HCI_EVENT_USER_CONFIRMATION_REQUEST => {
                // Just Works
                if let Some(addr) = addr_at(p, 2) {
                    tracing::info!("accepting pairing with {}", addr_str(&addr));
                    stack.ssp_confirmation_response(&addr);
                }
            }
            HCI_EVENT_PIN_CODE_REQUEST => {
                // Legacy pairing: the default PIN
                if let Some(addr) = addr_at(p, 2) {
                    stack.pin_code_response(&addr, "0000");
                }
            }
            HCI_EVENT_CONNECTION_COMPLETE => {
                let (Some(status), Some(handle), Some(addr)) = (u8_at(p, 2), le16(p, 3), addr_at(p, 5)) else {
                    return;
                };
                if status != 0 {
                    tracing::warn!(
                        "HCI connection to {} failed: 0x{status:02X} ({})",
                        addr_str(&addr),
                        hci_error_string(status)
                    );
                } else {
                    tracing::info!("HCI ACL connection established to {}", addr_str(&addr));
                    if self.remote_addr == Some(addr) {
                        self.shared.update(|s| {
                            s.target_acl_down_reason = 0;
                            s.target_acl_handle = Some(handle & 0x0FFF);
                        });
                    }
                }
            }
            GAP_EVENT_INQUIRY_RESULT => {
                let (Some(addr), Some(cod), Some(rssi)) = (addr_at(p, 2), le24(p, 9), u8_at(p, 15)) else { return };
                let name = if u8_at(p, 25) == Some(1) {
                    let len = u8_at(p, 26).unwrap_or(0) as usize;
                    p.get(27..27 + len).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default()
                } else {
                    String::new()
                };
                tracing::info!("inquiry result: {} CoD=0x{cod:06X} RSSI={} name={name}", addr_str(&addr), rssi as i8);
                self.shared.update(|s| {
                    if !s.inquiry_done {
                        s.discovered.push(DiscoveredDevice { addr, name, class_of_device: cod, rssi: rssi as i8 });
                    }
                });
            }
            GAP_EVENT_INQUIRY_COMPLETE => {
                self.shared.update(|s| s.inquiry_done = true);
            }
            HCI_EVENT_DISCONNECTION_COMPLETE => {
                let (Some(handle), Some(reason)) = (le16(p, 3), u8_at(p, 5)) else { return };
                let handle = handle & 0x0FFF;
                tracing::info!(
                    "HCI disconnection (handle 0x{handle:04x}, reason 0x{reason:02x}: {})",
                    hci_error_string(reason)
                );
                let lost = self.shared.update(|s| {
                    s.acl_down = true;
                    if s.target_acl_handle == Some(handle) {
                        s.target_acl_down_reason = reason;
                        s.target_acl_handle = None;
                    }
                    // Other links (e.g. a device used before that drops its
                    // idle link a moment later) must not end this stream
                    if let Some(a2dp) = s.a2dp_con_handle
                        && a2dp != handle
                    {
                        tracing::info!("not the A2DP link (handle 0x{a2dp:04x}), stream unaffected");
                        return false;
                    }
                    s.a2dp_con_handle = None;
                    if s.connected || s.streaming {
                        s.connected = false;
                        s.streaming = false;
                        s.disconnect_occurred = true;
                        return true;
                    }
                    false
                });
                if lost {
                    self.set_streaming(false);
                    self.can_send_requested = false;
                    self.stats.queue_depth.store(0, Ordering::Relaxed);
                    tracing::warn!("connection lost");
                    let _ = self.events.send(HostEvent::ConnectionLost);
                }
            }
            HCI_EVENT_A2DP_META => self.handle_a2dp(stack, p),
            HCI_EVENT_AVRCP_META => self.handle_avrcp(stack, p),
            _ => {}
        }
    }

    fn on_working(&mut self, stack: &Stack) {
        let elapsed = self.init_started.elapsed().as_millis();
        tracing::info!("HCI ready (took {elapsed} ms)");
        if self.setup.realtek_pid != 0 {
            if self.rtk_version_seen {
                // Judged by the LMP subversion, as BTstack's Realtek driver
                // does: firmware loaded in an earlier session survives until
                // the adapter loses power, and the download is then skipped
                if !is_realtek_rom_lmp(self.rtk_lmp_first) {
                    tracing::info!(
                        "Realtek firmware already running from an earlier session (LMP subversion 0x{:04X}), download skipped",
                        self.rtk_lmp_first
                    );
                } else if stack.cmd_read_local_version() == ERROR_CODE_SUCCESS {
                    // HCI init read the version only before the download
                    self.rtk_verify_pending = true;
                } else {
                    tracing::info!(
                        "Realtek firmware download ran (ROM LMP subversion 0x{:04X}); not verified",
                        self.rtk_lmp_first
                    );
                }
            } else if elapsed < 500 {
                tracing::warn!("HCI init took < 500 ms: Realtek firmware likely NOT loaded");
            }
        }
        self.shared.update(|s| {
            s.hci_ready = true;
            s.hci_off = false;
            s.init_done = Some(Ok(()));
        });
        self.afh_ticks = 0;
        self.afh_policy_version_seen = None;
        self.afh_sent_map = None;
        stack.timer_start(TIMER_AFH, 200);
    }

    fn on_local_version(&mut self, stack: &Stack, p: &[u8]) {
        // Command Complete: [5] status [6] HCI version [9] LMP version
        // [10..11] manufacturer [12..13] LMP subversion
        if u8_at(p, 5) != Some(0) {
            return;
        }
        let hci_ver = u8_at(p, 6).unwrap_or(0);
        let manufacturer = le16(p, 10).unwrap_or(0);
        let lmp_subver = le16(p, 12).unwrap_or(0);
        tracing::info!(
            "controller: company 0x{manufacturer:04X} ({}), HCI version {hci_ver}, LMP subversion 0x{lmp_subver:04X}",
            crate::chips::company_name(manufacturer).unwrap_or("unknown")
        );
        if manufacturer == COMPANY_ID_REALTEK && self.setup.realtek_pid != 0 {
            // The first read of an init comes before the firmware download
            if !self.rtk_version_seen {
                self.rtk_lmp_first = lmp_subver;
            }
            self.rtk_version_seen = true;
            if self.rtk_verify_pending {
                self.rtk_verify_pending = false;
                if is_realtek_rom_lmp(lmp_subver) {
                    tracing::warn!(
                        "Realtek firmware NOT loaded (LMP subversion 0x{lmp_subver:04X} is the ROM code); check the firmware files"
                    );
                } else {
                    tracing::info!(
                        "Realtek firmware loaded (LMP subversion 0x{:04X} -> 0x{lmp_subver:04X})",
                        self.rtk_lmp_first
                    );
                }
            }
        }
        if matches!(manufacturer, 0x0046 | 0x001D | 0x0045) {
            tracing::warn!(
                "this vendor needs a firmware download that is not implemented; the controller runs its ROM firmware only"
            );
        }
        // Broadcom (Cypress / Infineon map to it inside BTstack): the
        // PatchRAM goes in during custom init, which follows this command;
        // after the upload BTstack resets without re-reading the version
        if self.bcm_checked || self.setup.realtek_pid != 0 || self.setup.tcp {
            return;
        }
        if manufacturer != COMPANY_ID_BROADCOM || stack.hci_state() != HciState::Initializing {
            return;
        }
        self.bcm_checked = true;
        match find_bcm_hcd(&self.setup.firmware_dir, &self.setup.broadcom_ids) {
            Some(path) => {
                tracing::info!("Broadcom PatchRAM {} (experimental)", path.display());
                stack.set_chipset_bcm(&path.to_string_lossy());
            }
            None => tracing::info!(
                "Broadcom controller: no usable .hcd in {}, running ROM firmware",
                self.setup.firmware_dir.display()
            ),
        }
    }

    // ======================================================================
    // A2DP / AVDTP
    // ======================================================================

    fn handle_sep(&mut self, p: &[u8]) {
        if event_type(p) != Some(HCI_EVENT_AVDTP_META) {
            return;
        }
        match subevent(p) {
            Some(AVDTP_SUBEVENT_SIGNALING_SEP_FOUND) => {
                let (seid, in_use, media, sep_type) = (
                    u8_at(p, 5).unwrap_or(0),
                    u8_at(p, 6).unwrap_or(0) != 0,
                    u8_at(p, 7).unwrap_or(0xFF),
                    u8_at(p, 8).unwrap_or(0xFF),
                );
                if sep_type == AVDTP_SINK && media == AVDTP_AUDIO {
                    self.sep_sinks += 1;
                    if in_use {
                        self.sep_sinks_in_use += 1;
                    }
                }
                tracing::info!(
                    "remote SEP {seid}: {} {}{}",
                    if sep_type == AVDTP_SINK { "sink" } else { "source" },
                    if media == AVDTP_AUDIO { "audio" } else { "other" },
                    if in_use { ", IN USE (codec not queried)" } else { "" }
                );
            }
            Some(AVDTP_SUBEVENT_SIGNALING_SEP_DICOVERY_DONE) => {
                // Counted per discovery: a later one replaces the result
                self.caps.audio_sinks = self.sep_sinks;
                self.caps.audio_sinks_in_use = self.sep_sinks_in_use;
                self.sep_sinks = 0;
                self.sep_sinks_in_use = 0;
                if self.caps.audio_sinks_in_use > 0 {
                    tracing::info!(
                        "remote SEPs in use by another connection: {} of {} audio sink SEP(s); their codecs stay unavailable \
                         while the device streams to it (e.g. a phone on a dual connection)",
                        self.caps.audio_sinks_in_use,
                        self.caps.audio_sinks
                    );
                }
            }
            _ => {}
        }
    }

    fn handle_a2dp(&mut self, stack: &Stack, p: &[u8]) {
        match subevent(p) {
            Some(A2DP_SUBEVENT_SIGNALING_CONNECTION_ESTABLISHED) => {
                let (Some(cid), Some(handle), Some(status)) = (le16(p, 3), le16(p, 11), u8_at(p, 13)) else { return };
                if self.a2dp_cid != 0 && cid != self.a2dp_cid {
                    // Late event of an old / aborted connection: must not
                    // clobber the one being established
                    tracing::info!(
                        "ignoring a signaling event of stale cid 0x{cid:04x} (current 0x{:04x})",
                        self.a2dp_cid
                    );
                    return;
                }
                self.a2dp_cid = cid;
                if status != ERROR_CODE_SUCCESS {
                    tracing::warn!("signaling connection failed (0x{status:02x})");
                    // Page timeout: no answer. Connection / L2CAP RTX timeout
                    // or a connection BTstack gave up on: the link came up and
                    // then carried nothing.
                    let failure = match status {
                        ERROR_CODE_PAGE_TIMEOUT => ConnectFailure::NoAnswer,
                        L2CAP_CONNECTION_RESPONSE_RESULT_REFUSED_SECURITY
                        | L2CAP_CONNECTION_PIN_OR_LINK_KEY_MISSING
                        | ERROR_CODE_AUTHENTICATION_FAILURE
                        | ERROR_CODE_PIN_OR_KEY_MISSING => ConnectFailure::AuthFailed,
                        ERROR_CODE_CONNECTION_TIMEOUT
                        | L2CAP_CONNECTION_RESPONSE_RESULT_RTX_TIMEOUT
                        | L2CAP_CONNECTION_BASEBAND_DISCONNECT
                        | ERROR_CODE_UNSPECIFIED_ERROR => ConnectFailure::LinkLost,
                        _ => ConnectFailure::Other,
                    };
                    self.shared.update(|s| {
                        s.last_connect_failure = Some(failure);
                        s.connect_done = Some(false);
                    });
                } else {
                    tracing::info!("signaling connection established (cid 0x{cid:04x}, handle 0x{handle:04x})");
                    // Not done yet: the capability discovery follows
                    self.shared.update(|s| s.a2dp_con_handle = Some(handle & 0x0FFF));
                }
            }
            Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_OTHER_CAPABILITY) => {
                let seid = u8_at(p, 5).unwrap_or(0);
                let len = le16(p, 9).unwrap_or(0) as usize;
                let Some(info) = p.get(11..11 + len) else { return };
                match self.caps.add_vendor(seid, info) {
                    VendorCapability::Taken(kind) => {
                        tracing::info!("remote supports {kind} (SEID {seid}, {info:02X?})")
                    }
                    VendorCapability::Ignored(kind, why) => {
                        tracing::info!("remote supports {kind} (SEID {seid}) - {why}, ignored")
                    }
                    VendorCapability::AptxAdaptive => tracing::info!(
                        "remote supports aptX Adaptive (SEID {seid}) - never selected (no open encoder); \
                         classic aptX is used only if the remote lists it"
                    ),
                    VendorCapability::Unknown { vendor_id, codec_id } => tracing::info!(
                        "remote vendor codec 0x{vendor_id:08X}/0x{codec_id:04X} (SEID {seid}) - unsupported"
                    ),
                    VendorCapability::TooShort => {}
                }
            }
            Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_SBC_CAPABILITY) => {
                let seid = u8_at(p, 5).unwrap_or(0);
                // 12111111111: subevent, cid, SEID, media type, then the
                // bitmaps and the bitpool range
                let at = |i: usize| u8_at(p, i).unwrap_or(0);
                let sbc = SbcCapabilities {
                    sampling_frequency: at(7),
                    channel_mode: at(8),
                    block_length: at(9),
                    subbands: at(10),
                    allocation_method: at(11),
                    min_bitpool: at(12),
                    max_bitpool: at(13),
                };
                self.caps.add_sbc_caps(seid, sbc);
                tracing::info!(
                    "remote supports SBC (SEID {seid}, frequencies 0x{:X}, channel modes 0x{:X}, blocks 0x{:X}, \
                     subbands 0x{:X}, allocation 0x{:X}, bitpool {}-{}){}",
                    sbc.sampling_frequency,
                    sbc.channel_mode,
                    sbc.block_length,
                    sbc.subbands,
                    sbc.allocation_method,
                    sbc.min_bitpool,
                    sbc.max_bitpool,
                    if sbc.allows_xq() { " - SBC XQ possible" } else { "" }
                );
            }
            Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_MPEG_AAC_CAPABILITY) => {
                let seid = u8_at(p, 5).unwrap_or(0);
                self.caps.add_aac(seid);
                tracing::info!("remote supports AAC (SEID {seid})");
            }
            Some(A2DP_SUBEVENT_SIGNALING_CAPABILITIES_COMPLETE) => {
                let c = &self.caps;
                tracing::info!(
                    "capability discovery complete (LDAC={} aptX HD={} aptX LL={} aptX={} aptX Adaptive={} [not encodable] \
                     SBC={} SBC XQ={} AAC={} LHDC V5={} Opus={} Opus 05={} FastStream={}{})",
                    c.ldac.is_some(),
                    c.aptx_hd.is_some(),
                    c.aptx_ll.is_some(),
                    c.aptx.is_some(),
                    c.aptx_adaptive,
                    c.sbc.is_some(),
                    c.has(CodecKind::SbcXq),
                    c.aac.is_some(),
                    c.lhdcv5.is_some(),
                    c.opus.is_some(),
                    c.opus05.is_some(),
                    c.faststream.is_some(),
                    if c.audio_sinks_in_use > 0 {
                        format!(", {} of {} audio sink SEP(s) in use", c.audio_sinks_in_use, c.audio_sinks)
                    } else {
                        String::new()
                    }
                );
                let caps = self.caps.clone();
                self.shared.update(|s| {
                    s.remote_caps = caps;
                    s.connected = true;
                    s.connect_done = Some(true);
                });
                // AVRCP for volume and buttons
                if let Some(addr) = self.remote_addr
                    && let Err(status) = stack.avrcp_connect(&addr)
                {
                    tracing::warn!("AVRCP connect request failed (0x{status:02x})");
                }
            }
            Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_SBC_CONFIGURATION)
            | Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_MPEG_AAC_CONFIGURATION)
            | Some(A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_OTHER_CONFIGURATION) => {
                tracing::info!(
                    "codec configured (local {}, remote {})",
                    u8_at(p, 5).unwrap_or(0),
                    u8_at(p, 6).unwrap_or(0)
                );
            }
            Some(A2DP_SUBEVENT_STREAM_ESTABLISHED) => {
                let status = u8_at(p, 13).unwrap_or(0xFF);
                if status != ERROR_CODE_SUCCESS {
                    tracing::warn!("stream establishment failed (0x{status:02x})");
                    self.shared.update(|s| s.stream_done = Some(false));
                    return;
                }
                self.local_seid = u8_at(p, 11).unwrap_or(0);
                self.remote_seid = u8_at(p, 12).unwrap_or(0);
                let remote_mtu = stack.a2dp_max_media_payload_size(self.a2dp_cid, self.local_seid).unwrap_or(679);
                // Encoders size packets from this; the limit also keeps every
                // payload within a queue slot
                let limit = self.controls.media_payload_limit.load(Ordering::Acquire) as u16;
                let mtu = remote_mtu.min(limit);
                tracing::info!(
                    "stream established (local {}, remote {}, mtu {remote_mtu}, used {mtu}, limit {limit})",
                    self.local_seid,
                    self.remote_seid
                );
                self.shared.update(|s| {
                    s.media_mtu = mtu;
                    s.stream_done = Some(true);
                });
            }
            Some(A2DP_SUBEVENT_STREAM_STARTED) => {
                tracing::info!("streaming started");
                self.stats.mark_stream_start();
                self.set_streaming(true);
                self.can_send_requested = false;
                self.shared.update(|s| {
                    s.streaming = true;
                    s.start_done = Some(true);
                });
                self.kick_media(stack);
            }
            Some(A2DP_SUBEVENT_STREAMING_CAN_SEND_MEDIA_PACKET_NOW) => self.can_send_now(stack),
            Some(A2DP_SUBEVENT_STREAM_SUSPENDED) => {
                tracing::info!("stream suspended");
                self.set_streaming(false);
                self.shared.update(|s| s.streaming = false);
            }
            Some(A2DP_SUBEVENT_STREAM_RELEASED) => {
                tracing::info!("stream released");
                self.set_streaming(false);
                self.shared.update(|s| s.streaming = false);
            }
            Some(A2DP_SUBEVENT_SIGNALING_CONNECTION_RELEASED) => {
                let cid = le16(p, 3).unwrap_or(0);
                if self.a2dp_cid != 0 && cid != self.a2dp_cid {
                    // Late release of an old / aborted connection
                    tracing::info!(
                        "ignoring a signaling release of stale cid 0x{cid:04x} (current 0x{:04x})",
                        self.a2dp_cid
                    );
                    return;
                }
                if self.avrcp_cid != 0 {
                    stack.avrcp_disconnect(self.avrcp_cid);
                    self.avrcp_cid = 0;
                }
                if self.a2dp_cid == 0 {
                    // No current connection: wake a pending disconnect wait,
                    // but report no loss
                    tracing::info!("signaling connection released (cid 0x{cid:04x}, no current connection)");
                    self.shared.update(|s| s.released = true);
                    return;
                }
                tracing::info!("signaling connection released (cid 0x{cid:04x})");
                self.a2dp_cid = 0;
                self.set_streaming(false);
                self.can_send_requested = false;
                self.stats.queue_depth.store(0, Ordering::Relaxed);
                self.shared.update(|s| {
                    s.connected = false;
                    s.streaming = false;
                    s.disconnect_occurred = true;
                    s.released = true;
                });
                let _ = self.events.send(HostEvent::ConnectionLost);
            }
            Some(A2DP_SUBEVENT_COMMAND_REJECTED) => {
                // 12111: cid, local seid, is initiator, signal identifier.
                // A rejected SET_CONFIGURATION / START of ours ends its wait
                // now instead of at the timeout (BTstack reports neither as
                // a failed STREAM_ESTABLISHED / STREAM_STARTED).
                let initiator = u8_at(p, 6).unwrap_or(0) != 0;
                let signal = u8_at(p, 7).unwrap_or(0);
                tracing::warn!(
                    "A2DP command rejected (signal 0x{signal:02x}{})",
                    if initiator { "" } else { ", remote's" }
                );
                if initiator {
                    match signal {
                        AVDTP_SI_SET_CONFIGURATION => self.shared.update(|s| {
                            s.stream_done.get_or_insert(false);
                        }),
                        AVDTP_SI_START => self.shared.update(|s| {
                            s.start_done.get_or_insert(false);
                        }),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // ======================================================================
    // AVRCP
    // ======================================================================

    fn handle_avrcp(&mut self, stack: &Stack, p: &[u8]) {
        match subevent(p) {
            Some(AVRCP_SUBEVENT_CONNECTION_ESTABLISHED) => {
                let (Some(status), Some(cid)) = (u8_at(p, 3), le16(p, 4)) else { return };
                if status != ERROR_CODE_SUCCESS {
                    tracing::warn!("AVRCP connection failed (0x{status:02x})");
                    return;
                }
                if self.a2dp_cid == 0 {
                    // Requested at capability discovery, but A2DP was torn
                    // down before it came up: an orphan AVRCP link keeps the
                    // ACL open and makes the next avrcp_connect fail
                    tracing::info!("AVRCP connected (cid 0x{cid:04x}) after A2DP closed, disconnecting");
                    stack.avrcp_disconnect(cid);
                    return;
                }
                self.avrcp_cid = cid;
                tracing::info!("AVRCP connected (cid 0x{cid:04x})");
                self.volume_in_flight = None;
                // Remembered volume first, while AVRCP is idle, so the audio
                // does not start at the headphones' own (maybe loud) level
                let restore = self.controls.restore_volume.load(Ordering::Acquire);
                if restore >= 0 {
                    tracing::info!("AVRCP: restoring volume {restore} ({}%)", restore * 100 / 127);
                    self.controls.volume_request.store(restore, Ordering::Release);
                    self.send_pending_volume(stack);
                }
                stack.avrcp_enable_notification(cid, AVRCP_NOTIFICATION_EVENT_VOLUME_CHANGED);
                // Target side: buttons arrive as pass-through commands; the
                // playback status lets the headphones pick play or pause
                stack.avrcp_target_support_event(cid, AVRCP_NOTIFICATION_EVENT_PLAYBACK_STATUS_CHANGED);
                self.playback_status = AVRCP_PLAYBACK_STATUS_STOPPED;
                self.playback_prev_frames = self.stats.capture_frames.load(Ordering::Relaxed);
                self.playback_idle_ticks = 0;
                self.playback_hold_until = Instant::now();
                stack.timer_start(TIMER_PLAYBACK, PLAYBACK_TICK_MS);
            }
            Some(AVRCP_SUBEVENT_CONNECTION_RELEASED) => {
                tracing::info!("AVRCP disconnected (cid 0x{:04x})", le16(p, 3).unwrap_or(0));
                stack.timer_stop(TIMER_PLAYBACK);
                self.avrcp_cid = 0;
                // A reconnect restores the volume the headphones had last
                let c = &self.controls;
                let v = c.volume.load(Ordering::Acquire);
                if c.restore_volume.load(Ordering::Acquire) >= 0 && v >= 0 {
                    c.restore_volume.store(v, Ordering::Release);
                }
                c.volume.store(-1, Ordering::Release);
                c.volume_request.store(-1, Ordering::Release);
            }
            Some(AVRCP_SUBEVENT_NOTIFICATION_VOLUME_CHANGED) => {
                let vol = u8_at(p, 6).unwrap_or(0);
                tracing::info!("AVRCP volume changed to {vol} ({}%)", vol as u32 * 100 / 127);
                self.controls.volume.store(vol as i32, Ordering::Release);
                let _ = self.events.send(HostEvent::Volume(vol));
                // AVRCP needs a new registration after each notification
                stack.avrcp_enable_notification(self.avrcp_cid, AVRCP_NOTIFICATION_EVENT_VOLUME_CHANGED);
                self.send_pending_volume(stack);
            }
            Some(AVRCP_SUBEVENT_SET_ABSOLUTE_VOLUME_RESPONSE) => {
                if le16(p,3)!=Some(self.avrcp_cid) { return; }
                self.volume_in_flight = None;
                let Some(vol) = confirmed_volume_reply(p,self.avrcp_cid) else {
                    tracing::warn!("AVRCP: volume write refused or malformed response"); self.send_pending_volume(stack); return;
                };
                tracing::info!("AVRCP absolute volume confirmed: {vol} ({}%)", vol as u32 * 100 / 127);
                self.volume_in_flight = None;
                self.controls.volume.store(vol as i32, Ordering::Release);
                let _ = self.events.send(HostEvent::Volume(vol));
                self.send_pending_volume(stack);
            }
            Some(AVRCP_SUBEVENT_OPERATION) => {
                if le16(p, 3) != Some(self.avrcp_cid) {
                    return;
                }
                let (Some(op), Some(pressed)) = (u8_at(p, 5), u8_at(p, 6)) else { return };
                self.on_button(stack, op, pressed != 0);
            }
            Some(AVRCP_SUBEVENT_PLAY_STATUS_QUERY) => {
                // GetPlayStatus: no track length or position to report
                if let Some(cid) = le16(p, 3) {
                    stack.avrcp_target_play_status(cid, 0xFFFF_FFFF, 0xFFFF_FFFF, self.playback_status);
                }
            }
            _ => {}
        }
    }

    fn on_button(&mut self, stack: &Stack, op: u8, pressed: bool) {
        // A button sends a press and then a release; act on the press
        if !pressed {
            return;
        }
        let key = match op {
            // Windows has one Play/Pause key. Headphones send PLAY or PAUSE
            // by what they believe the state is, which may be stale: a
            // toggle always does what the tap meant.
            AVRCP_OPERATION_ID_PLAY | AVRCP_OPERATION_ID_PAUSE => MediaKey::PlayPause,
            AVRCP_OPERATION_ID_STOP => MediaKey::Stop,
            AVRCP_OPERATION_ID_FORWARD => MediaKey::Next,
            AVRCP_OPERATION_ID_BACKWARD => MediaKey::Previous,
            _ => {
                // FAST_FORWARD / REWIND have no Windows media key; VOLUME_UP /
                // DOWN come from headphones without absolute volume, which
                // set their own level
                tracing::info!("AVRCP button 0x{op:02x} ignored");
                return;
            }
        };
        if !self.controls.media_keys.load(Ordering::Acquire) {
            tracing::info!("AVRCP button {key:?} ignored (media keys off)");
            return;
        }
        tracing::info!("AVRCP button {key:?} -> media key");
        let _ = self.events.send(HostEvent::MediaKey(key));
        // Tell the headphones the state the button asked for right away; the
        // capture check takes over once the player had time to react
        let status = match op {
            AVRCP_OPERATION_ID_PLAY => AVRCP_PLAYBACK_STATUS_PLAYING,
            AVRCP_OPERATION_ID_PAUSE => AVRCP_PLAYBACK_STATUS_PAUSED,
            AVRCP_OPERATION_ID_STOP => AVRCP_PLAYBACK_STATUS_STOPPED,
            _ => return,
        };
        self.set_playback_status(stack, status, "button");
        self.playback_hold_until = Instant::now() + PLAYBACK_HOLD;
        self.playback_idle_ticks = 0;
        self.playback_prev_frames = self.stats.capture_frames.load(Ordering::Relaxed);
    }

    fn set_playback_status(&mut self, stack: &Stack, status: u8, why: &str) {
        if self.avrcp_cid == 0 || status == self.playback_status {
            return;
        }
        let name = |s: u8| match s {
            AVRCP_PLAYBACK_STATUS_STOPPED => "stopped",
            AVRCP_PLAYBACK_STATUS_PLAYING => "playing",
            AVRCP_PLAYBACK_STATUS_PAUSED => "paused",
            _ => "other",
        };
        tracing::info!("AVRCP playback status {} -> {} ({why})", name(self.playback_status), name(status));
        self.playback_status = status;
        stack.avrcp_target_set_playback_status(self.avrcp_cid, status);
    }

    fn playback_tick(&mut self, stack: &Stack) {
        // Loopback capture gets no audio while nothing plays: audio arriving
        // means playing, none for a while means paused
        let frames = self.stats.capture_frames.load(Ordering::Relaxed);
        let audio = frames != self.playback_prev_frames;
        self.playback_prev_frames = frames;
        self.playback_idle_ticks = if audio { 0 } else { self.playback_idle_ticks + 1 };
        if Instant::now() < self.playback_hold_until {
            return;
        }
        if audio {
            self.set_playback_status(stack, AVRCP_PLAYBACK_STATUS_PLAYING, "audio");
        } else if self.playback_idle_ticks >= PLAYBACK_IDLE_TICKS
            && self.playback_status == AVRCP_PLAYBACK_STATUS_PLAYING
        {
            self.set_playback_status(stack, AVRCP_PLAYBACK_STATUS_PAUSED, "no audio");
        }
    }

    // ======================================================================
    // AFH classification and radio readings
    // ======================================================================

    fn afh_tick(&mut self, stack: &Stack) {
        if !self.shared.lock().hci_ready {
            return;
        }
        let version = self.controls.afh_version.load(Ordering::Acquire);
        let classify =
            self.afh_policy_version_seen != Some(version) || self.afh_ticks.is_multiple_of(AFH_CLASSIFY_EVERY_TICKS);
        self.afh_ticks += 1;

        // 1. The classification, when it changes (one command per tick)
        if classify {
            let policy = self.controls.afh_policy();
            let c: Option<AfhClassification> = self.afh.as_mut().and_then(|a| a.classify(&policy));
            self.stats.with_radio(|r| {
                r.afh_usable = c.as_ref().map(|c| c.usable);
                r.afh_avoided = c.as_ref().map(|c| c.avoided.clone()).unwrap_or_default();
            });
            // Nothing to pass and nothing passed before: leave the controller alone
            let changed = match (&c, &self.afh_sent_map) {
                (Some(c), Some(sent)) => &c.map != sent,
                (Some(_), None) => true,
                (None, sent) => sent.is_some(),
            };
            if changed && stack.can_send_command_now() {
                // All channels "unknown" clears the host classification
                let map =
                    c.as_ref().map(|c| c.map).unwrap_or([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
                stack.cmd_set_afh_classification(&map);
                tracing::info!(
                    "AFH host classification: {}",
                    match &c {
                        Some(c) => format!("avoid Wi-Fi {} ({} of 79 channels usable)", c.avoided, c.usable),
                        None => "cleared".into(),
                    }
                );
                self.afh_sent_map = c.map(|c| c.map);
                self.afh_policy_version_seen = Some(version);
                return;
            }
            if !changed {
                self.afh_policy_version_seen = Some(version);
            }
        }

        // 2. Standard read-only radio diagnostics, one command per second.
        let handle = self.shared.lock().a2dp_con_handle;
        let Some(handle) = handle else {
            self.stats.with_radio(|r| {
                r.rssi = None;
                r.afh_in_use = None;
                r.link_quality = None;
                r.tx_power_dbm = None;
                r.tx_power_max_dbm = None;
                r.link_mode = None;
                r.link_policy = None;
            });
            return;
        };
        if !stack.can_send_command_now() {
            return;
        }
        let mode = stack.link_mode(handle);
        self.stats.with_radio(|r| r.link_mode = mode);
        if self.streaming && mode == Some(2) && std::env::var("LHDC_SWITCH_ACTIVE_LINK").as_deref() == Ok("1") {
            let result = stack.exit_sniff(handle);
            tracing::info!("LHDC Switch: requested Active mode from Sniff (status 0x{result:02x})");
            return;
        }
        let kind = self.radio_read_next;
        if stack.radio_read(handle, kind) == ERROR_CODE_SUCCESS {
            if kind == 3 || kind == 4 { self.power_read_max = kind == 4; }
            self.radio_read_next = (kind + 1) % 6;
        }
    }

    fn on_radio_command_complete(&mut self, opcode: u16, p: &[u8]) {
        let rp = p.get(5..).unwrap_or(&[]);
        if let Some(handle) = self.shared.lock().a2dp_con_handle {
            self.stats.with_radio(|r| r.read_status(opcode,rp,handle,self.power_read_max));
        }
        match opcode {
            HCI_OPCODE_SET_AFH_HOST_CHANNEL_CLASSIFICATION => {
                if rp.first().is_some_and(|&s| s != ERROR_CODE_SUCCESS) {
                    tracing::warn!("AFH host classification refused (0x{:02x})", rp[0]);
                }
            }
            HCI_OPCODE_READ_RSSI if rp.len() >= 4 => {
                let ok = rp[0] == ERROR_CODE_SUCCESS;
                self.stats.with_radio(|r| r.rssi = ok.then_some(rp[3] as i8));
            }
            HCI_OPCODE_READ_AFH_CHANNEL_MAP if rp.len() >= 14 => {
                // status, handle, mode, map[10]
                let in_use = if rp[0] != ERROR_CODE_SUCCESS {
                    None
                } else if rp[3] == 1 {
                    Some(rp[4..14].iter().map(|b| b.count_ones()).sum::<u32>().min(79))
                } else {
                    Some(79)
                };
                self.stats.with_radio(|r| r.afh_in_use = in_use);
            }
            _ => {}
        }
    }
}

impl App for HostApp {
    fn on_packet(&mut self, stack: &Stack, source: Source, packet_type: u8, _channel: u16, packet: &[u8]) {
        if packet_type != HCI_EVENT_PACKET {
            return;
        }
        match source {
            Source::Hci => {
                self.handle_hci(stack, packet);
                if let Some(le) = self.le.as_mut() {
                    le.on_hci(stack, packet);
                }
            }
            Source::A2dp => self.handle_a2dp(stack, packet),
            Source::AvdtpSink => self.handle_sep(packet),
            Source::Avrcp | Source::AvrcpController | Source::AvrcpTarget => {
                if event_type(packet) == Some(HCI_EVENT_AVRCP_META) {
                    self.handle_avrcp(stack, packet)
                }
            }
            // LE (only after Stack::le_init)
            Source::Sm => {
                if let Some(le) = self.le.as_mut() {
                    le.on_sm(stack, packet);
                }
            }
            Source::GattClient => {
                if let Some(le) = self.le.as_mut() {
                    le.on_gatt(stack, packet);
                }
            }
            Source::AttServer => {
                if let Some(le) = self.le.as_mut() {
                    le.on_att_event(packet);
                }
            }
            Source::Iso => {
                if let Some(le) = self.le.as_mut() {
                    le.on_iso(stack, packet);
                }
            }
        }
    }

    fn on_wake(&mut self, stack: &Stack) {
        while let Ok(job) = self.jobs.try_recv() {
            job(self, stack);
        }
        if stack.transport_failed() {
            let first = self.shared.update(|s| !std::mem::replace(&mut s.stalled, true));
            if first {
                let _ = self.events.send(HostEvent::TransportFailed);
            }
        }
        self.kick_media(stack);
    }

    fn on_timer(&mut self, stack: &Stack, id: u32) {
        match id {
            TIMER_MEDIA => { self.media_timer_due=None; self.kick_media(stack); }
            TIMER_AFH => {
                self.afh_tick(stack);
                stack.timer_start(TIMER_AFH, AFH_TICK_MS);
            }
            TIMER_PLAYBACK => {
                self.playback_tick(stack);
                stack.timer_start(TIMER_PLAYBACK, PLAYBACK_TICK_MS);
            }
            TIMER_LE => {
                if let Some(le) = self.le.as_mut() {
                    le.on_timer(stack);
                }
            }
            TIMER_ISO => {
                if let Some(le) = self.le.as_mut() {
                    le.on_iso_timer(stack);
                }
            }
            _ => {}
        }
    }

    fn on_intel_firmware(&mut self, stack: &Stack, result: i32) {
        if result != 0 {
            tracing::error!(
                "Intel firmware download failed ({result}). Place the matching ibt-*.sfi and ibt-*.ddc \
                 (linux-firmware intel/) in {}. Newer TLV-mode controllers (AX210 and later) are not supported by BTstack yet.",
                self.setup.firmware_dir.display()
            );
            self.init_failed(InitFailure::Failed);
            return;
        }
        tracing::info!("Intel controller operational");
        self.setup_hci_and_power_on(stack, None);
    }

    fn on_exit(&mut self, stack: &Stack) {
        self.set_streaming(false);
        if let Some(le) = self.le.as_mut() {
            le.stack_deinit(stack);
        }
    }

    fn on_att_read(&mut self, stack: &Stack, con: u16, handle: u16, offset: u16) -> Result<Vec<u8>, u8> {
        match self.le.as_mut() {
            Some(le) => le.att_read(stack, con, handle, offset),
            None => Err(ATT_ERROR_READ_NOT_PERMITTED),
        }
    }

    fn on_att_write(&mut self, stack: &Stack, write: &btstack_sys::AttWriteRequest) -> u8 {
        match self.le.as_mut() {
            Some(le) => le.att_write(stack, write),
            None => ATT_ERROR_WRITE_NOT_PERMITTED,
        }
    }
}

/// BTstack's Realtek driver opens "${folder}/${stem}" (no extension):
/// create it from "${stem}.bin", and replace one that differs from it (a
/// copy of an older file, or a hard link to a .bin replaced since: BTstack
/// would upload the old one). The sizes go to the log, for telling why the
/// firmware was not loaded.
fn ensure_alias(dir: &Path, stem: &str) {
    if stem.is_empty() {
        return;
    }
    let alias = dir.join(stem);
    let source = dir.join(format!("{stem}.bin"));
    let alias_data = std::fs::read(&alias).ok();
    let Ok(source_data) = std::fs::read(&source) else {
        match alias_data {
            Some(a) => tracing::info!("{stem}.bin missing, using {} ({} bytes)", alias.display(), a.len()),
            None => tracing::warn!("firmware file missing: {}", source.display()),
        }
        return;
    };
    tracing::info!("{stem}.bin: {} bytes", source_data.len());
    if let Some(a) = alias_data {
        if a == source_data {
            return;
        }
        tracing::info!("{stem} ({} bytes) differs from {stem}.bin, replacing it", a.len());
        if let Err(e) = std::fs::remove_file(&alias) {
            tracing::error!("could not delete {}: {e}", alias.display());
            return;
        }
    }
    if std::fs::hard_link(&source, &alias).is_ok() {
        tracing::info!("linked {stem} -> {}", source.display());
    } else if let Err(e) = std::fs::copy(&source, &alias) {
        tracing::error!("could not create {}: {e}", alias.display());
    }
}

/// A Broadcom .hcd from the firmware folder: linux-firmware brcm/ names them
/// <chip>-<vid>-<pid>.hcd; prefer the one for a present adapter, take a lone
/// .hcd as meant for it
fn find_bcm_hcd(dir: &Path, adapters: &[(u16, u16)]) -> Option<PathBuf> {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("hcd")))
        .collect();
    for (vid, pid) in adapters {
        let tag = format!("-{vid:04x}-{pid:04x}.");
        if let Some(f) =
            files.iter().find(|f| f.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&tag)))
        {
            return Some(f.clone());
        }
    }
    if files.len() == 1 {
        return Some(files[0].clone());
    }
    if !files.is_empty() {
        tracing::info!("{} .hcd files but none named for the adapter's VID:PID - skipping PatchRAM", files.len());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::ensure_alias;
    #[test]
    fn absolute_volume_confirmation_requires_valid_accepted_reply() {
        let valid=[0xc0,5,0x10,1,0,9,47];
        assert_eq!(super::confirmed_volume_reply(&valid,1),Some(47));
        assert_eq!(super::confirmed_volume_reply(&valid,2),None);
        assert_eq!(super::confirmed_volume_reply(&valid[..6],1),None);
        let mut reject=valid; reject[5]=10;
        assert_eq!(super::confirmed_volume_reply(&reject,1),None);
        reject[5]=9;reject[6]=200;
        assert_eq!(super::confirmed_volume_reply(&reject,1),None);
    }

    #[test]
    fn firmware_alias_follows_the_bin() {
        let dir = std::env::temp_dir().join(format!("a2dpwb_alias_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (alias, bin) = (dir.join("rtl8761bu_fw"), dir.join("rtl8761bu_fw.bin"));

        ensure_alias(&dir, "rtl8761bu_fw"); // neither there: nothing to do
        assert!(!alias.exists());
        std::fs::write(&bin, b"new firmware").unwrap();
        ensure_alias(&dir, "rtl8761bu_fw");
        assert_eq!(std::fs::read(&alias).unwrap(), b"new firmware");

        // An alias left from an older file is replaced
        std::fs::remove_file(&alias).unwrap();
        std::fs::write(&alias, b"old").unwrap();
        ensure_alias(&dir, "rtl8761bu_fw");
        assert_eq!(std::fs::read(&alias).unwrap(), b"new firmware");

        // Without the .bin the alias stays
        std::fs::remove_file(&bin).unwrap();
        ensure_alias(&dir, "rtl8761bu_fw");
        assert_eq!(std::fs::read(&alias).unwrap(), b"new firmware");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
