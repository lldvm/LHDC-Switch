//! Round trip of the codec crate's AacEncoder through the fdk-aac decoder:
//! a 1 kHz (left) / 1.5 kHz (right) tone at half scale, 44.1 / 48 kHz, mono
//! and stereo, every quality, decoded from the LATM packets one by one as a
//! sink does. Needs the `decoder` feature:
//! `cargo test -p fdkaac-sys --features decoder`.

use codec::aac::AacEncoder;
use codec::{Encoder, EncoderConfig, Pcm, PeerParams, Quality};
use fdkaac_sys::{Decoder, ffi};
use std::f64::consts::PI;

const LEFT_HZ: f64 = 1000.0;
const RIGHT_HZ: f64 = 1500.0;
const AMPLITUDE: f64 = 0.5;
const SECONDS: usize = 3;

fn tone(n: usize, ch: usize, sample_rate: u32) -> i16 {
    let hz = if ch == 0 { LEFT_HZ } else { RIGHT_HZ };
    (AMPLITUDE * 32767.0 * (2.0 * PI * hz * n as f64 / f64::from(sample_rate)).sin()).round() as i16
}

/// Least-squares amplitude of `hz` in `x` (a window of whole periods)
fn amplitude_at(x: &[f64], hz: f64, sample_rate: u32) -> f64 {
    let w = 2.0 * PI * hz / f64::from(sample_rate);
    let (mut i, mut q) = (0.0, 0.0);
    for (n, v) in x.iter().enumerate() {
        i += v * (w * n as f64).cos();
        q += v * (w * n as f64).sin();
    }
    2.0 * (i * i + q * q).sqrt() / x.len() as f64
}

fn zero_crossing_hz(x: &[f64], sample_rate: u32) -> f64 {
    let crossings = x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
    crossings as f64 / 2.0 * f64::from(sample_rate) / x.len() as f64
}

fn check_tone(x: &[f64], hz: f64, other_hz: Option<f64>, sample_rate: u32, what: &str) {
    let amp = amplitude_at(x, hz, sample_rate);
    assert!((amp - AMPLITUDE).abs() < AMPLITUDE * 0.03, "{what}: amplitude {amp} at {hz} Hz");
    let f = zero_crossing_hz(x, sample_rate);
    assert!((f - hz).abs() < 2.0, "{what}: {f} Hz by zero crossings, expected {hz}");
    if let Some(other) = other_hz {
        let leak = amplitude_at(x, other, sample_rate);
        assert!(leak < 0.01, "{what}: {leak} of the other channel's {other} Hz");
    }
    // Noise and distortion stay 30 dB below the tone
    let rms = (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
    let residual = (rms * rms - amp * amp / 2.0).max(0.0).sqrt();
    assert!(residual < amp / 2f64.sqrt() * 0.0316, "{what}: residual {residual} (rms {rms}, tone {amp})");
}

fn round_trip(sample_rate: u32, channels: u32, quality: Quality) {
    let what = format!("{sample_rate} Hz {channels} ch {quality:?}");
    let cfg = EncoderConfig {
        sample_rate,
        channels,
        quality,
        mtu: 1000,
        bits_per_sample: 16,
        abr: false,
        peer: PeerParams::default(),
    };
    let mut enc = AacEncoder::new(&cfg).unwrap();
    let mut dec = Decoder::open(ffi::TT_MP4_LATM_MCP1, 1).unwrap();
    let ch = channels as usize;
    let per_call = enc.pcm_frames_per_encode();
    let calls = SECONDS * sample_rate as usize / per_call;
    let mut out = vec![0u8; enc.max_output_bytes()];
    let mut pcm = vec![0i16; per_call * ch];
    let mut frame = vec![0i16; 2048 * 8];
    let mut decoded = vec![Vec::new(); ch];
    let mut frames_decoded = 0;
    for call in 0..calls {
        for f in 0..per_call {
            for c in 0..ch {
                pcm[f * ch + c] = tone(call * per_call + f, c, sample_rate);
            }
        }
        let e = enc.encode(Pcm::I16(&pcm), &mut out).unwrap();
        // One packet = one AudioMuxElement, decoded on its own
        assert_eq!(dec.fill(&out[..e.bytes]).unwrap(), 0, "{what}: packet not taken whole");
        dec.decode_frame(&mut frame, 0).unwrap_or_else(|err| panic!("{what}: frame {call}: {err}"));
        let info = dec.stream_info().unwrap();
        assert_eq!(info.sample_rate, sample_rate as i32, "{what}");
        assert_eq!(info.num_channels, channels as i32, "{what}");
        assert_eq!(info.frame_size, 1024, "{what}");
        for (n, v) in frame[..1024 * ch].iter().enumerate() {
            decoded[n % ch].push(f64::from(*v) / 32768.0);
        }
        frames_decoded += 1;
    }
    assert_eq!(frames_decoded, calls);

    // Skip 0.5 s (codec delay, start-up), analyse exactly 1 s: whole periods
    // of both tones at both rates
    let start = sample_rate as usize / 2;
    let window = start..start + sample_rate as usize;
    if ch == 2 {
        check_tone(&decoded[0][window.clone()], LEFT_HZ, Some(RIGHT_HZ), sample_rate, &format!("{what} left"));
        check_tone(&decoded[1][window], RIGHT_HZ, Some(LEFT_HZ), sample_rate, &format!("{what} right"));
    } else {
        check_tone(&decoded[0][window], LEFT_HZ, None, sample_rate, &format!("{what} mono"));
    }
}

#[test]
fn tone_round_trip() {
    for rate in [44100, 48000] {
        for channels in [1, 2] {
            for quality in [Quality::High, Quality::Standard, Quality::Mobile, Quality::MediumHigh] {
                round_trip(rate, channels, quality);
            }
        }
    }
}

#[test]
fn mtu_capped_stream_decodes() {
    // 660-byte payload (672-byte L2CAP MTU): bit rate capped, still clean
    for rate in [44100, 48000] {
        let cfg = EncoderConfig {
            sample_rate: rate,
            channels: 2,
            quality: Quality::High,
            mtu: 660,
            bits_per_sample: 16,
            abr: false,
            peer: PeerParams::default(),
        };
        let mut enc = AacEncoder::new(&cfg).unwrap();
        let mut dec = Decoder::open(ffi::TT_MP4_LATM_MCP1, 1).unwrap();
        let mut out = vec![0u8; enc.max_output_bytes()];
        let mut pcm = vec![0i16; 2048];
        let mut frame = vec![0i16; 2048 * 8];
        let mut left = Vec::new();
        for call in 0..(2 * rate as usize / 1024) {
            for f in 0..1024 {
                pcm[f * 2] = tone(call * 1024 + f, 0, rate);
                pcm[f * 2 + 1] = tone(call * 1024 + f, 1, rate);
            }
            let e = enc.encode(Pcm::I16(&pcm), &mut out).unwrap();
            assert!(e.bytes <= 660);
            assert_eq!(dec.fill(&out[..e.bytes]).unwrap(), 0);
            dec.decode_frame(&mut frame, 0).unwrap();
            left.extend(frame[..2048].iter().step_by(2).map(|v| f64::from(*v) / 32768.0));
        }
        let start = rate as usize / 2;
        check_tone(&left[start..start + rate as usize], LEFT_HZ, Some(RIGHT_HZ), rate, "MTU 660 left");
    }
}
