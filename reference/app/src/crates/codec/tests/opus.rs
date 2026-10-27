//! Opus encoder: a test tone through every quality, frame duration, channel
//! layout and typical MTU, decoded again with libopus: packet sizes within
//! the media payload, one 10 / 20 ms frame per packet, the bit rate, and the
//! decoded tone. PipeWire's Opus 05 goes through the same encoder: its
//! stereo stream is one coupled multistream stream, a plain Opus packet.

#![cfg(feature = "opus")]

use codec::opus::{MPL_HDR_LEN, OpusEncoder};
use codec::{CodecKind, Encoder, EncoderConfig, EncoderError, Pcm, PeerParams, Quality};
use opus_sys::OpusDec;

/// Media MTUs (RTP header excluded) of sinks with L2CAP MTUs 672 (BlueZ),
/// 679, 1023, 1679
const MTUS: [u16; 4] = [672 - 12, 679 - 12, 1023 - 12, 1679 - 12];

fn config(quality: Quality, channels: u32, mtu: u16, frame_ms: u8, wire: u8) -> EncoderConfig {
    EncoderConfig {
        sample_rate: 48000,
        channels,
        quality,
        mtu,
        bits_per_sample: 16,
        abr: false,
        peer: PeerParams { opus_frame_ms: frame_ms, opus_channels: wire, ..Default::default() },
    }
}

/// 1 kHz left, 1.5 kHz right (mono: 1 kHz), amplitude 0.5
fn tone(n0: usize, frames: usize, channels: usize) -> Vec<i16> {
    let mut v = Vec::with_capacity(frames * channels);
    for i in 0..frames {
        let t = (n0 + i) as f64 / 48000.0;
        for ch in 0..channels {
            let f = if ch == 0 { 1000.0 } else { 1500.0 };
            v.push((0.5 * (2.0 * std::f64::consts::PI * f * t).sin() * 32767.0).round() as i16);
        }
    }
    v
}

/// Amplitude of `freq` in `xs` (Goertzel), full scale 1.0
fn amplitude(xs: &[i16], freq: f64) -> f64 {
    let k = 2.0 * (2.0 * std::f64::consts::PI * freq / 48000.0).cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for &x in xs {
        let s = f64::from(x) / 32768.0 + k * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    2.0 * (s1 * s1 + s2 * s2 - k * s1 * s2).max(0.0).sqrt() / xs.len() as f64
}

struct Run {
    max_packet: usize,
    avg_kbps: f64,
    amps: Vec<f64>,
}

/// Encodes 1 s of tone, decodes every packet; checks per packet
fn run(cfg: &EncoderConfig) -> Run {
    run_kind(CodecKind::Opus, cfg)
}

fn run_kind(kind: CodecKind, cfg: &EncoderConfig) -> Run {
    let mut enc = OpusEncoder::new(kind, cfg).unwrap();
    assert_eq!(enc.kind(), kind);
    let frame_ms = if cfg.peer.opus_frame_ms == 10 { 10 } else { 20 };
    let frame = 48 * frame_ms;
    assert_eq!(enc.pcm_frames_per_encode(), frame);
    assert_eq!(enc.pcm_frames_per_codec_frame(), frame);
    let budget = usize::from(cfg.mtu) - MPL_HDR_LEN;
    assert_eq!(enc.max_output_bytes(), budget.min(1275));
    let wire = if cfg.peer.opus_channels == 0 { cfg.channels as usize } else { usize::from(cfg.peer.opus_channels) };

    let mut dec = OpusDec::new(48000, wire as u32).unwrap();
    let mut out = vec![0u8; enc.max_output_bytes()];
    let mut pcm_out = vec![0i16; 5760 * wire];
    let mut decoded: Vec<i16> = Vec::new();
    let (mut bytes, mut max_packet) = (0usize, 0usize);
    let packets = 1000 / frame_ms;
    for p in 0..packets {
        let pcm = tone(p * frame, frame, cfg.channels as usize);
        let r = enc.encode(Pcm::I16(&pcm), &mut out).unwrap();
        assert_eq!(r.frames, 1);
        assert!(r.bytes > 2 && r.bytes <= budget, "{} bytes for a {budget}-byte payload", r.bytes);
        let packet = &out[..r.bytes];
        assert_eq!(opus_sys::packet_frames(packet), Ok(1));
        assert_eq!(opus_sys::packet_samples_per_frame(packet, 48000), Some(frame));
        assert_eq!(opus_sys::packet_channels(packet), Some(wire));
        let n = dec.decode(packet, &mut pcm_out).unwrap();
        assert_eq!(n, frame);
        decoded.extend_from_slice(&pcm_out[..n * wire]);
        bytes += r.bytes;
        max_packet = max_packet.max(r.bytes);
    }
    // The second half: past the encoder's start-up
    let half = &decoded[decoded.len() / 2..];
    let amps = (0..wire)
        .map(|ch| {
            let xs: Vec<i16> = half.iter().skip(ch).step_by(wire).copied().collect();
            let f = if ch == 1 && cfg.channels == 2 { 1500.0 } else { 1000.0 };
            amplitude(&xs, f)
        })
        .collect();
    Run { max_packet, avg_kbps: bytes as f64 * 8.0 / 1000.0, amps }
}

#[test]
fn qualities_and_mtus() {
    for mtu in MTUS {
        for (quality, kbps) in
            [(Quality::High, 256), (Quality::MediumHigh, 256), (Quality::Standard, 192), (Quality::Mobile, 128)]
        {
            let cfg = config(quality, 2, mtu, 20, 2);
            let enc = OpusEncoder::new(CodecKind::Opus, &cfg).unwrap();
            // 20 ms frames in a 659-byte payload carry at most 263 kbps
            let cap = (u32::from(mtu) - 1) * 8 / 20;
            assert_eq!(enc.bitrate_kbps(), kbps.min(cap), "mtu {mtu} {quality:?}");
            let r = run(&cfg);
            // Constrained VBR: the average stays near the target
            let target = f64::from(enc.bitrate_kbps());
            assert!(r.avg_kbps > target * 0.6 && r.avg_kbps < target * 1.1, "{} kbps for {target}", r.avg_kbps);
            for a in &r.amps {
                assert!((a - 0.5).abs() < 0.03, "mtu {mtu} {quality:?}: tone amplitude {a}");
            }
        }
    }
}

#[test]
fn ten_ms_frames_and_small_mtu() {
    let r = run(&config(Quality::High, 2, MTUS[2], 10, 2));
    assert!(r.max_packet < 1011);
    for a in &r.amps {
        assert!((a - 0.5).abs() < 0.03, "{a}");
    }
    // A small payload caps the bit rate; packets stay within it
    let cfg = config(Quality::High, 2, 200, 20, 2);
    assert_eq!(OpusEncoder::new(CodecKind::Opus, &cfg).unwrap().bitrate_kbps(), 79);
    let r = run(&cfg);
    assert!(r.max_packet <= 199);
}

#[test]
fn mono() {
    // Mono stream: 128 kbps
    let cfg = config(Quality::High, 1, MTUS[2], 20, 1);
    assert_eq!(OpusEncoder::new(CodecKind::Opus, &cfg).unwrap().bitrate_kbps(), 128);
    let r = run(&cfg);
    assert!((r.amps[0] - 0.5).abs() < 0.03);
    // Mono input to a stereo-only sink: duplicated, 256 kbps
    let cfg = config(Quality::High, 1, MTUS[2], 20, 2);
    assert_eq!(OpusEncoder::new(CodecKind::Opus, &cfg).unwrap().bitrate_kbps(), 256);
    let r = run(&cfg);
    assert_eq!(r.amps.len(), 2);
    for a in &r.amps {
        assert!((a - 0.5).abs() < 0.03, "{a}");
    }
}

#[test]
fn rejects_what_a2dp_opus_does_not_carry() {
    let bad = |c: EncoderConfig| matches!(OpusEncoder::new(CodecKind::Opus, &c), Err(EncoderError::Unsupported { .. }));
    assert!(bad(EncoderConfig { sample_rate: 44100, ..config(Quality::High, 2, 1011, 20, 2) }));
    assert!(bad(EncoderConfig { bits_per_sample: 24, ..config(Quality::High, 2, 1011, 20, 2) }));
    assert!(bad(config(Quality::High, 2, 1011, 5, 2)));
    assert!(bad(config(Quality::High, 2, 1011, 20, 1)));
    assert!(bad(config(Quality::High, 2, 2, 20, 2)));

    let mut enc = OpusEncoder::new(CodecKind::Opus, &config(Quality::High, 2, 1011, 20, 2)).unwrap();
    let mut out = vec![0u8; 1010];
    let short = tone(0, 959, 2);
    assert!(matches!(enc.encode(Pcm::I16(&short), &mut out), Err(EncoderError::InputLength { .. })));
    let pcm = tone(0, 960, 2);
    assert!(matches!(enc.encode(Pcm::I16(&pcm), &mut out[..100]), Err(EncoderError::OutputTooSmall { .. })));
    assert!(enc.encode(Pcm::I32(&[0; 1920]), &mut out).is_err());
}

#[test]
fn created_through_the_codec_registry() {
    let enc = codec::create(CodecKind::Opus, &config(Quality::Standard, 2, 1011, 20, 0)).unwrap();
    assert_eq!(enc.kind(), CodecKind::Opus);
    assert_eq!(enc.bitrate_kbps(), 192);
    assert_eq!(CodecKind::from_key("opus"), Some(CodecKind::Opus));
    assert_eq!(CodecKind::Opus.name(), "Opus");
    assert_eq!(CodecKind::Opus.media_header_len(), 1);
    assert!(CodecKind::Opus.uses_rtp());
}

#[test]
fn opus05_stereo_music() {
    // PipeWire's opus_05 configuration: 10 ms, stereo, 312 * 1024 bps
    let peer = |frame_ms, max| PeerParams {
        opus_frame_ms: frame_ms,
        opus_channels: 2,
        opus_max_bitrate: max,
        ..Default::default()
    };
    let cfg = EncoderConfig { peer: peer(10, 312 * 1024), ..config(Quality::High, 2, MTUS[2], 10, 2) };
    let enc = codec::create(CodecKind::Opus05, &cfg).unwrap();
    assert_eq!((enc.kind(), enc.bitrate_kbps()), (CodecKind::Opus05, 256));
    let r = run_kind(CodecKind::Opus05, &cfg);
    assert!(r.max_packet < 1011);
    for a in &r.amps {
        assert!((a - 0.5).abs() < 0.03, "{a}");
    }
    // The sink's bit rate limit (128 * 1024 bps) caps the target; mono
    // input goes out as stereo
    let cfg = EncoderConfig { peer: peer(20, 128 * 1024), ..config(Quality::High, 1, MTUS[2], 20, 2) };
    assert_eq!(OpusEncoder::new(CodecKind::Opus05, &cfg).unwrap().bitrate_kbps(), 131);
    let r = run_kind(CodecKind::Opus05, &cfg);
    assert_eq!(r.amps.len(), 2);
    assert!(r.avg_kbps < 131.0 * 1.1, "{} kbps", r.avg_kbps);
    // Android's Opus has no such limit
    let cfg = EncoderConfig { peer: peer(20, 128 * 1024), ..config(Quality::High, 2, MTUS[2], 20, 2) };
    assert_eq!(OpusEncoder::new(CodecKind::Opus, &cfg).unwrap().bitrate_kbps(), 256);
    // Not an Opus codec
    assert!(matches!(OpusEncoder::new(CodecKind::Sbc, &cfg), Err(EncoderError::Unsupported { .. })));
}
