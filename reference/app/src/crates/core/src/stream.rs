// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! A streaming session: connect, pick the codec, set up capture, encoder and
//! stream, then keep streaming and reconnect when the link drops — v1's
//! A2dpService::streaming_thread_func_inner, on the session thread of
//! [`crate::Service`].

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use audio_capture::{Capture, CaptureOptions, Format, Running, SampleFormat, Sink, Source};
use bt_host::{BtHost, ConfigureError, ConnectFailure, MediaSink, ReconnectError};
use codec::caps::{self, ConfigError, Configuration, RemoteCaps, Unavailable};
use codec::{CodecKind, Encoder, EncoderConfig, PeerParams, Quality};
use pipeline::{BufferPreset, CaptureInput, InputFormat, Pipeline, PipelineConfig, SampleType};

use crate::messages::Msg;
use crate::service::Inner;

/// What to stream (from a profile, or the command line)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRequest {
    pub device: [u8; 6],
    pub device_name: String,
    /// None = Auto
    pub codec: Option<CodecKind>,
    pub codec_fallback: bool,
    pub quality: Quality,
    pub abr: bool,
    /// None = the device's rate
    pub sample_rate: Option<u32>,
    /// None = auto (24-bit where the codec carries it)
    pub bit_depth: Option<u8>,
    pub source: SourceRequest,
    pub volume_sync: VolumeSync,
    pub max_media_payload: u16,
    /// LE Audio (experimental) instead of A2DP: the le_stream session
    /// streams to `device` and these extra devices. None: A2DP.
    pub le: Option<bt_host::LeRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRequest {
    SystemLoopback,
    VirtualDevice { id: String, name: String, auto_switch: bool },
    Application { exe: String },
    TestTone,
}

impl SourceRequest {
    /// The source as the UI shows it (not waiting: the session says when
    /// an application is not running)
    pub fn info(&self) -> SourceInfo {
        let (kind, name) = match self {
            SourceRequest::SystemLoopback => ("loopback", String::new()),
            SourceRequest::VirtualDevice { name, .. } => ("virtual", name.clone()),
            SourceRequest::Application { exe } => ("app", exe.clone()),
            SourceRequest::TestTone => ("tone", String::new()),
        };
        SourceInfo { kind, name, waiting: false }
    }
}

impl std::fmt::Display for SourceRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceRequest::SystemLoopback => f.write_str("system output"),
            SourceRequest::VirtualDevice { name, .. } => write!(f, "virtual device \"{name}\""),
            SourceRequest::Application { exe } => write!(f, "app {exe}"),
            SourceRequest::TestTone => f.write_str("test tone"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeSync {
    Off,
    /// The device the audio comes from (the default output in application
    /// mode)
    Auto,
    Device(String),
}

/// What the session reports about the stream
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StreamInfo {
    pub codec: String,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub channels: u32,
    pub source_sample_rate: u32,
    pub source_channels: u32,
    pub source_bits: u32,
    /// The profile's codec the device did not offer (codec fallback)
    pub requested_codec: Option<String>,
    /// ... maybe because another device streams on its endpoint
    pub requested_in_use: bool,
    /// Adaptive bit rate on (LDAC, LHDC V5)
    pub abr: bool,
    /// The device's maximum bit rate lowered the profile's (LHDC V5)
    pub bitrate_capped: bool,
    /// Where the audio comes from (sent again when an application capture
    /// starts or stops waiting for the app)
    pub source: SourceInfo,
}

/// The audio source of a stream
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SourceInfo {
    /// "loopback", "virtual", "app" (profile capture modes) or "tone"
    pub kind: &'static str,
    /// The virtual device's name or the application's executable
    pub name: String,
    /// Application capture: the application is not running
    pub waiting: bool,
}

/// Codec keys ("ldac", "sbc", ...) of what a device offered
pub fn codec_keys(remote: &caps::RemoteCaps) -> Vec<String> {
    CodecKind::ALL.into_iter().filter(|&k| remote.has(k)).map(|k| k.key().to_string()).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum StreamState {
    Idle,
    Connecting,
    Streaming,
    Reconnecting,
    Error,
}

/// Feeds captured audio into the pipeline. Shared so an application
/// capture can be replaced (the app quits and starts again) while the
/// pipeline keeps running; only one capture writes at a time, so the lock is
/// never contended.
#[derive(Clone)]
struct SharedInput(Arc<Mutex<CaptureInput>>);

impl Sink for SharedInput {
    fn audio(&mut self, data: &[u8], _frames: usize, _format: Format) {
        if let Ok(mut input) = self.0.try_lock() {
            input.push(data);
        }
    }

    fn silence(&mut self, frames: usize, _format: Format) {
        if let Ok(mut input) = self.0.try_lock() {
            input.push_silence(frames);
        }
    }
}

pub(crate) fn input_format(f: Format) -> InputFormat {
    InputFormat {
        sample_rate: f.sample_rate,
        channels: f.channels,
        sample: match f.sample_format {
            SampleFormat::F32 => SampleType::F32,
            SampleFormat::I16 => SampleType::I16,
            SampleFormat::I32 => SampleType::I32,
        },
    }
}

/// Application capture that follows the app: attaches by executable name and
/// notices when the process exits, so it can attach again when it is
/// started anew
struct AppSource {
    exe: String,
    sample_rate: u32,
    running: Option<Running>,
    process_id: u32,
    failed_pid: u32,
}

impl AppSource {
    fn attach(&mut self, input: &SharedInput) -> bool {
        let Some(pid) = audio_capture::find_process(&self.exe) else { return false };
        let options = CaptureOptions { preferred_rate: Some(self.sample_rate), fill_silence: true };
        let started = Capture::open(&Source::Application { process_id: pid }, &options)
            .and_then(|c| c.start(Box::new(input.clone())));
        match started {
            Ok(r) => {
                tracing::info!("capturing app {} (PID {pid})", self.exe);
                self.running = Some(r);
                self.process_id = pid;
                true
            }
            Err(e) => {
                if self.failed_pid != pid {
                    tracing::warn!("cannot capture {} (PID {pid}): {e}", self.exe);
                }
                self.failed_pid = pid;
                false
            }
        }
    }

    fn exited(&self) -> bool {
        self.running.is_some()
            && (audio_capture::process_exited(self.process_id) || self.running.as_ref().is_some_and(|r| r.ended()))
    }

    fn detach(&mut self) {
        if let Some(r) = self.running.take() {
            r.stop();
        }
    }
}

enum ActiveSource {
    Capture(Running),
    App(AppSource),
}

impl ActiveSource {
    /// An application capture whose app is not running
    fn waiting(&self) -> bool {
        matches!(self, ActiveSource::App(a) if a.running.is_none())
    }

    fn stop(self) {
        match self {
            ActiveSource::Capture(r) => r.stop(),
            ActiveSource::App(mut a) => a.detach(),
        }
    }
}

/// Restores the default output device when the session ends (virtual
/// device mode switched it)
pub(crate) struct DefaultDeviceGuard(pub(crate) Option<String>);

impl DefaultDeviceGuard {
    /// Makes `id` the default output device. The device before is restored
    /// at the end even when the switch failed: Windows may have switched
    /// one role (Console, Multimedia) before the other failed.
    pub(crate) fn switch_to(id: &str, name: &str) -> DefaultDeviceGuard {
        let before = audio_capture::default_render_device();
        if let Err(e) = audio_capture::set_default_render_device(id) {
            tracing::warn!("failed to make '{name}' the default output device: {e}");
        }
        DefaultDeviceGuard(restore_target(before, id))
    }
}

/// The device to restore: the one before, unless that is the one switched to
fn restore_target(before: Option<String>, switched_to: &str) -> Option<String> {
    before.filter(|b| b != switched_to)
}

/// Logs the gaps WASAPI reports in the captured audio as they increase, at
/// most every 2 s (v1 WasapiCapture: "Data discontinuity (count=N)"; the
/// capture already skips the ones right after it starts)
#[derive(Default)]
pub(crate) struct DiscontinuityLog {
    reported: u64,
    last: Option<Instant>,
}

const DISCONTINUITY_LOG_INTERVAL: Duration = Duration::from_secs(2);

impl DiscontinuityLog {
    /// The capture's count now; Some((count, new since the last log)) when
    /// it is time to log. A new capture (count below the last) starts over.
    pub(crate) fn check(&mut self, count: u64, now: Instant) -> Option<(u64, u64)> {
        if count < self.reported {
            self.reported = 0;
        }
        if count == self.reported
            || self.last.is_some_and(|t| now.saturating_duration_since(t) < DISCONTINUITY_LOG_INTERVAL)
        {
            return None;
        }
        let new = count - self.reported;
        self.reported = count;
        self.last = Some(now);
        Some((count, new))
    }

    pub(crate) fn tick(&mut self, count: u64) {
        if let Some((count, new)) = self.check(count, Instant::now()) {
            tracing::warn!("capture: data discontinuity (count={count}, {new} new)");
        }
    }
}

impl Drop for DefaultDeviceGuard {
    fn drop(&mut self) {
        if let Some(id) = self.0.take()
            && let Err(e) = audio_capture::set_default_render_device(&id)
        {
            tracing::warn!("could not restore the default output device: {e}");
        }
    }
}

/// Why a stack is restarted after sleep (also in the log and the flow)
pub(crate) const RESUMED: &str = "the PC resumed from sleep";
const ADAPTER_STOPPED: &str = "the Bluetooth adapter stopped working";
const RECONNECT_ATTEMPTS: u32 = 10;

/// What the streaming loop does about the link
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkCheck {
    Fine,
    /// The device dropped the link: connect it again on the same stack
    Reconnect,
    /// The stack cannot be used any more: start a new one first
    Restart(&'static str),
    /// The PC is going to sleep and took the adapter (or the link) with it:
    /// stop the stack and wait for the resume, spending no attempt
    SleepThrough,
}

fn check_link(lost: bool, stalled: bool, stale: bool, suspending: bool) -> LinkCheck {
    if suspending {
        // A stack left from an earlier sleep is replaced after this one;
        // while the adapter still works, the stream goes on
        return if lost || stalled { LinkCheck::SleepThrough } else { LinkCheck::Fine };
    }
    if stale {
        LinkCheck::Restart(RESUMED)
    } else if stalled {
        LinkCheck::Restart(ADAPTER_STOPPED)
    } else if lost {
        LinkCheck::Reconnect
    } else {
        LinkCheck::Fine
    }
}

/// The PC is going to sleep: the stack is shut down quietly (the pipeline's
/// media sink belongs to it) and the session waits for the resume. Returns
/// why the stack is then started anew, None when the session was stopped.
fn sleep_through(inner: &Inner, pipeline: &mut Option<Pipeline>) -> Option<&'static str> {
    tracing::info!("the PC is going to sleep, reconnecting after it resumes");
    if let Some(p) = pipeline.take() {
        let _ = p.stop();
    }
    inner.shutdown_host_for_sleep();
    if !inner.wait_for_resume() {
        return None;
    }
    Some(if inner.power.stale() { RESUMED } else { ADAPTER_STOPPED })
}

fn connect_error(f: Option<ConnectFailure>) -> Msg {
    Msg::new(match f {
        Some(ConnectFailure::Stalled) => "error.btstack_no_answer",
        Some(ConnectFailure::NoAnswer) => "error.connect_no_answer",
        Some(ConnectFailure::LinkLost) => "error.connect_link_lost",
        Some(ConnectFailure::Refused) => "error.connect_refused",
        Some(ConnectFailure::AuthFailed) => "error.connect_auth_failed",
        _ => "error.pairing_hint",
    })
}

fn peer_params(remote: &RemoteCaps, kind: CodecKind, channels: u32) -> PeerParams {
    let (opus_frame_ms, opus_channels, opus_max_bitrate) = remote.opus_params(kind, channels);
    PeerParams {
        lhdcv5_bitrate_limits: remote.lhdcv5_bitrate_limits(),
        sbc_min_bitpool: remote.sbc_min_bitpool(),
        sbc_max_bitpool: remote.sbc_max_bitpool(),
        opus_frame_ms,
        opus_channels,
        opus_max_bitrate,
    }
}

#[derive(Debug)]
enum ConfigFailure {
    /// No configuration of the codec for this capture
    Build(ConfigError),
    /// BTstack or the device refused it, or no answer
    Host(ConfigureError),
}

/// SET_CONFIGURATION of a `kind` stream; the media payload size
fn configure_stream(
    host: &BtHost,
    remote: &RemoteCaps,
    kind: CodecKind,
    sample_rate: u32,
    channels: u32,
    bits: u8,
) -> Result<(Configuration, u16), ConfigFailure> {
    let config = remote.configuration(kind, sample_rate, channels, bits).map_err(ConfigFailure::Build)?;
    let mtu = host.configure(&config).map_err(ConfigFailure::Host)?;
    Ok((config, mtu))
}

/// SBC XQ refused by the device (one that lists dual channel but does not
/// take it): worth one more try with plain SBC when the codec was Auto or
/// codec fallback is on
fn sbc_xq_fallback(kind: CodecKind, failure: &ConfigFailure, requested: Option<CodecKind>, fallback: bool) -> bool {
    kind == CodecKind::SbcXq
        && matches!(failure, ConfigFailure::Host(ConfigureError::Refused | ConfigureError::NoAnswer))
        && (requested.is_none() || fallback)
}

/// Starts the encode thread; ABR as the encoder runs it
fn start_pipeline(
    encoder: Box<dyn Encoder>,
    format: Format,
    channels: u32,
    mtu: u16,
    sink: MediaSink,
    host: &BtHost,
) -> (Pipeline, CaptureInput) {
    let abr = encoder.abr_active();
    Pipeline::start(
        PipelineConfig { encoder, input: input_format(format), channels, mtu, abr, buffer: BufferPreset::Standard },
        sink,
        host.stats().clone(),
    )
}

/// The session thread. Returns when stopped or on an error (state reported).
pub(crate) fn run(inner: &Arc<Inner>, req: StreamRequest) {
    let addr = bt_host::addr_str(&req.device);
    tracing::info!(
        "stream: device {addr} codec {} quality {:?} bit depth {} rate {} source {}",
        req.codec.map_or("auto", |c| c.name()),
        req.quality,
        req.bit_depth.map_or("auto".into(), |b| format!("{b}-bit")),
        req.sample_rate.map_or("device's".into(), |r| format!("{r} Hz")),
        req.source
    );
    let stopped = || inner.stop_requested();

    inner.notify(StreamState::Connecting, Msg::new("status.initializing_btstack"));
    let mut host = match inner.ensure_host() {
        Ok(h) => h,
        Err(m) => return inner.notify(StreamState::Error, m),
    };
    host.set_media_payload_limit(req.max_media_payload);
    if stopped() {
        return inner.notify(StreamState::Idle, Msg::new("status.ready"));
    }

    // Remembered headphone volume, set as soon as AVRCP connects
    let restore = inner.saved_volume(&addr);
    host.set_restore_volume(restore);

    // ---- connect ----
    inner.notify(StreamState::Connecting, Msg::new("status.connecting_device"));
    let connect = |h: &BtHost| {
        h.connect_retrying(req.device, &stopped, &mut |attempt, attempts| {
            inner.notify(StreamState::Connecting, Msg::with("status.connecting_retry", &[&attempt, &attempts]));
        })
    };
    let mut result = connect(&host);
    if result.is_err() && !stopped() && host.last_connect_failure() == Some(ConnectFailure::Stalled) {
        // Not the device: the stack it went through stopped working
        inner.notify(StreamState::Connecting, Msg::new("status.initializing_btstack"));
        host = match inner.restart_host("the Bluetooth adapter stopped working", req.max_media_payload) {
            Ok(h) => h,
            Err(m) => return inner.notify(StreamState::Error, m),
        };
        if stopped() {
            return inner.notify(StreamState::Idle, Msg::new("status.ready"));
        }
        inner.notify(StreamState::Connecting, Msg::new("status.connecting_device"));
        result = connect(&host);
    }
    let mut remote = match result {
        Ok(caps) => caps,
        Err(_) if stopped() => return inner.notify(StreamState::Idle, Msg::new("status.ready")),
        Err(f) => return inner.notify(StreamState::Error, connect_error(Some(f))),
    };
    inner.record_codecs(&addr, codec_keys(&remote));
    tracing::info!("device connected, choosing the codec");
    if stopped() {
        host.disconnect();
        return inner.notify(StreamState::Idle, Msg::new("status.ready"));
    }

    // ---- codec ----
    let choice = match caps::choose(&remote, req.codec, req.codec_fallback) {
        Ok(c) => c,
        Err(why) => {
            host.disconnect();
            let msg = match why {
                None => Msg::new("error.no_compatible_codec"),
                Some((codec, Unavailable::SepInUse { in_use, total })) => {
                    Msg::with("error.codec_sep_in_use", &[&codec.name(), &in_use, &total])
                }
                Some((codec, Unavailable::AdaptiveOnly)) => Msg::with("error.codec_adaptive_only", &[&codec.name()]),
                Some((codec, Unavailable::NotSupported)) => Msg::with("error.codec_not_supported", &[&codec.name()]),
            };
            return inner.notify(StreamState::Error, msg);
        }
    };
    let mut kind = choice.kind;
    let mut replaced = choice.replaced;
    if let Some((wanted, why)) = &replaced {
        tracing::warn!("codec fallback: {} not available ({why:?}), using {}", wanted.name(), kind.name());
    }
    tracing::info!("codec selected: {}", kind.name());

    // ---- audio source ----
    inner.notify(StreamState::Connecting, Msg::new("status.initializing_audio"));
    let test_tone = req.source == SourceRequest::TestTone;
    let mut preferred = match (&req.source, req.sample_rate) {
        (_, Some(r)) => r,
        (SourceRequest::TestTone, None) => 48000,
        (SourceRequest::VirtualDevice { id, .. }, None) => audio_capture::mix_rate(Some(id)).unwrap_or(48000),
        (_, None) => audio_capture::mix_rate(None).unwrap_or(48000),
    };
    if preferred > kind.max_sample_rate() {
        tracing::info!(
            "limiting the capture rate from {preferred} to {} Hz for {}",
            kind.max_sample_rate(),
            kind.name()
        );
        preferred = kind.max_sample_rate();
    }
    // aptX family: 44.1 / 48 kHz as the sink lists them; LHDC V5: its rates;
    // Opus, Opus 05: 48 kHz only. WASAPI resamples to the requested rate.
    let picked = match kind {
        CodecKind::Aptx | CodecKind::AptxHd | CodecKind::AptxLl => remote.pick_aptx_sample_rate(kind, preferred),
        CodecKind::LhdcV5 => remote.pick_lhdcv5_sample_rate(preferred),
        CodecKind::FastStream => remote.pick_faststream_sample_rate(preferred),
        CodecKind::Opus | CodecKind::Opus05 => 48000,
        CodecKind::Sbc | CodecKind::SbcXq | CodecKind::Aac if preferred != 44100 => 48000,
        _ => preferred,
    };
    if picked != preferred {
        tracing::info!("{}: capturing at {picked} Hz instead of {preferred} Hz (supported rate)", kind.name());
        preferred = picked;
    }

    let mut default_guard = DefaultDeviceGuard(None);
    let options = CaptureOptions { preferred_rate: Some(preferred), fill_silence: !test_tone };
    let (source, opened): (Option<Source>, Option<Capture>) = match &req.source {
        SourceRequest::TestTone => (Some(Source::TestTone), None),
        SourceRequest::SystemLoopback => (Some(Source::SystemLoopback), None),
        SourceRequest::VirtualDevice { id, name, auto_switch } => {
            if id.is_empty() {
                host.disconnect();
                return inner.notify(StreamState::Error, Msg::new("error.no_virtual_device_selected"));
            }
            if *auto_switch {
                default_guard = DefaultDeviceGuard::switch_to(id, name);
            }
            (Some(Source::RenderDevice { id: id.clone() }), None)
        }
        SourceRequest::Application { exe } => {
            let build = audio_capture::windows_build();
            let msg = if build < audio_capture::APP_CAPTURE_MIN_BUILD {
                Some(Msg::with("error.app_capture_unsupported", &[&build]))
            } else if exe.is_empty() {
                Some(Msg::new("error.no_app_selected"))
            } else {
                None
            };
            if let Some(m) = msg {
                host.disconnect();
                return inner.notify(StreamState::Error, m);
            }
            (None, None)
        }
    };
    let _ = opened;
    let capture = match &source {
        Some(s) => match Capture::open(s, &options) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::error!("audio capture: {e}");
                host.disconnect();
                let key = if matches!(req.source, SourceRequest::VirtualDevice { .. }) {
                    "error.virtual_capture_init"
                } else {
                    "error.wasapi_init"
                };
                return inner.notify(StreamState::Error, Msg::new(key));
            }
        },
        None => None,
    };
    // Application capture: float stereo at the requested rate, known before
    // the app runs
    let format = capture.as_ref().map(|c| c.format()).unwrap_or(Format {
        sample_rate: preferred,
        channels: 2,
        sample_format: SampleFormat::F32,
    });
    let channels = (format.channels as u32).min(2);
    tracing::info!(
        "capture: {} Hz, {} channel(s), {:?} -> {channels} channel(s)",
        format.sample_rate,
        format.channels,
        format.sample_format
    );

    // LDAC, aptX HD and LHDC V5 carry 24-bit unless the profile says 16
    // (LHDC V5 only when the sink has 24-bit)
    let bits: u8 = match (kind, req.bit_depth) {
        (_, Some(16)) => 16,
        (CodecKind::Ldac | CodecKind::AptxHd, _) => 24,
        (CodecKind::LhdcV5, _) if remote.lhdcv5_24bit() => 24,
        _ => 16,
    };

    // ---- stream configuration ----
    let mut configured = configure_stream(&host, &remote, kind, format.sample_rate, channels, bits);
    if let Err(failure) = &configured
        && !stopped()
        && sbc_xq_fallback(kind, failure, req.codec, req.codec_fallback)
    {
        // The sink lists dual channel but refuses it. BTstack's A2DP setup
        // of this connection ends with the refusal: connect again and take
        // plain SBC (SBC XQ is a dual-channel SBC: same rates, channels and
        // bits)
        tracing::warn!("SBC XQ: the device refused its configuration ({failure:?}), connecting again for SBC");
        host.disconnect();
        inner.notify(StreamState::Connecting, Msg::new("status.connecting_device"));
        remote = match connect(&host) {
            Ok(caps) => caps,
            Err(_) if stopped() => return inner.notify(StreamState::Idle, Msg::new("status.ready")),
            Err(f) => return inner.notify(StreamState::Error, connect_error(Some(f))),
        };
        kind = CodecKind::Sbc;
        if req.codec.is_some() && replaced.is_none() {
            replaced = Some((CodecKind::SbcXq, Unavailable::NotSupported));
        }
        configured = configure_stream(&host, &remote, kind, format.sample_rate, channels, bits);
    }
    let (mut config, mtu) = match configured {
        Ok(c) => c,
        Err(e) => {
            if let ConfigFailure::Build(e) = &e {
                tracing::error!("{e}");
            }
            host.disconnect();
            return if stopped() {
                inner.notify(StreamState::Idle, Msg::new("status.ready"))
            } else {
                inner.notify(StreamState::Error, Msg::new("error.codec_configure"))
            };
        }
    };
    let kind = kind;
    let fallback_text = replaced.as_ref().map(|(wanted, why)| {
        let key = if matches!(why, Unavailable::SepInUse { .. }) {
            "status.connected_fallback_in_use"
        } else {
            "status.connected_fallback"
        };
        Msg::with(key, &[&wanted.name(), &kind.name()])
    });
    let mut encoder_config = EncoderConfig {
        sample_rate: format.sample_rate,
        channels,
        quality: req.quality,
        mtu,
        bits_per_sample: bits,
        abr: req.abr && matches!(kind, CodecKind::Ldac | CodecKind::LhdcV5),
        peer: peer_params(&remote, kind, channels),
    };
    let encoder = match codec::create(kind, &encoder_config) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("encoder: {e}");
            host.disconnect();
            return inner.notify(StreamState::Error, Msg::new("error.encoder_init"));
        }
    };
    let bitrate = encoder.bitrate_kbps();
    let bitrate_capped = encoder.bitrate_capped();
    // ABR as the encoder runs it (LDAC keeps a fixed quality when its ABR
    // module fails to start)
    let abr = encoder.abr_active();
    tracing::info!("encoder ready: {} {bitrate} kbps, mtu {mtu}, {bits}-bit, ABR {abr}", kind.name());

    if !host.start_stream() {
        host.disconnect();
        return if stopped() {
            inner.notify(StreamState::Idle, Msg::new("status.ready"))
        } else {
            inner.notify(StreamState::Error, Msg::new("error.stream_start"))
        };
    }

    inner.notify(StreamState::Streaming, fallback_text.clone().unwrap_or_else(|| Msg::new("status.connected")));
    let mut info = StreamInfo {
        codec: kind.name().into(),
        bitrate_kbps: bitrate,
        sample_rate: format.sample_rate,
        channels,
        source_sample_rate: format.sample_rate,
        source_channels: format.channels as u32,
        source_bits: (format.sample_format.bytes_per_sample() * 8) as u32,
        requested_codec: replaced.as_ref().map(|(w, _)| w.name().to_string()),
        requested_in_use: matches!(replaced, Some((_, Unavailable::SepInUse { .. }))),
        abr,
        bitrate_capped,
        source: req.source.info(),
    };
    inner.save_device(&addr, &req.device_name);

    // ---- pipeline and capture ----
    let Some(sink) = host.take_media_sink() else {
        host.disconnect();
        return inner.notify(StreamState::Error, Msg::new("error.encode_thread"));
    };
    let (pipeline, input) = start_pipeline(encoder, format, channels, mtu, sink, &host);
    let input = SharedInput(Arc::new(Mutex::new(input)));
    let active = match (capture, &req.source) {
        (Some(c), _) => match c.start(Box::new(input.clone())) {
            Ok(r) => Some(ActiveSource::Capture(r)),
            Err(e) => {
                tracing::error!("audio capture start: {e}");
                None
            }
        },
        (None, SourceRequest::Application { exe }) => {
            let mut app =
                AppSource { exe: exe.clone(), sample_rate: preferred, running: None, process_id: 0, failed_pid: 0 };
            if !app.attach(&input) {
                // Not running yet is fine: the capture attaches once it is up
                tracing::info!("{exe} is not running yet, waiting for it");
            }
            Some(ActiveSource::App(app))
        }
        _ => None,
    };
    let Some(mut active) = active else {
        if let Some(s) = pipeline.stop() {
            host.return_media_sink(s);
        }
        host.disconnect();
        return inner.notify(StreamState::Error, Msg::new("error.audio_capture_start"));
    };
    info.source.waiting = active.waiting();
    inner.stream_info(info.clone());

    // Headphone volume and a Windows volume act as one (volume_sync.rs)
    let mut volume = crate::volume_sync::EndpointChoice::for_stream(&req.volume_sync, &req.source)
        .map(crate::volume_sync::VolumeSyncer::new);

    // A reconnect builds the configuration anew from what the device lists
    // then (v1 read its capabilities again)
    let rebuild = |r: &RemoteCaps| match r.configuration(kind, format.sample_rate, channels, bits) {
        Ok(c) => Some(c),
        Err(e) => {
            tracing::warn!("reconnect: {e}");
            None
        }
    };

    // ---- streaming ----
    let mut pipeline = Some(pipeline);
    let mut app_ticks = 0u32;
    let mut lost_for_good = false;
    let mut discontinuities = DiscontinuityLog::default();
    while !stopped() {
        inner.wait_tick(Duration::from_millis(200));
        if stopped() {
            break;
        }
        if let Some(v) = volume.as_mut() {
            v.tick(true, &*host);
        }
        inner.remember_volume(&addr, host.remote_volume());

        // Application capture follows the app: it may quit and start again
        if let ActiveSource::App(app) = &mut active {
            if app.exited() {
                tracing::info!("{} exited, waiting for it to start again", app.exe);
                app.detach();
            }
            app_ticks += 1;
            if app.running.is_none() && app_ticks.is_multiple_of(5) {
                app.attach(&input);
            }
        }
        if active.waiting() != info.source.waiting {
            info.source.waiting = active.waiting();
            inner.stream_info(info.clone());
        }
        let source_gaps = match &active {
            ActiveSource::Capture(r) => r.discontinuities(),
            ActiveSource::App(a) => a.running.as_ref().map_or(0, Running::discontinuities),
        };
        host.stats().capture_discontinuities.store(source_gaps,std::sync::atomic::Ordering::Relaxed);
        discontinuities.tick(source_gaps);

        // Sleep, or an adapter that stopped working, leaves a stack that
        // reports no disconnect: it is started anew and the device connected
        // again on it
        let lost = host.check_disconnected();
        let check = check_link(lost, host.stalled(), inner.power.stale(), inner.power.suspending());
        let mut restart_why = match check {
            LinkCheck::Fine => continue,
            LinkCheck::Reconnect => {
                tracing::warn!("connection lost, reconnecting");
                None
            }
            LinkCheck::Restart(RESUMED) => {
                tracing::info!("{RESUMED}, reconnecting");
                Some(RESUMED)
            }
            LinkCheck::Restart(why) => {
                tracing::warn!("{why}, reconnecting");
                Some(why)
            }
            LinkCheck::SleepThrough => {
                inner.notify(StreamState::Reconnecting, Msg::new("status.connection_lost"));
                let Some(why) = sleep_through(inner, &mut pipeline) else { break };
                tracing::info!("{why}, reconnecting");
                Some(why)
            }
        };
        inner.notify(StreamState::Reconnecting, Msg::new("status.connection_lost"));

        let mut reconnected = None;
        let mut codec_gone = false;
        let mut fresh_stack = false;
        let mut attempt = 0u32;
        while attempt < RECONNECT_ATTEMPTS {
            if stopped() {
                break;
            }
            // Sleep that comes while reconnecting: no attempt is spent on it
            if inner.power.suspending() {
                let Some(why) = sleep_through(inner, &mut pipeline) else { break };
                tracing::info!("{why}, reconnecting");
                restart_why = Some(why);
            }
            attempt += 1;
            inner.notify(
                StreamState::Reconnecting,
                Msg::with("status.reconnect_attempt", &[&attempt, &RECONNECT_ATTEMPTS]),
            );
            // React to a stop within the wait
            let until = Instant::now() + Duration::from_secs(3);
            while Instant::now() < until && !stopped() {
                inner.wait_tick(Duration::from_millis(100));
            }
            if stopped() {
                break;
            }
            if inner.power.suspending() {
                // Went to sleep in the wait: the next round sleeps through
                attempt -= 1;
                continue;
            }
            if restart_why.is_none() {
                if inner.power.stale() {
                    restart_why = Some(RESUMED);
                } else if host.stalled() {
                    restart_why = Some(ADAPTER_STOPPED);
                }
            }
            if let Some(why) = restart_why.take() {
                // The pipeline's media sink belongs to the old stack
                if let Some(p) = pipeline.take() {
                    let _ = p.stop();
                }
                match inner.restart_host(why, req.max_media_payload) {
                    Ok(h) => {
                        host = h;
                        fresh_stack = true;
                    }
                    Err(_) if inner.power.suspending() => {
                        // The adapter went away for sleep meanwhile
                        attempt -= 1;
                        continue;
                    }
                    Err(_) => {
                        tracing::warn!("reconnect attempt {attempt} failed (no Bluetooth adapter)");
                        restart_why = Some("the last restart failed");
                        continue;
                    }
                }
                host.set_restore_volume(inner.saved_volume(&addr));
                if stopped() {
                    break;
                }
            }
            // A new stack knows neither the device nor the stream: connect
            // and configure in full
            let result = if fresh_stack {
                match host.connect(req.device) {
                    Ok(caps) => host.configure_and_start(caps, &config, &rebuild),
                    Err(_) => Err(ReconnectError::Failed),
                }
            } else {
                host.reconnect(&rebuild)
            };
            match result {
                Ok(r) => {
                    tracing::info!("reconnected on attempt {attempt}");
                    reconnected = Some(r);
                    break;
                }
                Err(ReconnectError::CodecGone) => {
                    codec_gone = true;
                    break;
                }
                Err(ReconnectError::Failed) => {}
            }
            if inner.power.suspending() {
                tracing::info!("reconnect attempt {attempt} ended by sleep, not counted");
                attempt -= 1;
                continue;
            }
            tracing::warn!("reconnect attempt {attempt} failed");
        }
        let Some(r) = reconnected else {
            if !stopped() {
                let msg = if codec_gone {
                    Msg::with("error.codec_not_supported", &[&kind.name()])
                } else {
                    Msg::new("error.reconnect_failed")
                };
                inner.notify(StreamState::Error, msg);
                lost_for_good = true;
            }
            break;
        };
        // The encoder follows the new stream: its MTU and the device's
        // settings (e.g. its SBC bitpool range) may have changed
        let new_config = EncoderConfig { mtu: r.mtu, peer: peer_params(&r.remote, kind, channels), ..encoder_config };
        if r.mtu != encoder_config.mtu {
            tracing::info!("reconnect: media payload size {} -> {}", encoder_config.mtu, r.mtu);
        }
        let changed = new_config != encoder_config;
        encoder_config = new_config;
        config = r.config;
        // Resume with the same encoder when nothing changed; otherwise, and
        // on a new stack (its media sink is new), with a new pipeline
        if let Some(p) = pipeline.as_ref()
            && !changed
        {
            p.reset();
        } else {
            let sink = match pipeline.take() {
                Some(p) => p.stop(),
                None => host.take_media_sink(),
            };
            let Some(sink) = sink else {
                inner.notify(StreamState::Error, Msg::new("error.encode_thread"));
                lost_for_good = true;
                break;
            };
            let encoder = match codec::create(kind, &encoder_config) {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!("encoder: {e}");
                    host.return_media_sink(sink);
                    inner.notify(StreamState::Error, Msg::new("error.encoder_init"));
                    lost_for_good = true;
                    break;
                }
            };
            if changed {
                tracing::info!(
                    "encoder set up again: {} {} kbps, mtu {}, ABR {}",
                    kind.name(),
                    encoder.bitrate_kbps(),
                    r.mtu,
                    encoder.abr_active()
                );
            }
            info.bitrate_kbps = encoder.bitrate_kbps();
            info.bitrate_capped = encoder.bitrate_capped();
            info.abr = encoder.abr_active();
            let (p, new_input) = start_pipeline(encoder, format, channels, r.mtu, sink, &host);
            if let Ok(mut i) = input.0.lock() {
                *i = new_input;
            }
            pipeline = Some(p);
            inner.stream_info(info.clone());
        }
        inner.notify(StreamState::Streaming, Msg::new("status.reconnected"));
    }

    // ---- cleanup: producer first, then the encode thread, then the stream ----
    tracing::info!("streaming stopped, cleaning up");
    active.stop();
    if let Some(p) = pipeline.take()
        && let Some(s) = p.stop()
    {
        host.return_media_sink(s);
    }
    if let Some(mut v) = volume.take() {
        v.stop();
    }
    host.stop_stream();
    host.disconnect();
    host.set_restore_volume(None);
    drop(default_guard);
    if !lost_for_good {
        inner.notify(StreamState::Idle, Msg::new("status.ready"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_working_link_is_left_alone() {
        assert_eq!(check_link(false, false, false, false), LinkCheck::Fine);
    }

    #[test]
    fn failures_while_awake_reconnect_or_restart() {
        assert_eq!(check_link(true, false, false, false), LinkCheck::Reconnect);
        assert_eq!(check_link(false, true, false, false), LinkCheck::Restart(ADAPTER_STOPPED));
        assert_eq!(check_link(true, true, false, false), LinkCheck::Restart(ADAPTER_STOPPED));
    }

    #[test]
    fn after_resume_the_stack_is_restarted() {
        assert_eq!(check_link(false, false, true, false), LinkCheck::Restart(RESUMED));
        assert_eq!(check_link(true, true, true, false), LinkCheck::Restart(RESUMED));
    }

    #[test]
    fn going_to_sleep_sleeps_through_a_lost_adapter_or_link() {
        // The hardware log of 2026-10-17: transfers cancelled after the
        // going-to-sleep notification
        assert_eq!(check_link(false, true, false, true), LinkCheck::SleepThrough);
        assert_eq!(check_link(true, false, false, true), LinkCheck::SleepThrough);
        assert_eq!(check_link(true, true, false, true), LinkCheck::SleepThrough);
        // A stack still stale from an earlier sleep is not restarted now
        assert_eq!(check_link(false, true, true, true), LinkCheck::SleepThrough);
    }

    #[test]
    fn sources_as_the_ui_shows_them() {
        assert_eq!(SourceRequest::TestTone.info(), SourceInfo { kind: "tone", name: String::new(), waiting: false });
        let app = SourceRequest::Application { exe: "Spotify.exe".into() }.info();
        assert_eq!((app.kind, app.name.as_str()), ("app", "Spotify.exe"));
        let dev = SourceRequest::VirtualDevice { id: "{x}".into(), name: "CABLE".into(), auto_switch: true }.info();
        assert_eq!((dev.kind, dev.name.as_str()), ("virtual", "CABLE"));
        assert_eq!(SourceRequest::SystemLoopback.info().kind, "loopback");
    }

    #[test]
    fn codec_keys_of_a_device() {
        let mut remote = caps::RemoteCaps::default();
        assert!(codec_keys(&remote).is_empty());
        remote.add_sbc(1);
        remote.add_aac(2);
        let keys = codec_keys(&remote);
        assert!(keys.contains(&"sbc".to_string()) && keys.contains(&"aac".to_string()));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn going_to_sleep_keeps_a_working_stream() {
        assert_eq!(check_link(false, false, false, true), LinkCheck::Fine);
        assert_eq!(check_link(false, false, true, true), LinkCheck::Fine);
    }

    #[test]
    fn the_device_before_is_restored_unless_it_was_the_target() {
        assert_eq!(restore_target(Some("{a}".into()), "{b}"), Some("{a}".into()));
        assert_eq!(restore_target(Some("{b}".into()), "{b}"), None);
        assert_eq!(restore_target(None, "{b}"), None);
    }

    #[test]
    fn discontinuities_are_logged_as_they_increase_at_most_every_2_s() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut log = DiscontinuityLog::default();
        assert_eq!(log.check(0, at(0)), None);
        assert_eq!(log.check(1, at(200)), Some((1, 1)));
        // More within 2 s: held back, reported together after
        assert_eq!(log.check(3, at(400)), None);
        assert_eq!(log.check(4, at(2000)), None);
        assert_eq!(log.check(4, at(2200)), Some((4, 3)));
        // No change: nothing to say, however long
        assert_eq!(log.check(4, at(9000)), None);
        assert_eq!(log.check(5, at(9200)), Some((5, 1)));
        // A new capture counts from 0 again
        assert_eq!(log.check(2, at(12000)), Some((2, 2)));
    }

    #[test]
    fn a_refused_sbc_xq_falls_back_to_sbc_when_allowed() {
        let refused = ConfigFailure::Host(ConfigureError::Refused);
        let no_answer = ConfigFailure::Host(ConfigureError::NoAnswer);
        let xq = CodecKind::SbcXq;
        assert!(sbc_xq_fallback(xq, &refused, None, false));
        assert!(sbc_xq_fallback(xq, &no_answer, None, false));
        assert!(sbc_xq_fallback(xq, &refused, Some(xq), true));
        // Asked for SBC XQ without codec fallback: an error, as before
        assert!(!sbc_xq_fallback(xq, &refused, Some(xq), false));
        // Not connected any more, or no configuration of it: no retry
        assert!(!sbc_xq_fallback(xq, &ConfigFailure::Host(ConfigureError::NotConnected), None, false));
        assert!(!sbc_xq_fallback(xq, &ConfigFailure::Build(ConfigError::NotOffered(xq)), None, false));
        // Other codecs are not retried
        assert!(!sbc_xq_fallback(CodecKind::Ldac, &refused, None, true));
    }
}
