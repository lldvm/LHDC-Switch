//! The settings folder: %APPDATA%\A2DPWB, or A2DPWB_CONFIG_DIR (development
//! and tests: tools/emu pairs with virtual devices and must not touch the
//! user's real link keys). Shared with v1.

use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    let dir = match std::env::var_os("A2DPWB_CONFIG_DIR").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => match std::env::var_os("APPDATA") {
            Some(a) => PathBuf::from(a).join("A2DPWB"),
            None => PathBuf::from("."),
        },
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}
