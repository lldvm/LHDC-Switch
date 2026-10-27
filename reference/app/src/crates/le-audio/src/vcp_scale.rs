//! VCP (Volume Controller) values (v1 le_audio::vcp in le_audio_vcp.{h,cpp}):
//! the AVRCP / VolumeSync scale 0-127 against VCS Volume_Setting 0-255, the
//! Volume State value, Volume Control Point operations, and the
//! `--le-sim-volume` spec of the tests.

/// Volume State characteristic (Volume_Setting, Mute, Change_Counter)
pub const UUID_VOLUME_STATE: u16 = 0x2B7D;
pub const UUID_VOLUME_CONTROL_POINT: u16 = 0x2B7E;
pub const UUID_VOLUME_FLAGS: u16 = 0x2B7F;

pub const OP_RELATIVE_VOLUME_DOWN: u8 = 0x00;
pub const OP_RELATIVE_VOLUME_UP: u8 = 0x01;
pub const OP_UNMUTE_RELATIVE_VOLUME_DOWN: u8 = 0x02;
pub const OP_UNMUTE_RELATIVE_VOLUME_UP: u8 = 0x03;
pub const OP_SET_ABSOLUTE_VOLUME: u8 = 0x04;
pub const OP_UNMUTE: u8 = 0x05;
pub const OP_MUTE: u8 = 0x06;

/// VCS application error: the Change_Counter of the write was not the
/// current one (re-read Volume State and send again)
pub const ATT_ERROR_INVALID_CHANGE_COUNTER: u8 = 0x80;
pub const ATT_ERROR_OPCODE_NOT_SUPPORTED: u8 = 0x81;

/// Level 0-127 to Volume_Setting 0-255, rounded to the nearest value.
/// `to_level(to_setting(l)) == l` for every level, so a level sent to the
/// headset and reported back stays the same.
pub const fn to_setting(level: u8) -> u8 {
    if level >= 127 { 255 } else { ((level as u32 * 255 + 63) / 127) as u8 }
}

/// Volume_Setting 0-255 to level 0-127, rounded to the nearest value
pub const fn to_level(setting: u8) -> u8 {
    if setting == 255 { 127 } else { ((setting as u32 * 127 + 127) / 255) as u8 }
}

/// A Volume State value
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VolumeState {
    pub setting: u8,
    pub mute: bool,
    pub change_counter: u8,
}

impl VolumeState {
    /// Parses the 3-octet value; None if shorter
    pub fn parse(value: &[u8]) -> Option<VolumeState> {
        match *value {
            [setting, mute, change_counter, ..] => Some(VolumeState { setting, mute: mute != 0, change_counter }),
            _ => None,
        }
    }
}

/// Set Absolute Volume with the last known Change_Counter
pub fn cp_set_absolute_volume(change_counter: u8, setting: u8) -> [u8; 3] {
    [OP_SET_ABSOLUTE_VOLUME, change_counter, setting]
}

/// Unmute (or another operation without a parameter: OP_*)
pub fn cp_simple(opcode: u8, change_counter: u8) -> [u8; 2] {
    [opcode, change_counter]
}

/// The test-only simulated Windows volume of `--le-sim-volume
/// "<level>[,<seconds>:<level>...]"` (v1 SimulatedWindowsVolume::parse):
/// the simulated Windows volume (0-127) at the start, then the level it
/// changes to at the given seconds after the stream started. core runs
/// VolumeSync against it, so the emulator tests exercise the real sync
/// logic without ever touching the user's output device.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SimVolumeSpec {
    pub start: u8,
    /// (milliseconds after the stream started, level), ascending
    pub steps: Vec<(u32, u8)>,
}

impl SimVolumeSpec {
    /// None if malformed or a level is above 127
    pub fn parse(spec: &str) -> Option<SimVolumeSpec> {
        // v1: strtol / strtod, i.e. leading white space allowed, nothing
        // after the number
        let level = |s: &str| s.trim_start().parse::<i64>().ok().filter(|l| (0..=127).contains(l)).map(|l| l as u8);
        let mut items = spec.split(',');
        let start = level(items.next()?)?;
        let mut steps = Vec::new();
        for item in items {
            let (seconds, lvl) = item.split_once(':')?;
            let seconds: f64 = seconds.trim_start().parse().ok().filter(|s: &f64| s.is_finite() && *s >= 0.0)?;
            steps.push(((seconds * 1000.0) as u32, level(lvl)?));
        }
        Some(SimVolumeSpec { start, steps })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_round_trip() {
        for level in 0..=127u8 {
            assert_eq!(to_level(to_setting(level)), level, "level {level}");
        }
        assert_eq!((to_setting(0), to_setting(127), to_setting(200)), (0, 255, 255));
        assert_eq!((to_level(0), to_level(255)), (0, 127));
        // Every setting maps to a level whose setting is within one step
        for setting in 0..=255u8 {
            let back = to_setting(to_level(setting)) as i32;
            assert!((back - setting as i32).abs() <= 2, "setting {setting}");
        }
        // v1's integer formulas
        assert_eq!((to_setting(64), to_level(128), to_level(100)), (129, 64, 50));
        // const
        const MID: u8 = to_setting(64);
        assert_eq!(MID, 129);
    }

    #[test]
    fn volume_state_and_control_point() {
        assert_eq!(VolumeState::parse(&[100, 1, 7]), Some(VolumeState { setting: 100, mute: true, change_counter: 7 }));
        assert_eq!(VolumeState::parse(&[100, 0]), None);
        assert_eq!(cp_set_absolute_volume(7, 200), [0x04, 7, 200]);
        assert_eq!(cp_simple(OP_UNMUTE, 9), [0x05, 9]);
    }

    #[test]
    fn sim_volume_spec() {
        assert_eq!(SimVolumeSpec::parse("64"), Some(SimVolumeSpec { start: 64, steps: vec![] }));
        assert_eq!(
            SimVolumeSpec::parse("100,2:40,3.5:127"),
            Some(SimVolumeSpec { start: 100, steps: vec![(2000, 40), (3500, 127)] })
        );
        assert_eq!(SimVolumeSpec::parse(" 5, 1: 0"), Some(SimVolumeSpec { start: 5, steps: vec![(1000, 0)] }));
        for bad in ["", "128", "-1", "64,", "64,2", "64,2:", "64,x:3", "64,-1:3", "64,1:200", "6 4", "64,1:3x"] {
            assert_eq!(SimVolumeSpec::parse(bad), None, "{bad:?}");
        }
    }
}
