// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! The LHDC V5 encoder: a portable C99 port of the Rust encoder in AOSP
//! (Apache-2.0), built from the git submodule extern/lhdcv5-enc.
//!
//! [`ffi`] has hand-written declarations matching include/lhdcv5_enc.h.
//! [`LhdcV5Enc`] owns the encoder context and passes slice lengths along, so
//! that callers need no `unsafe` (the C functions check both the input and
//! the output length themselves).

use std::fmt;
#[cfg(not(feature = "aosp-rust"))]
use std::ptr::NonNull;

/// Declarations from include/lhdcv5_enc.h
#[allow(non_camel_case_types)]
pub mod ffi {
    use std::marker::{PhantomData, PhantomPinned};

    /// `lhdcv5_enc_t` (opaque)
    #[repr(C)]
    pub struct lhdcv5_enc_t {
        _data: (),
        _marker: PhantomData<(*mut u8, PhantomPinned)>,
    }

    pub const LHDC_FRET_SUCCESS: i32 = 0;
    pub const LHDC_FRET_INVALID_INPUT_PARAM: i32 = -1;
    pub const LHDC_FRET_INVALID_HANDLE_CB: i32 = -2;
    pub const LHDC_FRET_INVALID_CODEC: i32 = -7;
    pub const LHDC_FRET_CODEC_NOT_READY: i32 = -8;
    pub const LHDC_FRET_ERROR: i32 = -10;
    pub const LHDC_FRET_BUF_NOT_ENOUGH: i32 = -11;

    pub const LHDC_SR_44100HZ: u32 = 44100;
    pub const LHDC_SR_48000HZ: u32 = 48000;
    pub const LHDC_SR_96000HZ: u32 = 96000;
    pub const LHDC_SR_192000HZ: u32 = 192000;

    /// Input PCM formats: packed s16le, or packed s24le (3 bytes per sample)
    pub const LHDCBT_SMPL_FMT_S16: u32 = 16;
    pub const LHDCBT_SMPL_FMT_S24: u32 = 24;

    pub const LHDC_ENC_INTERVAL_10MS: u32 = 10;
    pub const LHDC_ENC_INTERVAL_20MS: u32 = 20;

    /// Bit rate indexes (kbps at 48 kHz; 44.1 kHz differs at LOW3 and MID)
    pub const LHDC_QUALITY_LOW0: u32 = 0; //   64
    pub const LHDC_QUALITY_LOW1: u32 = 1; //  160
    pub const LHDC_QUALITY_LOW2: u32 = 2; //  192
    pub const LHDC_QUALITY_LOW3: u32 = 3; //  240 / 256
    pub const LHDC_QUALITY_LOW4: u32 = 4; //  320
    pub const LHDC_QUALITY_LOW: u32 = 5; //   400
    pub const LHDC_QUALITY_MID: u32 = 6; //   480 / 500
    pub const LHDC_QUALITY_HIGH: u32 = 7; //  900
    pub const LHDC_QUALITY_HIGH1: u32 = 8; // 1000
    pub const LHDC_QUALITY_HIGH2: u32 = 9; // 1100
    pub const LHDC_QUALITY_HIGH3: u32 = 10; // 1200
    pub const LHDC_QUALITY_HIGH4: u32 = 11; // 1300
    pub const LHDC_QUALITY_HIGH5: u32 = 12; // 1400
    pub const LHDC_QUALITY_MAX_BITRATE: u32 = 12;
    /// Adaptive: starts at LHDC_QUALITY_LOW
    pub const LHDC_QUALITY_AUTO: u32 = 13;

    pub const LHDC_MTU_MIN: u32 = 300;
    pub const LHDC_MTU_2MBPS: u32 = 660;
    pub const LHDC_MTU_3MBPS: u32 = 1023;
    pub const LHDC_MTU_MAX: u32 = 8192;

    pub const LHDC_VERSION_1: u32 = 1;

    unsafe extern "C" {
        pub fn lhdcv5_enc_context_size() -> usize;
        pub fn lhdcv5_enc_new(version: u32) -> *mut lhdcv5_enc_t;
        pub fn lhdcv5_enc_free(h: *mut lhdcv5_enc_t);
        pub fn lhdcv5_enc_init_encoder(
            h: *mut lhdcv5_enc_t,
            sampling_freq: u32,
            bits_per_sample: u32,
            bitrate_inx: u32,
            mtu: u32,
            interval: u32,
        ) -> i32;
        pub fn lhdcv5_enc_get_block_size(h: *const lhdcv5_enc_t, samples_per_frame: *mut u32) -> i32;
        pub fn lhdcv5_enc_encode(
            h: *mut lhdcv5_enc_t,
            in_pcm: *const u8,
            in_pcm_len: usize,
            out_buf: *mut u8,
            out_buf_len: usize,
            written_bytes: *mut u32,
            written_frames: *mut u32,
        ) -> i32;
        pub fn lhdcv5_enc_set_bitrate_index(h: *mut lhdcv5_enc_t, bitrate_inx: u32, update_quality_status: bool)
        -> i32;
        pub fn lhdcv5_enc_set_max_bitrate_index(h: *mut lhdcv5_enc_t, max_bitrate_inx: u32) -> i32;
        pub fn lhdcv5_enc_set_min_bitrate_index(h: *mut lhdcv5_enc_t, min_bitrate_inx: u32) -> i32;
        pub fn lhdcv5_enc_get_bitrate_index(h: *const lhdcv5_enc_t, bitrate_kbps: u32, bitrate_inx: *mut u32) -> i32;
        pub fn lhdcv5_enc_get_last_bitrate(h: *const lhdcv5_enc_t, bitrate_kbps: *mut u32) -> i32;
        pub fn lhdcv5_enc_get_quality_mode(h: *const lhdcv5_enc_t, quality_status: *mut u32) -> i32;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The encoder context could not be allocated
    Alloc,
    /// An `LHDC_FRET_*` code
    Lhdc(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::Alloc => f.write_str("out of memory"),
            Error::Lhdc(code) => {
                let what = match code {
                    ffi::LHDC_FRET_INVALID_INPUT_PARAM => "invalid input parameter",
                    ffi::LHDC_FRET_INVALID_HANDLE_CB => "invalid handle",
                    ffi::LHDC_FRET_INVALID_CODEC => "invalid codec",
                    ffi::LHDC_FRET_CODEC_NOT_READY => "codec not ready",
                    ffi::LHDC_FRET_ERROR => "error",
                    ffi::LHDC_FRET_BUF_NOT_ENOUGH => "output buffer too small",
                    _ => "failure",
                };
                write!(f, "LHDC V5 encoder: {what} ({code})")
            }
        }
    }
}

impl std::error::Error for Error {}

#[cfg(not(feature = "aosp-rust"))]
fn check(ret: i32) -> Result<(), Error> {
    if ret == ffi::LHDC_FRET_SUCCESS { Ok(()) } else { Err(Error::Lhdc(ret)) }
}

/// What one [`LhdcV5Enc::encode`] call produced
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodeOutput {
    /// Bytes written: 0, or a whole packet's worth of frames
    pub bytes: usize,
    /// LHDC frames in them
    pub frames: u32,
}

/// An LHDC V5 encoder context (lhdcv5_enc_new), freed on drop
#[cfg(not(feature = "aosp-rust"))]
pub struct LhdcV5Enc {
    handle: NonNull<ffi::lhdcv5_enc_t>,
}

// SAFETY: the context is plain heap memory owned by this value, with no
// thread affinity or global state in the encoder; moving it to another
// thread is fine. It is not Sync; &mut self on every mutating call keeps
// calls on one thread at a time.
#[cfg(not(feature = "aosp-rust"))]
unsafe impl Send for LhdcV5Enc {}

#[cfg(not(feature = "aosp-rust"))]
impl LhdcV5Enc {
    /// lhdcv5_enc_new(LHDC_VERSION_1)
    pub fn new() -> Result<LhdcV5Enc, Error> {
        // SAFETY: plain argument; returns an owned context or NULL
        let raw = unsafe { ffi::lhdcv5_enc_new(ffi::LHDC_VERSION_1) };
        NonNull::new(raw).map(|handle| LhdcV5Enc { handle }).ok_or(Error::Alloc)
    }

    fn ptr(&self) -> *mut ffi::lhdcv5_enc_t {
        self.handle.as_ptr()
    }

    /// lhdcv5_enc_init_encoder: sample rate 44100 / 48000 / 96000 / 192000,
    /// 16 or 24 bits, an `LHDC_QUALITY_*` index, the MTU the packets are
    /// sized for (LHDC_MTU_MIN..=LHDC_MTU_MAX), `LHDC_ENC_INTERVAL_*`
    pub fn init_encoder(
        &mut self,
        sample_rate: u32,
        bits_per_sample: u32,
        bitrate_inx: u32,
        mtu: u32,
        interval: u32,
    ) -> Result<(), Error> {
        // SAFETY: the context is valid (owned by self); the encoder
        // validates the plain arguments
        check(unsafe {
            ffi::lhdcv5_enc_init_encoder(self.ptr(), sample_rate, bits_per_sample, bitrate_inx, mtu, interval)
        })
    }

    /// PCM frames (samples per channel) of one LHDC frame: 240 / 480 / 960
    pub fn block_size(&self) -> Result<u32, Error> {
        let mut n = 0;
        // SAFETY: the context is valid; `n` is a local
        check(unsafe { ffi::lhdcv5_enc_get_block_size(self.ptr(), &mut n) })?;
        Ok(n)
    }

    /// Encodes one frame of interleaved stereo PCM, packed s16le or s24le as
    /// initialized (block_size * 2 samples). Returns a packet every few
    /// calls; LHDC_FRET_BUF_NOT_ENOUGH if `out` cannot take it.
    pub fn encode(&mut self, pcm: &[u8], out: &mut [u8]) -> Result<EncodeOutput, Error> {
        let (mut written, mut frames) = (0, 0);
        // SAFETY: the context is valid. The encoder reads one frame from
        // pcm after checking pcm.len() against its size, and writes a packet
        // to out only after checking it fits out.len(). The out parameters
        // point to locals.
        check(unsafe {
            ffi::lhdcv5_enc_encode(
                self.ptr(),
                pcm.as_ptr(),
                pcm.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut written,
                &mut frames,
            )
        })?;
        Ok(EncodeOutput { bytes: (written as usize).min(out.len()), frames })
    }

    /// lhdcv5_enc_set_bitrate_index (clamped to the min / max indexes)
    pub fn set_bitrate_index(&mut self, bitrate_inx: u32, update_quality_status: bool) -> Result<(), Error> {
        // SAFETY: the context is valid; the encoder validates the index
        check(unsafe { ffi::lhdcv5_enc_set_bitrate_index(self.ptr(), bitrate_inx, update_quality_status) })
    }

    /// lhdcv5_enc_set_max_bitrate_index (LHDC_QUALITY_LOW..=MAX_BITRATE)
    pub fn set_max_bitrate_index(&mut self, inx: u32) -> Result<(), Error> {
        // SAFETY: the context is valid; the encoder validates the index
        check(unsafe { ffi::lhdcv5_enc_set_max_bitrate_index(self.ptr(), inx) })
    }

    /// lhdcv5_enc_set_min_bitrate_index (..=LHDC_QUALITY_LOW)
    pub fn set_min_bitrate_index(&mut self, inx: u32) -> Result<(), Error> {
        // SAFETY: the context is valid; the encoder validates the index
        check(unsafe { ffi::lhdcv5_enc_set_min_bitrate_index(self.ptr(), inx) })
    }

    /// lhdcv5_enc_get_bitrate_index: the lowest index whose bit rate (in the
    /// initialized sample rate's table) is at least `kbps`
    pub fn bitrate_index(&self, kbps: u32) -> Result<u32, Error> {
        let mut inx = 0;
        // SAFETY: the context is valid; `inx` is a local
        check(unsafe { ffi::lhdcv5_enc_get_bitrate_index(self.ptr(), kbps, &mut inx) })?;
        Ok(inx)
    }

    /// lhdcv5_enc_get_last_bitrate: the current target bit rate in kbps
    pub fn set_bitrate_kbps(&mut self, kbps: u32) -> Result<(), Error> {
        // The legacy C backend only has stock presets. Never round a custom
        // target up to 900 while presenting it as 700/750/800.
        if matches!(kbps,700|750|800) { return Err(Error::Lhdc(ffi::LHDC_FRET_INVALID_INPUT_PARAM)); }
        let index=self.bitrate_index(kbps)?;
        self.set_bitrate_index(index,false)
    }

    pub fn last_bitrate(&self) -> u32 {
        let mut kbps = 0;
        // SAFETY: the context is valid; `kbps` is a local. Only fails for
        // NULL pointers.
        let _ = unsafe { ffi::lhdcv5_enc_get_last_bitrate(self.ptr(), &mut kbps) };
        kbps
    }

    /// lhdcv5_enc_get_quality_mode: the quality index set (or AUTO)
    pub fn quality_mode(&self) -> u32 {
        let mut q = 0;
        // SAFETY: the context is valid; `q` is a local. Only fails for NULL
        // pointers.
        let _ = unsafe { ffi::lhdcv5_enc_get_quality_mode(self.ptr(), &mut q) };
        q
    }
}

#[cfg(not(feature = "aosp-rust"))]
impl Drop for LhdcV5Enc {
    fn drop(&mut self) {
        // SAFETY: the context came from lhdcv5_enc_new and is owned by
        // self; nothing uses it after this
        unsafe { ffi::lhdcv5_enc_free(self.ptr()) }
    }
}

/// Bytes of one encoder context (lhdcv5_enc_context_size)
#[cfg(not(feature = "aosp-rust"))]
pub fn context_size() -> usize {
    // SAFETY: no arguments, returns a constant
    unsafe { ffi::lhdcv5_enc_context_size() }
}

#[cfg(test)]
mod tests {
    use super::ffi::*;
    use super::*;

    #[test]
    #[cfg(not(feature = "aosp-rust"))]
    fn context_is_the_192k_build() {
        // ~117 KB with the default LHDCV5_MAX_SAMPLE_RATE (192 kHz)
        let size = context_size();
        assert!((100_000..140_000).contains(&size), "{size}");
    }

    #[test]
    fn init_and_encode() {
        let mut enc = LhdcV5Enc::new().unwrap();
        assert_eq!(
            enc.init_encoder(48000, 16, LHDC_QUALITY_LOW, 100, LHDC_ENC_INTERVAL_20MS),
            Err(Error::Lhdc(LHDC_FRET_INVALID_INPUT_PARAM))
        );
        assert!(enc.init_encoder(32000, 16, LHDC_QUALITY_LOW, 660, LHDC_ENC_INTERVAL_20MS).is_err());
        enc.init_encoder(48000, 16, LHDC_QUALITY_MID, 660, LHDC_ENC_INTERVAL_20MS).unwrap();
        assert_eq!(enc.block_size(), Ok(240));
        assert_eq!(enc.last_bitrate(), 500);
        assert_eq!(enc.bitrate_index(500), Ok(LHDC_QUALITY_MID));
        assert_eq!(enc.quality_mode(), LHDC_QUALITY_MID);

        let pcm = vec![0u8; 240 * 2 * 2];
        let mut out = vec![0u8; 660];
        // A short input is rejected, not over-read
        assert!(enc.encode(&pcm[..100], &mut out).is_err());
        let mut frames = 0;
        for _ in 0..40 {
            let r = enc.encode(&pcm, &mut out).unwrap();
            assert!(r.bytes <= 660);
            frames += r.frames;
        }
        assert!(frames > 0);
        // An output buffer too small for a packet is reported
        let mut tiny = [0u8; 8];
        let mut seen = false;
        for _ in 0..8 {
            if enc.encode(&pcm, &mut tiny) == Err(Error::Lhdc(LHDC_FRET_BUF_NOT_ENOUGH)) {
                seen = true;
                break;
            }
        }
        assert!(seen);
    }

    #[test]
    fn bitrate_limits() {
        let mut enc = LhdcV5Enc::new().unwrap();
        enc.init_encoder(96000, 24, LHDC_QUALITY_HIGH1, 1021, LHDC_ENC_INTERVAL_20MS).unwrap();
        assert_eq!(enc.block_size(), Ok(480));
        assert_eq!(enc.last_bitrate(), 1000);
        enc.set_max_bitrate_index(LHDC_QUALITY_LOW).unwrap();
        assert_eq!(enc.last_bitrate(), 400);
        enc.set_bitrate_index(LHDC_QUALITY_LOW1, false).unwrap();
        assert_eq!(enc.last_bitrate(), 160);
        enc.set_min_bitrate_index(LHDC_QUALITY_LOW3).unwrap();
        assert_eq!(enc.last_bitrate(), 256);
        assert!(enc.set_max_bitrate_index(LHDC_QUALITY_LOW4).is_err());
    }
}

#[cfg(feature = "aosp-rust")]
mod aosp;
#[cfg(feature = "aosp-rust")]
pub use aosp::{LhdcV5Enc, context_size};
pub const BACKEND: &str = if cfg!(feature = "aosp-rust") { "AOSP Rust" } else { "A: C port" };
pub const CUSTOM_BITRATES: bool = cfg!(feature = "aosp-rust");
