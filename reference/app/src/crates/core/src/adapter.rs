// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! Which controller to open and how to bring it up: the Realtek chip for the
//! firmware download (setting, or the detected adapter as v1's "Auto"), the
//! Intel bootloader download and Broadcom PatchRAM (experimental), and the
//! AFH classification from the PC's Wi-Fi.

use std::path::Path;

use bt_host::afh;
use bt_host::chips::{self, ChipVendor};
use bt_host::{AfhClassification, AfhSource, HostConfig, TransportConfig};
use win_sys::usb_adapters::{self, UsbDevice};

use crate::settings::Settings;

/// A USB Bluetooth adapter as the UI and the stack setup see it
#[derive(Debug, Clone)]
pub struct Adapter {
    pub device: UsbDevice,
    /// Realtek PID for the firmware lookup (also for OEM adapters)
    pub realtek_pid: Option<u16>,
    pub vendor: ChipVendor,
    pub display_name: String,
}

/// Present USB devices that look like Bluetooth adapters (any driver).
/// `probe_winusb` opens WinUSB-bound ones for a moment (not while the stack
/// has the adapter open).
pub fn adapters(probe_winusb: bool) -> Vec<Adapter> {
    let ids = bt_host::realtek_usb_ids();
    let devices = match usb_adapters::enumerate(probe_winusb) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("cannot list USB devices: {e}");
            return Vec::new();
        }
    };
    devices
        .into_iter()
        .filter_map(|d| {
            let realtek_pid = chips::resolve_realtek_pid(d.vid, d.pid, &ids);
            let vendor = chips::vendor_for_usb(d.vid, d.pid, realtek_pid);
            let bluetooth =
                d.looks_like_bluetooth() || realtek_pid.is_some() || chips::is_bluetooth_vendor_vid(d.vid, d.pid);
            bluetooth.then(|| Adapter {
                display_name: chips::display_name(d.vid, d.pid, realtek_pid, vendor),
                device: d,
                realtek_pid,
                vendor,
            })
        })
        .collect()
}

/// Vendor-class (FF/01/01) controllers v1's WinUSB transport knew built in
/// (BTstack's list): they do not identify as Bluetooth class
const BUILTIN_VENDOR_CLASS: &[(u16, u16)] = &[
    (0x0A5C, 0x21E8), // BCM20702A0, DeLOCK Bluetooth 4.0
    (0x0B05, 0x17CB), // BCM20702A0, ASUS BT400
    (0x0A5C, 0x22BE), // BCM20702B0, generic class 1
    (0x2FE3, 0x0100), // nRF5x Zephyr USB HCI
    (0x2FE3, 0x000B),
];

/// VID:PIDs the USB transport opens besides Bluetooth-class devices (bt-host
/// adds BTstack's Realtek table, CSR and Intel): v1's built-in vendor-class
/// list, the OEM adapters with a Realtek chip, and the WinUSB adapters whose
/// interface 0 was found to be vendor-class Bluetooth with the HCI pipes
pub fn usb_known_ids(vendor_class: &[(u16, u16)]) -> Vec<(u16, u16)> {
    let mut ids: Vec<(u16, u16)> = BUILTIN_VENDOR_CLASS.to_vec();
    ids.extend(chips::OEM_TABLE.iter().map(|m| (m.vid, m.pid)));
    for id in vendor_class {
        if !ids.contains(id) {
            ids.push(*id);
        }
    }
    ids
}

/// Fills the chipset part of `config` from the settings and the present
/// adapters
pub fn configure(config: &mut HostConfig, settings: &Settings, _config_dir: &Path) {
    config.afh = Some(Box::new(WifiAfh::default()));
    if matches!(config.transport, TransportConfig::Tcp(_)) {
        return; // a virtual controller: no chipset, no firmware
    }
    let auto = chips::is_auto_chip(settings.bt_chip_pid, &settings.bt_chip_fw_stem);
    // The stack is not running yet (start and restart shut the old one
    // down first): the probe reads interface 0 of each WinUSB adapter, as
    // v1 did before opening one, and tells one another program holds
    let present = adapters(true);
    let winusb: Vec<&Adapter> =
        present.iter().filter(|a| a.device.driver_kind == win_sys::driver::DriverKind::WinUsb).collect();
    if let TransportConfig::Usb(sel) = &mut config.transport {
        let vendor_class: Vec<(u16, u16)> = winusb
            .iter()
            .filter(|a| a.device.winusb_bluetooth() == Some(true))
            .map(|a| (a.device.vid, a.device.pid))
            .collect();
        for (vid, pid) in &vendor_class {
            tracing::info!("vendor-class (FF/01/01) adapter {vid:04X}:{pid:04X}: the transport may open it");
        }
        for id in usb_known_ids(&vendor_class) {
            if !sel.known.contains(&id) {
                sel.known.push(id);
            }
        }
    }
    let pid =
        chips::resolve_chip_pid(settings.bt_chip_pid, &settings.bt_chip_fw_stem, winusb.iter().map(|a| a.realtek_pid));
    if pid != 0 {
        tracing::info!(
            "chip: {} (0x{pid:04X}){}",
            chips::realtek_chip_name(pid).unwrap_or("unknown"),
            if auto { " (auto)" } else { "" }
        );
        config.realtek_pid = pid;
    } else {
        tracing::info!(
            "no Realtek chip {}, skipping the Realtek chipset init",
            if auto { "detected" } else { "selected" }
        );
    }
    config.realtek_files = chips::firmware_files(pid, &settings.bt_chip_fw_stem).map(|(fw, cfg)| {
        let stem = |s: String| s.strip_suffix(".bin").map(str::to_owned).unwrap_or(s);
        (stem(fw), stem(cfg))
    });
    if pid == 0 {
        // Other vendors (experimental): Intel needs its bootloader
        // firmware, Broadcom may take a PatchRAM
        // One held by another program cannot be loaded either (v1 skipped
        // it too)
        config.intel_loader = winusb.iter().any(|a| a.vendor == ChipVendor::Intel && !a.device.in_use());
        for a in winusb.iter().filter(|a| a.vendor == ChipVendor::Intel && a.device.in_use()) {
            tracing::info!(
                "Intel adapter {:04X}:{:04X} is in use by another program: no bootloader download for it",
                a.device.vid,
                a.device.pid
            );
        }
        config.broadcom_ids =
            winusb.iter().filter(|a| a.vendor == ChipVendor::Broadcom).map(|a| (a.device.vid, a.device.pid)).collect();
        for a in &winusb {
            if matches!(a.vendor, ChipVendor::MediaTek | ChipVendor::Qualcomm) {
                tracing::warn!(
                    "{} adapter {:04X}:{:04X}: firmware loading for this vendor is not implemented; it may not work",
                    a.vendor.name(),
                    a.device.vid,
                    a.device.pid
                );
            }
        }
    }
}

/// AFH host channel classification from the 2.4 GHz channel this PC's Wi-Fi
/// is connected on ("auto")
#[derive(Default)]
struct WifiAfh {
    connected: Vec<u8>,
    task: Option<std::thread::JoinHandle<Vec<u8>>>,
    next_survey: Option<std::time::Instant>,
}

impl AfhSource for WifiAfh {
    fn classify(&mut self, policy: &str) -> Option<AfhClassification> {
        // WLAN API calls may block for tens of milliseconds. Never stall
        // the Bluetooth send thread to gather optional Wi-Fi diagnostics.
        if self.task.as_ref().is_some_and(|t| t.is_finished()) {
            self.connected = self.task.take().and_then(|t| t.join().ok()).unwrap_or_default();
        }
        let now = std::time::Instant::now();
        if policy == "auto" && self.task.is_none() && self.next_survey.is_none_or(|next| now >= next) {
            self.next_survey = Some(now + std::time::Duration::from_secs(30));
            self.task = std::thread::Builder::new().name("wifi-afh".into()).spawn(|| {
                match win_sys::wlan::survey() {
                    Ok(s) => s.connected_channels_24ghz(),
                    Err(e) => { tracing::debug!("Wi-Fi survey failed: {e}"); Vec::new() }
                }
            }).ok();
        }
        let c = afh::classify(policy, || self.connected.clone());
        c.passed.then(|| AfhClassification {
            map: c.map,
            usable: c.usable as u32,
            avoided: afh::describe(&c.avoided_wifi),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use win_sys::driver::{DriverInfo, DriverKind};
    use win_sys::usb_adapters::{InterfaceProbe, UsbClass};

    fn device(vid: u16, pid: u16, probe: Option<Result<InterfaceProbe, String>>) -> UsbDevice {
        UsbDevice {
            instance_id: format!(r"USB\VID_{vid:04X}&PID_{pid:04X}\1"),
            device_path: String::new(),
            vid,
            pid,
            name: String::new(),
            bus_reported_name: None,
            manufacturer: String::new(),
            hardware_ids: Vec::new(),
            compatible_ids: Vec::new(),
            usb_class: None,
            driver: DriverInfo::default(),
            driver_kind: DriverKind::WinUsb,
            problem_code: None,
            probe,
        }
    }

    #[test]
    fn known_ids_cover_v1s_vendor_class_adapters() {
        let ids = usb_known_ids(&[]);
        // v1's built-in list (BTstack's WinUSB transport)
        for id in [(0x0A5C, 0x21E8), (0x0B05, 0x17CB), (0x0A5C, 0x22BE), (0x2FE3, 0x0100), (0x2FE3, 0x000B)] {
            assert!(ids.contains(&id), "{id:04X?}");
        }
        // The OEM table (TP-Link UB500)
        assert!(ids.contains(&(0x2357, 0x0604)));
        // An enumerated vendor-class adapter, once
        let ids = usb_known_ids(&[(0x1234, 0x5678), (0x0B05, 0x17CB)]);
        assert_eq!(ids.iter().filter(|&&id| id == (0x1234, 0x5678)).count(), 1);
        assert_eq!(ids.iter().filter(|&&id| id == (0x0B05, 0x17CB)).count(), 1);
    }

    #[test]
    fn probe_tells_vendor_class_and_in_use() {
        let vendor = InterfaceProbe { class: UsbClass::VENDOR_BLUETOOTH, num_endpoints: 3, hci_endpoints: true };
        assert_eq!(device(0x1234, 0x5678, Some(Ok(vendor.clone()))).winusb_bluetooth(), Some(true));
        let no_pipes = InterfaceProbe { hci_endpoints: false, ..vendor };
        assert_eq!(device(0x1234, 0x5678, Some(Ok(no_pipes))).winusb_bluetooth(), None);
        assert!(device(0x8087, 0x0029, Some(Err("open failed (error 5)".into()))).in_use());
        assert!(!device(0x8087, 0x0029, Some(Err("open failed (error 2)".into()))).in_use());
        assert!(!device(0x8087, 0x0029, None).in_use());
    }
}
