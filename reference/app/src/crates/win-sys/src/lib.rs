//! Win32 APIs for A2DPWB that no Rust library covers. The only crate besides
//! the `*-sys` crates where `unsafe` is allowed; every function here is
//! safe to call.

pub mod audio_policy;
pub mod autostart;
pub mod bt_paired;
pub mod console;
pub mod driver;
pub mod elevation;
pub mod locale;
pub mod media_keys;
pub mod mmcss;
pub mod notify;
pub mod pipe;
pub mod power;
pub mod process;
pub mod single_instance;
pub mod usb_adapters;
pub mod wlan;
