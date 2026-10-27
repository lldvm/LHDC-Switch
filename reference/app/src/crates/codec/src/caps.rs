//! Codec information (A2DP "Media Codec Specific Information Elements"):
//! the local stream endpoints, the remote's capabilities, the configuration
//! sent with SET_CONFIGURATION, and the codec choice.
//!
//! Vendor codec information here is what BTstack calls the media codec
//! information: vendor ID (4 bytes LE), codec ID (2 bytes LE), then the
//! codec-specific bytes. The values follow v1 (btstack_transport.cpp), which
//! follows Android and PipeWire.

use crate::CodecKind;
use crate::sbc::{self, ChannelMode};

pub const LDAC_VENDOR_ID: u32 = 0x0000_012D; // Sony
pub const LDAC_CODEC_ID: u16 = 0x00AA;
pub const APTXHD_VENDOR_ID: u32 = 0x0000_00D7; // Qualcomm
pub const APTXHD_CODEC_ID: u16 = 0x0024;
pub const APTXLL_VENDOR_ID: u32 = 0x0000_000A; // CSR
pub const APTXLL_VENDOR_ID2: u32 = 0x0000_00D7; // some sinks list aptX LL under Qualcomm
pub const APTXLL_CODEC_ID: u16 = 0x0002;
pub const APTX_VENDOR_ID: u32 = 0x0000_004F; // APT
pub const APTX_CODEC_ID: u16 = 0x0001;
/// aptX Adaptive: no open-source encoder; detected for messages only
pub const APTXAD_VENDOR_ID: u32 = 0x0000_00D7;
pub const APTXAD_CODEC_ID: u16 = 0x00AD;
pub const LHDCV5_VENDOR_ID: u32 = 0x0000_053A; // Savitech
pub const LHDCV5_CODEC_ID: u16 = 0x4C35;
pub const OPUS_VENDOR_ID: u32 = 0x0000_00E0; // Google
pub const OPUS_CODEC_ID: u16 = 0x0001;
pub const OPUS05_VENDOR_ID: u32 = 0x0000_05F1; // The Linux Foundation (PipeWire)
pub const OPUS05_CODEC_ID: u16 = 0x1005;
pub const FASTSTREAM_VENDOR_ID: u32 = 0x0000_000A; // CSR
pub const FASTSTREAM_CODEC_ID: u16 = 0x0001;

/// AVDTP media codec types
pub const CODEC_TYPE_SBC: u8 = 0x00;
pub const CODEC_TYPE_AAC: u8 = 0x02;
pub const CODEC_TYPE_VENDOR: u8 = 0xFF;

// LDAC byte 6: 0x20 44.1k, 0x10 48k, 0x08 88.2k, 0x04 96k; byte 7: 0x04 mono, 0x02 dual, 0x01 stereo
const LDAC_FREQ_ALL: u8 = 0x3C;
const LDAC_CH_ALL: u8 = 0x07;

// aptX family byte 6: high nibble frequency (0x20 44.1k, 0x10 48k), low nibble channels (0x02 stereo, 0x01 mono)
const APTX_FREQ_44100: u8 = 0x20;
const APTX_FREQ_48000: u8 = 0x10;
const APTX_FREQ_MASK: u8 = 0xF0;
const APTX_CH_STEREO: u8 = 0x02;
const APTX_CAPS_ALL: u8 = APTX_FREQ_44100 | APTX_FREQ_48000 | APTX_CH_STEREO;
const APTX_CONFIG_DEFAULT: u8 = APTX_FREQ_48000 | APTX_CH_STEREO;

/// aptX HD: classic aptX 7 bytes + 4 reserved
const APTXHD_INFO_LEN: usize = 11;
/// aptX LL: classic 7 bytes + flags (bit0 bidirect_link, bit1 has_new_caps);
/// with has_new_caps the sink appends 9 bytes
const APTXLL_INFO_LEN: usize = 8;
const APTXLL_EXT_INFO_LEN: usize = 17;
const APTXLL_HAS_NEW_CAPS: u8 = 0x02;
/// PipeWire's defaults for the extended LL configuration, raised by 3/2
const APTXLL_TARGET_LEVEL: u16 = 180 * 3 / 2;
const APTXLL_INITIAL_LEVEL: u16 = 360 * 3 / 2;
const APTXLL_GOOD_LEVEL: u16 = 180 * 3 / 2;
const APTXLL_SRA_MAX_RATE: u8 = 50;
const APTXLL_SRA_AVG_TIME: u8 = 1;

// LHDC V5 (Android a2dp_vendor_lhdcv5_constants.h): 11 bytes
//   P6 rates 0x20 44.1k 0x10 48k 0x04 96k 0x01 192k
//   P7 bit depth 0x04 16 0x02 24, bits 7..4 bit rate limits
//   P8 version bitmap (0x01 V5 ver.1), 0x10 5 ms frames
const LHDCV5_INFO_LEN: usize = 11;
pub const LHDCV5_SR_44100: u8 = 0x20;
pub const LHDCV5_SR_48000: u8 = 0x10;
pub const LHDCV5_SR_96000: u8 = 0x04;
pub const LHDCV5_SR_192000: u8 = 0x01;
const LHDCV5_SR_MASK: u8 = 0x35;
const LHDCV5_BITS_16: u8 = 0x04;
const LHDCV5_BITS_24: u8 = 0x02;
const LHDCV5_BITS_MASK: u8 = 0x07;
const LHDCV5_BITRATE_LIMITS_MASK: u8 = 0xF0;
const LHDCV5_VERSION_1: u8 = 0x01;
const LHDCV5_VERSION_MASK: u8 = 0x0F;
const LHDCV5_FRAME_5MS: u8 = 0x10;
const LHDCV5_CAPS_P8: u8 = LHDCV5_VERSION_1 | LHDCV5_FRAME_5MS;

// Opus (Android a2dp_vendor_opus_constants.h; PipeWire OPUS_G_*): one byte,
//   bits 2..0 channel mode 0x01 mono 0x02 stereo 0x04 dual mono,
//   bits 4..3 frame duration 0x08 10 ms 0x10 20 ms, bit 7 0x80 48 kHz
const OPUS_INFO_LEN: usize = 7;
pub const OPUS_CH_MONO: u8 = 0x01;
pub const OPUS_CH_STEREO: u8 = 0x02;
pub const OPUS_CH_DUAL_MONO: u8 = 0x04;
pub const OPUS_FRAME_10MS: u8 = 0x08;
pub const OPUS_FRAME_20MS: u8 = 0x10;
const OPUS_FRAME_MASK: u8 = 0x18;
pub const OPUS_FREQ_48000: u8 = 0x80;
/// We offer mono and stereo, 10 and 20 ms
const OPUS_CAPS: u8 = OPUS_FREQ_48000 | OPUS_FRAME_10MS | OPUS_FRAME_20MS | OPUS_CH_MONO | OPUS_CH_STEREO;
/// Android's default configuration: 48 kHz, 20 ms, stereo
const OPUS_CONFIG_DEFAULT: u8 = OPUS_FREQ_48000 | OPUS_FRAME_20MS | OPUS_CH_STEREO;

// Opus 05 (PipeWire a2dp-codec-caps.h, a2dp_opus_05_t): 24 bytes, the
// vendor header then two 9-byte directions, "main" (source to sink: the
// music) and "bidi" (the return channel; 0 channels = none), each
//   +0 channels (capabilities: the most), +1 coupled streams,
//   +2..+5 audio location (LE; Bluetooth audio location bits), +6 frame
//   durations (0x01 2.5 ms, 0x02 5, 0x04 10, 0x08 20, 0x10 40), +7..+8
//   bit rate (LE, units of 1024 bps; 0 = no limit)
// A2DPWB offers and configures the main direction only: stereo (2
// channels, 1 coupled stream, front left + right) or mono, 10 or 20 ms.
const OPUS05_INFO_LEN: usize = 24;
const OPUS05_MAIN: usize = 6;
const OPUS05_BIDI: usize = 15;
pub const OPUS05_FRAME_10MS: u8 = 0x04;
pub const OPUS05_FRAME_20MS: u8 = 0x08;
const OPUS05_LOCATION_STEREO: u32 = 0x0000_0003; // front left, front right
/// PipeWire's largest Opus 05 stereo bit rate (BITRATE_MAX), the
/// configuration's limit when the sink sets none: 320000 / 1024
pub const OPUS05_MAX_BITRATE_UNITS: u16 = 312;
/// The bit rate field's unit
pub const OPUS05_BITRATE_UNIT: u32 = 1024;

/// One Opus 05 direction (9 bytes)
fn opus05_direction(channels: u8, coupled: u8, location: u32, frames: u8, bitrate: u16) -> [u8; 9] {
    let l = location.to_le_bytes();
    let b = bitrate.to_le_bytes();
    [channels, coupled, l[0], l[1], l[2], l[3], frames, b[0], b[1]]
}

// SBC codec information (A2DP 4.3.2), as BTstack's SBC capability event
// splits it: sampling frequency 0x08 16k 0x04 32k 0x02 44.1k 0x01 48k;
// channel mode 0x08 mono 0x04 dual 0x02 stereo 0x01 joint stereo; block
// length 0x08 4 .. 0x01 16; subbands 0x02 4 0x01 8; allocation 0x02 SNR
// 0x01 loudness
const SBC_FREQ_44100: u8 = 0x02;
const SBC_FREQ_48000: u8 = 0x01;
const SBC_CH_DUAL: u8 = 0x04;
const SBC_BLOCKS_16: u8 = 0x01;
const SBC_SUBBANDS_8: u8 = 0x01;
const SBC_ALLOC_LOUDNESS: u8 = 0x01;
/// SBC XQ needs the sink to take at least this bitpool per channel (the
/// lowest High / the Standard 44.1 kHz bitpool, ~507 kbps); below it the
/// sink gets plain SBC
pub const SBC_XQ_MIN_MAX_BITPOOL: u8 = 43;
/// The local SBC SEP's largest bitpool
const SBC_MAX_BITPOOL: u8 = 53;

/// A remote SEP's SBC capabilities (the bitmaps of BTstack's
/// A2DP_SUBEVENT_SIGNALING_MEDIA_CODEC_SBC_CAPABILITY)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SbcCapabilities {
    pub sampling_frequency: u8,
    pub channel_mode: u8,
    pub block_length: u8,
    pub subbands: u8,
    pub allocation_method: u8,
    pub min_bitpool: u8,
    pub max_bitpool: u8,
}

impl SbcCapabilities {
    /// Dual channel with 16 blocks, 8 subbands, loudness, 44.1 or 48 kHz and
    /// a bitpool up to at least [`SBC_XQ_MIN_MAX_BITPOOL`]
    pub fn allows_xq(&self) -> bool {
        self.channel_mode & SBC_CH_DUAL != 0
            && self.block_length & SBC_BLOCKS_16 != 0
            && self.subbands & SBC_SUBBANDS_8 != 0
            && self.allocation_method & SBC_ALLOC_LOUDNESS != 0
            && self.sampling_frequency & (SBC_FREQ_44100 | SBC_FREQ_48000) != 0
            && self.max_bitpool >= SBC_XQ_MIN_MAX_BITPOOL
            && self.min_bitpool <= SBC_XQ_MIN_MAX_BITPOOL
    }
}

/// The local SEP a codec streams from: SBC XQ shares SBC's (it offers all
/// channel modes and a bitpool up to 53)
pub fn endpoint_kind(kind: CodecKind) -> CodecKind {
    match kind {
        CodecKind::SbcXq => CodecKind::Sbc,
        k => k,
    }
}

// FastStream (PipeWire a2dp-codec-caps.h, a2dp_faststream_t): 8 bytes, byte
// 6 direction (0x01 sink = music to the headset, 0x02 source = voice back),
// byte 7 low nibble the sink rates (0x02 44.1k, 0x01 48k), high nibble the
// source rate (0x2 16k). A2DPWB offers and configures the sink direction
// only.
const FASTSTREAM_INFO_LEN: usize = 8;
const FASTSTREAM_DIRECTION_SINK: u8 = 0x01;
const FASTSTREAM_SINK_44100: u8 = 0x02;
const FASTSTREAM_SINK_48000: u8 = 0x01;
const FASTSTREAM_SINK_MASK: u8 = 0x03;

fn vendor_header(vendor_id: u32, codec_id: u16) -> Vec<u8> {
    let mut v = vendor_id.to_le_bytes().to_vec();
    v.extend_from_slice(&codec_id.to_le_bytes());
    v
}

fn read_vendor(info: &[u8]) -> Option<(u32, u16)> {
    if info.len() < 6 {
        return None;
    }
    Some((u32::from_le_bytes([info[0], info[1], info[2], info[3]]), u16::from_le_bytes([info[4], info[5]])))
}

/// A local stream endpoint: what we register with BTstack
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEndpoint {
    pub kind: CodecKind,
    /// AVDTP media codec type ([`CODEC_TYPE_SBC`], ...)
    pub codec_type: u8,
    /// Capabilities we offer
    pub caps: Vec<u8>,
    /// Default configuration (BTstack's initial value; the real one goes
    /// with SET_CONFIGURATION)
    pub config: Vec<u8>,
}

/// The endpoints of the built-in codecs, in v1's order (endpoint slots)
pub fn local_endpoints() -> Vec<LocalEndpoint> {
    let mut eps = Vec::new();
    let vendor = |kind: CodecKind, vid: u32, cid: u16, len: usize, caps_tail: &[u8], config_tail: &[u8]| {
        let mut caps = vendor_header(vid, cid);
        caps.extend_from_slice(caps_tail);
        caps.resize(len, 0);
        let mut config = vendor_header(vid, cid);
        config.extend_from_slice(config_tail);
        config.resize(len, 0);
        LocalEndpoint { kind, codec_type: CODEC_TYPE_VENDOR, caps, config }
    };
    if CodecKind::Ldac.available() {
        eps.push(vendor(
            CodecKind::Ldac,
            LDAC_VENDOR_ID,
            LDAC_CODEC_ID,
            8,
            &[LDAC_FREQ_ALL, LDAC_CH_ALL],
            &[0x10, 0x01],
        ));
    }
    if CodecKind::AptxHd.available() {
        eps.push(vendor(
            CodecKind::AptxHd,
            APTXHD_VENDOR_ID,
            APTXHD_CODEC_ID,
            APTXHD_INFO_LEN,
            &[APTX_CAPS_ALL],
            &[APTX_CONFIG_DEFAULT],
        ));
        eps.push(vendor(CodecKind::Aptx, APTX_VENDOR_ID, APTX_CODEC_ID, 7, &[APTX_CAPS_ALL], &[APTX_CONFIG_DEFAULT]));
        // One local SEP for sinks listing aptX LL under either vendor ID; the
        // configuration carries the remote's
        eps.push(vendor(
            CodecKind::AptxLl,
            APTXLL_VENDOR_ID,
            APTXLL_CODEC_ID,
            APTXLL_INFO_LEN,
            &[APTX_CAPS_ALL],
            &[APTX_CONFIG_DEFAULT],
        ));
    }
    // SBC: 44.1+48k, all channel modes, 16 blocks, 8 subbands, loudness,
    // bitpool 2..53 (SBC XQ streams from this SEP too, in dual channel)
    eps.push(LocalEndpoint {
        kind: CodecKind::Sbc,
        codec_type: CODEC_TYPE_SBC,
        caps: vec![0x3F, 0x15, 2, 53],
        config: vec![0x11, 0x15, 2, 53],
    });
    if CodecKind::Aac.available() {
        // MPEG-2 AAC-LC, 44.1+48k, 2 channels, 256 kbps
        eps.push(LocalEndpoint {
            kind: CodecKind::Aac,
            codec_type: CODEC_TYPE_AAC,
            caps: vec![0x80, 0x0C, 0x04, 0x03, 0xE8, 0x00],
            config: vec![0x80, 0x04, 0x04, 0x03, 0xE8, 0x00],
        });
    }
    if CodecKind::LhdcV5.available() {
        eps.push(vendor(
            CodecKind::LhdcV5,
            LHDCV5_VENDOR_ID,
            LHDCV5_CODEC_ID,
            LHDCV5_INFO_LEN,
            &[LHDCV5_SR_MASK, LHDCV5_BITS_16 | LHDCV5_BITS_24, LHDCV5_CAPS_P8],
            &[LHDCV5_SR_48000, LHDCV5_BITS_24, LHDCV5_CAPS_P8],
        ));
    }
    // FastStream: music to the sink at 44.1 / 48 kHz, no voice backchannel
    eps.push(vendor(
        CodecKind::FastStream,
        FASTSTREAM_VENDOR_ID,
        FASTSTREAM_CODEC_ID,
        FASTSTREAM_INFO_LEN,
        &[FASTSTREAM_DIRECTION_SINK, FASTSTREAM_SINK_44100 | FASTSTREAM_SINK_48000],
        &[FASTSTREAM_DIRECTION_SINK, FASTSTREAM_SINK_48000],
    ));
    if CodecKind::Opus.available() {
        eps.push(vendor(
            CodecKind::Opus,
            OPUS_VENDOR_ID,
            OPUS_CODEC_ID,
            OPUS_INFO_LEN,
            &[OPUS_CAPS],
            &[OPUS_CONFIG_DEFAULT],
        ));
    }
    if CodecKind::Opus05.available() {
        // Up to stereo (front left + right), 10 / 20 ms, no bit rate limit,
        // no return channel; the default configuration is PipeWire's stereo
        let frames = OPUS05_FRAME_10MS | OPUS05_FRAME_20MS;
        eps.push(vendor(
            CodecKind::Opus05,
            OPUS05_VENDOR_ID,
            OPUS05_CODEC_ID,
            OPUS05_INFO_LEN,
            &opus05_direction(2, 0, OPUS05_LOCATION_STEREO, frames, 0),
            &opus05_direction(2, 1, OPUS05_LOCATION_STEREO, OPUS05_FRAME_10MS, OPUS05_MAX_BITRATE_UNITS),
        ));
    }
    eps
}

/// A codec the remote offers on one of its SEPs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RemoteSep {
    pub seid: u8,
}

/// What the remote reported during capability discovery
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteCaps {
    pub sbc: Option<RemoteSep>,
    /// The SBC SEP's capabilities, when reported (SBC XQ needs them)
    pub sbc_caps: Option<SbcCapabilities>,
    pub aac: Option<RemoteSep>,
    pub ldac: Option<RemoteSep>,
    pub aptx: Option<RemoteSep>,
    pub aptx_hd: Option<RemoteSep>,
    pub aptx_ll: Option<RemoteSep>,
    pub lhdcv5: Option<RemoteSep>,
    pub opus: Option<RemoteSep>,
    pub opus05: Option<RemoteSep>,
    pub faststream: Option<RemoteSep>,
    /// FastStream sink (music) rates: 0x02 44.1k, 0x01 48k
    pub faststream_rates: u8,
    /// aptX family byte 6 of each (0 = not reported: assume 44.1k + 48k)
    pub aptx_caps: u8,
    pub aptxhd_caps: u8,
    pub aptxll_caps: u8,
    /// aptX LL: the vendor ID the remote used and its codec information
    pub aptxll_vendor_id: u32,
    pub aptxll_info: Vec<u8>,
    /// aptX Adaptive offered (never selected: no open encoder)
    pub aptx_adaptive: bool,
    /// LHDC V5 codec information (11 bytes) when usable
    pub lhdcv5_info: [u8; LHDCV5_INFO_LEN],
    /// Opus codec information byte 6 (channel modes, frame durations, rate)
    pub opus_caps: u8,
    /// Opus 05: the main direction's channels, frame durations and bit rate
    /// limit (units of 1024 bps, 0 = none), and whether the SEP taken has a
    /// return channel (a duplex SEP, used only when there is no other)
    pub opus05_channels: u8,
    pub opus05_frames: u8,
    pub opus05_bitrate: u16,
    pub opus05_bidi: bool,
    /// SEP discovery: audio sink SEPs, and how many of them were in use (a
    /// codec streaming to another device is missing from the above)
    pub audio_sinks: u8,
    pub audio_sinks_in_use: u8,
}

/// How a vendor codec capability was taken, for the log
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendorCapability {
    Taken(CodecKind),
    /// Known codec, but not usable (e.g. mono-only aptX, an LHDC version we
    /// do not encode)
    Ignored(CodecKind, &'static str),
    AptxAdaptive,
    Unknown {
        vendor_id: u32,
        codec_id: u16,
    },
    TooShort,
}

impl RemoteCaps {
    pub fn has(&self, kind: CodecKind) -> bool {
        self.sep(kind).is_some()
    }

    pub fn sep(&self, kind: CodecKind) -> Option<RemoteSep> {
        match kind {
            CodecKind::Sbc => self.sbc,
            CodecKind::SbcXq => self.sbc.filter(|_| self.sbc_caps.is_some_and(|c| c.allows_xq())),
            CodecKind::Aac => self.aac,
            CodecKind::Ldac => self.ldac,
            CodecKind::Aptx => self.aptx,
            CodecKind::AptxHd => self.aptx_hd,
            CodecKind::AptxLl => self.aptx_ll,
            CodecKind::LhdcV5 => self.lhdcv5,
            CodecKind::Opus => self.opus,
            CodecKind::Opus05 => self.opus05,
            CodecKind::FastStream => self.faststream,
        }
    }

    /// An SBC SEP whose capabilities are not known (no SBC XQ)
    pub fn add_sbc(&mut self, seid: u8) {
        self.sbc = Some(RemoteSep { seid });
        self.sbc_caps = None;
    }

    /// An SBC SEP with its capabilities. With several, the first that
    /// allows SBC XQ wins, else the last.
    pub fn add_sbc_caps(&mut self, seid: u8, caps: SbcCapabilities) {
        if self.sbc_caps.is_some_and(|c| c.allows_xq()) && !caps.allows_xq() {
            return;
        }
        self.sbc = Some(RemoteSep { seid });
        self.sbc_caps = Some(caps);
    }

    /// The sink's largest SBC bitpool (0 = not known)
    pub fn sbc_max_bitpool(&self) -> u8 {
        self.sbc_caps.map_or(0, |c| c.max_bitpool)
    }

    /// The sink's smallest SBC bitpool (0 = not known)
    pub fn sbc_min_bitpool(&self) -> u8 {
        self.sbc_caps.map_or(0, |c| c.min_bitpool)
    }

    /// The bitpool range of an SBC / SBC XQ SET_CONFIGURATION: up to `cap`
    /// within the sink's range (as PipeWire's select_config); 2..cap when
    /// the sink's capabilities are not known
    pub fn sbc_bitpool_range(&self, cap: u8) -> (u8, u8) {
        let (min, max) = self.sbc_caps.map_or((2, cap), |c| (c.min_bitpool, c.max_bitpool));
        let max = cap.min(max).max(2);
        (min.max(2).min(max), max)
    }

    pub fn add_aac(&mut self, seid: u8) {
        self.aac = Some(RemoteSep { seid });
    }

    /// A vendor codec capability (media codec information) of SEP `seid`
    pub fn add_vendor(&mut self, seid: u8, info: &[u8]) -> VendorCapability {
        let Some((vid, cid)) = read_vendor(info) else {
            return VendorCapability::TooShort;
        };
        let byte = |i: usize| info.get(i).copied().unwrap_or(0);
        if vid == LDAC_VENDOR_ID && cid == LDAC_CODEC_ID {
            self.ldac = Some(RemoteSep { seid });
            return VendorCapability::Taken(CodecKind::Ldac);
        }
        if vid == APTXHD_VENDOR_ID && cid == APTXHD_CODEC_ID {
            // Only stereo is sent: skip mono-only sinks
            let caps = byte(6);
            if info.len() >= 7 && caps & APTX_CH_STEREO == 0 {
                return VendorCapability::Ignored(CodecKind::AptxHd, "no stereo");
            }
            self.aptx_hd = Some(RemoteSep { seid });
            self.aptxhd_caps = caps;
            return VendorCapability::Taken(CodecKind::AptxHd);
        }
        if (vid == APTXLL_VENDOR_ID || vid == APTXLL_VENDOR_ID2) && cid == APTXLL_CODEC_ID {
            let caps = byte(6);
            if info.len() >= 7 && caps & APTX_CH_STEREO == 0 {
                return VendorCapability::Ignored(CodecKind::AptxLl, "no stereo");
            }
            // Keep the first usable LL SEP if listed under both IDs
            if self.aptx_ll.is_none() {
                self.aptx_ll = Some(RemoteSep { seid });
                self.aptxll_caps = caps;
                self.aptxll_vendor_id = vid;
                self.aptxll_info = info[..info.len().min(APTXLL_EXT_INFO_LEN)].to_vec();
            }
            return VendorCapability::Taken(CodecKind::AptxLl);
        }
        if vid == APTXAD_VENDOR_ID && cid == APTXAD_CODEC_ID {
            self.aptx_adaptive = true;
            return VendorCapability::AptxAdaptive;
        }
        if vid == LHDCV5_VENDOR_ID && cid == LHDCV5_CODEC_ID {
            let usable = info.len() >= LHDCV5_INFO_LEN
                && info[6] & LHDCV5_SR_MASK != 0
                && info[7] & (LHDCV5_BITS_16 | LHDCV5_BITS_24) != 0
                && info[8] & LHDCV5_VERSION_MASK & LHDCV5_VERSION_1 != 0
                && info[8] & LHDCV5_FRAME_5MS != 0;
            if !usable || !CodecKind::LhdcV5.available() {
                return VendorCapability::Ignored(CodecKind::LhdcV5, "version / frame length / rates");
            }
            if self.lhdcv5.is_none() {
                self.lhdcv5 = Some(RemoteSep { seid });
                self.lhdcv5_info.copy_from_slice(&info[..LHDCV5_INFO_LEN]);
            }
            return VendorCapability::Taken(CodecKind::LhdcV5);
        }
        if vid == OPUS_VENDOR_ID && cid == OPUS_CODEC_ID {
            // 48 kHz, a frame duration, and mono or stereo (dual mono, two
            // independent channels, is not sent)
            let caps = byte(6);
            let usable = info.len() >= OPUS_INFO_LEN
                && caps & OPUS_FREQ_48000 != 0
                && caps & OPUS_FRAME_MASK != 0
                && caps & (OPUS_CH_MONO | OPUS_CH_STEREO) != 0;
            if !usable {
                return VendorCapability::Ignored(CodecKind::Opus, "no 48 kHz / frame duration / mono or stereo");
            }
            if self.opus.is_none() {
                self.opus = Some(RemoteSep { seid });
                self.opus_caps = caps;
            }
            return VendorCapability::Taken(CodecKind::Opus);
        }
        if vid == OPUS05_VENDOR_ID && cid == OPUS05_CODEC_ID {
            // The music direction with a channel and 10 or 20 ms frames
            let (channels, frames) = (byte(OPUS05_MAIN), byte(OPUS05_MAIN + 6));
            if info.len() < OPUS05_INFO_LEN || channels == 0 || frames & (OPUS05_FRAME_10MS | OPUS05_FRAME_20MS) == 0 {
                return VendorCapability::Ignored(CodecKind::Opus05, "no music direction with 10 / 20 ms frames");
            }
            // PipeWire offers stereo on its opus_05 SEP and a return channel
            // on its opus_05_duplex SEP: prefer the one without
            let bidi = byte(OPUS05_BIDI) != 0;
            if self.opus05.is_none() || (self.opus05_bidi && !bidi) {
                self.opus05 = Some(RemoteSep { seid });
                self.opus05_channels = channels;
                self.opus05_frames = frames;
                self.opus05_bitrate = u16::from_le_bytes([byte(OPUS05_MAIN + 7), byte(OPUS05_MAIN + 8)]);
                self.opus05_bidi = bidi;
            }
            return VendorCapability::Taken(CodecKind::Opus05);
        }
        if vid == FASTSTREAM_VENDOR_ID && cid == FASTSTREAM_CODEC_ID {
            let rates = byte(7) & FASTSTREAM_SINK_MASK;
            if info.len() < FASTSTREAM_INFO_LEN || byte(6) & FASTSTREAM_DIRECTION_SINK == 0 || rates == 0 {
                return VendorCapability::Ignored(CodecKind::FastStream, "no music direction at 44.1 / 48 kHz");
            }
            if self.faststream.is_none() {
                self.faststream = Some(RemoteSep { seid });
                self.faststream_rates = rates;
            }
            return VendorCapability::Taken(CodecKind::FastStream);
        }
        if vid == APTX_VENDOR_ID && cid == APTX_CODEC_ID {
            let caps = byte(6);
            if info.len() >= 7 && caps & APTX_CH_STEREO == 0 {
                return VendorCapability::Ignored(CodecKind::Aptx, "no stereo");
            }
            self.aptx = Some(RemoteSep { seid });
            self.aptx_caps = caps;
            return VendorCapability::Taken(CodecKind::Aptx);
        }
        VendorCapability::Unknown { vendor_id: vid, codec_id: cid }
    }

    pub fn lhdcv5_rates(&self) -> u8 {
        self.lhdcv5_info[6] & LHDCV5_SR_MASK
    }

    pub fn lhdcv5_24bit(&self) -> bool {
        self.lhdcv5_info[7] & LHDCV5_BITS_24 != 0
    }

    pub fn lhdcv5_bitrate_limits(&self) -> u8 {
        self.lhdcv5_info[7] & LHDCV5_BITRATE_LIMITS_MASK
    }

    /// Opus: the frame duration (ms) and the channels on the wire for
    /// `channels` of input. 20 ms when offered (the only duration Android
    /// has a bit rate for; PipeWire prefers it too), else 10 ms. Mono only
    /// for mono input to a sink that has it.
    pub fn opus_stream(&self, channels: u32) -> (u8, u8) {
        let frame_ms =
            if self.opus_caps & OPUS_FRAME_10MS != 0 && self.opus_caps & OPUS_FRAME_20MS == 0 { 10 } else { 20 };
        let wire = if channels == 1 && self.opus_caps & OPUS_CH_MONO != 0 { 1 } else { 2 };
        (frame_ms, wire)
    }

    /// Opus 05: the frame duration (ms) and the channels on the wire. 10 ms
    /// when offered (PipeWire's choice), else 20 ms; stereo unless the sink
    /// takes one channel only (mono input is duplicated, as PipeWire's own
    /// opus_05 sends stereo)
    pub fn opus05_stream(&self) -> (u8, u8) {
        let frame_ms = if self.opus05_frames & OPUS05_FRAME_10MS != 0 { 10 } else { 20 };
        (frame_ms, if self.opus05_channels == 1 { 1 } else { 2 })
    }

    /// Opus 05: the bit rate limit of the configuration, units of 1024 bps:
    /// the sink's limit, at most PipeWire's 312 (320 kbps)
    pub fn opus05_bitrate_units(&self) -> u16 {
        match self.opus05_bitrate {
            0 => OPUS05_MAX_BITRATE_UNITS,
            b => b.min(OPUS05_MAX_BITRATE_UNITS),
        }
    }

    /// Opus / Opus 05 encoder parameters for `kind` and `channels` of input:
    /// (frame duration in ms, channels on the wire, bit rate limit in bps
    /// or 0); zeros for other codecs
    pub fn opus_params(&self, kind: CodecKind, channels: u32) -> (u8, u8, u32) {
        match kind {
            CodecKind::Opus => {
                let (frame_ms, wire) = self.opus_stream(channels);
                (frame_ms, wire, 0)
            }
            CodecKind::Opus05 => {
                let (frame_ms, wire) = self.opus05_stream();
                (frame_ms, wire, u32::from(self.opus05_bitrate_units()) * OPUS05_BITRATE_UNIT)
            }
            _ => (0, 0, 0),
        }
    }

    /// The best codec both sides have (Auto)
    pub fn best(&self) -> Option<CodecKind> {
        CodecKind::AUTO_PRIORITY.into_iter().find(|&k| k.available() && self.has(k))
    }

    /// Capture rate for the aptX family: 44.1 or 48 kHz as the remote lists
    /// them, preferring `wanted` (other rates: 48k, then 44.1k)
    pub fn pick_aptx_sample_rate(&self, kind: CodecKind, wanted: u32) -> u32 {
        let caps = match kind {
            CodecKind::Aptx => self.aptx_caps,
            CodecKind::AptxHd => self.aptxhd_caps,
            CodecKind::AptxLl => self.aptxll_caps,
            _ => return wanted,
        };
        let freqs = caps & APTX_FREQ_MASK;
        let has441 = freqs == 0 || freqs & APTX_FREQ_44100 != 0;
        let has48 = freqs == 0 || freqs & APTX_FREQ_48000 != 0;
        if wanted == 48000 && has48 {
            48000
        } else if wanted == 44100 && has441 {
            44100
        } else if has48 {
            48000
        } else if has441 {
            44100
        } else {
            wanted
        }
    }

    /// Capture rate for FastStream: 44.1 or 48 kHz as the sink lists them,
    /// preferring `wanted` (other rates: 48k, then 44.1k)
    pub fn pick_faststream_sample_rate(&self, wanted: u32) -> u32 {
        let has441 = self.faststream_rates & FASTSTREAM_SINK_44100 != 0;
        let has48 = self.faststream_rates & FASTSTREAM_SINK_48000 != 0;
        if (wanted == 44100 && has441) || !has48 { 44100 } else { 48000 }
    }

    /// Capture rate for LHDC V5: 44.1 / 48 / 96 / 192 kHz as the remote
    /// lists them, preferring `wanted` (88.2k maps to 96k)
    pub fn pick_lhdcv5_sample_rate(&self, wanted: u32) -> u32 {
        let rates = self.lhdcv5_rates();
        let has = |bit: u8| rates & bit != 0;
        let wanted = if wanted == 88200 { 96000 } else { wanted };
        match wanted {
            44100 if has(LHDCV5_SR_44100) => return 44100,
            48000 if has(LHDCV5_SR_48000) => return 48000,
            96000 if has(LHDCV5_SR_96000) => return 96000,
            192000 if has(LHDCV5_SR_192000) => return 192000,
            _ => {}
        }
        for (bit, rate) in
            [(LHDCV5_SR_48000, 48000), (LHDCV5_SR_44100, 44100), (LHDCV5_SR_96000, 96000), (LHDCV5_SR_192000, 192000)]
        {
            if has(bit) {
                return rate;
            }
        }
        wanted
    }

    /// The SET_CONFIGURATION for a stream of `kind`
    pub fn configuration(
        &self,
        kind: CodecKind,
        sample_rate: u32,
        channels: u32,
        bits_per_sample: u8,
    ) -> Result<Configuration, ConfigError> {
        let Some(sep) = self.sep(kind) else {
            return Err(ConfigError::NotOffered(kind));
        };
        let remote_seid = sep.seid;
        let mono = channels == 1;
        let request = match kind {
            CodecKind::Sbc => {
                if sample_rate != 44100 && sample_rate != 48000 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                // Up to 53 (the local SEP's maximum) within the sink's range;
                // the encoder's bitpool is capped the same way
                let (min_bitpool, max_bitpool) = self.sbc_bitpool_range(SBC_MAX_BITPOOL);
                ConfigRequest::Sbc {
                    sampling_frequency: sample_rate as u16,
                    channel_mode: if mono { ChannelMode::Mono } else { ChannelMode::JointStereo },
                    min_bitpool,
                    max_bitpool,
                }
            }
            CodecKind::SbcXq => {
                if sample_rate != 44100 && sample_rate != 48000 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                // Dual channel (mono for one channel). The bitpool range as
                // PipeWire's select_config: up to the largest XQ bitpool of
                // the rate, within the sink's range; the encoder's bitpool
                // (by quality) is never above it.
                let (min_bitpool, max_bitpool) = self.sbc_bitpool_range(sbc::xq_max_bitpool(sample_rate));
                ConfigRequest::Sbc {
                    sampling_frequency: sample_rate as u16,
                    channel_mode: if mono { ChannelMode::Mono } else { ChannelMode::DualChannel },
                    min_bitpool,
                    max_bitpool,
                }
            }
            CodecKind::Aac => {
                ConfigRequest::Aac { sampling_frequency: sample_rate, channels: channels as u8, bit_rate: 256_000 }
            }
            CodecKind::Ldac => {
                let freq = match sample_rate {
                    44100 => 0x20,
                    48000 => 0x10,
                    88200 => 0x08,
                    _ => 0x04,
                };
                let mut info = vendor_header(LDAC_VENDOR_ID, LDAC_CODEC_ID);
                info.extend_from_slice(&[freq, if mono { 0x04 } else { 0x01 }]);
                ConfigRequest::Vendor(info)
            }
            CodecKind::AptxHd => {
                let freq = aptx_freq_bits(kind, self.aptxhd_caps, sample_rate)?;
                let mut info = vendor_header(APTXHD_VENDOR_ID, APTXHD_CODEC_ID);
                // Always stereo on the wire; the encoder duplicates mono input
                info.push(freq | APTX_CH_STEREO);
                info.resize(APTXHD_INFO_LEN, 0);
                ConfigRequest::Vendor(info)
            }
            CodecKind::Aptx => {
                let freq = aptx_freq_bits(kind, self.aptx_caps, sample_rate)?;
                let mut info = vendor_header(APTX_VENDOR_ID, APTX_CODEC_ID);
                info.push(freq | APTX_CH_STEREO);
                ConfigRequest::Vendor(info)
            }
            CodecKind::AptxLl => {
                let freq = aptx_freq_bits(kind, self.aptxll_caps, sample_rate)?;
                let mut info = vendor_header(self.aptxll_vendor_id, APTXLL_CODEC_ID);
                info.push(freq | APTX_CH_STEREO);
                info.push(0); // no backchannel
                let rc = &self.aptxll_info;
                if rc.len() >= APTXLL_EXT_INFO_LEN && rc[7] & APTXLL_HAS_NEW_CAPS != 0 {
                    // The sink sent the extended caps: echo them back like
                    // PipeWire, with the buffer levels at least its defaults
                    let le16 = |i: usize| u16::from_le_bytes([rc[i], rc[i + 1]]);
                    let target = le16(9).max(APTXLL_TARGET_LEVEL);
                    let initial = le16(11).max(APTXLL_INITIAL_LEVEL);
                    let good = le16(15).max(APTXLL_GOOD_LEVEL);
                    info[7] = APTXLL_HAS_NEW_CAPS;
                    info.push(rc[8]);
                    info.extend_from_slice(&target.to_le_bytes());
                    info.extend_from_slice(&initial.to_le_bytes());
                    info.push(if rc[13] != 0 { rc[13] } else { APTXLL_SRA_MAX_RATE });
                    info.push(if rc[14] != 0 { rc[14] } else { APTXLL_SRA_AVG_TIME });
                    info.extend_from_slice(&good.to_le_bytes());
                }
                ConfigRequest::Vendor(info)
            }
            CodecKind::FastStream => {
                // The music direction at one rate; stereo on the wire (the
                // encoder duplicates mono input)
                let freq = match sample_rate {
                    44100 => FASTSTREAM_SINK_44100,
                    48000 => FASTSTREAM_SINK_48000,
                    _ => 0,
                };
                if freq & self.faststream_rates == 0 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                let mut info = vendor_header(FASTSTREAM_VENDOR_ID, FASTSTREAM_CODEC_ID);
                info.extend_from_slice(&[FASTSTREAM_DIRECTION_SINK, freq]);
                ConfigRequest::Vendor(info)
            }
            CodecKind::LhdcV5 => {
                // One rate, one bit depth, the sink's bit rate limits echoed,
                // V5 ver.1, 5 ms frames, no optional features; stereo only
                let freq = match sample_rate {
                    44100 => LHDCV5_SR_44100,
                    48000 => LHDCV5_SR_48000,
                    96000 => LHDCV5_SR_96000,
                    192000 => LHDCV5_SR_192000,
                    _ => 0,
                };
                if freq & self.lhdcv5_rates() == 0 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                let bits = if bits_per_sample == 24 { LHDCV5_BITS_24 } else { LHDCV5_BITS_16 };
                if bits & self.lhdcv5_info[7] & LHDCV5_BITS_MASK == 0 {
                    return Err(ConfigError::BitDepth(kind, bits_per_sample));
                }
                let mut info = vendor_header(LHDCV5_VENDOR_ID, LHDCV5_CODEC_ID);
                info.extend_from_slice(&[freq, bits | self.lhdcv5_bitrate_limits(), LHDCV5_CAPS_P8]);
                info.resize(LHDCV5_INFO_LEN, 0);
                ConfigRequest::Vendor(info)
            }
            CodecKind::Opus => {
                if sample_rate != 48000 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                let (frame_ms, wire) = self.opus_stream(channels);
                let ch = if wire == 1 { OPUS_CH_MONO } else { OPUS_CH_STEREO };
                if self.opus_caps & ch == 0 {
                    return Err(ConfigError::Channels(kind, wire));
                }
                let frame = if frame_ms == 10 { OPUS_FRAME_10MS } else { OPUS_FRAME_20MS };
                let mut info = vendor_header(OPUS_VENDOR_ID, OPUS_CODEC_ID);
                info.push(OPUS_FREQ_48000 | frame | ch);
                ConfigRequest::Vendor(info)
            }
            CodecKind::Opus05 => {
                // The rate is not in the configuration: 48 kHz is the one
                // PipeWire lists first and the only one A2DPWB encodes
                if sample_rate != 48000 {
                    return Err(ConfigError::SampleRate(kind, sample_rate));
                }
                // As PipeWire's select_config / set_channel_conf for
                // opus_05: one frame duration, stereo as one coupled stream
                // at front left + right (mono: one stream, no location), the
                // bit rate limit, no return channel
                let (frame_ms, wire) = self.opus05_stream();
                let frame = if frame_ms == 10 { OPUS05_FRAME_10MS } else { OPUS05_FRAME_20MS };
                let (coupled, location) = if wire == 2 { (1, OPUS05_LOCATION_STEREO) } else { (0, 0) };
                let mut info = vendor_header(OPUS05_VENDOR_ID, OPUS05_CODEC_ID);
                info.extend_from_slice(&opus05_direction(wire, coupled, location, frame, self.opus05_bitrate_units()));
                info.resize(OPUS05_INFO_LEN, 0);
                ConfigRequest::Vendor(info)
            }
        };
        Ok(Configuration { kind, remote_seid, request })
    }
}

fn aptx_freq_bits(kind: CodecKind, remote_caps: u8, sample_rate: u32) -> Result<u8, ConfigError> {
    let freq = match sample_rate {
        44100 => APTX_FREQ_44100,
        48000 => APTX_FREQ_48000,
        _ => return Err(ConfigError::SampleRate(kind, sample_rate)),
    };
    let remote = remote_caps & APTX_FREQ_MASK;
    if remote != 0 && remote & freq == 0 {
        return Err(ConfigError::SampleRate(kind, sample_rate));
    }
    Ok(freq)
}

/// SET_CONFIGURATION of a stream
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    pub kind: CodecKind,
    pub remote_seid: u8,
    pub request: ConfigRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigRequest {
    /// 16 blocks, 8 subbands, loudness; SBC joint stereo, SBC XQ dual
    /// channel (both mono for one channel)
    Sbc { sampling_frequency: u16, channel_mode: ChannelMode, min_bitpool: u8, max_bitpool: u8 },
    /// MPEG-2 AAC-LC, constant bit rate
    Aac { sampling_frequency: u32, channels: u8, bit_rate: u32 },
    /// Vendor codec information
    Vendor(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("the device does not offer {0}")]
    NotOffered(CodecKind),
    #[error("{0} does not support {1} Hz here")]
    SampleRate(CodecKind, u32),
    #[error("{0} does not support {1}-bit here")]
    BitDepth(CodecKind, u8),
    #[error("{0} does not support {1} channel(s) here")]
    Channels(CodecKind, u8),
}

/// Why the codec of a profile is not used
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// The codec may be on a SEP another device streams to (multipoint):
    /// BTstack does not query SEPs in use
    SepInUse {
        in_use: u8,
        total: u8,
    },
    /// The sink offers only aptX Adaptive, which has no open encoder
    AdaptiveOnly,
    NotSupported,
}

/// The codec to stream with
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub kind: CodecKind,
    /// The requested codec was replaced by `kind` (codec fallback)
    pub replaced: Option<(CodecKind, Unavailable)>,
}

/// Picks the codec for `requested` (None = Auto). Without `fallback`, a
/// requested codec the device does not offer is an error.
pub fn choose(
    caps: &RemoteCaps,
    requested: Option<CodecKind>,
    fallback: bool,
) -> Result<Choice, Option<(CodecKind, Unavailable)>> {
    let Some(wanted) = requested else {
        return caps.best().map(|kind| Choice { kind, replaced: None }).ok_or(None);
    };
    if wanted.available() && caps.has(wanted) {
        return Ok(Choice { kind: wanted, replaced: None });
    }
    let why = if caps.audio_sinks_in_use > 0 {
        Unavailable::SepInUse { in_use: caps.audio_sinks_in_use, total: caps.audio_sinks }
    } else if matches!(wanted, CodecKind::Aptx | CodecKind::AptxHd | CodecKind::AptxLl) && caps.aptx_adaptive {
        Unavailable::AdaptiveOnly
    } else {
        Unavailable::NotSupported
    };
    if fallback && let Some(kind) = caps.best() {
        return Ok(Choice { kind, replaced: Some((wanted, why)) });
    }
    Err(Some((wanted, why)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_prefers_ldac_then_aptx_family() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac(2);
        assert_eq!(caps.best(), if CodecKind::Aac.available() { Some(CodecKind::Aac) } else { Some(CodecKind::Sbc) });
        caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x22]);
        if CodecKind::Aptx.available() {
            assert_eq!(caps.best(), Some(CodecKind::Aptx));
        }
        caps.add_vendor(4, &[0x2D, 0x01, 0, 0, 0xAA, 0x00, 0x3C, 0x07]);
        if CodecKind::Ldac.available() {
            assert_eq!(caps.best(), Some(CodecKind::Ldac));
        }
    }

    #[test]
    fn mono_only_aptx_is_ignored() {
        let mut caps = RemoteCaps::default();
        let r = caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x21]);
        assert_eq!(r, VendorCapability::Ignored(CodecKind::Aptx, "no stereo"));
        assert!(!caps.has(CodecKind::Aptx));
    }

    #[test]
    fn aptx_ll_extended_config_echoes_raised_levels() {
        let mut caps = RemoteCaps::default();
        let mut info = vec![0x0A, 0, 0, 0, 0x02, 0x00, 0x32, 0x02, 0x00];
        info.extend_from_slice(&100u16.to_le_bytes()); // target
        info.extend_from_slice(&1000u16.to_le_bytes()); // initial
        info.extend_from_slice(&[0, 0]); // sra
        info.extend_from_slice(&50u16.to_le_bytes()); // good
        assert_eq!(caps.add_vendor(7, &info), VendorCapability::Taken(CodecKind::AptxLl));
        let c = caps.configuration(CodecKind::AptxLl, 48000, 2, 16).unwrap();
        let ConfigRequest::Vendor(v) = c.request else { panic!() };
        assert_eq!(v.len(), 17);
        assert_eq!(v[6], 0x12);
        assert_eq!(v[7], APTXLL_HAS_NEW_CAPS);
        assert_eq!(u16::from_le_bytes([v[9], v[10]]), APTXLL_TARGET_LEVEL);
        assert_eq!(u16::from_le_bytes([v[11], v[12]]), 1000);
        assert_eq!(v[13], APTXLL_SRA_MAX_RATE);
        assert_eq!(u16::from_le_bytes([v[15], v[16]]), APTXLL_GOOD_LEVEL);
    }

    #[test]
    fn fallback_reports_why() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.audio_sinks = 2;
        caps.audio_sinks_in_use = 1;
        let c = choose(&caps, Some(CodecKind::Ldac), true).unwrap();
        assert_eq!(c.kind, CodecKind::Sbc);
        assert_eq!(c.replaced, Some((CodecKind::Ldac, Unavailable::SepInUse { in_use: 1, total: 2 })));
        assert!(choose(&caps, Some(CodecKind::Ldac), false).is_err());
    }

    /// A headset's SBC SEP: everything, bitpool 2..53
    fn sbc_all() -> SbcCapabilities {
        SbcCapabilities {
            sampling_frequency: 0x0F,
            channel_mode: 0x0F,
            block_length: 0x0F,
            subbands: 0x03,
            allocation_method: 0x03,
            min_bitpool: 2,
            max_bitpool: 53,
        }
    }

    #[test]
    fn sbc_xq_negotiation() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        assert!(caps.has(CodecKind::SbcXq) && caps.has(CodecKind::Sbc));
        assert_eq!(caps.sep(CodecKind::SbcXq), Some(RemoteSep { seid: 1 }));
        assert_eq!(caps.best(), Some(CodecKind::SbcXq));
        assert_eq!(caps.sbc_max_bitpool(), 53);
        assert_eq!(CodecKind::from_key("SBCXQ"), Some(CodecKind::SbcXq));
        assert_eq!((CodecKind::SbcXq.name(), CodecKind::SbcXq.media_header_len()), ("SBC XQ", 1));
        assert!(CodecKind::SbcXq.uses_rtp());
        // Above SBC, below AAC
        caps.add_aac(2);
        if CodecKind::Aac.available() {
            assert_eq!(caps.best(), Some(CodecKind::Aac));
        }

        let c = caps.configuration(CodecKind::SbcXq, 44100, 2, 16).unwrap();
        assert_eq!(endpoint_kind(c.kind), CodecKind::Sbc);
        assert_eq!(
            c.request,
            ConfigRequest::Sbc {
                sampling_frequency: 44100,
                channel_mode: ChannelMode::DualChannel,
                min_bitpool: 2,
                max_bitpool: 47
            }
        );
        let c = caps.configuration(CodecKind::SbcXq, 48000, 1, 16).unwrap();
        assert_eq!(
            c.request,
            ConfigRequest::Sbc {
                sampling_frequency: 48000,
                channel_mode: ChannelMode::Mono,
                min_bitpool: 2,
                max_bitpool: 43
            }
        );
        assert!(caps.configuration(CodecKind::SbcXq, 32000, 2, 16).is_err());
        // Plain SBC is unchanged
        let c = caps.configuration(CodecKind::Sbc, 48000, 2, 16).unwrap();
        assert_eq!(
            c.request,
            ConfigRequest::Sbc {
                sampling_frequency: 48000,
                channel_mode: ChannelMode::JointStereo,
                min_bitpool: 2,
                max_bitpool: 53
            }
        );

        // A sink taking up to 45: XQ capped there
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, SbcCapabilities { max_bitpool: 45, ..sbc_all() });
        let c = caps.configuration(CodecKind::SbcXq, 44100, 2, 16).unwrap();
        assert!(matches!(c.request, ConfigRequest::Sbc { max_bitpool: 45, .. }));
    }

    #[test]
    fn sbc_uses_the_sinks_bitpool_range() {
        // Not known (no capabilities): 2..53
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        assert_eq!(caps.sbc_bitpool_range(53), (2, 53));
        let c = caps.configuration(CodecKind::Sbc, 48000, 2, 16).unwrap();
        assert!(matches!(c.request, ConfigRequest::Sbc { min_bitpool: 2, max_bitpool: 53, .. }));
        // (sink min, sink max) -> configured (min, max)
        for ((smin, smax), want) in [
            ((2, 53), (2, 53)),
            ((2, 35), (2, 35)),
            ((2, 250), (2, 53)),
            ((10, 40), (10, 40)),
            ((0, 0), (2, 2)),
            ((60, 70), (53, 53)),
        ] {
            let mut caps = RemoteCaps::default();
            caps.add_sbc_caps(1, SbcCapabilities { min_bitpool: smin, max_bitpool: smax, ..sbc_all() });
            assert_eq!((caps.sbc_min_bitpool(), caps.sbc_max_bitpool()), (smin, smax));
            let c = caps.configuration(CodecKind::Sbc, 44100, 2, 16).unwrap();
            let ConfigRequest::Sbc { min_bitpool, max_bitpool, channel_mode, .. } = c.request else { panic!() };
            assert_eq!(((min_bitpool, max_bitpool), channel_mode), (want, ChannelMode::JointStereo), "{smin}-{smax}");
        }
    }

    #[test]
    fn sbc_xq_falls_back_to_sbc() {
        let no_xq = [
            SbcCapabilities { channel_mode: 0x0B, ..sbc_all() }, // no dual channel
            SbcCapabilities { max_bitpool: 42, ..sbc_all() },    // bitpool too low
            SbcCapabilities { block_length: 0x0E, ..sbc_all() }, // no 16 blocks
            SbcCapabilities { subbands: 0x02, ..sbc_all() },     // 4 subbands only
            SbcCapabilities { allocation_method: 0x02, ..sbc_all() }, // SNR only
            SbcCapabilities { sampling_frequency: 0x0C, ..sbc_all() }, // 16 / 32k only
        ];
        for sbc in no_xq {
            let mut caps = RemoteCaps::default();
            caps.add_sbc_caps(1, sbc);
            assert!(!caps.has(CodecKind::SbcXq), "{sbc:?}");
            assert_eq!(caps.best(), Some(CodecKind::Sbc));
            assert_eq!(
                caps.configuration(CodecKind::SbcXq, 48000, 2, 16),
                Err(ConfigError::NotOffered(CodecKind::SbcXq))
            );
            let c = choose(&caps, Some(CodecKind::SbcXq), true).unwrap();
            assert_eq!(c.kind, CodecKind::Sbc);
            assert_eq!(c.replaced, Some((CodecKind::SbcXq, Unavailable::NotSupported)));
            assert!(choose(&caps, Some(CodecKind::SbcXq), false).is_err());
        }
        // Capabilities not known: SBC only
        let mut caps = RemoteCaps::default();
        caps.add_sbc(3);
        assert!(!caps.has(CodecKind::SbcXq));
        // A second SBC SEP without dual channel keeps the one that allows XQ
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        caps.add_sbc_caps(2, SbcCapabilities { channel_mode: 0x01, ..sbc_all() });
        assert_eq!(caps.sep(CodecKind::SbcXq), Some(RemoteSep { seid: 1 }));
    }

    #[test]
    fn faststream_negotiation() {
        // A CSR headset: music 44.1 / 48 kHz plus the 16 kHz voice direction
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        let info = [0x0A, 0, 0, 0, 0x01, 0x00, 0x03, 0x23];
        assert_eq!(caps.add_vendor(5, &info), VendorCapability::Taken(CodecKind::FastStream));
        assert!(caps.has(CodecKind::FastStream));
        // Never Auto's choice, not even over SBC
        assert_eq!(caps.best(), Some(CodecKind::SbcXq));
        let mut only = RemoteCaps::default();
        only.add_vendor(5, &info);
        assert_eq!(only.best(), None);
        assert_eq!(choose(&only, Some(CodecKind::FastStream), false).unwrap().kind, CodecKind::FastStream);

        let c = caps.configuration(CodecKind::FastStream, 48000, 2, 16).unwrap();
        assert_eq!(c.remote_seid, 5);
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x01]));
        let c = caps.configuration(CodecKind::FastStream, 44100, 1, 16).unwrap();
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x02]));
        assert!(caps.configuration(CodecKind::FastStream, 32000, 2, 16).is_err());
        assert_eq!(caps.pick_faststream_sample_rate(44100), 44100);
        assert_eq!(caps.pick_faststream_sample_rate(96000), 48000);

        // 44.1 kHz only
        let mut caps = RemoteCaps::default();
        caps.add_vendor(5, &[0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x02]);
        assert_eq!(caps.pick_faststream_sample_rate(48000), 44100);
        assert!(caps.configuration(CodecKind::FastStream, 48000, 2, 16).is_err());

        // Voice only, no rate, too short: ignored; aptX LL (same vendor) is not FastStream
        for bad in [
            &[0x0A, 0, 0, 0, 0x01, 0x00, 0x02, 0x20][..],
            &[0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x20],
            &[0x0A, 0, 0, 0, 0x01, 0x00, 0x01],
        ] {
            let mut caps = RemoteCaps::default();
            assert!(matches!(caps.add_vendor(5, bad), VendorCapability::Ignored(CodecKind::FastStream, _)));
            assert!(!caps.has(CodecKind::FastStream));
        }
        let mut caps = RemoteCaps::default();
        assert_eq!(
            caps.add_vendor(6, &[0x0A, 0, 0, 0, 0x02, 0x00, 0x32, 0x00]),
            VendorCapability::Taken(CodecKind::AptxLl)
        );
        assert!(!caps.has(CodecKind::FastStream));

        // Fallback from FastStream goes to the best codec
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        let c = choose(&caps, Some(CodecKind::FastStream), true).unwrap();
        assert_eq!((c.kind, c.replaced), (CodecKind::Sbc, Some((CodecKind::FastStream, Unavailable::NotSupported))));

        // The local SEP: vendor header, music direction, 44.1 + 48 kHz
        let ep = local_endpoints().into_iter().find(|e| e.kind == CodecKind::FastStream).unwrap();
        assert_eq!(ep.caps, vec![0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x03]);
        assert_eq!(ep.config, vec![0x0A, 0, 0, 0, 0x01, 0x00, 0x01, 0x01]);
        assert_eq!(CodecKind::from_key("faststream"), Some(CodecKind::FastStream));
        assert!(!CodecKind::FastStream.uses_rtp());
        assert_eq!(CodecKind::FastStream.media_header_len(), 0);
    }

    #[test]
    fn opus_caps_and_configuration() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc(1);
        caps.add_aac(2);
        // Android's sink capabilities: 48 kHz, 20 ms, stereo
        let r = caps.add_vendor(5, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x92]);
        assert_eq!(r, VendorCapability::Taken(CodecKind::Opus));
        if CodecKind::Opus.available() {
            assert_eq!(caps.best(), Some(CodecKind::Opus));
        }
        assert_eq!(caps.opus_stream(2), (20, 2));
        // No mono on the sink: mono input goes out as stereo
        assert_eq!(caps.opus_stream(1), (20, 2));
        let c = caps.configuration(CodecKind::Opus, 48000, 2, 16).unwrap();
        assert_eq!(c.remote_seid, 5);
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x92]));
        assert_eq!(
            caps.configuration(CodecKind::Opus, 44100, 2, 16),
            Err(ConfigError::SampleRate(CodecKind::Opus, 44100))
        );
        // LDAC and the aptX family still win Auto
        caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x22]);
        if CodecKind::Aptx.available() {
            assert_eq!(caps.best(), Some(CodecKind::Aptx));
        }

        // PipeWire's capabilities: 10 + 20 ms, mono / stereo / dual mono
        let mut pw = RemoteCaps::default();
        pw.add_vendor(3, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x9F]);
        assert_eq!(pw.opus_stream(1), (20, 1));
        let c = pw.configuration(CodecKind::Opus, 48000, 1, 16).unwrap();
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x91]));
        // 10 ms only
        let mut short = RemoteCaps::default();
        short.add_vendor(3, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x8A]);
        assert_eq!(short.opus_stream(2), (10, 2));
        let c = short.configuration(CodecKind::Opus, 48000, 2, 16).unwrap();
        assert_eq!(c.request, ConfigRequest::Vendor(vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x8A]));

        // Unusable: no 48 kHz, no duration, dual mono only, too short
        let bad: [&[u8]; 4] = [
            &[0xE0, 0, 0, 0, 0x01, 0x00, 0x12],
            &[0xE0, 0, 0, 0, 0x01, 0x00, 0x82],
            &[0xE0, 0, 0, 0, 0x01, 0x00, 0x94],
            &[0xE0, 0, 0, 0, 0x01, 0x00],
        ];
        for info in bad {
            let mut c = RemoteCaps::default();
            assert!(matches!(c.add_vendor(3, info), VendorCapability::Ignored(CodecKind::Opus, _)), "{info:02X?}");
            assert!(!c.has(CodecKind::Opus));
        }
    }

    #[test]
    fn opus_local_endpoint() {
        let eps = local_endpoints();
        let Some(ep) = eps.iter().find(|e| e.kind == CodecKind::Opus) else {
            assert!(!CodecKind::Opus.available());
            return;
        };
        assert_eq!(ep.codec_type, CODEC_TYPE_VENDOR);
        assert_eq!(ep.caps, vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x9B]);
        assert_eq!(ep.config, vec![0xE0, 0, 0, 0, 0x01, 0x00, 0x92]);
        // BTstack's static endpoint pool (MAX_NR_AVDTP_STREAM_ENDPOINTS)
        assert!(eps.len() <= 10);
    }

    /// PipeWire's opus_05 sink SEP (codec_fill_caps): stereo at front left +
    /// right, every frame duration, no bit rate limit, no return channel
    fn pipewire_opus05(main_channels: u8, frames: u8, bitrate: u16, bidi_channels: u8) -> Vec<u8> {
        let mut info = vec![0xF1, 0x05, 0, 0, 0x05, 0x10];
        info.extend_from_slice(&opus05_direction(main_channels, 0, 0x3, frames, bitrate));
        info.extend_from_slice(&opus05_direction(bidi_channels, 0, if bidi_channels > 0 { 0x3 } else { 0 }, 0x1F, 0));
        info
    }

    #[test]
    fn opus05_caps_and_configuration() {
        let mut caps = RemoteCaps::default();
        caps.add_sbc_caps(1, sbc_all());
        let r = caps.add_vendor(6, &pipewire_opus05(2, 0x1F, 0, 0));
        assert_eq!(r, VendorCapability::Taken(CodecKind::Opus05));
        assert!(caps.has(CodecKind::Opus05));
        // Never Auto's choice
        assert_eq!(caps.best(), Some(CodecKind::SbcXq));
        let mut only = RemoteCaps::default();
        only.add_vendor(6, &pipewire_opus05(2, 0x1F, 0, 0));
        assert_eq!(only.best(), None);
        if CodecKind::Opus05.available() {
            assert_eq!(choose(&only, Some(CodecKind::Opus05), false).unwrap().kind, CodecKind::Opus05);
        }

        // 10 ms, stereo (mono input too), 312 * 1024 bps, no return channel
        assert_eq!(caps.opus05_stream(), (10, 2));
        assert_eq!(caps.opus_params(CodecKind::Opus05, 1), (10, 2, 312 * 1024));
        let mut want = vec![0xF1, 0x05, 0, 0, 0x05, 0x10, 2, 1, 0x03, 0, 0, 0, 0x04, 0x38, 0x01];
        want.resize(24, 0);
        for ch in [2, 1] {
            let c = caps.configuration(CodecKind::Opus05, 48000, ch, 16).unwrap();
            assert_eq!(c.remote_seid, 6);
            assert_eq!(c.request, ConfigRequest::Vendor(want.clone()));
        }
        assert_eq!(
            caps.configuration(CodecKind::Opus05, 44100, 2, 16),
            Err(ConfigError::SampleRate(CodecKind::Opus05, 44100))
        );

        // 20 ms only with a 128-unit limit; a mono-only sink
        let vendor_info = |caps: &RemoteCaps| match caps.configuration(CodecKind::Opus05, 48000, 2, 16).unwrap().request
        {
            ConfigRequest::Vendor(v) => v,
            r => panic!("{r:?}"),
        };
        let mut c20 = RemoteCaps::default();
        c20.add_vendor(6, &pipewire_opus05(2, 0x08, 128, 0));
        assert_eq!(c20.opus_params(CodecKind::Opus05, 2), (20, 2, 128 * 1024));
        let v = vendor_info(&c20);
        assert_eq!((v[12], u16::from_le_bytes([v[13], v[14]])), (0x08, 128));
        let mut mono = RemoteCaps::default();
        mono.add_vendor(6, &pipewire_opus05(1, 0x04, 1000, 0));
        assert_eq!(&vendor_info(&mono)[6..15], &[1, 0, 0, 0, 0, 0, 0x04, 0x38, 0x01]);

        // PipeWire's duplex SEP is taken only when there is no other
        let mut both = RemoteCaps::default();
        both.add_vendor(7, &pipewire_opus05(2, 0x1F, 0, 2));
        assert_eq!(both.sep(CodecKind::Opus05), Some(RemoteSep { seid: 7 }));
        both.add_vendor(6, &pipewire_opus05(8, 0x1F, 0, 0));
        both.add_vendor(8, &pipewire_opus05(2, 0x1F, 0, 2));
        assert_eq!(both.sep(CodecKind::Opus05), Some(RemoteSep { seid: 6 }));
        let v = vendor_info(&both);
        assert_eq!((v[6], v[15]), (2, 0));

        // Unusable: no channel, no 10 / 20 ms, too short; Android's Opus is not Opus 05
        let bad = [
            pipewire_opus05(0, 0x1F, 0, 0),
            pipewire_opus05(2, 0x13, 0, 0),
            pipewire_opus05(2, 0x1F, 0, 0)[..15].to_vec(),
        ];
        for info in bad {
            let mut c = RemoteCaps::default();
            assert!(matches!(c.add_vendor(3, &info), VendorCapability::Ignored(CodecKind::Opus05, _)), "{info:02X?}");
            assert!(!c.has(CodecKind::Opus05));
        }
        let mut android = RemoteCaps::default();
        android.add_vendor(5, &[0xE0, 0, 0, 0, 0x01, 0x00, 0x92]);
        assert!(android.has(CodecKind::Opus) && !android.has(CodecKind::Opus05));
        assert_eq!(android.opus_params(CodecKind::Opus, 2), (20, 2, 0));
        assert_eq!(CodecKind::from_key("opus05"), Some(CodecKind::Opus05));
        assert_eq!((CodecKind::Opus05.name(), CodecKind::Opus05.media_header_len()), ("Opus 05", 1));
        assert!(CodecKind::Opus05.uses_rtp() && CodecKind::Opus05.is_opus());
    }

    #[test]
    fn opus05_local_endpoint() {
        let eps = local_endpoints();
        let Some(ep) = eps.iter().find(|e| e.kind == CodecKind::Opus05) else {
            assert!(!CodecKind::Opus05.available());
            return;
        };
        assert_eq!(ep.codec_type, CODEC_TYPE_VENDOR);
        let mut caps = vec![0xF1, 0x05, 0, 0, 0x05, 0x10, 2, 0, 0x03, 0, 0, 0, 0x0C, 0, 0];
        caps.resize(24, 0);
        assert_eq!(ep.caps, caps);
        let mut config = vec![0xF1, 0x05, 0, 0, 0x05, 0x10, 2, 1, 0x03, 0, 0, 0, 0x04, 0x38, 0x01];
        config.resize(24, 0);
        assert_eq!(ep.config, config);
        // BTstack's static endpoint pool (MAX_NR_AVDTP_STREAM_ENDPOINTS)
        assert!(eps.len() <= 10);
    }

    #[test]
    fn aptx_rates() {
        let mut caps = RemoteCaps::default();
        caps.add_vendor(3, &[0x4F, 0, 0, 0, 0x01, 0x00, 0x22]);
        assert_eq!(caps.pick_aptx_sample_rate(CodecKind::Aptx, 48000), 44100);
        assert_eq!(caps.pick_aptx_sample_rate(CodecKind::Aptx, 96000), 44100);
        assert!(caps.configuration(CodecKind::Aptx, 48000, 2, 16).is_err());
    }
}
