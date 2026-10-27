//! Assigned numbers and small helpers (v1 le_audio_defs.h): UUIDs, LTV
//! types and enums from the Bluetooth SIG Assigned Numbers, PACS, ASCS and
//! BAP 1.0.x, shared by the BTstack side and the profile modules.

// ---- GATT services (Assigned Numbers 3.4) ----
pub const UUID_SERVICE_GAP: u16 = 0x1800;
pub const UUID_SERVICE_GATT: u16 = 0x1801;
/// Audio Input Control
pub const UUID_SERVICE_AICS: u16 = 0x1843;
/// Volume Control
pub const UUID_SERVICE_VCS: u16 = 0x1844;
/// Volume Offset Control
pub const UUID_SERVICE_VOCS: u16 = 0x1845;
/// Coordinated Set Identification
pub const UUID_SERVICE_CSIS: u16 = 0x1846;
/// Media Control
pub const UUID_SERVICE_MCS: u16 = 0x1848;
/// Generic Media Control
pub const UUID_SERVICE_GMCS: u16 = 0x1849;
/// Telephone Bearer
pub const UUID_SERVICE_TBS: u16 = 0x184B;
/// Generic Telephone Bearer
pub const UUID_SERVICE_GTBS: u16 = 0x184C;
/// Audio Stream Control
pub const UUID_SERVICE_ASCS: u16 = 0x184E;
/// Broadcast Audio Scan
pub const UUID_SERVICE_BASS: u16 = 0x184F;
/// Published Audio Capabilities
pub const UUID_SERVICE_PACS: u16 = 0x1850;
/// Common Audio
pub const UUID_SERVICE_CAS: u16 = 0x1853;
/// Hearing Access
pub const UUID_SERVICE_HAS: u16 = 0x1854;
/// Telephony and Media Audio
pub const UUID_SERVICE_TMAS: u16 = 0x1855;
/// Gaming Audio
pub const UUID_SERVICE_GMAS: u16 = 0x1858;

// ---- Characteristics ----
pub const UUID_CHR_DEVICE_NAME: u16 = 0x2A00;
pub const UUID_CHR_APPEARANCE: u16 = 0x2A01;
pub const UUID_CHR_SERVICE_CHANGED: u16 = 0x2A05;
pub const UUID_CHR_DATABASE_HASH: u16 = 0x2B2A;
pub const UUID_CHR_SINK_ASE: u16 = 0x2BC4;
pub const UUID_CHR_SOURCE_ASE: u16 = 0x2BC5;
pub const UUID_CHR_ASE_CONTROL_POINT: u16 = 0x2BC6;
pub const UUID_CHR_SINK_PAC: u16 = 0x2BC9;
pub const UUID_CHR_SINK_AUDIO_LOCATIONS: u16 = 0x2BCA;
pub const UUID_CHR_SOURCE_PAC: u16 = 0x2BCB;
pub const UUID_CHR_SOURCE_AUDIO_LOCATIONS: u16 = 0x2BCC;
pub const UUID_CHR_AVAILABLE_AUDIO_CONTEXTS: u16 = 0x2BCD;
pub const UUID_CHR_SUPPORTED_AUDIO_CONTEXTS: u16 = 0x2BCE;
/// Content Control ID (MCS / TBS)
pub const UUID_CHR_CCID: u16 = 0x2BBA;
/// TMAS, uint16
pub const UUID_CHR_TMAP_ROLE: u16 = 0x2B51;
/// GMAS, uint8
pub const UUID_CHR_GMAP_ROLE: u16 = 0x2C00;
pub const UUID_CHR_UGG_FEATURES: u16 = 0x2C01;
pub const UUID_CHR_UGT_FEATURES: u16 = 0x2C02;

pub const UUID_DESC_CCC: u16 = 0x2902;

// ---- Advertising data types used by the scan filter ----
pub const AD_FLAGS: u8 = 0x01;
pub const AD_UUID16_INCOMPLETE: u8 = 0x02;
pub const AD_UUID16_COMPLETE: u8 = 0x03;
pub const AD_NAME_SHORT: u8 = 0x08;
pub const AD_NAME_COMPLETE: u8 = 0x09;
pub const AD_SERVICE_DATA_UUID16: u8 = 0x16;
pub const AD_APPEARANCE: u8 = 0x19;
/// Resolvable Set Identifier (CSIP)
pub const AD_RSI: u8 = 0x2E;

// ---- Audio locations (Assigned Numbers 6.12.1) ----
pub const LOCATION_MONO: u32 = 0x0000_0000;
pub const LOCATION_FRONT_LEFT: u32 = 0x0000_0001;
pub const LOCATION_FRONT_RIGHT: u32 = 0x0000_0002;

// ---- Context types (Assigned Numbers 6.12.3) ----
pub const CONTEXT_UNSPECIFIED: u16 = 0x0001;
pub const CONTEXT_CONVERSATIONAL: u16 = 0x0002;
pub const CONTEXT_MEDIA: u16 = 0x0004;
pub const CONTEXT_GAME: u16 = 0x0008;

// ---- Content Control IDs of the PC's content control services ----
/// MCP: Generic Media Control Service (the value of its CCID
/// characteristic, and what `add_ccids` reports for a media stream)
pub const CCID_GMCS: u8 = 0x01;
/// CCP: Generic Telephone Bearer Service
pub const CCID_GTBS: u8 = 0x02;

// ---- Coding formats ----
pub const CODING_FORMAT_LC3: u8 = 0x06;

// ---- Codec Specific Capabilities LTV types (PAC records) ----
/// uint16 bitfield
pub const CAP_SAMPLING_FREQUENCIES: u8 = 0x01;
/// uint8 bitfield
pub const CAP_FRAME_DURATIONS: u8 = 0x02;
/// uint8 bitfield, bit n = n+1 channels
pub const CAP_CHANNEL_COUNTS: u8 = 0x03;
/// uint16 min, uint16 max
pub const CAP_OCTETS_PER_FRAME: u8 = 0x04;
/// uint8
pub const CAP_MAX_FRAMES_PER_SDU: u8 = 0x05;

// Supported Sampling Frequencies bits
pub const SF_BIT_8000: u16 = 1 << 0;
pub const SF_BIT_16000: u16 = 1 << 2;
pub const SF_BIT_24000: u16 = 1 << 4;
pub const SF_BIT_32000: u16 = 1 << 5;
pub const SF_BIT_44100: u16 = 1 << 6;
pub const SF_BIT_48000: u16 = 1 << 7;
// Supported Frame Durations bits
pub const FD_BIT_7500US: u8 = 1 << 0;
pub const FD_BIT_10000US: u8 = 1 << 1;

// ---- Codec Specific Configuration LTV types (ASE Config Codec) ----
/// uint8 code (SF_CODE_*)
pub const CFG_SAMPLING_FREQUENCY: u8 = 0x01;
/// 0x00 7.5 ms, 0x01 10 ms
pub const CFG_FRAME_DURATION: u8 = 0x02;
/// uint32 audio locations
pub const CFG_CHANNEL_ALLOCATION: u8 = 0x03;
/// uint16
pub const CFG_OCTETS_PER_FRAME: u8 = 0x04;
/// uint8
pub const CFG_FRAME_BLOCKS_PER_SDU: u8 = 0x05;

// Sampling Frequency configuration codes
pub const SF_CODE_8000: u8 = 0x01;
pub const SF_CODE_16000: u8 = 0x03;
pub const SF_CODE_24000: u8 = 0x05;
pub const SF_CODE_32000: u8 = 0x06;
pub const SF_CODE_44100: u8 = 0x07;
pub const SF_CODE_48000: u8 = 0x08;

// ---- Metadata LTV types ----
pub const META_PREFERRED_CONTEXTS: u8 = 0x01;
pub const META_STREAMING_CONTEXTS: u8 = 0x02;
pub const META_CCID_LIST: u8 = 0x05;

// ---- ASCS ----

/// ASE state (ASCS 1.0 Table 3.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AseState {
    Idle,
    CodecConfigured,
    QosConfigured,
    Enabling,
    Streaming,
    Disabling,
    Releasing,
    /// Not read yet, or a value ASCS does not define
    #[default]
    Unknown,
}

impl AseState {
    /// The state of an ASE characteristic value's second octet
    pub fn from_u8(value: u8) -> AseState {
        match value {
            0x00 => AseState::Idle,
            0x01 => AseState::CodecConfigured,
            0x02 => AseState::QosConfigured,
            0x03 => AseState::Enabling,
            0x04 => AseState::Streaming,
            0x05 => AseState::Disabling,
            0x06 => AseState::Releasing,
            _ => AseState::Unknown,
        }
    }

    /// Name for the log, e.g. "QoS Configured" (v1 ase_state_name)
    pub fn name(self) -> &'static str {
        match self {
            AseState::Idle => "Idle",
            AseState::CodecConfigured => "Codec Configured",
            AseState::QosConfigured => "QoS Configured",
            AseState::Enabling => "Enabling",
            AseState::Streaming => "Streaming",
            AseState::Disabling => "Disabling",
            AseState::Releasing => "Releasing",
            AseState::Unknown => "unknown",
        }
    }
}

// ASE Control Point opcodes
pub const ASE_OP_CONFIG_CODEC: u8 = 0x01;
pub const ASE_OP_CONFIG_QOS: u8 = 0x02;
pub const ASE_OP_ENABLE: u8 = 0x03;
pub const ASE_OP_RECEIVER_START_READY: u8 = 0x04;
pub const ASE_OP_DISABLE: u8 = 0x05;
pub const ASE_OP_RECEIVER_STOP_READY: u8 = 0x06;
pub const ASE_OP_UPDATE_METADATA: u8 = 0x07;
pub const ASE_OP_RELEASE: u8 = 0x08;

// ASCS Target_Latency (Config Codec)
pub const TARGET_LATENCY_LOW: u8 = 0x01;
pub const TARGET_LATENCY_BALANCED: u8 = 0x02;
pub const TARGET_LATENCY_HIGH_RELIABILITY: u8 = 0x03;
// PHY bits (Target_PHY uses 0x01 1M, 0x02 2M, 0x03 Coded; QoS uses a bitfield)
pub const PHY_1M: u8 = 0x01;
pub const PHY_2M: u8 = 0x02;

// ---- TMAP / GMAP (v1 le_audio_tmap_gmap.h) ----

// TMAP Role bits (TMAP 1.0, TMAP Role characteristic)
/// Call Gateway
pub const TMAP_ROLE_CG: u16 = 1 << 0;
/// Call Terminal
pub const TMAP_ROLE_CT: u16 = 1 << 1;
/// Unicast Media Sender
pub const TMAP_ROLE_UMS: u16 = 1 << 2;
/// Unicast Media Receiver
pub const TMAP_ROLE_UMR: u16 = 1 << 3;
/// Broadcast Media Sender
pub const TMAP_ROLE_BMS: u16 = 1 << 4;
/// Broadcast Media Receiver
pub const TMAP_ROLE_BMR: u16 = 1 << 5;

// GMAP Role bits (GMAP 1.0, GMAP Role characteristic)
/// Unicast Game Gateway
pub const GMAP_ROLE_UGG: u8 = 1 << 0;
/// Unicast Game Terminal
pub const GMAP_ROLE_UGT: u8 = 1 << 1;
/// Broadcast Game Sender
pub const GMAP_ROLE_BGS: u8 = 1 << 2;
/// Broadcast Game Receiver
pub const GMAP_ROLE_BGR: u8 = 1 << 3;

// UGG Features bits
/// Several LC3 frames (channels) per SDU block
pub const UGG_FEATURE_MULTIPLEX: u8 = 1 << 0;
/// Sends 96 kbps (48_3 / 48_4)
pub const UGG_FEATURE_96KBPS_SRC: u8 = 1 << 1;
/// Receives >= 2 channels in separate CIS
pub const UGG_FEATURE_MULTISINK: u8 = 1 << 2;

// UGT Features bits
pub const UGT_FEATURE_SOURCE: u8 = 1 << 0;
pub const UGT_FEATURE_80KBPS_SRC: u8 = 1 << 1;
pub const UGT_FEATURE_SINK: u8 = 1 << 2;
pub const UGT_FEATURE_64KBPS_SINK: u8 = 1 << 3;
pub const UGT_FEATURE_MULTIPLEX: u8 = 1 << 4;
pub const UGT_FEATURE_MULTISINK: u8 = 1 << 5;
pub const UGT_FEATURE_MULTISOURCE: u8 = 1 << 6;

/// What the PC's TMAS says
pub const PC_TMAP_ROLE: u16 = TMAP_ROLE_UMS;
/// What the PC's GMAS says
pub const PC_GMAP_ROLE: u8 = GMAP_ROLE_UGG;
pub const PC_UGG_FEATURES: u8 = UGG_FEATURE_MULTIPLEX | UGG_FEATURE_96KBPS_SRC;

// ---- Sampling frequencies ----

/// LC3 sampling frequency code (SF_CODE_*) to Hz; 0 when unknown
pub fn sf_code_to_hz(code: u8) -> u32 {
    match code {
        SF_CODE_8000 => 8000,
        0x02 => 11025,
        SF_CODE_16000 => 16000,
        0x04 => 22050,
        SF_CODE_24000 => 24000,
        SF_CODE_32000 => 32000,
        SF_CODE_44100 => 44100,
        SF_CODE_48000 => 48000,
        _ => 0,
    }
}

/// Hz to the sampling frequency code (SF_CODE_*); 0 when unknown
pub fn sf_hz_to_code(hz: u32) -> u8 {
    match hz {
        8000 => SF_CODE_8000,
        16000 => SF_CODE_16000,
        24000 => SF_CODE_24000,
        32000 => SF_CODE_32000,
        44100 => SF_CODE_44100,
        48000 => SF_CODE_48000,
        _ => 0,
    }
}

/// Hz to the Supported Sampling Frequencies bit (SF_BIT_*); 0 when unknown
pub fn sf_hz_to_cap_bit(hz: u32) -> u16 {
    match hz {
        8000 => SF_BIT_8000,
        16000 => SF_BIT_16000,
        24000 => SF_BIT_24000,
        32000 => SF_BIT_32000,
        44100 => SF_BIT_44100,
        48000 => SF_BIT_48000,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ase_states() {
        for v in 0..=6u8 {
            assert_ne!(AseState::from_u8(v), AseState::Unknown);
        }
        assert_eq!(AseState::from_u8(0x02), AseState::QosConfigured);
        assert_eq!(AseState::from_u8(0x02).name(), "QoS Configured");
        assert_eq!(AseState::from_u8(0x07), AseState::Unknown);
        assert_eq!(AseState::from_u8(0xFF).name(), "unknown");
    }

    #[test]
    fn sampling_frequencies() {
        for hz in [8000, 16000, 24000, 32000, 44100, 48000] {
            assert_eq!(sf_code_to_hz(sf_hz_to_code(hz)), hz);
            assert_ne!(sf_hz_to_cap_bit(hz), 0);
        }
        assert_eq!(sf_code_to_hz(0x02), 11025);
        assert_eq!(sf_code_to_hz(0x04), 22050);
        assert_eq!(sf_hz_to_code(11025), 0);
        assert_eq!(sf_hz_to_cap_bit(12000), 0);
        assert_eq!(sf_hz_to_cap_bit(48000), 0x80);
    }
}
