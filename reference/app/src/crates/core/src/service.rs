// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! The service: owns the Bluetooth stack (started on demand, restarted when
//! it stops working or after sleep), runs streaming sessions on their own
//! thread and reports what happens as [`ServiceEvent`]s.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use bt_host::{
    BtHost, DiscoveredDevice, HciCapture, HostConfig, HostEvent, InitFailure, LeConfig, LeScanResult, LinkStats,
    TransportConfig,
};
use hci_transport::UsbSelector;

use crate::devices::SavedDevice;
use crate::messages::Msg;
use crate::settings::Settings;
use crate::stream::{self, StreamInfo, StreamRequest, StreamState};

/// Wait for the controller to answer when checking a stack before use
const CONTROLLER_PROBE: Duration = Duration::from_secs(2);
/// Time for a USB adapter to come back after resume before it is opened
const RESUME_SETTLE: Duration = Duration::from_secs(3);
/// Waiting for the resume after the adapter went away for sleep: ticks of
/// [`RESUME_TICK`], so about a minute while the PC stays awake (a wait across
/// sleep ends once on wake, whatever the clock says)
const RESUME_TICK: Duration = Duration::from_millis(200);
const RESUME_WAIT_TICKS: u32 = 300;
/// LE Audio (experimental): the LE scan after the classic one (v1)
const LE_SCAN: Duration = Duration::from_secs(5);
/// ... once the LE core is ready (HCI works, so at once or a moment later)
const LE_SCAN_READY_WAIT: Duration = Duration::from_secs(1);

/// LE Audio (experimental) from the command line (`--le-audio` and the
/// test options): LE is on whatever the setting says
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeCliOptions {
    /// `--mcp-dry-run` (implied by `--hci-tcp`): MCP answers from a
    /// simulated player and only logs the commands
    pub mcp_dry_run: bool,
    /// `--ccp-sim "<t>:<cmd>;..."`
    pub ccp_sim: Option<String>,
    /// `--ccp-console`
    pub ccp_console: bool,
}

/// A device of a scan ([`Service::scan_devices`]): the classic ones, then
/// the LE Audio ones (experimental). A dual-mode device has two entries: its
/// classic entry is never an LE Audio one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScannedDevice {
    /// XX:XX:XX:XX:XX:XX
    pub address: String,
    pub name: String,
    pub rssi: i8,
    /// Classic: the class of device; 0 for LE Audio entries
    pub class_of_device: u32,
    /// Classic: Audio/Video class; LE Audio entries are audio devices
    pub audio: bool,
    /// An LE Audio entry
    pub le_audio: bool,
    /// LE Audio: member of a coordinated set (advertises a CSIP RSI);
    /// connecting it streams to the whole set
    pub le_audio_set: bool,
}

impl ScannedDevice {
    fn classic(d: &DiscoveredDevice) -> ScannedDevice {
        ScannedDevice {
            address: bt_host::addr_str(&d.addr),
            name: d.name.clone(),
            rssi: d.rssi,
            class_of_device: d.class_of_device,
            audio: d.is_audio(),
            le_audio: false,
            le_audio_set: false,
        }
    }
}

/// The LE Audio entries of an LE scan (v1 A2dpService scan thread): LE
/// Audio advertisers only; a saved LE Audio device is listed (with its new
/// name); of a coordinated set nobody saved, one member (connecting it
/// finds the others). Classic entries of the same address are another
/// list. Returns the entries and the number of LE Audio advertisers seen.
fn le_scan_entries(results: &[LeScanResult], saved_le: &[SavedDevice]) -> (Vec<ScannedDevice>, usize) {
    let mut out: Vec<ScannedDevice> = Vec::new();
    let mut sets_listed: Vec<u32> = Vec::new();
    let mut found = 0;
    for r in results.iter().filter(|r| r.le_audio) {
        found += 1;
        let address = r.addr.to_string();
        let set_member = r.rsi.is_some();
        if let Some(e) = out.iter_mut().find(|e| e.address == address) {
            // Seen twice (advertising and scan response)
            if !r.name.is_empty() {
                e.name = r.name.clone();
            }
            e.le_audio_set |= set_member;
            continue;
        }
        let saved = saved_le.iter().any(|d| d.address.eq_ignore_ascii_case(&address));
        if !saved && let Some(set) = r.set_id {
            if sets_listed.contains(&set) {
                continue;
            }
            sets_listed.push(set);
        }
        out.push(ScannedDevice {
            address,
            name: r.name.clone(),
            rssi: r.rssi,
            class_of_device: 0,
            audio: true,
            le_audio: true,
            le_audio_set: set_member,
        });
    }
    (out, found)
}

/// Sleep and resume as the service sees them (set by the power
/// notifications, read by the session)
#[derive(Debug, Default)]
pub(crate) struct PowerState {
    /// Between the going-to-sleep notification and the resume: the adapter
    /// goes away, so transport failures and a dropped link are expected and
    /// nothing is restarted until the PC is back
    suspending: AtomicBool,
    /// The PC resumed from sleep since the stack started: the adapter lost
    /// power or was re-enumerated, so the stack is restarted before the
    /// next use (v1 needed an app restart)
    stale: AtomicBool,
    resumed_at: Mutex<Option<Instant>>,
}

impl PowerState {
    /// The going-to-sleep notification; false when it was seen already
    pub fn suspend(&self) -> bool {
        !self.suspending.swap(true, Ordering::AcqRel)
    }

    /// A resume notification; true for the first of one resume (Windows
    /// sends two), which marks the stack stale
    pub fn resume(&self) -> bool {
        let was_suspending = self.suspending.swap(false, Ordering::AcqRel);
        let was_stale = self.stale.swap(true, Ordering::AcqRel);
        let first = was_suspending || !was_stale;
        if first {
            *self.resumed_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
        }
        first
    }

    /// Going to sleep: the adapter is going away
    pub fn suspending(&self) -> bool {
        self.suspending.load(Ordering::Acquire)
    }

    /// The running stack is left from before sleep
    pub fn stale(&self) -> bool {
        self.stale.load(Ordering::Acquire)
    }

    /// A new stack starts
    fn clear_stale(&self) {
        self.stale.store(false, Ordering::Release);
    }

    /// No resume came after all: losses count as failures again
    fn end_suspend(&self) {
        self.suspending.store(false, Ordering::Release);
    }

    fn resumed_at(&self) -> Option<Instant> {
        *self.resumed_at.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceEvent {
    State(StreamState, Msg),
    StreamInfo(StreamInfo),
    /// The headphones reported their volume (0..127)
    Volume(u8),
}

pub struct ServiceOptions {
    pub config_dir: PathBuf,
    /// H4 over TCP to a virtual controller instead of USB (tools/emu)
    pub hci_tcp: Option<String>,
    pub capture: HciCapture,
    /// AFH policy instead of the settings' (command line)
    pub afh: Option<String>,
    /// The USB adapter to open (the command line's -u); default: the
    /// first one that works. Not used with hci_tcp.
    pub usb: UsbSelector,
}

/// The LE Audio configuration of the next stack, None: LE off
fn le_config(le_on: bool, cli: Option<&LeCliOptions>, opts: &ServiceOptions) -> Option<LeConfig> {
    if !le_on {
        return None;
    }
    // --mcp-dry-run, implied by --hci-tcp (the emulator): the simulated
    // player answers, nothing in Windows is read or controlled
    let dry_run = cli.is_some_and(|c| c.mcp_dry_run) || opts.hci_tcp.is_some();
    Some(LeConfig {
        bond_dir: Some(opts.config_dir.clone()),
        media: if dry_run { None } else { crate::le_media::windows_backend() },
        ccp_sim: cli.and_then(|c| c.ccp_sim.clone()),
        ccp_console: cli.is_some_and(|c| c.ccp_console),
    })
}

pub(crate) struct Inner {
    opts: ServiceOptions,
    pub settings: Mutex<Settings>,
    events: Mutex<Sender<ServiceEvent>>,
    host: Mutex<Option<Arc<BtHost>>>,
    stats: Arc<LinkStats>,
    stop: AtomicBool,
    running: AtomicBool,
    pub power: PowerState,
    tick: (Mutex<()>, Condvar),
    state: Mutex<StreamState>,
    /// Codec keys each device (address) offered at its last connection
    codecs: Mutex<BTreeMap<String, Vec<String>>>,
    /// The chip or firmware setting changed while the stack was in use:
    /// the next use starts a new stack (which loads the firmware again)
    reload: AtomicBool,
    /// LE Audio forced on by the command line
    le_cli: Mutex<Option<LeCliOptions>>,
    /// The running stack was started with LE Audio
    le_active: AtomicBool,
    /// A device scan runs
    scanning: AtomicBool,
    /// The LE Audio stream being sent (le_stream)
    le_stream: Mutex<Option<crate::le_stream::LeStreamDetails>>,
}

impl Inner {
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub fn notify(&self, state: StreamState, msg: Msg) {
        tracing::info!("state {state:?}: {}", msg.english());
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = state;
        let _ = self.events.lock().unwrap_or_else(|e| e.into_inner()).send(ServiceEvent::State(state, msg));
    }

    pub fn stream_info(&self, info: StreamInfo) {
        let _ = self.events.lock().unwrap_or_else(|e| e.into_inner()).send(ServiceEvent::StreamInfo(info));
    }

    /// Sleeps up to `d`, returning early on a stop
    pub fn wait_tick(&self, d: Duration) {
        let guard = self.tick.0.lock().unwrap_or_else(|e| e.into_inner());
        if !self.stop_requested() {
            let _ = self.tick.1.wait_timeout(guard, d);
        }
    }

    fn wait_after_resume(&self) {
        let Some(at) = self.power.resumed_at() else { return };
        let since = at.elapsed();
        if since >= RESUME_SETTLE {
            return;
        }
        tracing::info!("waiting {} ms for the adapter after resume", (RESUME_SETTLE - since).as_millis());
        while at.elapsed() < RESUME_SETTLE && !self.stop_requested() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// LE Audio (experimental) for the next stack: the setting, or the
    /// command line's --le-audio
    pub fn le_wanted(&self) -> bool {
        self.le_cli.lock().unwrap_or_else(|e| e.into_inner()).is_some()
            || self.settings.lock().unwrap_or_else(|e| e.into_inner()).le_audio_experimental
    }

    /// The running stack has LE Audio
    pub fn le_active(&self) -> bool {
        self.le_active.load(Ordering::Acquire)
    }

    fn host_config(&self, le_on: bool) -> HostConfig {
        let s = self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let transport = match &self.opts.hci_tcp {
            Some(addr) => TransportConfig::Tcp(addr.clone()),
            // bt-host adds the adapters it knows to `known`
            None => TransportConfig::Usb(self.opts.usb.clone()),
        };
        let mut c = HostConfig::new(transport);
        c.link_key_dir = Some(self.opts.config_dir.clone());
        c.firmware_dir = Some(self.opts.config_dir.clone());
        c.capture = self.opts.capture.clone();
        crate::adapter::configure(&mut c, &s, &self.opts.config_dir);
        let cli = self.le_cli.lock().unwrap_or_else(|e| e.into_inner()).clone();
        c.le_audio = le_config(le_on, cli.as_ref(), &self.opts);
        c
    }

    fn start_host(self: &Arc<Self>) -> Result<Arc<BtHost>, Msg> {
        // A controller that did not answer at all is opened once more right
        // away: opening it resets its USB pipes, which can be enough
        let attempts = 2;
        let mut last = InitFailure::Failed;
        let le_on = self.le_wanted();
        if le_on {
            tracing::info!("LE Audio (experimental) is on: BTstack starts with LE");
        }
        for attempt in 1..=attempts {
            if attempt > 1 {
                tracing::warn!("the Bluetooth adapter did not answer, opening it again ({attempt}/{attempts})");
            }
            match BtHost::start(self.host_config(le_on), self.stats.clone()) {
                Ok(h) => {
                    let h = Arc::new(h);
                    let s = self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    h.set_media_keys(s.media_keys);
                    h.set_afh_policy(self.opts.afh.as_deref().unwrap_or(&s.afh));
                    self.le_active.store(le_on, Ordering::Release);
                    self.pump_events(&h);
                    return Ok(h);
                }
                Err(f) => {
                    last = f;
                    if f != InitFailure::NoAnswer {
                        break;
                    }
                }
            }
        }
        Err(Msg::new(if last == InitFailure::NoAnswer { "error.btstack_no_answer" } else { "error.btstack_init" }))
    }

    /// Headphone buttons to Windows media keys, volume reports to the
    /// service's events
    fn pump_events(self: &Arc<Self>, host: &Arc<BtHost>) {
        let Some(rx) = host.take_events() else { return };
        let weak: Weak<Inner> = Arc::downgrade(self);
        let _ = std::thread::Builder::new().name("host-events".into()).spawn(move || {
            while let Ok(e) = rx.recv() {
                match e {
                    HostEvent::MediaKey(k) => {
                        let key = match k {
                            bt_host::MediaKey::PlayPause => win_sys::media_keys::MediaKey::PlayPause,
                            bt_host::MediaKey::Stop => win_sys::media_keys::MediaKey::Stop,
                            bt_host::MediaKey::Next => win_sys::media_keys::MediaKey::Next,
                            bt_host::MediaKey::Previous => win_sys::media_keys::MediaKey::Previous,
                        };
                        if let Err(e) = win_sys::media_keys::send(key) {
                            tracing::warn!("media key {k:?}: {e}");
                        }
                    }
                    HostEvent::Volume(v) => {
                        if let Some(inner) = weak.upgrade() {
                            let _ =
                                inner.events.lock().unwrap_or_else(|e| e.into_inner()).send(ServiceEvent::Volume(v));
                        }
                    }
                    HostEvent::ConnectionLost | HostEvent::TransportFailed => {}
                }
            }
        });
    }

    /// The running stack, started (or restarted) as needed
    pub fn ensure_host(self: &Arc<Self>) -> Result<Arc<BtHost>, Msg> {
        let mut slot = self.host.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = slot.as_ref() {
            // A stack that stopped working stays so until it is started anew
            // (v1 issues #42, #43: only an app restart helped)
            let le_wanted = self.le_wanted();
            let why = if self.power.stale() {
                Some(stream::RESUMED)
            } else if self.reload.load(Ordering::Acquire) {
                Some("the adapter settings changed")
            } else if self.le_active() != le_wanted {
                // Turned on or off while streaming or scanning: applied now,
                // before the next scan or connection (v1)
                Some(if le_wanted {
                    "LE Audio (experimental) was turned on"
                } else {
                    "LE Audio (experimental) was turned off"
                })
            } else if !h.controller_responds(CONTROLLER_PROBE) {
                Some("the Bluetooth adapter stopped answering")
            } else {
                None
            };
            match why {
                None => return Ok(h.clone()),
                Some(why) => {
                    tracing::warn!("restarting BTstack: {why}");
                    h.shutdown();
                    *slot = None;
                }
            }
        }
        self.power.clear_stale();
        self.reload.store(false, Ordering::Release);
        self.wait_after_resume();
        let h = self.start_host()?;
        *slot = Some(h.clone());
        Ok(h)
    }

    /// Shuts the stack down and starts a new one (the session thread, for a
    /// stack that stopped working or is left from before sleep)
    pub fn restart_host(self: &Arc<Self>, why: &str, max_media_payload: u16) -> Result<Arc<BtHost>, Msg> {
        if why == stream::RESUMED {
            tracing::info!("restarting BTstack: {why}");
        } else {
            tracing::warn!("restarting BTstack: {why}");
        }
        self.shutdown_host();
        let h = self.ensure_host()?;
        h.set_media_payload_limit(max_media_payload);
        Ok(h)
    }

    /// The PC is going to sleep and the stack lost (or is losing) its
    /// adapter: shut down without the noise of a failure; after the resume
    /// a new one starts
    pub fn shutdown_host_for_sleep(&self) {
        let old = self.host.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(h) = old {
            tracing::info!("stopping BTstack until the PC resumes");
            h.shutdown_lost();
        }
    }

    /// Waits for the resume notification while the PC goes to sleep. Gives
    /// up after about a minute awake without one (the stack is then
    /// restarted as after a failure). False when the session was stopped.
    pub fn wait_for_resume(&self) -> bool {
        let mut ticks = 0;
        while self.power.suspending() && !self.stop_requested() {
            if ticks == RESUME_WAIT_TICKS {
                tracing::warn!("no resume notification after going to sleep");
                self.power.end_suspend();
                bt_host::expect_transport_loss(false);
                break;
            }
            self.wait_tick(RESUME_TICK);
            ticks += 1;
        }
        !self.stop_requested()
    }

    fn shutdown_host(&self) {
        let old = self.host.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(h) = old {
            // Also when the session still holds the Arc: the adapter must be
            // free for the next stack
            h.shutdown();
        }
    }

    pub fn saved_volume(&self, addr: &str) -> Option<u8> {
        let s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if !s.remember_volume {
            return None;
        }
        s.saved_volumes.get(&addr.to_uppercase()).and_then(|&v| u8::try_from(v).ok())
    }

    /// Keeps the headphones' volume for the next connection (saved with the
    /// settings when it changes)
    pub fn remember_volume(&self, addr: &str, volume: Option<u8>) {
        let Some(v) = volume else { return };
        let mut s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if !s.remember_volume {
            return;
        }
        let key = addr.to_uppercase();
        if s.saved_volumes.get(&key) == Some(&(v as i32)) {
            return;
        }
        s.saved_volumes.insert(key, v as i32);
        if let Err(e) = s.save(&self.opts.config_dir) {
            tracing::warn!("cannot save the settings: {e}");
        }
    }

    /// What a device offered (after capability discovery)
    pub fn record_codecs(&self, addr: &str, codecs: Vec<String>) {
        self.codecs.lock().unwrap_or_else(|e| e.into_inner()).insert(addr.to_uppercase(), codecs);
    }

    pub fn save_device(&self, addr: &str, name: &str) {
        if let Err(e) = crate::devices::save(&self.opts.config_dir, addr, name) {
            tracing::warn!("cannot save the device: {e}");
        }
    }

    /// The LE Audio stream now (set before its StreamInfo is sent)
    pub fn set_le_stream(&self, details: Option<crate::le_stream::LeStreamDetails>) {
        *self.le_stream.lock().unwrap_or_else(|e| e.into_inner()) = details;
    }

    /// An LE Audio device that streamed (le_audio_devices.txt)
    pub fn save_le_device(&self, addr: &str, name: &str) {
        if let Err(e) = crate::devices::save_le(&self.opts.config_dir, addr, name) {
            tracing::warn!("cannot save the LE Audio device: {e}");
        }
    }
}

pub struct Service {
    inner: Arc<Inner>,
    worker: Mutex<Option<JoinHandle<()>>>,
    _power: Option<win_sys::power::PowerNotifier>,
}

impl Service {
    pub fn new(opts: ServiceOptions, events: Sender<ServiceEvent>) -> Service {
        // A folder v1 used is backed up and brought to v2's schema first
        crate::migrate::prepare(&opts.config_dir);
        let settings = Settings::load(&opts.config_dir);
        let inner = Arc::new(Inner {
            opts,
            settings: Mutex::new(settings),
            events: Mutex::new(events),
            host: Mutex::new(None),
            stats: Arc::new(LinkStats::default()),
            stop: AtomicBool::new(false),
            running: AtomicBool::new(false),
            power: PowerState::default(),
            tick: (Mutex::new(()), Condvar::new()),
            state: Mutex::new(StreamState::Idle),
            codecs: Mutex::new(BTreeMap::new()),
            reload: AtomicBool::new(false),
            le_cli: Mutex::new(None),
            le_active: AtomicBool::new(false),
            scanning: AtomicBool::new(false),
            le_stream: Mutex::new(None),
        });
        // Without a window, also while only the tray runs
        let weak = Arc::downgrade(&inner);
        let power = win_sys::power::PowerNotifier::new(move |e| {
            let Some(inner) = weak.upgrade() else { return };
            match e {
                win_sys::power::PowerEvent::Suspend => {
                    if inner.power.suspend() {
                        tracing::info!("the PC is going to sleep");
                        // The USB transfers get cancelled now: not an error
                        bt_host::expect_transport_loss(true);
                    }
                }
                win_sys::power::PowerEvent::Resume => {
                    // Both resume kinds may come for one resume
                    if inner.power.resume() {
                        tracing::info!("the PC resumed from sleep, Bluetooth will be restarted");
                    }
                    bt_host::expect_transport_loss(false);
                }
            }
        });
        let power = match power {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!("no sleep / resume notifications: {e}");
                None
            }
        };
        pipeline::set_thread_priority_hook(|| {
            Box::new(win_sys::mmcss::MmcssGuard::register(win_sys::mmcss::MmcssTask::ProAudio).ok())
        });
        pipeline::set_packet_wait_hook(win_sys::mmcss::media_wait);
        hci_transport::set_acl_out_priority_hook(|| {
            Box::new(win_sys::mmcss::MmcssGuard::register(win_sys::mmcss::MmcssTask::ProAudio).ok())
        });
        Service { inner, worker: Mutex::new(None), _power: power }
    }

    pub fn stats(&self) -> Arc<LinkStats> {
        self.inner.stats.clone()
    }

    pub fn config_dir(&self) -> &std::path::Path {
        &self.inner.opts.config_dir
    }

    pub fn capture(&self) -> &HciCapture {
        &self.inner.opts.capture
    }

    /// Replaces the settings (saved by the caller) and applies what affects
    /// a running stack.
    ///
    /// LE Audio (experimental) on / off (v1 OnToggleLeAudio): LE (SM, GATT,
    /// ISO) is set up when BTstack starts, so an idle stack is stopped and
    /// the next scan or connection starts one with the new setting; while
    /// streaming or scanning the stack keeps running and is restarted
    /// before the next scan or connection ([`Service::le_audio_deferred`]
    /// says so). The low-latency setting applies to the next LE stream.
    pub fn set_settings(&self, settings: Settings) {
        let host = self.host();
        if let Some(h) = &host {
            h.set_media_keys(settings.media_keys);
            h.set_afh_policy(self.inner.opts.afh.as_deref().unwrap_or(&settings.afh));
        }
        let old = std::mem::replace(&mut *self.inner.settings.lock().unwrap_or_else(|e| e.into_inner()), settings);
        let new_le = self.inner.settings.lock().unwrap_or_else(|e| e.into_inner()).le_audio_experimental;
        if old.le_audio_experimental != new_le && host.is_some() && self.inner.le_active() != self.inner.le_wanted() {
            if self.is_running() || self.is_scanning() {
                tracing::info!(
                    "LE Audio (experimental) turned {}: BTstack restarts before the next scan or connection",
                    if new_le { "on" } else { "off" }
                );
            } else {
                tracing::info!(
                    "LE Audio (experimental) turned {}: stopping BTstack, the next scan or connection starts it anew",
                    if new_le { "on" } else { "off" }
                );
                self.inner.shutdown_host();
            }
        }
    }

    /// LE Audio (experimental) for every stack this service starts, whatever
    /// the setting says (the command line's --le-audio)
    pub fn set_le_audio_cli(&self, options: LeCliOptions) {
        *self.inner.le_cli.lock().unwrap_or_else(|e| e.into_inner()) = Some(options);
    }

    /// The running stack has LE Audio (experimental)
    pub fn le_audio_active(&self) -> bool {
        self.host().is_some() && self.inner.le_active()
    }

    /// The LE Audio setting changed while the stack was in use: it is
    /// applied before the next scan or connection (settings.le_audio_deferred)
    pub fn le_audio_deferred(&self) -> bool {
        self.host().is_some() && self.inner.le_active() != self.inner.le_wanted()
    }

    /// What the LE Audio (experimental) stream was set up with, while one
    /// runs (with its [`ServiceEvent::StreamInfo`]; codec "LC3 (LE Audio)")
    pub fn le_stream(&self) -> Option<crate::le_stream::LeStreamDetails> {
        self.inner.le_stream.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// A device scan runs
    pub fn is_scanning(&self) -> bool {
        self.inner.scanning.load(Ordering::Acquire)
    }

    /// The running stack, started (or restarted, as for a connection) as
    /// needed: the command line's LE Audio flow
    pub fn ensure_host(&self) -> Result<Arc<BtHost>, Msg> {
        self.inner.ensure_host()
    }

    /// Stops the stack, releasing the adapter (e.g. before its driver is
    /// changed); the next use starts a new one
    pub fn release_adapter(&self) {
        self.shutdown();
    }

    /// The chip or firmware setting changed (v1 reset_btstack): an idle
    /// stack is released now, one in use is replaced at its next use (the
    /// next connection or scan), so the firmware is loaded again. `idle`:
    /// nothing streams or scans. True when it was released now.
    pub fn reload_adapter(&self, idle: bool) -> bool {
        if idle && !self.is_running() {
            self.inner.reload.store(false, Ordering::Release);
            self.inner.shutdown_host();
            true
        } else {
            self.inner.reload.store(self.host().is_some(), Ordering::Release);
            false
        }
    }

    /// Codec keys each device (upper-case address) offered when it last
    /// connected
    pub fn device_codecs(&self) -> BTreeMap<String, Vec<String>> {
        self.inner.codecs.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn settings(&self) -> Settings {
        self.inner.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn state(&self) -> StreamState {
        *self.inner.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }

    /// The running stack, if any (for diagnostics)
    pub fn host(&self) -> Option<Arc<BtHost>> {
        self.inner.host.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Starts a session; a running one is stopped first
    pub fn start_streaming(&self, req: StreamRequest) {
        self.stop_streaming();
        self.inner.stop.store(false, Ordering::Release);
        self.inner.running.store(true, Ordering::Release);
        self.inner.notify(StreamState::Connecting, Msg::new("status.initializing"));
        let inner = self.inner.clone();
        let t = std::thread::Builder::new()
            .name("session".into())
            .spawn(move || {
                // A panic in the session must not take the process down: the
                // state says what happened
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // LE Audio (experimental) profiles have their own session
                    if req.le.is_some() { crate::le_stream::run(&inner, req) } else { stream::run(&inner, req) }
                }));
                if r.is_err() {
                    tracing::error!("the streaming session panicked");
                    inner.notify(StreamState::Error, Msg::new("error.btstack_crash_stream"));
                }
                inner.running.store(false, Ordering::Release);
            })
            .expect("spawning the session thread");
        *self.worker.lock().unwrap_or_else(|e| e.into_inner()) = Some(t);
    }

    /// Stops the session and waits for it (bounded by the stack's own
    /// timeouts: blocking waits are cancelled)
    pub fn stop_streaming(&self) {
        let Some(t) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        self.inner.stop.store(true, Ordering::Release);
        self.inner.tick.1.notify_all();
        if let Some(h) = self.host() {
            h.cancel_pending_waits();
        }
        let _ = t.join();
    }

    /// Inquiry for nearby (classic) devices
    pub fn scan(&self, seconds: u32) -> Result<Vec<DiscoveredDevice>, Msg> {
        if self.is_running() {
            return Err(Msg::new("status.streaming"));
        }
        let _scanning = ScanningGuard::new(&self.inner.scanning);
        self.inner.notify(StreamState::Idle, Msg::new("status.initializing_scan"));
        let host = self.inner.ensure_host()?;
        self.inner.notify(StreamState::Idle, Msg::new("status.scanning_devices"));
        let found = host.scan(seconds);
        self.inner.notify(StreamState::Idle, Msg::with("status.scan_complete", &[&found.len()]));
        Ok(found)
    }

    /// Inquiry for nearby devices, then, when the stack has LE Audio
    /// (experimental), a 5 s LE scan for LE Audio devices (v1's scan): the
    /// classic devices, then the LE Audio ones ([`ScannedDevice`])
    pub fn scan_devices(&self, seconds: u32) -> Result<Vec<ScannedDevice>, Msg> {
        if self.is_running() {
            return Err(Msg::new("status.streaming"));
        }
        let _scanning = ScanningGuard::new(&self.inner.scanning);
        self.inner.notify(StreamState::Idle, Msg::new("status.initializing_scan"));
        let host = self.inner.ensure_host()?;
        self.inner.notify(StreamState::Idle, Msg::new("status.scanning_devices"));
        let classic = host.scan(seconds);
        let mut list: Vec<ScannedDevice> = classic.iter().map(ScannedDevice::classic).collect();
        let mut le_found = 0;
        if self.inner.le_active() {
            let until = Instant::now() + LE_SCAN_READY_WAIT;
            while !host.le_ready() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(100));
            }
            if host.le_ready() {
                self.inner.notify(StreamState::Idle, Msg::new("status.le_scanning"));
                let results = host.le_scan(LE_SCAN);
                let saved = crate::devices::load_le(&self.inner.opts.config_dir);
                let (entries, found) = le_scan_entries(&results, &saved);
                le_found = found;
                list.extend(entries);
            } else {
                tracing::warn!("LE Audio not ready, LE scan skipped");
            }
        }
        self.inner.notify(StreamState::Idle, Msg::with("status.scan_complete", &[&(classic.len() + le_found)]));
        Ok(list)
    }

    /// Stops streaming and the stack (the adapter is released)
    pub fn shutdown(&self) {
        self.stop_streaming();
        self.inner.shutdown_host();
    }

    pub fn saved_volumes(&self) -> BTreeMap<String, i32> {
        self.settings().saved_volumes
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Marks a scan while it runs (also when it returns early)
struct ScanningGuard<'a>(&'a AtomicBool);

impl<'a> ScanningGuard<'a> {
    fn new(flag: &'a AtomicBool) -> ScanningGuard<'a> {
        flag.store(true, Ordering::Release);
        ScanningGuard(flag)
    }
}

impl Drop for ScanningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_marks_suspending_until_resume() {
        let p = PowerState::default();
        assert!(!p.suspending() && !p.stale());
        assert!(p.suspend());
        // A repeated notification
        assert!(!p.suspend());
        assert!(p.suspending());
        assert!(!p.stale());
        assert!(p.resumed_at().is_none());
        assert!(p.resume());
        assert!(!p.suspending());
        assert!(p.stale());
        assert!(p.resumed_at().is_some());
    }

    #[test]
    fn the_second_resume_notification_is_ignored() {
        let p = PowerState::default();
        p.suspend();
        assert!(p.resume());
        let at = p.resumed_at();
        assert!(!p.resume());
        assert_eq!(p.resumed_at(), at);
        assert!(p.stale() && !p.suspending());
    }

    #[test]
    fn a_resume_without_suspend_still_marks_the_stack_stale() {
        let p = PowerState::default();
        assert!(p.resume());
        assert!(p.stale() && !p.suspending());
    }

    #[test]
    fn a_new_sleep_counts_while_the_stack_is_still_stale() {
        // Slept and woke, nothing used the stack since, then slept again
        let p = PowerState::default();
        p.suspend();
        p.resume();
        let first = p.resumed_at().unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(p.suspend());
        assert!(p.suspending() && p.stale());
        assert!(p.resume());
        assert!(p.resumed_at().unwrap() > first);
    }

    fn le(addr: &str, name: &str, set: Option<u32>) -> LeScanResult {
        LeScanResult {
            addr: bt_host::LeAddr { addr: bt_host::parse_addr(addr).unwrap(), addr_type: 0 },
            name: name.into(),
            le_audio: true,
            rsi: set.map(|_| [1, 2, 3, 4, 5, 6]),
            set_id: set,
            ..LeScanResult::default()
        }
    }

    #[test]
    fn le_scan_lists_one_member_per_new_set() {
        let results = [
            le("00:A2:D0:00:00:12", "Buds L", Some(0)),
            le("00:A2:D0:00:00:13", "Buds R", Some(0)),
            le("00:A2:D0:00:00:20", "Speaker", None),
            LeScanResult { le_audio: false, ..le("00:A2:D0:00:00:30", "Mouse", None) },
            le("00:A2:D0:00:00:20", "Speaker 2", None),
            le("00:A2:D0:00:00:40", "Other L", Some(1)),
        ];
        let (list, found) = le_scan_entries(&results, &[]);
        let got: Vec<_> = list.iter().map(|d| (d.address.as_str(), d.name.as_str(), d.le_audio_set)).collect();
        assert_eq!(
            got,
            [
                ("00:A2:D0:00:00:12", "Buds L", true),
                ("00:A2:D0:00:00:20", "Speaker 2", false),
                ("00:A2:D0:00:00:40", "Other L", true)
            ]
        );
        assert_eq!(found, 5);
        assert!(list.iter().all(|d| d.le_audio && d.audio && d.class_of_device == 0));
    }

    #[test]
    fn le_scan_keeps_saved_members() {
        // The right earbud is saved: it is listed, and the left one still
        // stands for the set
        let saved = [SavedDevice { address: "00:A2:D0:00:00:13".into(), name: "R".into(), le_audio: true }];
        let results = [le("00:A2:D0:00:00:13", "Buds R", Some(0)), le("00:A2:D0:00:00:12", "Buds L", Some(0))];
        let (list, _) = le_scan_entries(&results, &saved);
        assert_eq!(list.len(), 2);
        // A third member is a duplicate of the listed set
        let results = [
            le("00:A2:D0:00:00:12", "Buds L", Some(0)),
            le("00:A2:D0:00:00:14", "Buds C", Some(0)),
            le("00:A2:D0:00:00:13", "Buds R", Some(0)),
        ];
        let (list, _) = le_scan_entries(&results, &saved);
        let got: Vec<_> = list.iter().map(|d| d.address.as_str()).collect();
        assert_eq!(got, ["00:A2:D0:00:00:12", "00:A2:D0:00:00:13"]);
    }

    #[test]
    fn a_dual_mode_device_keeps_its_classic_entry() {
        let classic = DiscoveredDevice {
            addr: bt_host::parse_addr("00:A2:D0:00:00:12").unwrap(),
            name: "Buds".into(),
            class_of_device: 0x240404,
            rssi: -40,
        };
        let c = ScannedDevice::classic(&classic);
        assert!(!c.le_audio && c.audio);
        let (le_list, _) = le_scan_entries(&[le("00:A2:D0:00:00:12", "Buds", None)], &[]);
        assert_eq!(le_list[0].address, c.address);
        assert!(le_list[0].le_audio);
    }

    #[test]
    fn le_config_from_the_setting_or_the_command_line() {
        let opts = ServiceOptions {
            config_dir: PathBuf::from("cfg"),
            hci_tcp: None,
            capture: HciCapture::new(),
            afh: None,
            usb: UsbSelector::default(),
        };
        assert!(le_config(false, None, &opts).is_none());
        let cli = LeCliOptions { mcp_dry_run: true, ccp_sim: Some("1:incoming x".into()), ccp_console: true };
        let c = le_config(true, Some(&cli), &opts).unwrap();
        assert_eq!(c.bond_dir.as_deref(), Some(std::path::Path::new("cfg")));
        assert!(c.media.is_none());
        assert_eq!(c.ccp_sim.as_deref(), Some("1:incoming x"));
        assert!(c.ccp_console);
        // The emulator transport implies the MCP dry run
        let tcp = ServiceOptions { hci_tcp: Some("127.0.0.1:9510".into()), ..opts };
        let c = le_config(true, Some(&LeCliOptions::default()), &tcp).unwrap();
        assert!(c.media.is_none() && c.ccp_sim.is_none() && !c.ccp_console);
    }

    #[test]
    fn a_new_stack_clears_stale_but_not_suspending() {
        let p = PowerState::default();
        p.suspend();
        p.resume();
        p.clear_stale();
        assert!(!p.stale());
        p.suspend();
        p.clear_stale();
        assert!(p.suspending());
        p.end_suspend();
        assert!(!p.suspending() && !p.stale());
    }
}
