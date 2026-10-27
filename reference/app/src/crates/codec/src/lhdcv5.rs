// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! LHDC V5: Savitech's high-resolution Bluetooth codec (44.1 / 48 / 96 /
//! 192 kHz, 16 or 24-bit stereo, 5 ms frames, 64 to 1000 kbps over A2DP),
//! encoded by a portable C port of the encoder Android ships (crate
//! `lhdcv5-sys`; the port and the AOSP original are Apache-2.0; LHDC itself
//! is Savitech's, whose licensing terms for products advertising LHDC are
//! separate from the code licence).
//!
//! The encoder takes one 5 ms frame of interleaved stereo PCM per call
//! (packed s16le or s24le: mono is duplicated, 24-bit input is repacked
//! from the top of the int32 container) and returns a complete packet of
//! one or more frames, each with its own 2-byte frame header, every few
//! calls. Parameters follow Android's a2dp_vendor_lhdcv5_encoder.cc as v1
//! did: 20 ms encode interval, packets sized for the MTU minus the 2-byte
//! media payload header, the sink's bit rate limits applied after init, and
//! a time-based LHDC Switch ABR policy ([`LhdcV5Encoder::abr_adjust`]).

use lhdcv5_sys::{LhdcV5Enc, ffi};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, Quality};

const CODEC: CodecKind = CodecKind::LhdcV5;
pub fn encoder_backend() -> &'static str { lhdcv5_sys::BACKEND }

/// The A2DP media payload header in front of the frames
/// (A2DP_LHDC_MPL_HDR_LEN)
const MPL_HDR_LEN: usize = 2;

#[path = "lhdc_abr.rs"]
mod adaptive;
use adaptive::Abr;
use std::time::Instant;

pub struct LhdcV5Encoder {
    enc: LhdcV5Enc,
    format: PcmFormat,
    channels: usize,
    /// PCM frames per LHDC frame (240 / 480 / 960)
    block_size: usize,
    /// MTU the encoder sizes its packets for (media MTU - payload header)
    enc_mtu: usize,
    /// Input repacked for the encoder: interleaved stereo s16le or packed
    /// s24le, one frame
    packed: Vec<u8>,
    abr: Option<Abr>,
    abr_clock: Instant,
    /// The sink's maximum bit rate (P7) is below the quality mode's
    peer_capped: bool,
}

#[cfg(test)]
mod switch_tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn rust_custom_fixed_and_adaptive_rates_are_real_and_peer_bounded() {
        if !lhdcv5_sys::CUSTOM_BITRATES { return; }
        for rate in [44100,48000,96000,192000] {
            for target in [700,750,800] {
                let cfg=EncoderConfig {sample_rate:rate,channels:2,quality:Quality::MediumHigh,
                    mtu:667,bits_per_sample:24,abr:false,peer:Default::default()};
                let mut enc=LhdcV5Encoder::with_bitrates(&cfg,900,Some(target)).unwrap();
                assert_eq!(enc.bitrate_kbps(),target);
                let mut out=vec![0u8;8192];let pcm=vec![0i32;enc.block_size*2];
                let (mut bytes,mut frames)=(0,0);
                for _ in 0..200 {let e=enc.encode(Pcm::I32(&pcm),&mut out).unwrap();assert!(e.bytes<=665);bytes+=e.bytes;frames+=e.frames;}
                let expected=target as f64*125.0*frames as f64*enc.block_size as f64/rate as f64;
                assert!((bytes as f64-expected).abs()<expected*0.02,"{rate} Hz {target} kbps: {bytes} bytes, expected {expected}");
                let auto=EncoderConfig {abr:true,..cfg};
                let mut enc=LhdcV5Encoder::with_bitrates(&auto,target,None).unwrap();
                for t in 1..150 {enc.abr_clock=Instant::now()-Duration::from_secs(t);enc.abr_feedback(Default::default());assert!(enc.bitrate_kbps()<=target);}
                assert_eq!(enc.bitrate_kbps(),target);
                enc.abr_clock=Instant::now()-Duration::from_secs(150);enc.abr_adjust(64);
                assert!(enc.bitrate_kbps()<=400);
                for t in 151..240 {enc.abr_clock=Instant::now()-Duration::from_secs(t);enc.abr_feedback(Default::default());}
                assert_eq!(enc.bitrate_kbps(),target);
                let limited=EncoderConfig {peer:crate::PeerParams {lhdcv5_bitrate_limits:0x20,..Default::default()},..cfg};
                let capped=LhdcV5Encoder::with_bitrates(&limited,900,Some(target)).unwrap();
                assert!(capped.bitrate_kbps()<=500);
            }
        }
    }
    #[test]
    fn switching_between_custom_rates_and_900_updates_actual_packets() {
        if !lhdcv5_sys::CUSTOM_BITRATES { return; }
        let cfg=EncoderConfig {sample_rate:48000,channels:2,quality:Quality::MediumHigh,mtu:667,bits_per_sample:24,abr:false,peer:Default::default()};
        let mut enc=LhdcV5Encoder::with_bitrates(&cfg,900,Some(700)).unwrap();
        let pcm=vec![0i32;480];let mut out=vec![0u8;8192];
        for target in [700,750,800,900,800,750,700,500] {
            enc.enc.set_bitrate_kbps(target).unwrap();
            for _ in 0..8 {enc.encode(Pcm::I32(&pcm),&mut out).unwrap();}
            let (mut bytes,mut frames)=(0,0);
            for _ in 0..200 {let r=enc.encode(Pcm::I32(&pcm),&mut out).unwrap();bytes+=r.bytes;frames+=r.frames;assert!(r.bytes<=665);}
            let expected=target as f64*125.0*frames as f64*240.0/48000.0;
            assert!((bytes as f64-expected).abs()<expected*0.02,"{target} kbps: {bytes} bytes, expected {expected}");
            assert_eq!(enc.bitrate_kbps(),target);
        }
        enc.enc.set_bitrate_index(ffi::LHDC_QUALITY_HIGH,false).unwrap();
        assert_eq!(enc.bitrate_kbps(),900);
    }
    #[test]
    fn feedback_ignores_packet_count_but_responds_to_age_and_new_loss() {
        let cfg=EncoderConfig {sample_rate:48000,channels:2,quality:Quality::High,
            mtu:667,bits_per_sample:24,abr:true,peer:Default::default()};
        let mut enc=LhdcV5Encoder::new(&cfg).unwrap();
        enc.abr_clock=Instant::now()-Duration::from_secs(10);
        enc.abr_feedback(crate::CongestionFeedback {packets:8,audio_us:10_000,oldest_us:1000,..Default::default()});
        assert!(enc.bitrate_kbps()>=400);
        let previous=enc.bitrate_kbps();
        enc.abr_feedback(crate::CongestionFeedback {packets:1,audio_us:5000,oldest_us:160_000,..Default::default()});
        assert!(enc.bitrate_kbps()<previous);
        let previous=enc.bitrate_kbps();
        enc.abr_clock=Instant::now()-Duration::from_secs(11);
        enc.abr_feedback(crate::CongestionFeedback {lost_packets:1,..Default::default()});
        assert!(enc.bitrate_kbps()<previous);
    }
    #[test]
    fn normal_twenty_ms_packets_recover_from_the_floor_without_reconnecting() {
        let cfg=EncoderConfig {sample_rate:48000,channels:2,quality:Quality::High,
            mtu:667,bits_per_sample:24,abr:true,peer:Default::default()};
        let mut enc=LhdcV5Encoder::new(&cfg).unwrap();
        enc.abr=Some(Abr::new(&bitrate_table(48000),0,8,900));
        for t in [10,11,12,13,14] {
            enc.abr_clock=Instant::now()-Duration::from_secs(t);
            enc.abr_feedback(crate::CongestionFeedback {packets:16,audio_us:200_000,oldest_us:200_000,..Default::default()});
        }
        assert_eq!(enc.bitrate_kbps(),160);
        // Replay the stable conditions in the user's 16:40-16:42 trace:
        // two 20 ms packets, head age about 20 ms, zero local losses.
        for t in 15..130 {
            enc.abr_clock=Instant::now()-Duration::from_secs(t);
            enc.abr_feedback(crate::CongestionFeedback {packets:2,audio_us:40_000,oldest_us:22_000,..Default::default()});
        }
        assert_eq!(enc.bitrate_kbps(),900);
        let pcm=vec![0i32;enc.block_size*2];let mut out=vec![0u8;8192];
        let mut bytes=0;
        for _ in 0..20 {bytes+=enc.encode(Pcm::I32(&pcm),&mut out).unwrap().bytes;}
        assert!(bytes>8000,"the actual Rust encoder must leave the 160 kbps floor");
    }
    #[test]
    fn high_ceiling_changes_real_encoder_without_restart() {
        for rate in [44100,48000,96000,192000] {
            let cfg=EncoderConfig { sample_rate:rate,channels:2,quality:Quality::High,
                mtu:667,bits_per_sample:24,abr:true,peer:Default::default() };
            let mut enc=LhdcV5Encoder::new(&cfg).unwrap();
            let max=fitting_bitrate_index(rate,enc.block_size,enc.enc_mtu) as usize;
            enc.abr=Some(Abr::new(&bitrate_table(rate), if rate>=96000 {3} else {0},max,1000));
            let mut seen=vec![400];
            for t in [6,27,48] {
                enc.abr_clock=Instant::now()-Duration::from_secs(t);
                enc.abr_adjust(0);
                let target=enc.bitrate_kbps();
                seen.push(target);
                let pcm=vec![0i32;enc.block_size*2]; let mut out=vec![0u8;8192];
                let mut bytes=0;
                for _ in 0..100 { let r=enc.encode(Pcm::I32(&pcm),&mut out).unwrap(); assert!(r.bytes<=enc.enc_mtu); bytes+=r.bytes; }
                let expected=target as f64*125.0*enc.block_size as f64*100.0/rate as f64;
                assert!((bytes as f64)>expected*0.85 && (bytes as f64)<expected*1.15,
                    "{rate} Hz, {target} kbps: produced {bytes} bytes, expected about {expected}");
            }
            assert_eq!(*seen.last().unwrap(),bitrate_table(rate)[max]);
            assert!(seen.windows(2).all(|w| w[1]>=w[0]));
            enc.abr_adjust(64); assert!(enc.bitrate_kbps()<=400);
            enc.abr_reset(); assert_eq!(enc.bitrate_kbps(),400);
        }
    }
}

/// Sink capability P7[5:4] -> encoder quality index
/// (A2DP_MaxBitRatetoQualityLevelLhdcV5)
fn peer_max_bitrate_index(p7: u8) -> u32 {
    match p7 & 0x30 {
        0x10 => ffi::LHDC_QUALITY_LOW,  // 400 kbps
        0x20 => ffi::LHDC_QUALITY_MID,  // 500 kbps
        0x30 => ffi::LHDC_QUALITY_HIGH, // 900 kbps
        _ => ffi::LHDC_QUALITY_HIGH1,   // 1000 kbps = no upper limit
    }
}

/// Whether the sink lowers a fixed quality. Adaptive limits are reported
/// separately and its stage list is clamped to the negotiated peer/MTU.
fn peer_caps_bitrate(abr: bool, bitrate_inx: u32, p7: u8) -> bool {
    !abr && peer_max_bitrate_index(p7) < bitrate_inx
}

/// Sink capability P7[7:6] -> encoder quality index
/// (A2DP_MinBitRatetoQualityLevelLhdcV5)
fn peer_min_bitrate_index(p7: u8) -> u32 {
    match p7 & 0xC0 {
        0x40 => ffi::LHDC_QUALITY_LOW1, // 160 kbps
        0x80 => ffi::LHDC_QUALITY_LOW3, // 256 kbps
        0xC0 => ffi::LHDC_QUALITY_LOW,  // 400 kbps
        _ => ffi::LHDC_QUALITY_LOW0,    // 64 kbps = no lower limit
    }
}

/// The encoder's bit rate table (kbps) from 64 kbps up to
/// LHDC_QUALITY_HIGH1 (lhdcv5_api.c g_bitrate_table_*)
fn bitrate_table(sample_rate: u32) -> [u32; 9] {
    if sample_rate == 44100 {
        [64, 160, 192, 240, 320, 400, 480, 900, 1000]
    } else {
        [64, 160, 192, 256, 320, 400, 500, 900, 1000]
    }
}

fn adaptive_policy(sample_rate: u32, min: u32, max: u32, cap: u32) -> Abr {
    let native=bitrate_table(sample_rate);
    let mut rates=native[min as usize..=max as usize].to_vec();
    if lhdcv5_sys::CUSTOM_BITRATES {
        rates.extend([700,750,800].into_iter().filter(|r| *r>=native[min as usize] && *r<=native[max as usize]));
        rates.sort_unstable();rates.dedup();
    }
    Abr::new(&rates,0,rates.len()-1,cap)
}

/// Highest bit rate index up to LHDC_QUALITY_HIGH1 whose frames fit `mtu`.
/// The encoder sizes a frame for the bit rate over the frame's duration
/// (block_size / sample_rate), and the frame header rounds it up to an even
/// size plus 2 bytes; this estimate is that, rounded up once more.
fn fitting_bitrate_index(sample_rate: u32, block_size: usize, mtu: usize) -> u32 {
    let table = bitrate_table(sample_rate);
    let mut inx = ffi::LHDC_QUALITY_HIGH1;
    while inx > ffi::LHDC_QUALITY_LOW {
        let payload = (u64::from(table[inx as usize]) * 125 * block_size as u64).div_ceil(u64::from(sample_rate));
        if 2 + 2 * payload.div_ceil(2) as usize <= mtu {
            break;
        }
        inx -= 1;
    }
    inx
}

impl LhdcV5Encoder {
    pub fn new(config: &EncoderConfig) -> Result<LhdcV5Encoder, EncoderError> {
        let cap=std::env::var("LHDC_SWITCH_ABR_MAX_KBPS").ok()
            .and_then(|v|v.parse::<u32>().ok()).filter(|v|[400,500,700,750,800,900,1000].contains(v)).unwrap_or(400);
        let fixed=std::env::var("LHDC_SWITCH_FIXED_KBPS").ok()
            .and_then(|v|v.parse::<u32>().ok()).filter(|v|[700,750,800].contains(v));
        Self::with_bitrates(config,cap,fixed)
    }

    fn with_bitrates(config: &EncoderConfig, cap: u32, fixed: Option<u32>) -> Result<LhdcV5Encoder, EncoderError> {
        tracing::info!("LHDC Switch encoder backend: {}", lhdcv5_sys::BACKEND);
        let unsupported = |what: String| EncoderError::Unsupported { codec: CODEC, what };
        if !matches!(config.sample_rate, 44100 | 48000 | 96000 | 192000) {
            return Err(unsupported(format!(
                "sample rate {} Hz (use 44100, 48000, 96000 or 192000)",
                config.sample_rate
            )));
        }
        if !matches!(config.channels, 1 | 2) {
            return Err(unsupported(format!("{} channels", config.channels)));
        }
        let (format, encoder_bits) = match config.bits_per_sample {
            16 => (PcmFormat::I16, ffi::LHDCBT_SMPL_FMT_S16),
            24 => (PcmFormat::I32, ffi::LHDCBT_SMPL_FMT_S24),
            n => return Err(unsupported(format!("{n}-bit samples (use 16 or 24)"))),
        };

        // Quality mode -> target bit rate index (clamped to the sink's
        // limits below). Adaptive stages are applied after negotiation.
        let bitrate_inx = if config.abr {
            ffi::LHDC_QUALITY_AUTO
        } else {
            match config.quality {
                Quality::High => ffi::LHDC_QUALITY_HIGH1,      // 1000 kbps
                Quality::MediumHigh => ffi::LHDC_QUALITY_HIGH, // 900 kbps
                Quality::Standard => ffi::LHDC_QUALITY_MID,    // 500 kbps (480 at 44.1 kHz)
                Quality::Mobile => ffi::LHDC_QUALITY_LOW,      // 400 kbps
            }
        };

        // The encoder sizes its packets for this MTU: the media MTU (RTP
        // header already excluded) minus the 2-byte LHDC media payload header
        let enc_mtu = usize::from(config.mtu).saturating_sub(MPL_HDR_LEN);
        if enc_mtu < ffi::LHDC_MTU_MIN as usize {
            return Err(unsupported(format!(
                "media MTU {} (LHDC V5 needs at least {})",
                config.mtu,
                ffi::LHDC_MTU_MIN as usize + MPL_HDR_LEN
            )));
        }
        let enc_mtu = enc_mtu.min(ffi::LHDC_MTU_MAX as usize);

        let init_err = |e: lhdcv5_sys::Error| EncoderError::Init { codec: CODEC, what: e.to_string() };
        let mut enc = LhdcV5Enc::new().map_err(init_err)?;
        enc.init_encoder(config.sample_rate, encoder_bits, bitrate_inx, enc_mtu as u32, ffi::LHDC_ENC_INTERVAL_20MS)
            .map_err(init_err)?;

        let block_size = match enc.block_size() {
            Ok(n) if n > 0 => n as usize,
            r => {
                return Err(EncoderError::Init { codec: CODEC, what: format!("no frame size ({r:?})") });
            }
        };

        // Peer bit rate limits, applied after init as Android does; a
        // failure leaves the encoder's own limits (as v1)
        let p7 = config.peer.lhdcv5_bitrate_limits;
        let (mut max_inx, min_inx) = (peer_max_bitrate_index(p7), peer_min_bitrate_index(p7));
        let peer_capped = peer_caps_bitrate(config.abr, bitrate_inx, p7);
        // The encoder never splits a frame: one that does not fit the MTU
        // goes out whole, over the sink's MTU, and is dropped. Only 1000 kbps
        // at 44.1 kHz with the smallest MTUs gets there (a 5 ms frame sized
        // for 1000 kbps lasts 5.44 ms there: 682 bytes), so cap the bit rate
        // at what fits (900 kbps then). v1 sent those packets anyway.
        let fitting = fitting_bitrate_index(config.sample_rate, block_size, enc_mtu);
        if fitting < max_inx {
            tracing::info!(
                "LHDC V5: {} kbps frames do not fit the {enc_mtu}-byte MTU at {} Hz, capping the bit rate",
                bitrate_table(config.sample_rate)[max_inx as usize],
                config.sample_rate
            );
            max_inx = fitting;
        }
        if let Err(e) = enc.set_max_bitrate_index(max_inx) {
            tracing::warn!("LHDC V5: set_max_bitrate_index({max_inx}) failed: {e}");
        }
        if let Err(e) = enc.set_min_bitrate_index(min_inx) {
            tracing::warn!("LHDC V5: set_min_bitrate_index({min_inx}) failed: {e}");
        }

        let packed = vec![0u8; block_size * 2 * (encoder_bits as usize / 8)];

        let abr = if config.abr {
            let floor = if config.sample_rate >= 96000 { min_inx.max(3) } else { min_inx };
            let policy = adaptive_policy(config.sample_rate, floor, max_inx, cap);
            let rate = policy.table[policy.start];
            enc.set_bitrate_kbps(rate).map_err(init_err)?;
            tracing::info!("LHDC Switch: adaptive ceiling {} kbps, start {} kbps", policy.table.last().unwrap(), rate);
            Some(policy)
        } else {
            if let Some(requested)=fixed {
                if !lhdcv5_sys::CUSTOM_BITRATES {return Err(unsupported("exact custom bitrates require the Rust backend".into()));}
                let rates=bitrate_table(config.sample_rate);
                let target=requested.clamp(rates[min_inx as usize],rates[max_inx as usize]);
                enc.set_bitrate_kbps(target).map_err(init_err)?;
                tracing::info!("LHDC Switch: fixed request {requested} kbps, effective {target} kbps");
            }
            None
        };

        let encoder = LhdcV5Encoder {
            enc,
            format,
            channels: config.channels as usize,
            block_size,
            enc_mtu,
            packed,
            abr,
            abr_clock: Instant::now(),
            peer_capped,
        };
        tracing::info!(
            "LHDC V5: initialized (rate={}, ch={}, {}-bit, mtu={}, frame={} samples, bitrate={} kbps{}, peer limits idx {}..{}{})",
            config.sample_rate,
            config.channels,
            encoder_bits,
            enc_mtu,
            block_size,
            encoder.bitrate_kbps(),
            if config.abr { " ABR" } else { "" },
            min_inx,
            max_inx,
            if peer_capped { ", capped by the sink's maximum" } else { "" }
        );
        Ok(encoder)
    }

    /// Whether adaptive bit rate is active
    pub fn abr_enabled(&self) -> bool {
        self.abr.is_some()
    }

    /// Repacks one frame of input into `packed`: interleaved stereo, s16le
    /// or packed s24le (the top 24 bits of the MSB-aligned int32)
    fn pack(&mut self, pcm: Pcm<'_>) {
        let channels = self.channels;
        match pcm {
            Pcm::I16(s) => {
                for (frame, dst) in s.chunks_exact(channels).zip(self.packed.as_chunks_mut::<4>().0) {
                    let (l, r) = (frame[0], frame[channels - 1]);
                    dst[..2].copy_from_slice(&l.to_le_bytes());
                    dst[2..].copy_from_slice(&r.to_le_bytes());
                }
            }
            Pcm::I32(s) => {
                for (frame, dst) in s.chunks_exact(channels).zip(self.packed.as_chunks_mut::<6>().0) {
                    let (l, r) = (frame[0].to_le_bytes(), frame[channels - 1].to_le_bytes());
                    dst[..3].copy_from_slice(&l[1..]);
                    dst[3..].copy_from_slice(&r[1..]);
                }
            }
        }
    }
}

impl Encoder for LhdcV5Encoder {
    fn kind(&self) -> CodecKind {
        CODEC
    }

    fn pcm_format(&self) -> PcmFormat {
        self.format
    }

    /// One LHDC frame (5 ms: 240 / 480 / 960 PCM frames) per call
    fn pcm_frames_per_encode(&self) -> usize {
        self.block_size
    }

    /// Packets never exceed the MTU the encoder was given (media MTU - 2)
    fn max_output_bytes(&self) -> usize {
        self.enc_mtu
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let expected = self.block_size * self.channels;
        if pcm.samples() != expected {
            return Err(EncoderError::InputLength { expected, got: pcm.samples() });
        }
        let format_ok = matches!((pcm, self.format), (Pcm::I16(_), PcmFormat::I16) | (Pcm::I32(_), PcmFormat::I32));
        if !format_ok {
            return Err(EncoderError::Encode { codec: CODEC, what: format!("input is not {:?} PCM", self.format) });
        }
        if out.len() < self.enc_mtu {
            return Err(EncoderError::OutputTooSmall { have: out.len(), need: self.enc_mtu });
        }
        self.pack(pcm);
        let r = self.enc.encode(&self.packed, out).map_err(|e| EncoderError::Encode {
            codec: CODEC,
            what: format!("{e} (output buffer {} bytes)", out.len()),
        })?;
        Ok(Encoded { bytes: r.bytes, frames: r.frames })
    }

    /// The current target bit rate (changes under ABR)
    fn bitrate_kbps(&self) -> u32 {
        self.enc.last_bitrate()
    }

    fn bitrate_capped(&self) -> bool {
        self.peer_capped
    }

    /// Change encoder rate in place; do not restart the Bluetooth stream.
    fn abr_adjust(&mut self, queue_depth: u32) {
        let Some(mut abr) = self.abr.take() else { return };
        if let Some(index) = abr.target(queue_depth, self.abr_clock.elapsed()) {
            let rate = abr.table[index];
            let previous = self.enc.last_bitrate();
            if self.enc.set_bitrate_kbps(rate).is_ok() {
                    tracing::info!("LHDC Switch: ABR {} -> {} kbps (pressure {})", previous, rate, queue_depth);
                    abr.index = index;
            }
        }
        self.abr = Some(abr);
    }

    fn abr_feedback(&mut self, feedback: crate::CongestionFeedback) {
        // Packet counts change drastically with bitrate and MTU. Use the
        // measured audio backlog and oldest waiting packet instead. A newly
        // observed local send failure acts immediately, even without a full queue.
        // At 160 kbps a normal packet holds 20 ms; two queued packets are
        // 40 ms even when the link keeps up. Exclude one packetization window
        // and allow capture/timer jitter in the head-age threshold. Otherwise
        // a rate reduction itself creates permanent 'congestion' at the floor.
        let backlog = feedback.audio_us.saturating_sub(20_000);
        let level = if feedback.lost_packets > 0 || backlog >= 120_000 || feedback.oldest_us>=150_000 {24}
            else if backlog >= 40_000 || feedback.oldest_us>=60_000 {4}
            else if backlog>20_000 || feedback.oldest_us>30_000 {2} else {0};
        self.abr_adjust(level);
    }

    fn abr_active(&self) -> bool {
        self.abr.is_some()
    }

    /// Resume at a conservative rate and reset all congestion history.
    fn abr_reset(&mut self) {
        let Some(abr) = self.abr.as_mut() else { return };
        let rates = abr.table.clone();
        *abr = Abr::new(&rates, 0, rates.len()-1, *rates.last().unwrap());
        self.abr_clock = Instant::now();
        if let Err(e) = self.enc.set_bitrate_kbps(abr.table[abr.start]) { tracing::warn!("LHDC Switch: ABR reset failed: {e}"); }
    }
}
