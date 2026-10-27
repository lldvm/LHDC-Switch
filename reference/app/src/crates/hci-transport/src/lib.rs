//! HCI transports for BTstack ([`btstack_sys::HciBackend`]):
//!
//! - [`usb::UsbTransport`]: a USB Bluetooth controller bound to WinUSB,
//!   driven through `nusb` (HCI over USB, "H2")
//! - [`tcp::TcpH4`]: H4 over TCP to a virtual controller (tools/emu)
//!
//! Both deliver whole HCI packets: they reassemble events and ACL packets
//! that arrive split over several transfers or reads.

#![forbid(unsafe_code)]

pub mod reassembly;
pub mod tcp;
pub mod usb;

pub use tcp::TcpH4;
pub use usb::{UsbCandidate, UsbSelector, UsbTransport};

/// The USB ACL writer raises its priority (MMCSS "Pro Audio" on Windows,
/// as the encode thread) through a hook core installs, so this crate stays
/// free of Win32. What the hook returns is dropped when the thread ends.
pub type ThreadPriorityHook = fn() -> Box<dyn std::any::Any>;

static ACL_OUT_PRIORITY_HOOK: std::sync::OnceLock<ThreadPriorityHook> = std::sync::OnceLock::new();

pub fn set_acl_out_priority_hook(hook: ThreadPriorityHook) {
    let _ = ACL_OUT_PRIORITY_HOOK.set(hook);
}

fn acl_out_priority() -> Option<Box<dyn std::any::Any>> {
    ACL_OUT_PRIORITY_HOOK.get().map(|h| h())
}
