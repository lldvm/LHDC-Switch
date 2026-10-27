//! settings.json, read and written in v1's format. Keys v2 does not know are
//! kept (written back as they were), so going back to v1 loses nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use bt_host::media::{MEDIA_PAYLOAD_LIMIT_DEFAULT, clamp_media_payload_limit};

/// settings.json's format: 1 is v1's (which wrote no schema_version), 2 is
/// v2's, 3 has LE Audio on by default (see migrate.rs)
pub const SCHEMA_VERSION: u32 = 3;
/// The first schema_version v2 wrote: a lower one is v1's folder
pub const FIRST_V2_SCHEMA: u32 = 2;

fn v1_schema() -> u32 {
    1
}
/// The languages of app/lang
pub const LANGUAGES: [&str; 8] = ["en", "ja", "de", "fr", "es", "ko", "zh-CN", "zh-TW"];

/// One of LANGUAGES for a BCP 47 name ("ja-JP", "zh-Hant-HK", "pt-BR"):
/// None when the app has no strings for it
pub fn app_language_for(tag: &str) -> Option<&'static str> {
    let tag = tag.trim().to_ascii_lowercase().replace('_', "-");
    let mut parts = tag.split('-');
    let primary = parts.next().unwrap_or_default();
    if primary == "zh" {
        // Traditional script, or a region that writes it, unless the script
        // says otherwise
        let rest: Vec<&str> = parts.collect();
        let traditional = if rest.contains(&"hant") {
            true
        } else if rest.contains(&"hans") {
            false
        } else {
            rest.iter().any(|r| matches!(*r, "tw" | "hk" | "mo"))
        };
        return Some(if traditional { "zh-TW" } else { "zh-CN" });
    }
    LANGUAGES.iter().copied().find(|l| *l == primary)
}

/// A new settings file's language: the first of the user's Windows display
/// languages the app has, else English (v1's installer wrote the setup's
/// language; this also covers the portable copy)
fn default_language() -> String {
    pick_language(&win_sys::locale::preferred_ui_languages()).into()
}

fn pick_language(tags: &[String]) -> &'static str {
    tags.iter().find_map(|t| app_language_for(t)).unwrap_or("en")
}
fn default_theme() -> String {
    "dark".into()
}
fn default_true() -> bool {
    true
}
fn default_afh() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Missing in v1's files
    #[serde(default = "v1_schema")]
    pub schema_version: u32,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_theme")]
    pub theme: String,

    pub start_with_windows: bool,
    pub start_minimized: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,

    #[serde(default = "default_true")]
    pub check_updates_on_startup: bool,
    /// ISO date of the last automatic check
    pub last_update_check: String,
    /// Also offer releases marked as pre-release on GitHub (beta channel)
    pub include_prereleases: bool,

    /// The headphones' buttons act as Windows media keys
    #[serde(default = "default_true")]
    pub media_keys: bool,
    /// Remember the headphones' volume per device (address -> AVRCP volume
    /// 0..127) and set it again when they connect
    pub remember_volume: bool,
    pub saved_volumes: BTreeMap<String, i32>,

    pub debug_mode: bool,

    /// Realtek chip for the firmware download: 0 = auto; a Realtek PID for
    /// OEM adapters whose VID is not Realtek's
    pub bt_chip_pid: u16,
    /// Firmware stem for chips not in the chip table (e.g. "rtl8761bu")
    pub bt_chip_fw_stem: String,

    /// AFH host channel classification: "auto", "off" or Wi-Fi channels "6,11"
    #[serde(default = "default_afh")]
    pub afh: String,

    /// Peer receiver test: measuring receiver, host[:port] or "auto"
    pub remote_sink: String,

    pub last_profile: String,

    /// LE Audio (experimental): BTstack starts with LE (SM, GATT, ISO) and
    /// LE Audio profiles can stream. On by default since 2.0.1 (schema 3):
    /// choosing an LE Audio device is the user's call.
    pub le_audio_experimental: bool,
    /// Before 2.0.1: GMAP low latency for every LE Audio profile. Now a
    /// profile option (`le_low_latency`); kept only so the file round-trips.
    pub le_audio_low_latency: bool,

    /// Before v1.1 the max media packet size was a setting; it now seeds
    /// profiles saved without one. Read, not written.
    #[serde(skip_serializing)]
    pub max_media_payload: Option<i64>,

    /// Keys of other versions, kept as they are
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_json::Value>,
}

/// "dark", "light" or "system"; anything else is dark, as v1 read it
pub fn normalize_theme(theme: &str) -> &'static str {
    match theme {
        "light" => "light",
        "system" => "system",
        _ => "dark",
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema_version: SCHEMA_VERSION,
            language: default_language(),
            theme: default_theme(),
            start_with_windows: false,
            start_minimized: false,
            minimize_to_tray: true,
            check_updates_on_startup: true,
            last_update_check: String::new(),
            include_prereleases: false,
            media_keys: true,
            remember_volume: false,
            saved_volumes: BTreeMap::new(),
            debug_mode: false,
            bt_chip_pid: 0,
            bt_chip_fw_stem: String::new(),
            afh: default_afh(),
            remote_sink: String::new(),
            last_profile: String::new(),
            le_audio_experimental: true,
            le_audio_low_latency: false,
            max_media_payload: None,
            other: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join("settings.json")
    }

    /// Missing or unreadable: defaults (a broken file is kept as .bad)
    pub fn load(dir: &Path) -> Settings {
        let path = Settings::path(dir);
        let Ok(text) = std::fs::read_to_string(&path) else { return Settings::default() };
        match serde_json::from_str::<Settings>(&text) {
            Ok(mut s) => {
                s.saved_volumes.retain(|_, v| (0..=127).contains(v));
                s.theme = normalize_theme(&s.theme).into();
                s
            }
            Err(e) => {
                tracing::warn!("settings: {} is not valid ({e}), using defaults", path.display());
                let _ = std::fs::copy(&path, path.with_extension("json.bad"));
                Settings::default()
            }
        }
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let mut s = self.clone();
        s.schema_version = SCHEMA_VERSION;
        let text = serde_json::to_string_pretty(&s).map_err(std::io::Error::other)?;
        let path = Settings::path(dir);
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path)
    }

    /// The media packet size limit of a settings file from before v1.1, for
    /// profiles saved without one
    pub fn legacy_max_media_payload(&self) -> u16 {
        self.max_media_payload
            .map(|v| clamp_media_payload_limit(v.clamp(0, u32::MAX as i64) as u32))
            .unwrap_or(MEDIA_PAYLOAD_LIMIT_DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_from_the_windows_display_languages() {
        assert_eq!(app_language_for("ja-JP"), Some("ja"));
        assert_eq!(app_language_for("en-GB"), Some("en"));
        assert_eq!(app_language_for("de"), Some("de"));
        assert_eq!(app_language_for("fr-CA"), Some("fr"));
        assert_eq!(app_language_for("es-419"), Some("es"));
        assert_eq!(app_language_for("ko-KR"), Some("ko"));
        assert_eq!(app_language_for("zh-CN"), Some("zh-CN"));
        assert_eq!(app_language_for("zh-SG"), Some("zh-CN"));
        assert_eq!(app_language_for("zh-Hans-HK"), Some("zh-CN"));
        assert_eq!(app_language_for("zh-TW"), Some("zh-TW"));
        assert_eq!(app_language_for("zh-HK"), Some("zh-TW"));
        assert_eq!(app_language_for("zh-Hant-TW"), Some("zh-TW"));
        assert_eq!(app_language_for("pt-BR"), None);
        assert_eq!(app_language_for(""), None);
        let tags = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(pick_language(&tags(&["pt-BR", "fr-FR", "en-US"])), "fr");
        assert_eq!(pick_language(&tags(&["pt-BR"])), "en");
        assert_eq!(pick_language(&[]), "en");
        // A file without the key gets one of them
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(LANGUAGES.contains(&s.language.as_str()));
    }

    #[test]
    fn v1_file_round_trips_unknown_keys() {
        let v1 = r#"{"language":"ja","theme":"light","media_keys":false,"saved_volumes":{"AA:BB:CC:DD:EE:FF":64,"X":999},
                    "le_audio_experimental":true,"max_media_payload":1500,"window_x":10}"#;
        let s: Settings = serde_json::from_str(v1).unwrap();
        assert_eq!(s.language, "ja");
        assert!(!s.media_keys);
        assert!(s.minimize_to_tray);
        assert_eq!(s.legacy_max_media_payload(), 1500);
        assert!(s.le_audio_experimental);
        assert!(!s.le_audio_low_latency);
        assert!(!s.other.contains_key("le_audio_experimental"));
        let out = serde_json::to_value(&s).unwrap();
        assert_eq!(out["window_x"], 10);
        // v1 wrote both LE Audio keys
        assert_eq!(out["le_audio_experimental"], true);
        assert_eq!(out["le_audio_low_latency"], false);
        // v1 wrote no schema; save() writes the current one (migrate.rs)
        assert_eq!(out["schema_version"], 1);
        assert!(out.get("max_media_payload").is_none());
    }

    #[test]
    fn unknown_theme_is_dark_as_in_v1() {
        let dir = std::env::temp_dir().join(format!("a2dpwb-theme-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (v1, v2) in [("white", "dark"), ("light", "light"), ("system", "system"), ("", "dark")] {
            std::fs::write(Settings::path(&dir), format!(r#"{{"theme":"{v1}"}}"#)).unwrap();
            assert_eq!(Settings::load(&dir).theme, v2, "{v1}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
