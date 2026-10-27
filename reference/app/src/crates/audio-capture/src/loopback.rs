//! WASAPI loopback capture: a render device (the default one or a chosen
//! one) or one application's process tree.
//!
//! Every COM object of a stream lives on the capture thread. The thread is
//! started by `open` (which waits for it to report the format), parks until
//! `start` hands it the sink, then runs the event-driven capture loop.
//! Keeping the interfaces on one thread is what lets this crate stay free of
//! `unsafe`: the `wasapi` types are not `Send`.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use wasapi::{
    AudioCaptureClient, AudioClient, DeviceEnumerator, Direction, Handle, SampleType, StreamMode, WaveFormat,
};
use win_sys::audio_policy::ComApartment;
use win_sys::process::ProcessWatch;

use crate::format::{self, FormatPlan, MixFormat};
use crate::silence::SilenceFill;
use crate::{APP_CAPTURE_MIN_BUILD, CaptureError, CaptureOptions, EndReason, Format, Shared, Sink};

/// The stream buffer v1 asks for: 10 ms (in 100 ns units). In shared mode
/// the engine period decides the packet size, normally also 10 ms.
const BUFFER_HNS: i64 = 100_000;
/// Used when the client cannot tell its period (process loopback)
const DEFAULT_PERIOD: Duration = Duration::from_millis(10);
/// Windows usually flags the first buffers after Start() as discontinuous:
/// nothing was lost, so those are not counted (v1)
const DISCONTINUITY_GRACE: Duration = Duration::from_millis(500);
/// How often an application capture checks whether the app is still there
const PROCESS_CHECK: Duration = Duration::from_millis(250);
/// Polling fallback interval (v1)
const POLL_INTERVAL: Duration = Duration::from_millis(5);
/// How long `open` waits for the thread to activate and initialise the
/// client. Process loopback activation is asynchronous, and v1 gave it 5 s.
const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

/// What to capture from
#[derive(Clone, Debug)]
pub(crate) enum Target {
    /// Loopback of a render endpoint; None = the default one (Console role)
    Device(Option<String>),
    /// Process loopback of this process and its children
    Process(u32),
}

/// HRESULTs that mean the source went away rather than a fault of ours
const AUDCLNT_E_DEVICE_INVALIDATED: u32 = 0x8889_0004;
const AUDCLNT_E_SERVICE_NOT_RUNNING: u32 = 0x8889_0010;

enum Command {
    Start(Box<dyn Sink>, mpsc::SyncSender<Result<(), CaptureError>>),
}

/// An opened stream whose thread waits for `start`
pub(crate) struct Opened {
    pub format: Format,
    commands: mpsc::Sender<Command>,
    thread: JoinHandle<()>,
    shared: Arc<Shared>,
}

impl Opened {
    pub fn open(target: Target, options: &CaptureOptions) -> Result<Opened, CaptureError> {
        // Checked here, before a thread exists, since this one is cheap
        if let Target::Process(_) = target {
            let build = crate::windows_build();
            if build < APP_CAPTURE_MIN_BUILD {
                return Err(CaptureError::UnsupportedBuild(build));
            }
        }
        let shared = Arc::new(Shared::default());
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let options = options.clone();
        let thread_shared = shared.clone();
        let thread = thread::Builder::new()
            .name("audio-capture".into())
            .spawn(move || thread_main(target, options, ready_tx, cmd_rx, thread_shared))
            .map_err(|e| CaptureError::Thread(e.to_string()))?;
        match ready_rx.recv_timeout(OPEN_TIMEOUT) {
            Ok(Ok(format)) => Ok(Opened { format, commands: cmd_tx, thread, shared }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // The activation is stuck inside Windows. The thread cannot
                // be interrupted; it exits on its own if the call ever
                // returns (it finds the command channel closed), so it is
                // left detached rather than joined here.
                tracing::warn!("audio capture: opening timed out");
                Err(CaptureError::Timeout("opening the audio client"))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = thread.join();
                Err(CaptureError::ThreadGone)
            }
        }
    }

    /// Hand the sink to the thread and start the stream
    pub fn start(self, sink: Box<dyn Sink>) -> Result<(JoinHandle<()>, Arc<Shared>), CaptureError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        if self.commands.send(Command::Start(sink, reply_tx)).is_err() {
            let _ = self.thread.join();
            return Err(CaptureError::ThreadGone);
        }
        match reply_rx.recv() {
            Ok(Ok(())) => Ok((self.thread, self.shared)),
            Ok(Err(e)) => {
                let _ = self.thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = self.thread.join();
                Err(CaptureError::ThreadGone)
            }
        }
    }

    /// Close without starting
    pub fn close(self) {
        drop(self.commands);
        let _ = self.thread.join();
    }
}

/// Everything the loop needs, created on the capture thread
struct Stream {
    client: AudioClient,
    capture: AudioCaptureClient,
    /// None = polling fallback (a driver that refuses event-driven loopback)
    event: Option<Handle>,
    format: Format,
    period: Duration,
    /// Application capture: the app, to notice when it exits
    process: Option<ProcessWatch>,
}

fn thread_main(
    target: Target,
    options: CaptureOptions,
    ready: mpsc::SyncSender<Result<Format, CaptureError>>,
    commands: mpsc::Receiver<Command>,
    shared: Arc<Shared>,
) {
    let _panic_guard = crate::PanicGuard(shared.clone());
    // COM per thread, in the MTA (process loopback activation completes on
    // a Windows MTA thread, and the stream is used only here)
    let _com = match ComApartment::init_mta() {
        Ok(c) => c,
        Err(e) => {
            let _ = ready.send(Err(CaptureError::windows("CoInitializeEx", e.code().0, e.message())));
            return;
        }
    };
    let stream = match open_stream(&target, &options) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("audio capture: {e}");
            let _ = ready.send(Err(e));
            return;
        }
    };
    if ready.send(Ok(stream.format)).is_err() {
        return; // `open` gave up waiting
    }
    // Park until started; a closed channel means the Capture was dropped
    let Ok(Command::Start(sink, reply)) = commands.recv() else {
        return;
    };
    if let Err(e) = stream.client.start_stream() {
        let _ = reply.send(Err(CaptureError::from_wasapi("starting the stream", e)));
        return;
    }
    let _ = reply.send(Ok(()));
    tracing::info!(
        "audio capture started: {} Hz, {} ch, {:?}, {} mode, period {} ms",
        stream.format.sample_rate,
        stream.format.channels,
        stream.format.sample_format,
        if stream.event.is_some() { "event" } else { "polling" },
        stream.period.as_secs_f64() * 1000.0
    );
    let reason = capture_loop(&stream, sink, &options, &shared);
    let _ = stream.client.stop_stream();
    if let Some(reason) = reason {
        tracing::info!("audio capture ended: {reason:?}");
        shared.end(reason);
    } else {
        tracing::info!("audio capture stopped");
    }
}

fn open_stream(target: &Target, options: &CaptureOptions) -> Result<Stream, CaptureError> {
    match target {
        Target::Device(id) => open_device(id.as_deref(), options),
        Target::Process(pid) => open_process(*pid, options),
    }
}

/// v1 WasapiCapture::init
fn open_device(id: Option<&str>, options: &CaptureOptions) -> Result<Stream, CaptureError> {
    let enumerator = DeviceEnumerator::new().map_err(|e| CaptureError::from_wasapi("device enumerator", e))?;
    let device = match id {
        Some(id) => enumerator.get_device(id).map_err(|_| CaptureError::DeviceNotFound(id.to_owned()))?,
        None => enumerator.get_default_device(&Direction::Render).map_err(|_| CaptureError::NoDevice)?,
    };
    if device.get_direction() != Direction::Render {
        // A microphone's ID: loopback needs an output device
        return Err(CaptureError::DeviceNotFound(id.unwrap_or_default().to_owned()));
    }
    let name = device.get_friendlyname().unwrap_or_default();

    let mut client = device.get_iaudioclient().map_err(|e| CaptureError::activation(&e))?;
    let mix = client.get_mixformat().map_err(|e| CaptureError::from_wasapi("reading the mix format", e))?;
    let plan = format::plan_loopback(mix_format(&mix)?, options.preferred_rate);
    let wave = wave_format(&plan, &mix);
    tracing::info!(
        "audio capture: loopback of \"{name}\": mix {} Hz {} ch {} bit, capturing {} Hz {:?}{}",
        mix.get_samplespersec(),
        mix.get_nchannels(),
        mix.get_bitspersample(),
        plan.format.sample_rate,
        plan.format.sample_format,
        if plan.autoconvert { " (converted by Windows)" } else { "" }
    );

    let events = StreamMode::EventsShared { autoconvert: plan.autoconvert, buffer_duration_hns: BUFFER_HNS };
    let event = match client.initialize_client(&wave, &Direction::Capture, &events) {
        Ok(()) => Some(client.set_get_eventhandle().map_err(|e| CaptureError::from_wasapi("SetEventHandle", e))?),
        Err(e) => {
            // Some drivers do not support event-driven loopback (v1). A
            // failed Initialize can leave the client unusable, so the
            // polling attempt uses a fresh one.
            tracing::warn!("audio capture: event-driven init failed ({e}), trying polling mode");
            client = device.get_iaudioclient().map_err(|e| CaptureError::activation(&e))?;
            let polling = StreamMode::PollingShared { autoconvert: plan.autoconvert, buffer_duration_hns: BUFFER_HNS };
            client
                .initialize_client(&wave, &Direction::Capture, &polling)
                .map_err(|e| CaptureError::from_wasapi("initializing the audio client", e))?;
            None
        }
    };
    let period = client
        .get_device_period()
        .ok()
        .and_then(|(default, _min)| u64::try_from(default).ok())
        .filter(|&hns| hns > 0)
        .map(|hns| Duration::from_nanos(hns * 100))
        .unwrap_or(DEFAULT_PERIOD);
    let capture =
        client.get_audiocaptureclient().map_err(|e| CaptureError::from_wasapi("GetService(IAudioCaptureClient)", e))?;
    Ok(Stream { client, capture, event, format: plan.format, period, process: None })
}

/// v1 WasapiCapture::init_process
fn open_process(pid: u32, options: &CaptureOptions) -> Result<Stream, CaptureError> {
    // Opened first: it is how we tell the app has exited, and it fails for
    // a PID that does not exist (activation would succeed and capture
    // nothing)
    let process = ProcessWatch::open(pid).ok_or(CaptureError::ProcessNotFound(pid))?;
    let fmt = format::plan_application(options.preferred_rate);
    let wave = WaveFormat::new(32, 32, &SampleType::Float, fmt.sample_rate as usize, 2, None);
    // Microsoft documents process loopback from build 20348 only. If an
    // earlier build rejects AUTOCONVERTPCM, retry without it (as OBS does)
    // on a freshly activated client, since a failed Initialize can leave
    // the old one unusable.
    let mut last_err = None;
    let mut ready = None;
    for autoconvert in [true, false] {
        let mut client =
            AudioClient::new_application_loopback_client(pid, true).map_err(|e| CaptureError::activation(&e))?;
        let mode = StreamMode::EventsShared { autoconvert, buffer_duration_hns: BUFFER_HNS };
        match client.initialize_client(&wave, &Direction::Capture, &mode) {
            Ok(()) => {
                ready = Some(client);
                break;
            }
            Err(e) => {
                tracing::warn!("audio capture: process loopback Initialize (autoconvert={autoconvert}) failed: {e}");
                last_err = Some(e);
            }
        }
    }
    let Some(client) = ready else {
        let e = last_err.expect("loop ran");
        return Err(CaptureError::from_wasapi("initializing the audio client", e));
    };
    let event = client.set_get_eventhandle().map_err(|e| CaptureError::from_wasapi("SetEventHandle", e))?;
    let capture =
        client.get_audiocaptureclient().map_err(|e| CaptureError::from_wasapi("GetService(IAudioCaptureClient)", e))?;
    tracing::info!("audio capture: process {pid} and its child processes, {} Hz, 2 ch, 32-bit float", fmt.sample_rate);
    Ok(Stream {
        client,
        capture,
        event: Some(event),
        format: fmt,
        // GetDevicePeriod does not work on a process loopback client
        period: DEFAULT_PERIOD,
        process: Some(process),
    })
}

fn mix_format(mix: &WaveFormat) -> Result<MixFormat, CaptureError> {
    let float = match mix.get_subformat() {
        Ok(SampleType::Float) => true,
        Ok(SampleType::Int) => false,
        Err(e) => return Err(CaptureError::UnsupportedFormat(e.to_string())),
    };
    Ok(MixFormat {
        sample_rate: mix.get_samplespersec(),
        channels: mix.get_nchannels(),
        float,
        container_bits: mix.get_bitspersample(),
    })
}

/// The WAVEFORMATEXTENSIBLE for a plan: the mix format's channel mask and,
/// for I32, its valid bits (24 in 32 stays 24 in 32)
fn wave_format(plan: &FormatPlan, mix: &WaveFormat) -> WaveFormat {
    let f = &plan.format;
    let (bits, valid, sample_type) = match f.sample_format {
        crate::SampleFormat::F32 => (32, 32, SampleType::Float),
        crate::SampleFormat::I16 => (16, 16, SampleType::Int),
        crate::SampleFormat::I32 => {
            let valid = match mix.get_validbitspersample() {
                v @ 1..=32 => v as usize,
                _ => 32,
            };
            (32, valid, SampleType::Int)
        }
    };
    WaveFormat::new(
        bits,
        valid,
        &sample_type,
        f.sample_rate as usize,
        f.channels as usize,
        Some(mix.get_dwchannelmask()),
    )
}

/// The capture loop (v1 capture_loop). Returns why the source ended, or
/// None when it was stopped.
fn capture_loop(
    stream: &Stream,
    mut sink: Box<dyn Sink>,
    options: &CaptureOptions,
    shared: &Shared,
) -> Option<EndReason> {
    let _mmcss = crate::register_mmcss();
    let fmt = stream.format;
    let frame_bytes = fmt.bytes_per_frame();
    let start = Instant::now();
    let mut fill = options.fill_silence.then(|| SilenceFill::new(fmt.sample_rate, stream.period, Duration::ZERO));
    // Allocated up front (100 ms) and only grown for an unusually large
    // packet, so the loop does not allocate
    let initial = fmt.frames_in(Duration::from_millis(100)).max(1) * frame_bytes;
    let mut buf = vec![0u8; initial];
    let mut zeros = vec![0u8; initial];
    // Wake at least once a period, to fill silence and to see `stop`
    let wait_ms = (stream.period.as_millis() as u32).clamp(1, 100);
    let mut last_process_check = start;

    loop {
        if shared.stop.load(Ordering::Acquire) {
            return None;
        }
        match &stream.event {
            // A timeout is normal: loopback of a silent device signals
            // nothing
            Some(event) => {
                let _ = event.wait_for_event(wait_ms);
            }
            None => thread::sleep(POLL_INTERVAL),
        }
        if shared.stop.load(Ordering::Acquire) {
            return None;
        }

        loop {
            let frames = match stream.capture.get_next_packet_size() {
                Ok(n) => n.unwrap_or(0) as usize,
                Err(e) => return Some(end_reason(e)),
            };
            if frames == 0 {
                break;
            }
            let bytes = frames * frame_bytes;
            if buf.len() < bytes {
                buf.resize(bytes, 0);
            }
            let (frames, info) = match stream.capture.read_from_device(&mut buf) {
                Ok((n, info)) => (n as usize, info),
                Err(e) => return Some(end_reason(e)),
            };
            if frames == 0 {
                break;
            }
            let bytes = frames * frame_bytes;
            let now = start.elapsed();
            if info.flags.data_discontinuity && now >= DISCONTINUITY_GRACE {
                // A gap in the captured audio (CPU load, other audio apps)
                shared.discontinuities.fetch_add(1, Ordering::Relaxed);
            }
            if info.flags.silent && fill.is_some() {
                // The engine says the buffer is silent (whatever it holds):
                // a stream is open but nothing audible plays. Process
                // loopback delivers such packets all the time while the app
                // is quiet. With fill_silence the caller tells fill from
                // audio, so this goes out as silence too: "audio arrives =
                // playing" stays true to what is heard.
                sink.silence(frames, fmt);
                shared.silence_frames.fetch_add(frames as u64, Ordering::Relaxed);
            } else if info.flags.silent {
                // Without fill: zeros as audio, as v1 delivered them
                if zeros.len() < bytes {
                    zeros.resize(bytes, 0);
                }
                sink.audio(&zeros[..bytes], frames, fmt);
                shared.frames.fetch_add(frames as u64, Ordering::Relaxed);
            } else {
                sink.audio(&buf[..bytes], frames, fmt);
                shared.frames.fetch_add(frames as u64, Ordering::Relaxed);
            }
            // A packet arrived, silent or not: the stream is flowing, and
            // the fill clock must not add silence for the same time
            if let Some(fill) = fill.as_mut() {
                fill.on_audio(now);
            }
        }

        let now = start.elapsed();
        if let Some(fill) = fill.as_mut() {
            let n = fill.poll(now);
            if n > 0 {
                sink.silence(n, fmt);
                shared.silence_frames.fetch_add(n as u64, Ordering::Relaxed);
            }
        }
        if let Some(process) = &stream.process
            && now.saturating_sub(last_process_check.duration_since(start)) >= PROCESS_CHECK
        {
            last_process_check = Instant::now();
            if process.exited() {
                return Some(EndReason::ProcessExited);
            }
        }
    }
}

fn end_reason(e: wasapi::WasapiError) -> EndReason {
    match CaptureError::from_wasapi("capturing", e).hresult() {
        Some(AUDCLNT_E_DEVICE_INVALIDATED) => EndReason::DeviceInvalidated,
        Some(AUDCLNT_E_SERVICE_NOT_RUNNING) => EndReason::ServiceStopped,
        Some(hr) => EndReason::Error(hr),
        None => EndReason::Error(0),
    }
}
