//! aptX / aptX HD / aptX LL: encode a 1 kHz (left) / 1.5 kHz (right) tone at
//! half scale, decode it with libopenaptx's decoder and check what comes back,
//! plus output sizes, frame counts, bit rates and rejected configurations.

#![cfg(feature = "aptx")]

use codec::aptx::{AptxEncoder, aptx_ll_max_packet_bytes};
use codec::{CodecKind, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, PeerParams, Quality};
use openaptx_sys::AptxContext;
use std::f64::consts::PI;

const LEFT_HZ: f64 = 1000.0;
const RIGHT_HZ: f64 = 1500.0;
const AMPLITUDE: f64 = 0.5;
const SECONDS: usize = 3;
const KINDS: [CodecKind; 3] = [CodecKind::Aptx, CodecKind::AptxHd, CodecKind::AptxLl];

fn config(sample_rate: u32, channels: u32, bits: u8) -> EncoderConfig {
    EncoderConfig {
        sample_rate,
        channels,
        quality: Quality::High,
        mtu: 660,
        bits_per_sample: bits,
        abr: false,
        peer: PeerParams::default(),
    }
}

/// Sample `n` of the tone of channel `ch` (mono: the left tone), -1..1
fn tone(n: usize, ch: usize, sample_rate: u32) -> f64 {
    let hz = if ch == 0 { LEFT_HZ } else { RIGHT_HZ };
    AMPLITUDE * (2.0 * PI * hz * n as f64 / f64::from(sample_rate)).sin()
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

/// Frequency from the zero crossings of `x`
fn zero_crossing_hz(x: &[f64], sample_rate: u32) -> f64 {
    let crossings = x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
    crossings as f64 / 2.0 * f64::from(sample_rate) / x.len() as f64
}

/// Checks one decoded channel: a clean tone of `hz` at half scale
fn check_tone(x: &[f64], hz: f64, other_hz: f64, sample_rate: u32, what: &str) {
    let amp = amplitude_at(x, hz, sample_rate);
    assert!((amp - AMPLITUDE).abs() < AMPLITUDE * 0.03, "{what}: amplitude {amp} at {hz} Hz");
    let f = zero_crossing_hz(x, sample_rate);
    assert!((f - hz).abs() < 2.0, "{what}: {f} Hz by zero crossings, expected {hz}");
    let leak = amplitude_at(x, other_hz, sample_rate);
    assert!(leak < 0.005, "{what}: {leak} of the other channel's {other_hz} Hz");
    // Whatever is not the tone (noise, distortion) stays 40 dB below it
    let rms = (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
    let residual = (rms * rms - amp * amp / 2.0).max(0.0).sqrt();
    assert!(residual < amp / 2f64.sqrt() / 100.0, "{what}: residual {residual} (rms {rms}, tone {amp})");
}

/// Encodes SECONDS of the tone; returns the bitstream and checks the
/// per-call invariants on the way
fn encode_tone(enc: &mut AptxEncoder, cfg: &EncoderConfig) -> Vec<u8> {
    let ch = cfg.channels as usize;
    let per_call = enc.pcm_frames_per_encode();
    let calls = SECONDS * cfg.sample_rate as usize / per_call;
    let mut out = vec![0u8; enc.max_output_bytes()];
    let mut stream = Vec::new();
    let mut i16_buf = vec![0i16; per_call * ch];
    let mut i32_buf = vec![0i32; per_call * ch];
    let mut timestamp = 0usize;
    for call in 0..calls {
        for f in 0..per_call {
            for c in 0..ch {
                let v = tone(call * per_call + f, c, cfg.sample_rate);
                i16_buf[f * ch + c] = (v * 32767.0).round() as i16;
                // MSB-aligned 24-bit in 32 bits, as v1's convert_f32_to_i32
                i32_buf[f * ch + c] = (v * 2147483647.0).round() as i32;
            }
        }
        let pcm = match enc.pcm_format() {
            PcmFormat::I16 => Pcm::I16(&i16_buf),
            PcmFormat::I32 => Pcm::I32(&i32_buf),
        };
        let encoded = enc.encode(pcm, &mut out).unwrap();
        assert_eq!(encoded.bytes, enc.max_output_bytes());
        assert_eq!(encoded.frames as usize * enc.pcm_frames_per_codec_frame(), per_call);
        timestamp += encoded.frames as usize * enc.pcm_frames_per_codec_frame();
        stream.extend_from_slice(&out[..encoded.bytes]);
    }
    // The RTP timestamp (frames x 4) follows the input exactly
    assert_eq!(timestamp, calls * per_call);
    stream
}

/// Decodes with libopenaptx into per-channel samples (-1..1)
fn decode(stream: &[u8], hd: bool) -> [Vec<f64>; 2] {
    let mut dec = AptxContext::new(hd).unwrap();
    let groups = stream.len() / openaptx_sys::encoded_group_bytes(hd);
    let mut pcm = vec![0u8; groups * openaptx_sys::PCM_GROUP_BYTES];
    let (consumed, written) = dec.decode(stream, &mut pcm);
    assert_eq!(consumed, stream.len(), "parity error in the aptX stream");
    // The decoder swallows the 90-sample codec latency
    assert_eq!(written, (groups * 4 - 90) * 6);
    let mut channels = [Vec::new(), Vec::new()];
    for frame in pcm[..written].as_chunks::<6>().0 {
        for (c, s) in frame.as_chunks::<3>().0.iter().enumerate() {
            let v = i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8;
            channels[c].push(f64::from(v) / f64::from(1 << 23));
        }
    }
    channels
}

fn round_trip(kind: CodecKind, sample_rate: u32, channels: u32, bits: u8) {
    let what = format!("{kind} {sample_rate} Hz {channels} ch {bits}-bit");
    let cfg = config(sample_rate, channels, bits);
    let mut enc = AptxEncoder::new(kind, &cfg).unwrap();
    let hd = kind == CodecKind::AptxHd;
    assert_eq!(enc.kind(), kind);
    assert_eq!(enc.pcm_format(), if bits == 24 { PcmFormat::I32 } else { PcmFormat::I16 });
    assert_eq!(enc.pcm_frames_per_encode(), 128);
    assert_eq!(enc.pcm_frames_per_codec_frame(), 4);
    assert_eq!(enc.max_output_bytes(), if hd { 192 } else { 128 });

    let stream = encode_tone(&mut enc, &cfg);
    // Bit rate: what the stream really carries per second
    let frames = stream.len() / if hd { 6 } else { 4 } * 4;
    let kbps = stream.len() as f64 * 8.0 / (frames as f64 / f64::from(sample_rate)) / 1000.0;
    assert!((kbps - f64::from(enc.bitrate_kbps())).abs() < 1.0, "{what}: {kbps} kbps vs {}", enc.bitrate_kbps());

    let decoded = decode(&stream, hd);
    // Skip the first 0.5 s (filter settling), analyse exactly 1 s: whole
    // periods of both tones at both rates
    let start = sample_rate as usize / 2;
    let window = start..start + sample_rate as usize;
    check_tone(&decoded[0][window.clone()], LEFT_HZ, RIGHT_HZ, sample_rate, &format!("{what} left"));
    if channels == 2 {
        check_tone(&decoded[1][window], RIGHT_HZ, LEFT_HZ, sample_rate, &format!("{what} right"));
    } else {
        // Mono is sent as stereo with the channel duplicated
        check_tone(&decoded[1][window], LEFT_HZ, RIGHT_HZ, sample_rate, &format!("{what} right (mono)"));
    }
}

#[test]
fn tone_round_trip_stereo_16bit() {
    for kind in KINDS {
        for rate in [44100, 48000] {
            round_trip(kind, rate, 2, 16);
        }
    }
}

#[test]
fn tone_round_trip_24bit() {
    // v1 feeds aptX HD 24-bit (int32 MSB-aligned) unless the profile says
    // 16-bit; the others accept it too
    for kind in KINDS {
        for rate in [44100, 48000] {
            round_trip(kind, rate, 2, 24);
        }
    }
}

#[test]
fn tone_round_trip_mono() {
    for kind in KINDS {
        round_trip(kind, 48000, 1, 16);
    }
    round_trip(CodecKind::AptxHd, 44100, 1, 24);
}

#[test]
fn bitrates() {
    let rate = |kind, sample_rate| AptxEncoder::new(kind, &config(sample_rate, 2, 16)).unwrap().bitrate_kbps();
    assert_eq!(rate(CodecKind::Aptx, 44100), 352);
    assert_eq!(rate(CodecKind::Aptx, 48000), 384);
    assert_eq!(rate(CodecKind::AptxLl, 44100), 352);
    assert_eq!(rate(CodecKind::AptxLl, 48000), 384);
    assert_eq!(rate(CodecKind::AptxHd, 44100), 529);
    assert_eq!(rate(CodecKind::AptxHd, 48000), 576);
}

#[test]
fn hd_keeps_24_bits() {
    // A signal below 16-bit resolution (+-64 in 24 bits = 1/4 LSB of 16-bit)
    // survives aptX HD's 24-bit path but not a 16-bit input
    let cfg = config(48000, 2, 24);
    let mut enc = AptxEncoder::new(CodecKind::AptxHd, &cfg).unwrap();
    let mut out = vec![0u8; enc.max_output_bytes()];
    let mut stream = Vec::new();
    let mut buf = vec![0i32; 256];
    for call in 0..600 {
        for f in 0..128 {
            let v = 64.0 * (2.0 * PI * 1000.0 * (call * 128 + f) as f64 / 48000.0).sin();
            buf[f * 2] = (v.round() as i32) << 8;
            buf[f * 2 + 1] = (v.round() as i32) << 8;
        }
        let n = enc.encode(Pcm::I32(&buf), &mut out).unwrap().bytes;
        stream.extend_from_slice(&out[..n]);
    }
    let decoded = decode(&stream, true);
    let amp = amplitude_at(&decoded[0][24000..72000], 1000.0, 48000) * f64::from(1 << 23);
    assert!((amp - 64.0).abs() < 64.0 * 0.1, "amplitude {amp} in 24-bit LSBs");
}

#[test]
fn small_mtu_shrinks_the_call() {
    // v1 cut one call's output at the MTU; here the call takes fewer frames
    let mut cfg = config(44100, 2, 16);
    cfg.mtu = 100;
    let mut enc = AptxEncoder::new(CodecKind::Aptx, &cfg).unwrap();
    assert_eq!(enc.pcm_frames_per_encode(), 100);
    assert_eq!(enc.max_output_bytes(), 100);
    let mut out = [0u8; 100];
    let e = enc.encode(Pcm::I16(&[0i16; 200]), &mut out).unwrap();
    assert_eq!((e.bytes, e.frames), (100, 25));

    let enc = AptxEncoder::new(CodecKind::AptxHd, &cfg).unwrap();
    assert_eq!(enc.pcm_frames_per_encode(), 64);
    assert_eq!(enc.max_output_bytes(), 96);
}

#[test]
fn aptx_ll_packet_limit() {
    assert_eq!(aptx_ll_max_packet_bytes(44100), 328);
    assert_eq!(aptx_ll_max_packet_bytes(48000), 360);
}

#[test]
fn rejects_invalid_configurations() {
    let unsupported = |kind, cfg: EncoderConfig| {
        assert!(
            matches!(AptxEncoder::new(kind, &cfg), Err(EncoderError::Unsupported { .. })),
            "{kind} {cfg:?} accepted"
        );
    };
    for kind in KINDS {
        unsupported(kind, config(32000, 2, 16));
        unsupported(kind, config(96000, 2, 16));
        unsupported(kind, config(48000, 0, 16));
        unsupported(kind, config(48000, 3, 16));
        unsupported(kind, config(48000, 2, 8));
        unsupported(kind, config(48000, 2, 32));
        let mut tiny = config(48000, 2, 16);
        tiny.mtu = 3;
        unsupported(kind, tiny);
    }
    unsupported(CodecKind::Sbc, config(48000, 2, 16));
    unsupported(CodecKind::Ldac, config(48000, 2, 16));
}

#[test]
fn rejects_wrong_input() {
    let mut enc = AptxEncoder::new(CodecKind::Aptx, &config(48000, 2, 16)).unwrap();
    let mut out = [0u8; 128];
    assert!(matches!(
        enc.encode(Pcm::I16(&[0; 255]), &mut out),
        Err(EncoderError::InputLength { expected: 256, got: 255 })
    ));
    assert!(matches!(
        enc.encode(Pcm::I16(&[0; 256]), &mut [0u8; 127]),
        Err(EncoderError::OutputTooSmall { have: 127, need: 128 })
    ));
    assert!(matches!(enc.encode(Pcm::I32(&[0; 256]), &mut out), Err(EncoderError::Encode { .. })));
    // Still works after the rejected calls
    assert_eq!(enc.encode(Pcm::I16(&[0; 256]), &mut out).unwrap().bytes, 128);
}

#[test]
fn encoder_is_send() {
    fn send<T: Send>() {}
    send::<AptxEncoder>();
    // Created on one thread, used on another (the encode thread)
    let mut enc = AptxEncoder::new(CodecKind::AptxHd, &config(48000, 2, 24)).unwrap();
    std::thread::spawn(move || {
        let mut out = [0u8; 192];
        enc.encode(Pcm::I32(&[0; 256]), &mut out).unwrap();
    })
    .join()
    .unwrap();
}

#[test]
fn create_dispatches() {
    for kind in KINDS {
        let enc = codec::create(kind, &config(44100, 2, 16)).unwrap();
        assert_eq!(enc.kind(), kind);
    }
}
