// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! The messages between a2dpwb-core.exe (server) and the UI (client).
//!
//! 1. The client sends [`ClientMessage::Hello`]; a different major protocol
//!    version is answered with [`ServerMessage::Reject`].
//! 2. The server sends [`ServerMessage::Welcome`] and a
//!    [`ServerMessage::Snapshot`] of its whole state with a revision.
//! 3. Then [`ServerMessage::Event`]s, each replacing one part of the state,
//!    with the next revision. A gap in the revisions means a lost event: the
//!    client asks for a new snapshot ([`Method::StateSnapshot`]).
//! 4. Requests ([`ClientMessage::Request`]) are answered by a
//!    [`ServerMessage::Response`] with the same id.
//!
//! Errors and status texts are translation keys of app/lang ([`Msg`]): the
//! UI translates them.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const PROTOCOL_VERSION: u32 = 1;

/// A message of app/lang: key and printf-style arguments
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct Msg {
    pub key: String,
    pub args: Vec<String>,
}

impl Msg {
    pub fn new(key: impl Into<String>) -> Msg {
        Msg { key: key.into(), args: Vec::new() }
    }
}

/// An error answer: a code the UI can act on ("adapter.driver_missing"),
/// and the message to show
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct ErrorInfo {
    pub code: String,
    pub message: Msg,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
pub enum AdapterStatus {
    /// No stack running (started on the first scan or connection)
    #[default]
    Stopped,
    Initializing,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DriverKind {
    /// WinUSB: usable by A2DPWB
    Winusb,
    /// Windows' Bluetooth driver: the adapter is Windows'
    Bluetooth,
    Other,
    None,
}

/// A USB Bluetooth adapter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct AdapterInfo {
    pub instance_id: String,
    pub vid: u16,
    pub pid: u16,
    /// "RTL8761BU (0x2357:0x0604)"
    pub display_name: String,
    /// Windows' name of the device
    pub device_name: String,
    pub vendor: String,
    pub driver: DriverKind,
    /// Driver service (WinUSB, BTHUSB, ...)
    pub driver_service: String,
    /// Realtek chip for the firmware download, if one
    pub realtek_chip: Option<String>,
    /// The firmware files for it are in the settings folder (None: no
    /// firmware needed)
    pub firmware_present: Option<bool>,
    /// The firmware file names it needs ("rtl8761bu_fw.bin")
    pub firmware_files: Vec<String>,
    /// One of them that is there but is no Realtek firmware (a saved web
    /// page): firmware.invalid
    #[serde(default)]
    pub firmware_invalid: Option<String>,
    /// What to tell about other vendors' firmware (firmware.vendor_intel,
    /// _broadcom, _csr, _unsupported); None for Realtek and unknown chips
    #[serde(default)]
    pub vendor_note: Option<Msg>,
}

/// A Realtek firmware pair found in the settings folder
/// (adapter.firmware_files): the choices besides the chip table
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct FirmwareFile {
    /// "rtl8761bu" (the file name without "_fw.bin")
    pub stem: String,
    /// From the chip table, else the stem in capitals ("RTL8723BU")
    pub display_name: String,
    /// Realtek PID of the chip table entry it matches
    pub pid: Option<u16>,
    /// The matching *_config.bin is there too
    pub has_config: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct AdapterState {
    pub status: AdapterStatus,
    pub adapters: Vec<AdapterInfo>,
    pub error: Option<Msg>,
}

/// A device known from the saved list or a scan
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct Device {
    /// "XX:XX:XX:XX:XX:XX"
    pub address: String,
    pub name: String,
    pub saved: bool,
    /// Audio class (headphones, speakers)
    pub audio: bool,
    /// From the last scan
    pub rssi: Option<i8>,
    /// Codec keys the device offered when it last connected since the core
    /// started ("ldac", "aac", ...); empty while not known
    #[serde(default)]
    pub codecs: Vec<String>,
    /// An LE Audio entry (experimental; le_audio_devices.txt and the LE
    /// scan). A dual-mode device has two entries with the same address, a
    /// classic one and an LE Audio one: address and this flag identify an
    /// entry. The UI lists them only while Settings.le_audio_experimental
    /// is on.
    #[serde(default)]
    pub le_audio: bool,
    /// LE Audio: a member of a coordinated set (it advertised a CSIP RSI;
    /// both earbuds stream together)
    #[serde(default)]
    pub le_audio_set: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
pub enum StreamState {
    #[default]
    Idle,
    Connecting,
    Streaming,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct StreamInfo {
    pub codec: String,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub channels: u32,
    pub source_sample_rate: u32,
    pub source_channels: u32,
    pub source_bits: u32,
    /// The profile's codec, when the device did not offer it (fallback)
    pub requested_codec: Option<String>,
    pub requested_in_use: bool,
    /// Adaptive bit rate on (LDAC, LHDC V5): LHDC V5 then stays at 400
    /// kbps or below whatever the quality
    #[serde(default)]
    pub abr: bool,
    /// The device's maximum bit rate lowered the profile's (LHDC V5)
    #[serde(default)]
    pub bitrate_capped: bool,
    /// Where the audio comes from
    #[serde(default)]
    pub source: Option<SourceInfo>,
    /// An LE Audio stream (codec "LC3"; bitrate_kbps, sample_rate and
    /// channels those of the LC3 configuration): what was set up. None for
    /// A2DP.
    #[serde(default)]
    pub le: Option<LeStreamInfo>,
}

/// The LE Audio stream that was set up (BAP unicast; v1 StreamPlan)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct LeStreamInfo {
    /// The BAP preset ("48_4_2"; GMAP low latency "48_4_gr")
    pub preset: String,
    /// LC3 frame duration: 7500 or 10000
    pub frame_us: u32,
    /// LC3 octets per channel and frame
    pub octets: u32,
    /// Connected isochronous streams
    pub cis: u32,
    /// Devices streamed to (2 for both earbuds of a coordinated set)
    pub devices: u32,
    /// The GMAP low latency (gaming) configuration
    pub low_latency: bool,
    /// English, v1's StreamPlan::describe ("LC3 48 kHz 10 ms 120 octets (96
    /// kbps/ch), 2 ch over 2 CIS, 48_4_2")
    pub summary: String,
}

/// The audio source of a stream
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct SourceInfo {
    /// "loopback" (what Windows plays), "virtual" (a virtual device), "app"
    /// (one application), "tone" (the test tone)
    pub kind: String,
    /// The virtual device's name or the application's executable; empty
    /// for loopback and the tone
    pub name: String,
    /// Application capture: the application is not running; the stream
    /// sends silence until it starts
    pub waiting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct StreamStatus {
    pub state: StreamState,
    pub message: Msg,
    /// Profile being streamed
    pub profile: Option<String>,
    pub info: Option<StreamInfo>,
    /// Headphone volume (AVRCP, 0..127), None while unknown
    pub volume: Option<u8>,
}

/// A connection profile, as in profiles.json
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct Profile {
    pub name: String,
    pub device_address: String,
    pub device_name: String,
    /// auto, ldac, aptxhd, aptxll, aptx, sbcxq, sbc, aac, lhdcv5, opus, opus05, faststream
    pub codec: String,
    /// hq, sq, mq; mhq for LHDC V5
    pub quality: String,
    pub abr: bool,
    pub codec_fallback: bool,
    /// 0 = auto
    pub sample_rate: u32,
    /// 0 = auto, 16, 24
    pub bit_depth: u32,
    /// loopback, virtual, app
    pub capture_mode: String,
    pub audio_device_id: String,
    pub audio_device_name: String,
    pub auto_switch_device: bool,
    pub app_exe: String,
    pub app_name: String,
    /// auto, off, device
    pub volume_sync: String,
    pub volume_device_id: String,
    pub volume_device_name: String,
    pub max_media_payload: u16,
    /// An LE Audio device (experimental, LC3): codec, quality, abr,
    /// sample_rate and bit_depth do not apply, capture_mode is "loopback"
    /// or "virtual" (error.le_audio_capture), and streaming needs
    /// Settings.le_audio_experimental (error.le_audio_disabled)
    #[serde(default)]
    pub le_audio: bool,
    /// LE Audio: GMAP low-latency presets with a gaming headset (on by
    /// default; other headsets get the media configuration either way)
    #[serde(default = "yes")]
    pub le_low_latency: bool,
}

fn yes() -> bool {
    true
}

/// Application settings the UI edits
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct Settings {
    pub language: String,
    /// system, light, dark
    pub theme: String,
    pub start_with_windows: bool,
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    pub check_updates_on_startup: bool,
    pub include_prereleases: bool,
    pub media_keys: bool,
    pub remember_volume: bool,
    /// Developer mode: receiver test, HCI capture, debug details
    pub developer_mode: bool,
    /// 0 = auto, a Realtek PID, 0xFFFE = not Realtek
    pub bt_chip_pid: u16,
    pub bt_chip_fw_stem: String,
    /// auto, off, or Wi-Fi channels such as "6,11"
    pub afh: String,
    pub remote_sink: String,
    /// LE Audio (experimental; off by default): LE Audio devices in the
    /// scan and LE Audio profiles. BTstack sets LE up when it starts: while
    /// it streams or scans the change waits for the next scan or
    /// connection (settings.le_audio_deferred).
    #[serde(default)]
    pub le_audio_experimental: bool,
    /// LE Audio: GMAP low latency for gaming headsets (read when the next
    /// LE Audio stream is set up); only with le_audio_experimental
    #[serde(default)]
    pub le_audio_low_latency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct UpdateState {
    pub checking: bool,
    /// A newer release, if any
    pub available: Option<UpdateInfo>,
    pub current_version: String,
    /// The last check failed (update.error); None after one that worked
    #[serde(default)]
    pub error: Option<Msg>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct UpdateInfo {
    pub version: String,
    /// The release page
    pub url: String,
    pub prerelease: bool,
    /// The updater manifest attached to the release (latest.json): the UI
    /// installs from it when it is an installed copy with an updater key
    pub manifest_url: String,
}

/// The whole state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]

pub struct Snapshot {
    pub adapter: AdapterState,
    pub devices: Vec<Device>,
    pub scanning: bool,
    /// The scan's LE Audio part runs (status.le_scanning): after the
    /// classic inquiry, with LE Audio on
    #[serde(default)]
    pub le_scanning: bool,
    pub stream: StreamStatus,
    pub profiles: Vec<Profile>,
    pub active_profile: Option<String>,
    pub settings: Settings,
    pub update: UpdateState,
    /// The settings folder (shown, and opened from the UI)
    pub config_dir: String,
    /// The folder of the core's daily log files ("open logs")
    #[serde(default)]
    pub log_dir: String,
    /// A running HCI capture (diag.capture_start)
    #[serde(default)]
    pub capture: Option<CaptureState>,
    /// The core's local time of day (ms since local midnight) at time_ms 0:
    /// a line's time of day is (clock_ms + time_ms) % 86 400 000. None if
    /// the core does not know it.
    #[serde(default)]
    #[ts(optional = nullable, type = "number | null")]
    pub clock_ms: Option<u64>,
    /// A2DPWB v1 runs (its single-instance mutex is held): it may hold the
    /// Bluetooth adapter, so the UI asks to exit it
    #[serde(default)]
    pub v1_running: bool,
}

/// A running HCI capture
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct CaptureState {
    /// The .pklg file
    pub path: String,
    /// Written so far (refreshed every few seconds while it runs)
    #[ts(type = "number")]
    pub bytes: u64,
    /// When it started: milliseconds since 1970 (UTC)
    #[ts(type = "number")]
    pub started_ms: u64,
}

/// Link quality, 4 times a second while subscribed
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]

pub struct LinkSample {
    #[serde(default)]
    pub queue_audio_us: Option<u32>,
    #[serde(default)]
    pub queue_wait_us: Option<u32>,
    #[serde(default)]
    pub queue_wait_max_us: Option<u32>,
    #[serde(default)]
    pub pacing_late_max_us: Option<u32>,
    #[serde(default)]
    pub pacing_rebases: Option<u64>,
    #[serde(default)]
    pub encoder_backend: Option<String>,
    /// Host send timing and WASAPI flags, not over-the-air loss counters.
    #[serde(default)]
    pub send_gap_max_us: Option<u32>,
    #[serde(default)]
    pub send_gaps_30ms: Option<u64>,
    #[serde(default)]
    pub capture_discontinuities: Option<u64>,
    /// Milliseconds since the core started
    #[ts(type = "number")]
    pub time_ms: u64,
    pub rssi: Option<i8>,
    /// Standard HCI readings; optional when unsupported or not yet read.
    #[serde(default)]
    pub link_quality: Option<u8>,
    #[serde(default)]
    pub tx_power_dbm: Option<i8>,
    #[serde(default)]
    pub tx_power_max_dbm: Option<i8>,
    #[serde(default)]
    pub link_mode: Option<u8>,
    #[serde(default)]
    pub link_policy: Option<u16>,
    pub afh_in_use: Option<u32>,
    pub afh_usable: Option<u32>,
    pub afh_avoided: String,
    /// Media bytes sent per second (RTP and headers included)
    pub bytes_per_second: f64,
    pub packets_per_second: f64,
    pub queue_depth: u32,
    /// Live encoder target (LHDC Switch extension).
    #[serde(default)]
    pub encoder_kbps: Option<u32>,
    /// Packets lost before the radio since the stream started
    #[ts(type = "number")]
    pub dropped: u64,
    /// Captured frames dropped because the encoder fell behind
    #[ts(type = "number")]
    pub capture_dropped: u64,
    /// Share of the link's air time the stream takes, reply slots included
    /// (v1's link quality "air time")
    pub airtime: f64,
    // Optional: a core before these fields sends none
    /// Media packet size over the last interval (L2CAP payload: RTP and
    /// media headers included)
    #[ts(optional = nullable)]
    #[serde(default)]
    pub avg_packet_bytes: Option<f64>,
    /// The ACL packet type that size takes on an EDR 3 Mbps link; None
    /// while nothing is sent
    #[ts(optional = nullable)]
    #[serde(default)]
    pub acl_packet: Option<AclPacket>,
    /// Share of the EDR 3 Mbps link's usable rate (2178 kbps: 1021 bytes
    /// every 6 slots) the stream takes, L2CAP headers included
    #[ts(optional = nullable)]
    #[serde(default)]
    pub bandwidth_share: Option<f64>,
    /// Packets the send queue holds (queue_depth of this)
    #[ts(optional = nullable)]
    #[serde(default)]
    pub queue_capacity: Option<u32>,
    /// Capture rate of the stream (frames per second; 0 while not
    /// streaming): capture drops in ms = frames * 1000 / this
    #[ts(optional = nullable)]
    #[serde(default)]
    pub source_rate: Option<u32>,
    /// The cumulative counters. Since the stream started: total minus
    /// stream_start. "Since reset" (v1's Reset button): the view keeps the
    /// counters of the sample at its reset and subtracts them; the core
    /// keeps no baseline per client.
    #[ts(optional = nullable)]
    #[serde(default)]
    pub counters: Option<LinkCounters>,
    /// An LE Audio stream's isochronous channels (None for A2DP; the A2DP
    /// fields above stay at 0 / None then)
    #[ts(optional = nullable)]
    #[serde(default)]
    pub iso: Option<IsoCounters>,
}

/// An LE Audio stream's send counters (since the stream was set up; a
/// re-setup after a loss starts them again)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct IsoCounters {
    /// Connected isochronous streams
    pub cis: u32,
    /// SDU sets sent (one SDU per CIS and frame interval), silence included
    #[ts(type = "number")]
    pub sdu_sets: u64,
    /// Sets of silence sent because no audio was buffered
    #[ts(type = "number")]
    pub silence: u64,
    /// Times the jitter buffer ran empty
    #[ts(type = "number")]
    pub underruns: u64,
    /// Sets dropped because the jitter buffer was full
    #[ts(type = "number")]
    pub dropped: u64,
    /// The jitter buffer's prefill (frames buffered before sending starts;
    /// 3, growing to 7 after underruns)
    pub prefill: u32,
}

/// An ACL packet type and the slots one media packet takes in it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct AclPacket {
    /// "3-DH1", "3-DH3", "3-DH5"
    pub name: String,
    /// Slots per media packet with the reply slot (several 3-DH5 packets
    /// for one larger than 1021 bytes)
    pub slots: u32,
}

/// Media counters of the core (cumulative, never reset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct LinkTotals {
    /// Media packets handed to L2CAP, and their bytes
    #[ts(type = "number")]
    pub packets: u64,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Packets lost before the radio (queue full, L2CAP refused)
    #[ts(type = "number")]
    pub dropped: u64,
    /// Captured frames, and those dropped because the encoder fell behind
    #[ts(type = "number")]
    pub capture_frames: u64,
    #[ts(type = "number")]
    pub capture_dropped: u64,
}

impl LinkTotals {
    /// `self` since `base` (0 where `base` is larger)
    pub fn since(&self, base: &LinkTotals) -> LinkTotals {
        LinkTotals {
            packets: self.packets.saturating_sub(base.packets),
            bytes: self.bytes.saturating_sub(base.bytes),
            dropped: self.dropped.saturating_sub(base.dropped),
            capture_frames: self.capture_frames.saturating_sub(base.capture_frames),
            capture_dropped: self.capture_dropped.saturating_sub(base.capture_dropped),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct LinkCounters {
    /// Since the core started
    pub total: LinkTotals,
    /// `total` when the current (or last) stream started
    pub stream_start: LinkTotals,
    /// Streams started since the core started (0: none yet); a change means
    /// a new stream_start
    pub stream_starts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct LogLine {
    #[ts(type = "number")]
    pub time_ms: u64,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    /// 1, 2, ... for every line since the core started: FlowStepView.seq
    /// names the line that set a step, FlowView.first_seq an attempt's first
    #[ts(optional = nullable, type = "number | null")]
    #[serde(default)]
    pub seq: Option<u64>,
    /// The connection attempt (FlowView.session) the line belongs to; 0
    /// before the first
    #[ts(optional = nullable)]
    #[serde(default)]
    pub session: Option<u32>,
}

/// One part of the state changed, or a subscribed stream produced data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Event {
    Adapter(AdapterState),
    Devices(Vec<Device>),
    Scanning(bool),
    /// The scan's LE Audio part started or ended (state, le_scanning)
    LeScanning(bool),
    Stream(StreamStatus),
    Profiles {
        profiles: Vec<Profile>,
        active_profile: Option<String>,
    },
    Settings(Settings),
    Update(UpdateState),
    /// Subscription (diag.subscribe link)
    Link(LinkSample),
    /// Subscription (diag.subscribe logs)
    Log(LogLine),
    /// The HCI capture started, stopped, or wrote more (state)
    Capture(Option<CaptureState>),
    /// A2DPWB v1 started or exited (state, v1_running)
    V1Running(bool),
    /// Subscription (diag.subscribe receiver): the receiver test, once a
    /// second and on every change
    Receiver(Box<ReceiverState>),
    /// The core is shutting down
    Bye,
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

/// A request and its parameters. Results (in [`ServerMessage::Response`])
/// are noted per method; `null` where none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "method", content = "params")]
pub enum Method {
    /// -> Snapshot
    #[serde(rename = "state.snapshot")]
    StateSnapshot,
    /// Streams this profile (stopping one that runs)
    #[serde(rename = "stream.start")]
    StreamStart { profile: String },
    #[serde(rename = "stream.stop")]
    StreamStop,
    /// Streams without a saved profile (receiver test, the setup's test
    /// tone); the active profile stays. `codec`, `quality`, `sample_rate`,
    /// `bit_depth`, `max_media_payload` as in a Profile (0 / None: auto);
    /// `source`: "tone", "loopback", "virtual" (`source_id`: the device's
    /// id) or "app" (`source_id`: the executable). Volume sync is off;
    /// codec fallback only when asked.
    #[serde(rename = "stream.start_custom")]
    StreamStartCustom {
        device: String,
        #[ts(optional = nullable)]
        #[serde(default)]
        device_name: Option<String>,
        codec: String,
        quality: String,
        #[ts(optional = nullable)]
        #[serde(default)]
        sample_rate: Option<u32>,
        #[ts(optional = nullable)]
        #[serde(default)]
        bit_depth: Option<u32>,
        source: String,
        #[ts(optional = nullable)]
        #[serde(default)]
        source_id: Option<String>,
        #[ts(optional = nullable)]
        #[serde(default)]
        source_name: Option<String>,
        #[ts(optional = nullable)]
        #[serde(default)]
        max_media_payload: Option<u16>,
        #[ts(optional = nullable)]
        #[serde(default)]
        abr: Option<bool>,
        #[ts(optional = nullable)]
        #[serde(default)]
        codec_fallback: Option<bool>,
    },
    /// AVRCP absolute volume 0..127
    #[serde(rename = "stream.set_volume")]
    StreamSetVolume { volume: u8 },
    /// -> Device[] (also as an event)
    #[serde(rename = "device.scan")]
    DeviceScan { seconds: u32 },
    /// Removes a saved device; `le_audio` true: its LE Audio entry (absent
    /// or false: the classic one)
    #[serde(rename = "device.forget")]
    DeviceForget {
        address: String,
        #[ts(optional = nullable)]
        #[serde(default)]
        le_audio: Option<bool>,
    },
    /// Renames a device (it is saved with that name); `le_audio` as in
    /// device.forget
    #[serde(rename = "device.rename")]
    DeviceRename {
        address: String,
        name: String,
        #[ts(optional = nullable)]
        #[serde(default)]
        le_audio: Option<bool>,
    },
    /// Adds or replaces a profile (`original_name`: the one renamed)
    #[serde(rename = "profile.save")]
    ProfileSave { profile: Box<Profile>, original_name: Option<String> },
    #[serde(rename = "profile.delete")]
    ProfileDelete { name: String },
    #[serde(rename = "profile.set_active")]
    ProfileSetActive { name: String },
    /// -> SettingsApplied (the settings come back as an event too)
    #[serde(rename = "settings.set")]
    SettingsSet { settings: Box<Settings> },
    /// -> AdapterInfo[] (also as an event)
    #[serde(rename = "adapter.list")]
    AdapterList,
    /// Assigns WinUSB (UAC prompt) -> AdapterInfo[]
    #[serde(rename = "adapter.assign_winusb")]
    AdapterAssignWinusb { instance_id: String },
    /// Back to the previous driver (UAC prompt) -> AdapterInfo[]
    #[serde(rename = "adapter.restore_driver")]
    AdapterRestoreDriver { instance_id: String },
    /// Restarts the Bluetooth stack
    #[serde(rename = "adapter.restart")]
    AdapterRestart,
    /// -> FirmwareFile[]: the Realtek firmware pairs in the settings folder
    #[serde(rename = "adapter.firmware_files")]
    AdapterFirmwareFiles,
    /// -> AudioDevice[]
    #[serde(rename = "audio.devices")]
    AudioDevices,
    /// -> AudioApp[]
    #[serde(rename = "audio.apps")]
    AudioApps,
    /// Log lines at `logs` level and above (None: off), link samples, the
    /// receiver test's state (absent: off)
    #[serde(rename = "diag.subscribe")]
    DiagSubscribe {
        logs: Option<LogLevel>,
        link: bool,
        #[ts(optional = nullable)]
        #[serde(default)]
        receiver: Option<bool>,
    },
    /// -> the capture file's path
    #[serde(rename = "diag.capture_start")]
    DiagCaptureStart,
    /// -> CaptureState | null: the file and its final size
    #[serde(rename = "diag.capture_stop")]
    DiagCaptureStop,
    /// -> DiagInfo (text for "copy diagnostic info"); Bluetooth addresses
    /// masked unless `mask` is false. Without parameters as before.
    #[serde(rename = "diag.info")]
    DiagInfo(Option<DiagInfoParams>),
    /// -> FlowView: the connection flow as the core followed it (every log
    /// line, also while no window was open)
    #[serde(rename = "diag.flow")]
    DiagFlow,
    /// -> LogLine[]: the kept lines at `level` and above, oldest first, for
    /// a log view that opens after they were logged
    #[serde(rename = "diag.recent_logs")]
    DiagRecentLogs { level: LogLevel },
    /// -> ReceiverInfo[]: measuring receivers on the local network (UDP
    /// broadcast, about 1.5 s)
    #[serde(rename = "receiver.discover")]
    ReceiverDiscover,
    /// Connects to a receiver's statistics: "host[:port]", or "auto" / ""
    /// for the first one found; saved as settings.remote_sink. Reconnects
    /// until receiver.disconnect. -> ReceiverState
    #[serde(rename = "receiver.connect")]
    ReceiverConnect { target: String },
    #[serde(rename = "receiver.disconnect")]
    ReceiverDisconnect,
    /// Receiver settings for the next stream (None / 0: unchanged): the
    /// L2CAP MTU it offers, its playout buffer. Error receiver.not_connected.
    #[serde(rename = "receiver.configure")]
    ReceiverConfigure {
        #[ts(optional = nullable)]
        #[serde(default)]
        mtu: Option<u32>,
        #[ts(optional = nullable)]
        #[serde(default)]
        buffer_ms: Option<u32>,
    },
    /// -> ReceiverState
    #[serde(rename = "receiver.status")]
    ReceiverStatus,
    /// -> UpdateState; error update.failed (update.error) when the check
    /// failed
    #[serde(rename = "app.check_update")]
    AppCheckUpdate,
    /// The UI hid its main window to the tray: the core shows v1's "still
    /// running in the tray" balloon the first time
    #[serde(rename = "app.hidden_to_tray")]
    AppHiddenToTray,
    /// Ends the core (streaming stops)
    #[serde(rename = "app.quit")]
    AppQuit,
    /// Ends the core so that the UI can install an update, unless it
    /// streams or connects (error `app.streaming`): an update never cuts
    /// the audio
    #[serde(rename = "app.quit_for_update")]
    AppQuitForUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_virtual: bool,
    /// Windows' shared-mode mix format: what capturing it delivers without
    /// resampling (None: not known)
    #[serde(default)]
    pub mix_rate: Option<u32>,
    #[serde(default)]
    pub mix_channels: Option<u16>,
    #[serde(default)]
    pub mix_bits: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct AudioApp {
    pub exe: String,
    pub name: String,
}

/// What settings.set did besides saving the settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct SettingsApplied {
    /// LE Audio (experimental) was turned on or off while BTstack is in
    /// use (streaming or scanning): the change applies at the next scan or
    /// connection (v1's settings.le_audio_deferred)
    #[serde(default)]
    pub le_audio_deferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct DiagInfo {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct DiagInfoParams {
    /// Bluetooth addresses as AA:BB:CC:xx:xx:xx (default true)
    #[ts(optional = nullable)]
    #[serde(default)]
    pub mask: Option<bool>,
}

/// The connection flow of the diagnostics view: the steps of the current
/// (or last) streaming session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct FlowView {
    /// 1, 2, ... since the core started; 0 = no session yet
    pub session: u32,
    /// LogLine.seq of the attempt's first line ("this attempt only")
    #[ts(optional = nullable, type = "number | null")]
    #[serde(default)]
    pub first_seq: Option<u64>,
    #[ts(type = "number")]
    pub start_ms: u64,
    #[ts(type = "number | null")]
    pub stop_ms: Option<u64>,
    pub reconnects: u32,
    /// In order: start, btstack, connect, caps, codec, capture, config,
    /// encoder, stream, audio, link
    pub steps: Vec<FlowStepView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]

pub struct FlowStepView {
    /// "start", "btstack", ... "link"
    pub step: String,
    /// "pending", "running", "done", "warning" or "failed"
    pub status: String,
    /// Since the session started
    #[ts(type = "number | null")]
    pub elapsed_ms: Option<u64>,
    /// English; `detail_msg` translates it where the core computed it
    pub detail: String,
    pub detail_msg: Option<Msg>,
    /// LogLine.seq of the line that set the status (None: none yet)
    #[ts(optional = nullable, type = "number | null")]
    #[serde(default)]
    pub seq: Option<u64>,
    /// LogLine.time_ms of that line
    #[ts(optional = nullable, type = "number | null")]
    #[serde(default)]
    pub time_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Peer receiver test
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReceiverConnection {
    #[default]
    Off,
    Connecting,
    Connected,
    /// Not reachable; retried every few seconds (`detail` says why)
    Failed,
}

/// A measuring receiver (tools/linux_sink): from discovery or its hello
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]

pub struct ReceiverInfo {
    pub ip: String,
    pub port: u16,
    pub host: String,
    /// Its Bluetooth adapter: the device to stream to
    pub bt_address: String,
    pub bt_name: String,
    pub codecs: Vec<String>,
    /// L2CAP MTU it offers for AVDTP
    pub mtu: u32,
    /// Its playout buffer model (0 = not said)
    pub buffer_ms: u32,
}

/// The stream the receiver measures; counters per stream (`id`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]

pub struct ReceivedStream {
    pub id: u32,
    pub codec: String,
    pub config: String,
    /// It decodes (not only header checks)
    pub decoding: bool,
    #[ts(type = "number")]
    pub packets: u64,
    #[ts(type = "number")]
    pub bytes: u64,
    #[ts(type = "number")]
    pub frames: u64,
    #[ts(type = "number")]
    pub lost: u64,
    #[ts(type = "number")]
    pub late: u64,
    #[ts(type = "number")]
    pub ts_errors: u64,
    #[ts(type = "number")]
    pub frame_errors: u64,
    #[ts(type = "number")]
    pub decode_errors: u64,
    /// Dropouts of the modelled playout buffer, and skips
    #[ts(type = "number")]
    pub underruns: u64,
    #[ts(type = "number")]
    pub overflows: u64,
    /// The source paused sending
    #[ts(type = "number")]
    pub pauses: u64,
    /// The source sends nothing right now
    pub idle: bool,
    pub audio_ms: f64,
    pub underrun_ms: f64,
    pub overflow_ms: f64,
    pub max_gap_ms: f64,
    pub jitter_ms: f64,
    /// Modelled playout buffer, None = not playing
    pub buffer_ms: Option<f64>,
    /// The last reporting interval (about a second)
    #[ts(type = "number")]
    pub interval_packets: u64,
    #[ts(type = "number")]
    pub interval_bytes: u64,
    pub interval_max_gap_ms: f64,
}

/// A statistics report of the receiver
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]

pub struct ReceiverStats {
    /// A source is connected to it
    pub connected: bool,
    /// Media flows right now
    pub streaming: bool,
    /// Bluetooth address of that source
    pub device: String,
    pub buffer_target_ms: u32,
    /// L2CAP MTU it offers for new streams
    pub mtu: u32,
    pub codecs: Vec<String>,
    /// BR/EDR RSSI relative to the golden receive range (not dBm)
    pub rssi: Option<i32>,
    pub tx_power: Option<i32>,
    /// Why RSSI is missing, if it says
    pub rssi_note: String,
    /// Channels the link hops over, seen from the receiver
    pub afh_channels: Option<u32>,
    pub stream: Option<ReceivedStream>,
}

/// The receiver test as the core runs it (receiver.status, Event::Receiver)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]

pub struct ReceiverState {
    pub connection: ReceiverConnection,
    /// Why it failed (English, from the connection)
    pub detail: String,
    /// "host[:port]" or "auto"; empty while off
    pub target: String,
    /// The receiver, once found or introduced
    pub receiver: Option<ReceiverInfo>,
    /// Its latest report and how old that is (it reports once a second;
    /// older than 3 s means it stopped)
    pub stats: Option<ReceiverStats>,
    #[ts(type = "number | null")]
    pub stats_age_ms: Option<u64>,
    /// What this PC sent since the current (or last) stream started, to set
    /// beside the receiver's numbers; None before any stream
    pub sent: Option<LinkTotals>,
    /// LinkCounters.stream_starts: a change means `sent` starts again
    pub stream_starts: u32,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        protocol: u32,
        /// "a2dpwb-ui/2.0.0"
        client: String,
    },
    Request {
        #[ts(type = "number")]
        id: u64,
        #[serde(flatten)]
        call: Method,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        protocol: u32,
        /// "a2dpwb-core/2.0.0"
        server: String,
    },
    Reject {
        reason: String,
    },
    Snapshot {
        #[ts(type = "number")]
        rev: u64,
        state: Box<Snapshot>,
    },
    Event {
        #[ts(type = "number")]
        rev: u64,
        event: Box<Event>,
    },
    Response {
        #[ts(type = "number")]
        id: u64,
        #[ts(type = "unknown")]
        result: Option<serde_json::Value>,
        error: Option<ErrorInfo>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format() {
        let m = ClientMessage::Request { id: 7, call: Method::StreamStart { profile: "WH".into() } };
        let j = serde_json::to_value(&m).unwrap();
        assert_eq!(
            j,
            serde_json::json!({"type": "request", "id": 7, "method": "stream.start", "params": {"profile": "WH"}})
        );
        let back: ClientMessage = serde_json::from_value(j).unwrap();
        assert_eq!(back, m);
        let stop: ClientMessage =
            serde_json::from_value(serde_json::json!({"type": "request", "id": 1, "method": "stream.stop"})).unwrap();
        assert_eq!(stop, ClientMessage::Request { id: 1, call: Method::StreamStop });
        let e = ServerMessage::Event { rev: 3, event: Box::new(Event::Scanning(true)) };
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({"type": "event", "rev": 3, "event": {"kind": "scanning", "value": true}})
        );
    }

    /// Requests of a UI built before the additions still parse
    #[test]
    fn older_requests_parse() {
        let m = |j: serde_json::Value| serde_json::from_value::<Method>(j).unwrap();
        assert_eq!(m(serde_json::json!({"method": "diag.info"})), Method::DiagInfo(None));
        assert_eq!(m(serde_json::json!({"method": "diag.info", "params": null})), Method::DiagInfo(None));
        assert_eq!(
            m(serde_json::json!({"method": "diag.info", "params": {"mask": false}})),
            Method::DiagInfo(Some(DiagInfoParams { mask: Some(false) }))
        );
        assert_eq!(
            m(serde_json::json!({"method": "diag.subscribe", "params": {"logs": "info", "link": true}})),
            Method::DiagSubscribe { logs: Some(LogLevel::Info), link: true, receiver: None }
        );
        let custom = m(serde_json::json!({"method": "stream.start_custom", "params": {
            "device": "AA:BB:CC:DD:EE:FF", "codec": "sbc", "quality": "hq", "source": "tone"}}));
        assert!(matches!(custom, Method::StreamStartCustom { sample_rate: None, source_id: None, .. }));
    }

    /// State of a core built before the additions still parses
    #[test]
    fn older_state_parses() {
        let line: LogLine =
            serde_json::from_value(serde_json::json!({"time_ms": 1, "level": "info", "target": "t", "message": "m"}))
                .unwrap();
        assert_eq!((line.seq, line.session), (None, None));
        let sample: LinkSample = serde_json::from_value(serde_json::json!({
            "time_ms": 0, "rssi": null, "afh_in_use": null, "afh_usable": null, "afh_avoided": "",
            "bytes_per_second": 0.0, "packets_per_second": 0.0, "queue_depth": 0, "dropped": 0,
            "capture_dropped": 0, "airtime": 0.0}))
        .unwrap();
        assert!(sample.counters.is_none());
        let dev: Device = serde_json::from_value(
            serde_json::json!({"address": "A", "name": "", "saved": true, "audio": true, "rssi": null}),
        )
        .unwrap();
        assert!(dev.codecs.is_empty());
    }

    /// The LE Audio additions: absent in an older core's state and an older
    /// UI's requests, and they round-trip
    #[test]
    fn le_audio_fields_are_optional() {
        let m = |j: serde_json::Value| serde_json::from_value::<Method>(j).unwrap();
        assert_eq!(
            m(serde_json::json!({"method": "device.forget", "params": {"address": "A"}})),
            Method::DeviceForget { address: "A".into(), le_audio: None }
        );
        assert_eq!(
            m(serde_json::json!({"method": "device.rename", "params": {"address": "A", "name": "N"}})),
            Method::DeviceRename { address: "A".into(), name: "N".into(), le_audio: None }
        );
        let forget = Method::DeviceForget { address: "A".into(), le_audio: Some(true) };
        let j = serde_json::to_value(&forget).unwrap();
        assert_eq!(j, serde_json::json!({"method": "device.forget", "params": {"address": "A", "le_audio": true}}));
        assert_eq!(serde_json::from_value::<Method>(j).unwrap(), forget);

        let dev: Device = serde_json::from_value(
            serde_json::json!({"address": "A", "name": "", "saved": true, "audio": true, "rssi": null}),
        )
        .unwrap();
        assert!(!dev.le_audio && !dev.le_audio_set);
        let le = Device { le_audio: true, le_audio_set: true, ..dev };
        let back: Device = serde_json::from_value(serde_json::to_value(&le).unwrap()).unwrap();
        assert_eq!(back, le);

        let profile: Profile = serde_json::from_value(serde_json::json!({
            "name": "P", "device_address": "A", "device_name": "", "codec": "auto", "quality": "hq",
            "abr": false, "codec_fallback": true, "sample_rate": 0, "bit_depth": 0, "capture_mode": "loopback",
            "audio_device_id": "", "audio_device_name": "", "auto_switch_device": true, "app_exe": "",
            "app_name": "", "volume_sync": "auto", "volume_device_id": "", "volume_device_name": "",
            "max_media_payload": 1023}))
        .unwrap();
        assert!(!profile.le_audio);
        let le = Profile { le_audio: true, ..profile };
        assert_eq!(serde_json::from_value::<Profile>(serde_json::to_value(&le).unwrap()).unwrap(), le);

        let settings: Settings = serde_json::from_value(serde_json::json!({
            "language": "en", "theme": "system", "start_with_windows": false, "start_minimized": false,
            "minimize_to_tray": true, "check_updates_on_startup": true, "include_prereleases": false,
            "media_keys": true, "remember_volume": true, "developer_mode": false, "bt_chip_pid": 0,
            "bt_chip_fw_stem": "", "afh": "auto", "remote_sink": ""}))
        .unwrap();
        assert!(!settings.le_audio_experimental && !settings.le_audio_low_latency);
        let on = Settings { le_audio_experimental: true, le_audio_low_latency: true, ..settings };
        assert_eq!(serde_json::from_value::<Settings>(serde_json::to_value(&on).unwrap()).unwrap(), on);

        let info: StreamInfo = serde_json::from_value(serde_json::json!({
            "codec": "SBC", "bitrate_kbps": 328, "sample_rate": 48000, "channels": 2, "source_sample_rate": 48000,
            "source_channels": 2, "source_bits": 32, "requested_codec": null, "requested_in_use": false}))
        .unwrap();
        assert!(info.le.is_none());
        let lc3 = StreamInfo {
            codec: "LC3".into(),
            bitrate_kbps: 192,
            le: Some(LeStreamInfo {
                preset: "48_4_2".into(),
                frame_us: 10000,
                octets: 120,
                cis: 2,
                devices: 1,
                low_latency: false,
                summary: "LC3 48 kHz 10 ms 120 octets (96 kbps/ch), 2 ch over 2 CIS, 48_4_2".into(),
            }),
            ..info
        };
        assert_eq!(serde_json::from_value::<StreamInfo>(serde_json::to_value(&lc3).unwrap()).unwrap(), lc3);

        let sample: LinkSample = serde_json::from_value(serde_json::json!({
            "time_ms": 0, "rssi": null, "afh_in_use": null, "afh_usable": null, "afh_avoided": "",
            "bytes_per_second": 0.0, "packets_per_second": 0.0, "queue_depth": 0, "dropped": 0,
            "capture_dropped": 0, "airtime": 0.0}))
        .unwrap();
        assert!(sample.iso.is_none());
        let iso = LinkSample {
            iso: Some(IsoCounters { cis: 2, sdu_sets: 1000, silence: 3, underruns: 1, dropped: 0, prefill: 4 }),
            ..sample
        };
        assert_eq!(serde_json::from_value::<LinkSample>(serde_json::to_value(&iso).unwrap()).unwrap(), iso);

        // A snapshot without le_scanning, and the event
        let mut snap = serde_json::to_value(Snapshot {
            adapter: AdapterState::default(),
            devices: Vec::new(),
            scanning: false,
            le_scanning: true,
            stream: StreamStatus::default(),
            profiles: Vec::new(),
            active_profile: None,
            settings: on,
            update: UpdateState::default(),
            config_dir: String::new(),
            log_dir: String::new(),
            capture: None,
            clock_ms: None,
            v1_running: false,
        })
        .unwrap();
        snap.as_object_mut().unwrap().remove("le_scanning");
        assert!(!serde_json::from_value::<Snapshot>(snap).unwrap().le_scanning);
        let e = ServerMessage::Event { rev: 4, event: Box::new(Event::LeScanning(true)) };
        let j = serde_json::to_value(&e).unwrap();
        assert_eq!(j, serde_json::json!({"type": "event", "rev": 4, "event": {"kind": "le_scanning", "value": true}}));
        assert_eq!(serde_json::from_value::<ServerMessage>(j).unwrap(), e);
    }

    #[test]
    fn totals_since_a_baseline() {
        let now = LinkTotals { packets: 10, bytes: 1000, dropped: 2, capture_frames: 480, capture_dropped: 0 };
        let base = LinkTotals { packets: 4, bytes: 400, dropped: 3, capture_frames: 0, capture_dropped: 0 };
        assert_eq!(
            now.since(&base),
            LinkTotals { packets: 6, bytes: 600, dropped: 0, capture_frames: 480, capture_dropped: 0 }
        );
    }
}
