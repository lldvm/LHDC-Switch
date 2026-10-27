//! SBC encoder: Bluedroid's fixed-point encoder (the one BTstack bundles in
//! 3rd-party/bluedroid/encoder), ported from C to safe Rust. The output is
//! bit-exact with the C encoder as BTstack configures it; tests/sbc_bitexact.rs
//! compares the two over all configurations.
//!
//! Ported from Bluedroid, Copyright (C) 1999-2012 Broadcom Corporation,
//! licensed under the Apache License, Version 2.0
//! (http://www.apache.org/licenses/LICENSE-2.0). Each ported file carries
//! the notice.
//!
//! - [`SbcFrameEncoder`]: the general encoder, any [`SbcConfig`].
//! - [`SbcEncoder`]: the A2DP stream encoder (as v1's wrapper): 44.1 / 48
//!   kHz, joint stereo (or mono), 16 blocks, 8 subbands, loudness, the
//!   bitpool by [`Quality`]; [`SbcEncoder::new_xq`] is SBC XQ: dual channel
//!   with a high bitpool per channel ([`xq_bitpool`]).
//! - [`FastStreamEncoder`]: CSR FastStream, SBC frames of a fixed setup.

mod analysis;
mod bit_alloc;
mod faststream;
mod frame;
mod packing;

pub use faststream::{
    FASTSTREAM_BITPOOL, FASTSTREAM_FRAME_BYTES, FASTSTREAM_FRAMES_PER_PACKET, FASTSTREAM_MAX_PAYLOAD, FastStreamEncoder,
};
pub use frame::{Allocation, ChannelMode, SbcConfig, SbcFrameEncoder};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, Quality};

const BLOCKS: u8 = 16;
const SUBBANDS: u8 = 8;

/// SBC bitpool for a quality mode (joint stereo; mono takes the same)
pub fn quality_bitpool(quality: Quality) -> u8 {
    match quality {
        Quality::High | Quality::MediumHigh => 53,
        Quality::Standard => 35,
        Quality::Mobile => 19,
    }
}

/// SBC XQ bitpool (per channel, dual channel; mono takes the same) for a
/// quality mode and sample rate:
/// - High: 47 at 44.1 kHz (the "SBC XQ" of Android custom ROMs and
///   pulseaudio-modules-bt: 200-byte frames, ~551 kbps), 43 at 48 kHz (the
///   same ~552 kbps). Three frames and the media payload header fill one
///   2-DH5 packet, five one 3-DH5.
/// - Standard: 43 / 39, PipeWire's SBC-XQ default (a2dp-codec-sbc.c,
///   default_bitpool): the ~512 kbps every A2DP 1.2 sink has to take.
/// - Mobile: 32 / 29, PipeWire's dual channel default without XQ (~385 kbps).
pub fn xq_bitpool(quality: Quality, sample_rate: u32) -> u8 {
    let (at_44100, at_48000) = match quality {
        Quality::High | Quality::MediumHigh => (47, 43),
        Quality::Standard => (43, 39),
        Quality::Mobile => (32, 29),
    };
    if sample_rate == 44100 { at_44100 } else { at_48000 }
}

/// Largest SBC XQ bitpool at a sample rate (any quality): the maximum the
/// SET_CONFIGURATION gives
pub fn xq_max_bitpool(sample_rate: u32) -> u8 {
    xq_bitpool(Quality::High, sample_rate)
}

/// `wanted` within the sink's SBC bitpool range when it is known (as the
/// SET_CONFIGURATION's range, `caps::RemoteCaps::sbc_bitpool_range`)
fn sink_bitpool(wanted: u8, config: &EncoderConfig) -> u8 {
    let (min, max) = (config.peer.sbc_min_bitpool, config.peer.sbc_max_bitpool);
    if max < 2 {
        return wanted;
    }
    wanted.min(max).max(min.min(max)).max(2)
}

/// The A2DP SBC encoder: one SBC frame (128 PCM frames) per `encode`
pub struct SbcEncoder {
    kind: CodecKind,
    frame: SbcFrameEncoder,
    frame_len: usize,
    sample_rate: u32,
}

impl SbcEncoder {
    /// SBC: joint stereo (mono for one channel), the bitpool by quality
    /// within the sink's range
    pub fn new(config: &EncoderConfig) -> Result<SbcEncoder, EncoderError> {
        let bitpool = sink_bitpool(quality_bitpool(config.quality), config);
        Self::with_mode(CodecKind::Sbc, config, ChannelMode::JointStereo, bitpool)
    }

    /// SBC XQ: dual channel (mono for one channel), [`xq_bitpool`] within
    /// the sink's range
    pub fn new_xq(config: &EncoderConfig) -> Result<SbcEncoder, EncoderError> {
        let bitpool = sink_bitpool(xq_bitpool(config.quality, config.sample_rate), config);
        Self::with_mode(CodecKind::SbcXq, config, ChannelMode::DualChannel, bitpool)
    }

    fn with_mode(
        kind: CodecKind,
        config: &EncoderConfig,
        stereo_mode: ChannelMode,
        bitpool: u8,
    ) -> Result<SbcEncoder, EncoderError> {
        let unsupported = |what: String| EncoderError::Unsupported { codec: kind, what };
        if config.sample_rate != 44100 && config.sample_rate != 48000 {
            return Err(unsupported(format!("sample rate {} Hz (44100 or 48000)", config.sample_rate)));
        }
        let channel_mode = match config.channels {
            1 => ChannelMode::Mono,
            2 => stereo_mode,
            n => return Err(unsupported(format!("{n} channels"))),
        };
        let frame = SbcFrameEncoder::new(SbcConfig {
            sample_rate: config.sample_rate,
            channel_mode,
            blocks: BLOCKS,
            subbands: SUBBANDS,
            allocation: Allocation::Loudness,
            bitpool,
        })?;
        // Bitpool is never 0 here, so every frame has exactly this length
        let frame_len = frame.max_frame_len();
        Ok(SbcEncoder { kind, frame, frame_len, sample_rate: config.sample_rate })
    }

    /// Length of every SBC frame of this stream
    pub fn frame_len(&self) -> usize {
        self.frame_len
    }

    pub fn bitpool(&self) -> u8 {
        self.frame.config().bitpool
    }
}

impl Encoder for SbcEncoder {
    fn kind(&self) -> CodecKind {
        self.kind
    }

    fn pcm_format(&self) -> PcmFormat {
        PcmFormat::I16
    }

    fn pcm_frames_per_encode(&self) -> usize {
        BLOCKS as usize * SUBBANDS as usize
    }

    fn max_output_bytes(&self) -> usize {
        self.frame_len
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let Pcm::I16(pcm) = pcm else {
            return Err(EncoderError::Encode { codec: self.kind, what: "SBC takes 16-bit PCM".into() });
        };
        let bytes = self.frame.encode(pcm, out)?;
        Ok(Encoded { bytes, frames: 1 })
    }

    /// As v1: 8 * frame length * sample rate / (subbands * blocks), in kbps,
    /// rounded down
    fn bitrate_kbps(&self) -> u32 {
        (8 * self.frame_len as u32 * self.sample_rate) / (SUBBANDS as u32 * BLOCKS as u32) / 1000
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PeerParams;

    fn config(sample_rate: u32, channels: u32, quality: Quality) -> EncoderConfig {
        EncoderConfig {
            sample_rate,
            channels,
            quality,
            mtu: 895,
            bits_per_sample: 16,
            abr: false,
            peer: PeerParams::default(),
        }
    }

    #[test]
    fn frame_length_and_bitrate() {
        // (rate, channels, quality, bitpool, frame length, kbps)
        let cases = [
            (44100, 2, Quality::High, 53, 119, 327),
            (44100, 2, Quality::Standard, 35, 83, 228),
            (44100, 2, Quality::Mobile, 19, 51, 140),
            (48000, 2, Quality::High, 53, 119, 357),
            (48000, 2, Quality::Standard, 35, 83, 249),
            (48000, 2, Quality::Mobile, 19, 51, 153),
            (44100, 1, Quality::High, 53, 114, 314),
            (44100, 1, Quality::Standard, 35, 78, 214),
            (44100, 1, Quality::Mobile, 19, 46, 126),
            (48000, 1, Quality::High, 53, 114, 342),
            (48000, 1, Quality::Standard, 35, 78, 234),
            (48000, 1, Quality::Mobile, 19, 46, 138),
        ];
        for (rate, ch, quality, bitpool, len, kbps) in cases {
            let mut e = SbcEncoder::new(&config(rate, ch, quality)).unwrap();
            assert_eq!(e.bitpool(), bitpool);
            assert_eq!(e.frame_len(), len, "{rate} {ch} {quality:?}");
            assert_eq!(e.max_output_bytes(), len);
            assert_eq!(e.bitrate_kbps(), kbps, "{rate} {ch} {quality:?}");
            assert_eq!(e.pcm_frames_per_encode(), 128);
            assert_eq!(e.pcm_format(), PcmFormat::I16);

            // Every frame has that length, whatever the input
            let mut out = vec![0u8; len];
            let mut seed = 1u32;
            for i in 0..50 {
                let pcm: Vec<i16> = (0..128 * ch as usize)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        if i % 10 == 0 { 0 } else { seed as i16 }
                    })
                    .collect();
                let r = e.encode(Pcm::I16(&pcm), &mut out).unwrap();
                assert_eq!(r, Encoded { bytes: len, frames: 1 });
                assert_eq!(out[0], 0x9C);
                assert_eq!(out[2], bitpool);
            }
        }
    }

    #[test]
    fn medium_high_is_high() {
        let e = SbcEncoder::new(&config(48000, 2, Quality::MediumHigh)).unwrap();
        assert_eq!(e.bitpool(), 53);
    }

    #[test]
    fn header() {
        let mut e = SbcEncoder::new(&config(44100, 2, Quality::High)).unwrap();
        let mut out = [0u8; 119];
        e.encode(Pcm::I16(&[0; 256]), &mut out).unwrap();
        // 44.1 kHz (10), 16 blocks (11), joint stereo (11), loudness (0), 8
        // subbands (1)
        assert_eq!(out[1], 0b1011_1101);
        let mut e = SbcEncoder::new(&config(48000, 1, Quality::High)).unwrap();
        e.encode(Pcm::I16(&[0; 128]), &mut out).unwrap();
        // 48 kHz, 16 blocks, mono, loudness, 8 subbands
        assert_eq!(out[1], 0b1111_0001);
    }

    #[test]
    fn unsupported_configurations() {
        for (rate, ch) in [(32000, 2), (16000, 1), (96000, 2), (44100, 0), (48000, 3)] {
            assert!(matches!(
                SbcEncoder::new(&config(rate, ch, Quality::High)),
                Err(EncoderError::Unsupported { codec: CodecKind::Sbc, .. })
            ));
        }
    }

    #[test]
    fn input_and_output_length_errors() {
        let mut e = SbcEncoder::new(&config(44100, 2, Quality::High)).unwrap();
        let mut out = [0u8; 119];
        for n in [0, 128, 255, 257, 512] {
            let pcm = vec![0i16; n];
            assert!(matches!(
                e.encode(Pcm::I16(&pcm), &mut out),
                Err(EncoderError::InputLength { expected: 256, got }) if got == n
            ));
        }
        assert!(matches!(
            e.encode(Pcm::I16(&[0; 256]), &mut [0u8; 118]),
            Err(EncoderError::OutputTooSmall { have: 118, need: 119 })
        ));
        assert!(matches!(e.encode(Pcm::I32(&[0; 256]), &mut out), Err(EncoderError::Encode { .. })));
        // Still usable
        assert_eq!(e.encode(Pcm::I16(&[0; 256]), &mut out).unwrap().bytes, 119);

        let mut e = SbcEncoder::new(&config(48000, 1, Quality::Mobile)).unwrap();
        assert!(matches!(
            e.encode(Pcm::I16(&[0; 256]), &mut out),
            Err(EncoderError::InputLength { expected: 128, got: 256 })
        ));
    }

    #[test]
    fn create_returns_sbc() {
        let e = crate::create(CodecKind::Sbc, &config(48000, 2, Quality::Standard)).unwrap();
        assert_eq!(e.kind(), CodecKind::Sbc);
        assert_eq!(e.max_output_bytes(), 83);
    }

    #[test]
    fn xq_frame_length_and_bitrate() {
        // (rate, channels, quality, bitpool, frame length, kbps, header byte 1)
        let cases = [
            (44100, 2, Quality::High, 47, 200, 551, 0b1011_0101),
            (44100, 2, Quality::Standard, 43, 184, 507, 0b1011_0101),
            (44100, 2, Quality::Mobile, 32, 140, 385, 0b1011_0101),
            (48000, 2, Quality::High, 43, 184, 552, 0b1111_0101),
            (48000, 2, Quality::Standard, 39, 168, 504, 0b1111_0101),
            (48000, 2, Quality::Mobile, 29, 128, 384, 0b1111_0101),
            (44100, 1, Quality::High, 47, 102, 281, 0b1011_0001),
            (48000, 1, Quality::High, 43, 94, 282, 0b1111_0001),
        ];
        for (rate, ch, quality, bitpool, len, kbps, hdr) in cases {
            let mut e = SbcEncoder::new_xq(&config(rate, ch, quality)).unwrap();
            assert_eq!(e.kind(), CodecKind::SbcXq);
            assert_eq!(e.bitpool(), bitpool, "{rate} {ch} {quality:?}");
            assert_eq!(e.frame_len(), len, "{rate} {ch} {quality:?}");
            assert_eq!(e.bitrate_kbps(), kbps, "{rate} {ch} {quality:?}");
            let mut out = vec![0u8; len];
            let pcm: Vec<i16> = (0..128 * ch as usize).map(|i| ((i * 7919) % 20000) as i16 - 10000).collect();
            for _ in 0..4 {
                assert_eq!(e.encode(Pcm::I16(&pcm), &mut out).unwrap(), Encoded { bytes: len, frames: 1 });
                assert_eq!((out[0], out[1], out[2]), (0x9C, hdr, bitpool));
            }
        }
        assert_eq!(SbcEncoder::new_xq(&config(44100, 2, Quality::MediumHigh)).unwrap().bitpool(), 47);
        assert_eq!((xq_max_bitpool(44100), xq_max_bitpool(48000)), (47, 43));
    }

    #[test]
    fn xq_fills_packets() {
        // High: three frames and the media payload header in one 2-DH5 (679
        // bytes less 4 L2CAP and 12 RTP), five in one 3-DH5 (1021 less 16)
        for rate in [44100, 48000] {
            let len = SbcEncoder::new_xq(&config(rate, 2, Quality::High)).unwrap().frame_len();
            let packet = |frames: usize| 1 + frames * len;
            assert!(packet(3) <= 679 - 16 && packet(4) > 679 - 16, "{rate}");
            assert!(packet(5) <= 1021 - 16 && packet(6) > 1021 - 16, "{rate}");
        }
    }

    #[test]
    fn sbc_bitpool_within_the_sink_range() {
        // (sink min, sink max, quality) -> bitpool
        for (min, max, quality, want) in [
            (0, 0, Quality::High, 53), // not known
            (2, 53, Quality::High, 53),
            (2, 35, Quality::High, 35),
            (2, 35, Quality::Standard, 35),
            (2, 35, Quality::Mobile, 19),
            (2, 250, Quality::High, 53),
            (25, 40, Quality::Mobile, 25),
        ] {
            let mut c = config(48000, 2, quality);
            c.peer.sbc_min_bitpool = min;
            c.peer.sbc_max_bitpool = max;
            let mut e = SbcEncoder::new(&c).unwrap();
            assert_eq!(e.bitpool(), want, "{min}-{max} {quality:?}");
            let mut out = vec![0u8; e.frame_len()];
            e.encode(Pcm::I16(&[0; 256]), &mut out).unwrap();
            assert_eq!(out[2], want);
        }
        // 35: 83-byte frames, as Standard
        let mut c = config(48000, 2, Quality::High);
        c.peer.sbc_max_bitpool = 35;
        assert_eq!(SbcEncoder::new(&c).unwrap().frame_len(), 83);
    }

    #[test]
    fn xq_bitpool_capped_by_the_sink() {
        let mut c = config(44100, 2, Quality::High);
        c.peer.sbc_max_bitpool = 45;
        let e = SbcEncoder::new_xq(&c).unwrap();
        assert_eq!((e.bitpool(), e.frame_len()), (45, 192));
        c.peer.sbc_max_bitpool = 53;
        assert_eq!(SbcEncoder::new_xq(&c).unwrap().bitpool(), 47);
        let e = crate::create(CodecKind::SbcXq, &config(48000, 2, Quality::Standard)).unwrap();
        assert_eq!((e.kind(), e.max_output_bytes()), (CodecKind::SbcXq, 168));
        assert!(matches!(
            SbcEncoder::new_xq(&config(32000, 2, Quality::High)),
            Err(EncoderError::Unsupported { codec: CodecKind::SbcXq, .. })
        ));
    }

    #[test]
    fn frame_config_validation() {
        let ok = SbcConfig {
            sample_rate: 16000,
            channel_mode: ChannelMode::DualChannel,
            blocks: 12,
            subbands: 4,
            allocation: Allocation::Snr,
            bitpool: 200,
        };
        let e = SbcFrameEncoder::new(ok).unwrap();
        assert_eq!(ok.effective_bitpool(), 64);
        assert_eq!(e.pcm_samples(), 96);
        assert!(SbcFrameEncoder::new(SbcConfig { blocks: 6, ..ok }).is_err());
        assert!(SbcFrameEncoder::new(SbcConfig { subbands: 6, ..ok }).is_err());
        assert!(SbcFrameEncoder::new(SbcConfig { sample_rate: 22050, ..ok }).is_err());
    }
}
