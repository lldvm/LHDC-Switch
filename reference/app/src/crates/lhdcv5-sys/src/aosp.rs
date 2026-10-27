//! Native Windows adapter to the unmodified AOSP Rust encoder algorithms.
//! This build does not link the C port. Transport and ABR live above this API.
use crate::{EncodeOutput, Error, ffi};
use lhdcv5::lhdc_api::{self as api, Context};
use api::lhdc_api_internal::Error as Internal;

fn map(e: api::Error) -> Error {
    Error::Lhdc(match e {
        api::Error::InvalidInputParam => ffi::LHDC_FRET_INVALID_INPUT_PARAM,
        api::Error::InvalidCodec => ffi::LHDC_FRET_INVALID_CODEC,
        api::Error::Internal(Internal::BufTooSmall) => ffi::LHDC_FRET_BUF_NOT_ENOUGH,
        api::Error::Internal(Internal::CodecNotReady) => ffi::LHDC_FRET_CODEC_NOT_READY,
        api::Error::Internal(Internal::InvalidInputParam) => ffi::LHDC_FRET_INVALID_INPUT_PARAM,
        _ => ffi::LHDC_FRET_ERROR,
    })
}

pub struct LhdcV5Enc { ctx: Box<Context>, mtu: usize }
// SAFETY: exclusively owned heap context, no thread affinity or shared globals.
// All mutable access requires &mut self, just like the C-port adapter.
unsafe impl Send for LhdcV5Enc {}
impl LhdcV5Enc {
    pub fn new() -> Result<Self, Error> {
        Ok(Self { ctx: Box::new(Context::new(api::LHDC_VERSION_1).map_err(map)?), mtu: 0 })
    }
    pub fn init_encoder(&mut self, rate: u32, bits: u32, quality: u32, mtu: u32, interval: u32) -> Result<(), Error> {
        self.ctx.init_encoder(rate,bits,quality,api::LHDC_FRAME_5MS,mtu,interval).map_err(map)?;
        self.mtu=mtu as usize;
        Ok(())
    }
    pub fn block_size(&self) -> Result<u32, Error> { self.ctx.get_block_size().map_err(map) }
    pub fn encode(&mut self, pcm: &[u8], out: &mut [u8]) -> Result<EncodeOutput, Error> {
        // The AOSP packet copy assumes enough output space. Validate before
        // entering it, including calls that have buffered PCM from a prior frame.
        if out.len() < self.mtu { return Err(Error::Lhdc(ffi::LHDC_FRET_BUF_NOT_ENOUGH)); }
        let (mut bytes,mut frames)=(0,0);
        self.ctx.enc_process(pcm,out,&mut bytes,&mut frames).map_err(map)?;
        Ok(EncodeOutput { bytes: bytes as usize, frames })
    }
    pub fn set_bitrate_index(&mut self, i: u32, status: bool) -> Result<(), Error> { self.ctx.set_target_bitrate_inx(i,status).map(|_|()).map_err(map) }
    pub fn set_max_bitrate_index(&mut self, i: u32) -> Result<(), Error> { self.ctx.set_max_bitrate_inx(i).map(|_|()).map_err(map) }
    pub fn set_min_bitrate_index(&mut self, i: u32) -> Result<(), Error> { self.ctx.set_min_bitrate_inx(i).map(|_|()).map_err(map) }
    pub fn bitrate_index(&self, rate: u32) -> Result<u32, Error> { self.ctx.get_target_bitrate_inx(rate).map_err(map) }
    pub fn last_bitrate(&self) -> u32 { self.ctx.last_bitrate() }
    pub fn set_bitrate_kbps(&mut self, kbps: u32) -> Result<(), Error> { self.ctx.set_target_bitrate_kbps(kbps).map_err(map) }
    pub fn quality_mode(&self) -> u32 { self.ctx.quality_status() }
}
pub fn context_size() -> usize { std::mem::size_of::<Context>() }
