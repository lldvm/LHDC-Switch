//! Bit-exactness of the Rust SBC encoder (codec::sbc) against Bluedroid's C
//! encoder (btstack-sys, feature "sbc-reference"): every configuration,
//! a spread of bitpools, a few hundred consecutive frames of mixed input.
//!
//! Speed comparison (release): cargo test -p codec --release --test
//! sbc_bitexact -- --ignored --nocapture --test-threads=1

use btstack_sys::sbc_reference;
use codec::sbc::{Allocation, ChannelMode, SbcConfig, SbcFrameEncoder};
use proptest::prelude::*;
use std::sync::Mutex;

/// The C encoder keeps joint stereo scratch (s32LRSum / s32LRDiff in
/// sbc_encoder.c) in globals, so its instances race when tests run in
/// parallel; its calls are serialized
static C_ENCODER: Mutex<()> = Mutex::new(());

struct Reference(sbc_reference::Encoder);

impl Reference {
    fn new(config: &SbcConfig) -> Reference {
        Reference(sbc_reference::Encoder::new(
            config.sample_rate as u16,
            config.channel_mode as i32,
            config.blocks as i32,
            config.subbands as i32,
            config.allocation as i32,
            config.bitpool as i32,
        ))
    }

    fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
        let _lock = C_ENCODER.lock().unwrap_or_else(|e| e.into_inner());
        self.0.encode(pcm)
    }
}

const SAMPLE_RATES: [u32; 4] = [16000, 32000, 44100, 48000];
const BLOCKS: [u8; 4] = [4, 8, 12, 16];
const SUBBANDS: [u8; 2] = [4, 8];
const ALLOCATIONS: [Allocation; 2] = [Allocation::Loudness, Allocation::Snr];

/// Largest bitpool of a mode (one more is lowered to it by both encoders)
fn max_bitpool(mode: ChannelMode, subbands: u8) -> u32 {
    match mode {
        ChannelMode::Mono | ChannelMode::DualChannel => 16 * subbands as u32,
        _ => (32 * subbands as u32).min(255),
    }
}

fn bitpools(mode: ChannelMode, subbands: u8) -> Vec<u8> {
    let max = max_bitpool(mode, subbands);
    let mut v: Vec<u32> = vec![2, 3, 7, 16, 19, 31, 35, 53, 64, 100, 128, 200, 250, max - 1, max, max + 1];
    v.retain(|&b| b <= (max + 1).min(255));
    v.sort_unstable();
    v.dedup();
    v.into_iter().map(|b| b as u8).collect()
}

/// xorshift32: deterministic input without a dependency
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

/// Mixed test signal: the kind changes every few frames and sometimes
/// within a frame, so transitions are covered too
struct Signal {
    rng: Rng,
    t: u64,
    kind: u32,
    left: u32,
    freq: [f64; 2],
    sample_rate: f64,
}

impl Signal {
    fn new(seed: u32, sample_rate: u32) -> Signal {
        Signal { rng: Rng(seed | 1), t: 0, kind: 0, left: 0, freq: [440.0, 1000.0], sample_rate: sample_rate as f64 }
    }

    fn sample(&mut self, ch: usize) -> i16 {
        let t = self.t as f64 / self.sample_rate;
        let f = self.freq[ch];
        let phase = 2.0 * std::f64::consts::PI * f * t;
        match self.kind {
            // Sine
            0 => (phase.sin() * 20000.0) as i16,
            // Full-scale square
            1 => {
                if phase.sin() >= 0.0 {
                    i16::MAX
                } else {
                    i16::MIN
                }
            }
            // Noise
            2 => self.rng.next() as i16,
            // Silence
            3 => 0,
            // Clipped sine (2x over full scale)
            4 => (phase.sin() * 65536.0).clamp(i16::MIN as f64, i16::MAX as f64) as i16,
            // Quiet noise
            5 => (self.rng.next() as i16) >> 10,
            // DC at the extremes, different per channel
            6 => {
                if ch == 0 {
                    i16::MIN
                } else {
                    i16::MAX
                }
            }
            // Impulses
            7 => {
                if self.t.is_multiple_of(97) {
                    i16::MAX
                } else if self.t.is_multiple_of(89) {
                    i16::MIN
                } else {
                    0
                }
            }
            // Same signal on both channels (joint stereo sum only)
            8 => ((2.0 * std::f64::consts::PI * self.freq[0] * t).sin() * 30000.0) as i16,
            // Opposite signals (joint stereo difference only)
            _ => {
                let s = ((2.0 * std::f64::consts::PI * self.freq[0] * t).sin() * 30000.0) as i16;
                if ch == 0 { s } else { s.saturating_neg() }
            }
        }
    }

    /// One frame of `n` PCM frames, `channels` interleaved
    fn frame(&mut self, n: usize, channels: usize, out: &mut Vec<i16>) {
        out.clear();
        for _ in 0..n {
            if self.left == 0 {
                self.kind = self.rng.next() % 10;
                // Mostly whole frames, sometimes a switch mid-frame
                self.left = if self.rng.next().is_multiple_of(4) { 1 + self.rng.next() % 200 } else { n as u32 * 3 };
                self.freq = [50.0 + (self.rng.next() % 20000) as f64, 50.0 + (self.rng.next() % 20000) as f64];
            }
            self.left -= 1;
            for ch in 0..channels {
                let s = self.sample(ch);
                out.push(s);
            }
            self.t += 1;
        }
    }
}

/// Encodes `frames` frames with both encoders and compares every byte
fn compare(config: SbcConfig, frames: usize, seed: u32) {
    let mut rust = SbcFrameEncoder::new(config).unwrap();
    let mut c = Reference::new(&config);
    let mut signal = Signal::new(seed, config.sample_rate);
    let mut pcm = Vec::new();
    let mut out = vec![0u8; rust.max_frame_len()];
    for i in 0..frames {
        signal.frame(config.pcm_frames_per_frame(), config.channels(), &mut pcm);
        let expected = c.encode(&pcm);
        let n = rust.encode(&pcm, &mut out).unwrap();
        assert!(n <= rust.max_frame_len());
        if config.bitpool != 0 {
            assert_eq!(n, config.frame_len(), "{config:?}: frame {i}: length");
        }
        assert_eq!(&out[..n], &expected[..], "{config:?}: frame {i}");
    }
}

#[test]
fn all_configurations() {
    let mut runs = 0;
    for sample_rate in SAMPLE_RATES {
        for channel_mode in ChannelMode::ALL {
            for blocks in BLOCKS {
                for subbands in SUBBANDS {
                    for allocation in ALLOCATIONS {
                        for bitpool in bitpools(channel_mode, subbands) {
                            let config = SbcConfig { sample_rate, channel_mode, blocks, subbands, allocation, bitpool };
                            compare(config, 200, 0x1234_5678 ^ runs);
                            runs += 1;
                        }
                    }
                }
            }
        }
    }
    eprintln!("{runs} configurations compared");
}

/// The A2DP profile (SbcEncoder's configurations), longer streams
#[test]
fn a2dp_configurations_long() {
    for sample_rate in [44100, 48000] {
        for channel_mode in [ChannelMode::Mono, ChannelMode::JointStereo] {
            for bitpool in [19, 35, 53] {
                let config = SbcConfig {
                    sample_rate,
                    channel_mode,
                    blocks: 16,
                    subbands: 8,
                    allocation: Allocation::Loudness,
                    bitpool,
                };
                compare(config, 3000, bitpool as u32);
            }
        }
    }
}

/// Bitpool 0 and 1 (below what A2DP allows; Bluedroid still encodes)
#[test]
fn degenerate_bitpools() {
    for channel_mode in ChannelMode::ALL {
        for subbands in SUBBANDS {
            for blocks in BLOCKS {
                for allocation in ALLOCATIONS {
                    for bitpool in [0, 1] {
                        let config =
                            SbcConfig { sample_rate: 44100, channel_mode, blocks, subbands, allocation, bitpool };
                        compare(config, 50, 7);
                    }
                }
            }
        }
    }
}

/// The SbcEncoder (Encoder trait) produces the reference's frames
#[test]
fn sbc_encoder_matches_reference() {
    use codec::{EncoderConfig, Pcm, PeerParams, Quality};
    for (sample_rate, channels, quality, bitpool) in
        [(44100, 2, Quality::High, 53), (48000, 2, Quality::Mobile, 19), (48000, 1, Quality::Standard, 35)]
    {
        let config = EncoderConfig {
            sample_rate,
            channels,
            quality,
            mtu: 895,
            bits_per_sample: 16,
            abr: false,
            peer: PeerParams::default(),
        };
        let mut e = codec::create(codec::CodecKind::Sbc, &config).unwrap();
        let channel_mode = if channels == 1 { ChannelMode::Mono } else { ChannelMode::JointStereo };
        let mut c = Reference::new(&SbcConfig {
            sample_rate,
            channel_mode,
            blocks: 16,
            subbands: 8,
            allocation: Allocation::Loudness,
            bitpool,
        });
        let mut signal = Signal::new(99, sample_rate);
        let mut pcm = Vec::new();
        let mut out = vec![0u8; e.max_output_bytes()];
        for _ in 0..500 {
            signal.frame(e.pcm_frames_per_encode(), channels as usize, &mut pcm);
            let r = e.encode(Pcm::I16(&pcm), &mut out).unwrap();
            assert_eq!(r.frames, 1);
            assert_eq!(&out[..r.bytes], &c.encode(&pcm)[..]);
        }
    }
}

fn arb_config() -> impl Strategy<Value = SbcConfig> {
    (
        prop::sample::select(SAMPLE_RATES.to_vec()),
        prop::sample::select(ChannelMode::ALL.to_vec()),
        prop::sample::select(BLOCKS.to_vec()),
        prop::sample::select(SUBBANDS.to_vec()),
        prop::sample::select(ALLOCATIONS.to_vec()),
        2u32..=255,
    )
        .prop_map(|(sample_rate, channel_mode, blocks, subbands, allocation, bp)| {
            let bitpool = (2 + bp % (max_bitpool(channel_mode, subbands) - 1)) as u8;
            SbcConfig { sample_rate, channel_mode, blocks, subbands, allocation, bitpool }
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() })]

    /// Random configurations, arbitrary sample values
    #[test]
    fn random_input(config in arb_config(), frames in prop::collection::vec(prop::collection::vec(any::<i16>(), 256), 1..12)) {
        let mut rust = SbcFrameEncoder::new(config).unwrap();
        let mut c = Reference::new(&config);
        let mut out = vec![0u8; rust.max_frame_len()];
        let n = rust.pcm_samples();
        for f in &frames {
            // Up to 256 samples are drawn; frames of fewer take a prefix,
            // larger ones repeat it
            let pcm: Vec<i16> = f.iter().cycle().take(n).copied().collect();
            let len = rust.encode(&pcm, &mut out).unwrap();
            prop_assert_eq!(&out[..len], &c.encode(&pcm)[..]);
        }
    }
}

/// Rough speed against the C encoder (run in release with --ignored)
#[test]
#[ignore]
fn speed() {
    use std::time::Instant;
    for (channel_mode, bitpool) in [(ChannelMode::JointStereo, 53), (ChannelMode::Mono, 53)] {
        let config = SbcConfig {
            sample_rate: 48000,
            channel_mode,
            blocks: 16,
            subbands: 8,
            allocation: Allocation::Loudness,
            bitpool,
        };
        let frames = 20000;
        let mut signal = Signal::new(5, 48000);
        let mut pcm = Vec::with_capacity(frames * 256);
        let mut buf = Vec::new();
        for _ in 0..frames {
            signal.frame(128, config.channels(), &mut buf);
            pcm.extend_from_slice(&buf);
        }
        let n = 128 * config.channels();

        let mut rust = SbcFrameEncoder::new(config).unwrap();
        let mut out = vec![0u8; rust.max_frame_len()];
        let start = Instant::now();
        let mut sum = 0usize;
        for f in pcm.chunks_exact(n) {
            sum += rust.encode(f, &mut out).unwrap();
            sum += out[5] as usize;
        }
        let rust_time = start.elapsed();

        // Unlocked: nothing else runs with --ignored --test-threads=1
        let mut c = sbc_reference::Encoder::new(48000, channel_mode as i32, 16, 8, 0, bitpool as i32);
        let start = Instant::now();
        for f in pcm.chunks_exact(n) {
            let o = c.encode(f);
            sum += o.len() + o[5] as usize;
        }
        let c_time = start.elapsed();
        let audio = frames as f64 * 128.0 / 48000.0;
        eprintln!(
            "{channel_mode:?} bitpool {bitpool}: Rust {:.1} us/frame, C {:.1} us/frame (incl. its Vec), {:.0}x / {:.0}x real time ({sum})",
            rust_time.as_secs_f64() * 1e6 / frames as f64,
            c_time.as_secs_f64() * 1e6 / frames as f64,
            audio / rust_time.as_secs_f64(),
            audio / c_time.as_secs_f64(),
        );
    }
}
