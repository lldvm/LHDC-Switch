//! Sample formats as the sources deliver them, and the choice of the WASAPI
//! stream format (kept free of COM so it can be unit tested).

/// Sample type of the delivered data. The pipeline converts all three.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    /// 32-bit float, nominally -1.0..=1.0 (the shared-mode mix format of
    /// practically every device, application capture, the test tone)
    F32,
    /// 16-bit signed integer
    I16,
    /// 32-bit signed integer container. The valid bits may be fewer (24 in
    /// 32): the samples are MSB-aligned, so they read as full-scale i32.
    I32,
}

impl SampleFormat {
    pub fn bytes_per_sample(self) -> usize {
        match self {
            SampleFormat::F32 | SampleFormat::I32 => 4,
            SampleFormat::I16 => 2,
        }
    }

    /// The format of samples with this container size, if one we deliver.
    /// v1 dispatched on the container size too (wBitsPerSample, not the
    /// valid bits), so a 24-in-32 format is not dropped.
    pub fn from_container(float: bool, container_bits: u16) -> Option<SampleFormat> {
        match (float, container_bits) {
            (true, 32) => Some(SampleFormat::F32),
            (false, 16) => Some(SampleFormat::I16),
            (false, 32) => Some(SampleFormat::I32),
            _ => None,
        }
    }
}

/// Format of the delivered audio: interleaved frames of `channels` samples
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Format {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: SampleFormat,
}

impl Format {
    pub fn bytes_per_frame(&self) -> usize {
        self.channels as usize * self.sample_format.bytes_per_sample()
    }

    /// Frames in `duration` at this rate, rounded down
    pub fn frames_in(&self, duration: std::time::Duration) -> usize {
        (duration.as_nanos() * self.sample_rate as u128 / 1_000_000_000) as usize
    }
}

/// A device's shared-mode mix format, reduced to what the choice needs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MixFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub float: bool,
    pub container_bits: u16,
}

/// The stream format to ask WASAPI for, and whether that needs its
/// converter (AUTOCONVERTPCM | SRC_DEFAULT_QUALITY)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FormatPlan {
    pub format: Format,
    pub autoconvert: bool,
}

/// Loopback stream format, as v1's WasapiCapture::init chooses it: the mix
/// format, with the preferred sample rate instead of the mix rate when one
/// is given (WASAPI resamples then). A mix format we cannot deliver (24-bit
/// packed, 8-bit; not seen in shared mode in practice) is captured as float
/// instead, also through the converter.
pub(crate) fn plan_loopback(mix: MixFormat, preferred_rate: Option<u32>) -> FormatPlan {
    let rate = match preferred_rate {
        Some(r) if r > 0 => r,
        _ => mix.sample_rate,
    };
    let native = SampleFormat::from_container(mix.float, mix.container_bits);
    let sample_format = native.unwrap_or(SampleFormat::F32);
    FormatPlan {
        format: Format { sample_rate: rate, channels: mix.channels, sample_format },
        autoconvert: rate != mix.sample_rate || native.is_none(),
    }
}

/// Application capture: a process loopback client has no mix format, so
/// v1's init_process asks for float stereo and lets Windows convert to it
/// (48 kHz unless a rate is given)
pub(crate) fn plan_application(preferred_rate: Option<u32>) -> Format {
    Format {
        sample_rate: preferred_rate.filter(|&r| r > 0).unwrap_or(DEFAULT_RATE),
        channels: 2,
        sample_format: SampleFormat::F32,
    }
}

/// Rate of the sources that have none of their own (application capture,
/// test tone) when the caller gives none
pub const DEFAULT_RATE: u32 = 48_000;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const MIX_F32_48K: MixFormat = MixFormat { sample_rate: 48_000, channels: 2, float: true, container_bits: 32 };

    #[test]
    fn container_mapping() {
        assert_eq!(SampleFormat::from_container(true, 32), Some(SampleFormat::F32));
        assert_eq!(SampleFormat::from_container(false, 16), Some(SampleFormat::I16));
        assert_eq!(SampleFormat::from_container(false, 32), Some(SampleFormat::I32));
        assert_eq!(SampleFormat::from_container(false, 24), None);
        assert_eq!(SampleFormat::from_container(true, 64), None);
        assert_eq!(SampleFormat::from_container(false, 8), None);
    }

    #[test]
    fn frame_sizes() {
        let f = Format { sample_rate: 44_100, channels: 6, sample_format: SampleFormat::I16 };
        assert_eq!(f.bytes_per_frame(), 12);
        assert_eq!(f.frames_in(Duration::from_millis(10)), 441);
        assert_eq!(f.frames_in(Duration::from_secs(2)), 88_200);
    }

    #[test]
    fn mix_format_used_as_is() {
        let plan = plan_loopback(MIX_F32_48K, None);
        assert!(!plan.autoconvert);
        assert_eq!(plan.format.sample_rate, 48_000);
        assert_eq!(plan.format.channels, 2);
        assert_eq!(plan.format.sample_format, SampleFormat::F32);
        // Asking for the mix rate is the same as not asking
        assert_eq!(plan_loopback(MIX_F32_48K, Some(48_000)), plan);
        assert_eq!(plan_loopback(MIX_F32_48K, Some(0)), plan);
    }

    #[test]
    fn preferred_rate_resamples() {
        let plan = plan_loopback(MIX_F32_48K, Some(44_100));
        assert!(plan.autoconvert);
        assert_eq!(plan.format.sample_rate, 44_100);
        assert_eq!(plan.format.sample_format, SampleFormat::F32);
    }

    #[test]
    fn integer_mix_formats_kept() {
        let mix = MixFormat { sample_rate: 96_000, channels: 8, float: false, container_bits: 32 };
        let plan = plan_loopback(mix, None);
        assert_eq!(plan.format.sample_format, SampleFormat::I32);
        assert_eq!(plan.format.channels, 8);
        assert!(!plan.autoconvert);
        let mix16 = MixFormat { container_bits: 16, ..mix };
        assert_eq!(plan_loopback(mix16, None).format.sample_format, SampleFormat::I16);
    }

    #[test]
    fn unusual_mix_format_becomes_float() {
        let mix = MixFormat { sample_rate: 48_000, channels: 2, float: false, container_bits: 24 };
        let plan = plan_loopback(mix, None);
        assert_eq!(plan.format.sample_format, SampleFormat::F32);
        assert!(plan.autoconvert);
    }

    #[test]
    fn application_format() {
        let f = plan_application(None);
        assert_eq!(f, Format { sample_rate: 48_000, channels: 2, sample_format: SampleFormat::F32 });
        assert_eq!(plan_application(Some(96_000)).sample_rate, 96_000);
        assert_eq!(plan_application(Some(0)).sample_rate, 48_000);
    }
}
