//! The Wi-Fi around this PC, for AFH (the WLAN API part of v1 afh.cpp)
//!
//! Lists the access points Windows' own background scans found (v1
//! afh_survey_wifi), and which channel each connected interface is on: AFH
//! "auto" avoids only that channel, when it is a 2.4 GHz one. No scan is
//! requested: a scan would put more traffic on the
//! very band being measured. Deciding which Bluetooth channels to avoid is
//! bt-host's afh module; this returns plain data.

use std::ffi::c_void;

use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::NetworkManagement::WiFi::{
    DOT11_PHY_TYPE, WLAN_BSS_ENTRY, WLAN_BSS_LIST, WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO,
    WLAN_INTERFACE_INFO_LIST, WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanGetNetworkBssList,
    WlanOpenHandle, WlanQueryInterface, dot11_BSS_type_any, dot11_phy_type_dsss, dot11_phy_type_erp,
    dot11_phy_type_hrdsss, dot11_phy_type_ht, wlan_interface_state_connected, wlan_intf_opcode_channel_number,
    wlan_intf_opcode_current_connection,
};

/// One access point (BSS) a Wi-Fi interface heard
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiBss {
    pub ssid: String,
    pub bssid: [u8; 6],
    pub rssi_dbm: i32,
    /// Center frequency of its channel
    pub frequency_mhz: u32,
    /// The 2.4 GHz channel number (1-14), None for 5 / 6 GHz
    pub channel_24ghz: Option<u8>,
    /// This PC's interface is associated with it
    pub connected: bool,
}

/// One Wi-Fi interface of this PC
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiInterface {
    pub description: String,
    pub connected: bool,
    /// BSSID of the access point it is associated with
    pub connected_bssid: Option<[u8; 6]>,
    /// The 2.4 GHz channel (1-14) it is connected on; None when it is
    /// disconnected, on 5 / 6 GHz, or the band cannot be told
    pub channel_24ghz: Option<u8>,
}

/// What `survey` found; both empty on a PC without Wi-Fi
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WifiSurvey {
    pub interfaces: Vec<WifiInterface>,
    pub bss: Vec<WifiBss>,
}

impl WifiSurvey {
    /// 2.4 GHz channels this PC's interfaces are connected on (what AFH
    /// "auto" avoids); empty when no interface is on 2.4 GHz
    pub fn connected_channels_24ghz(&self) -> Vec<u8> {
        let mut channels: Vec<u8> =
            self.interfaces.iter().filter(|i| i.connected).filter_map(|i| i.channel_24ghz).collect();
        channels.sort_unstable();
        channels.dedup();
        channels
    }
}

/// The 2.4 GHz channel a connected interface is on. `bss_mhz` is the center
/// frequency of its access point in the scan list, when listed: that tells
/// the band for sure. Otherwise the interface's channel number counts only
/// with a PHY that cannot be on 6 GHz (802.11b/g/n), since 6 GHz channels
/// 1-13 share their numbers with 2.4 GHz ones (802.11n on 5 GHz uses
/// channels 36 and up).
pub fn connected_channel_24ghz(bss_mhz: Option<u32>, channel_number: Option<u32>, phy: DOT11_PHY_TYPE) -> Option<u8> {
    if let Some(mhz) = bss_mhz {
        return channel_from_mhz(mhz);
    }
    let pre_6ghz_phy = [dot11_phy_type_dsss, dot11_phy_type_hrdsss, dot11_phy_type_erp, dot11_phy_type_ht];
    match channel_number {
        Some(n @ 1..=14) if pre_6ghz_phy.contains(&phy) => Some(n as u8),
        _ => None,
    }
}

/// 2.4 GHz channel number of a center frequency (v1's mapping: 2412-2472
/// MHz in 5 MHz steps are channels 1-13, 2484 MHz is channel 14)
pub fn channel_from_mhz(mhz: u32) -> Option<u8> {
    match mhz {
        2484 => Some(14),
        2412..=2472 if (mhz - 2407).is_multiple_of(5) => Some(((mhz - 2407) / 5) as u8),
        _ => None,
    }
}

/// The access points Windows knows of, from its last background scans.
/// A PC without Wi-Fi (or with the WLAN AutoConfig service stopped) gives
/// an empty survey, not an error, as in v1.
pub fn survey() -> windows::core::Result<WifiSurvey> {
    let mut negotiated = 0u32;
    let mut client = HANDLE::default();
    // SAFETY: client version 2 (Vista and later), valid out pointers.
    let err = unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut client) };
    if err != ERROR_SUCCESS.0 {
        tracing::debug!("wlan: WlanOpenHandle failed ({err}): no Wi-Fi");
        return Ok(WifiSurvey::default());
    }
    let client = Client(client);

    let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
    // SAFETY: open client handle, valid out pointer.
    let err = unsafe { WlanEnumInterfaces(client.0, None, &mut list) };
    if err != ERROR_SUCCESS.0 {
        return Err(windows::Win32::Foundation::WIN32_ERROR(err).into());
    }
    let list = WlanMemory(list.cast());
    // SAFETY: WlanEnumInterfaces succeeded, so `list` points to a list
    // whose dwNumberOfItems entries follow in one allocation.
    let interfaces: &[WLAN_INTERFACE_INFO] = unsafe {
        let l = list.0.cast::<WLAN_INTERFACE_INFO_LIST>();
        std::slice::from_raw_parts(std::ptr::addr_of!((*l).InterfaceInfo).cast(), (*l).dwNumberOfItems as usize)
    };

    let mut result = WifiSurvey::default();
    for intf in interfaces {
        let connected = intf.isState == wlan_interface_state_connected;
        let association = if connected { association(&client, &intf.InterfaceGuid) } else { None };
        let connected_bssid = association.map(|(bssid, _)| bssid);
        let bss = bss_list(&client, &intf.InterfaceGuid, connected_bssid);
        let channel_24ghz = association.and_then(|(_, phy)| {
            let bss_mhz = bss.iter().find(|b| b.connected).map(|b| b.frequency_mhz);
            let number = if bss_mhz.is_none() { channel_number(&client, &intf.InterfaceGuid) } else { None };
            connected_channel_24ghz(bss_mhz, number, phy)
        });
        result.interfaces.push(WifiInterface {
            description: crate::autostart::utf16_string(&intf.strInterfaceDescription),
            connected,
            connected_bssid,
            channel_24ghz,
        });
        result.bss.extend(bss);
    }
    Ok(result)
}

/// The access points an interface's background scans found
fn bss_list(client: &Client, interface: &windows::core::GUID, connected_bssid: Option<[u8; 6]>) -> Vec<WifiBss> {
    let mut bss: *mut WLAN_BSS_LIST = std::ptr::null_mut();
    // SAFETY: open client, a GUID from the enumeration, no SSID filter,
    // valid out pointer.
    let err = unsafe { WlanGetNetworkBssList(client.0, interface, None, dot11_BSS_type_any, false, None, &mut bss) };
    if err != ERROR_SUCCESS.0 {
        tracing::debug!("wlan: WlanGetNetworkBssList failed ({err})");
        return Vec::new();
    }
    let bss = WlanMemory(bss.cast());
    // SAFETY: as in `survey`, dwNumberOfItems entries follow the header.
    let entries: &[WLAN_BSS_ENTRY] = unsafe {
        let l = bss.0.cast::<WLAN_BSS_LIST>();
        std::slice::from_raw_parts(std::ptr::addr_of!((*l).wlanBssEntries).cast(), (*l).dwNumberOfItems as usize)
    };
    entries
        .iter()
        .map(|e| {
            let frequency_mhz = e.ulChCenterFrequency / 1000; // reported in kHz
            let ssid_len = (e.dot11Ssid.uSSIDLength as usize).min(e.dot11Ssid.ucSSID.len());
            WifiBss {
                ssid: String::from_utf8_lossy(&e.dot11Ssid.ucSSID[..ssid_len]).into_owned(),
                bssid: e.dot11Bssid,
                rssi_dbm: e.lRssi,
                frequency_mhz,
                channel_24ghz: channel_from_mhz(frequency_mhz),
                connected: connected_bssid == Some(e.dot11Bssid),
            }
        })
        .collect()
}

/// BSSID and PHY of the access point a connected interface is associated
/// with
fn association(client: &Client, interface: &windows::core::GUID) -> Option<([u8; 6], DOT11_PHY_TYPE)> {
    let mut size = 0u32;
    let mut data: *mut c_void = std::ptr::null_mut();
    // SAFETY: open client, valid GUID and out pointers.
    let err = unsafe {
        WlanQueryInterface(client.0, interface, wlan_intf_opcode_current_connection, None, &mut size, &mut data, None)
    };
    if err != ERROR_SUCCESS.0 || data.is_null() {
        return None;
    }
    let data = WlanMemory(data);
    if (size as usize) < size_of::<WLAN_CONNECTION_ATTRIBUTES>() {
        return None;
    }
    // SAFETY: the opcode returns a WLAN_CONNECTION_ATTRIBUTES of `size`
    // bytes, checked above.
    let attrs = unsafe { &*data.0.cast::<WLAN_CONNECTION_ATTRIBUTES>() };
    Some((attrs.wlanAssociationAttributes.dot11Bssid, attrs.wlanAssociationAttributes.dot11PhyType))
}

/// The channel number a connected interface is on (no band: see
/// `connected_channel_24ghz`)
fn channel_number(client: &Client, interface: &windows::core::GUID) -> Option<u32> {
    let mut size = 0u32;
    let mut data: *mut c_void = std::ptr::null_mut();
    // SAFETY: open client, valid GUID and out pointers.
    let err = unsafe {
        WlanQueryInterface(client.0, interface, wlan_intf_opcode_channel_number, None, &mut size, &mut data, None)
    };
    if err != ERROR_SUCCESS.0 || data.is_null() {
        return None;
    }
    let data = WlanMemory(data);
    if (size as usize) < size_of::<u32>() {
        return None;
    }
    // SAFETY: the opcode returns a ULONG of `size` bytes, checked above.
    Some(unsafe { data.0.cast::<u32>().read_unaligned() })
}

/// A WLAN client handle, closed on drop
struct Client(HANDLE);

impl Drop for Client {
    fn drop(&mut self) {
        // SAFETY: opened by WlanOpenHandle, closed only here.
        unsafe { WlanCloseHandle(self.0, None) };
    }
}

/// Memory the WLAN API allocated, freed on drop
struct WlanMemory(*mut c_void);

impl Drop for WlanMemory {
    fn drop(&mut self) {
        // SAFETY: allocated by the WLAN API and freed only here.
        unsafe { WlanFreeMemory(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_mapping_matches_v1() {
        assert_eq!(channel_from_mhz(2412), Some(1));
        assert_eq!(channel_from_mhz(2437), Some(6));
        assert_eq!(channel_from_mhz(2462), Some(11));
        assert_eq!(channel_from_mhz(2472), Some(13));
        assert_eq!(channel_from_mhz(2484), Some(14));
        assert_eq!(channel_from_mhz(2414), None);
        assert_eq!(channel_from_mhz(5180), None);
        assert_eq!(channel_from_mhz(5955), None); // 6 GHz channel 1
    }

    #[test]
    fn connected_channel_prefers_the_listed_access_point() {
        use windows::Win32::NetworkManagement::WiFi::{dot11_phy_type_eht, dot11_phy_type_he, dot11_phy_type_vht};
        // Listed: its frequency decides, whatever the channel number says
        assert_eq!(connected_channel_24ghz(Some(2462), Some(36), dot11_phy_type_he), Some(11));
        assert_eq!(connected_channel_24ghz(Some(5180), Some(36), dot11_phy_type_ht), None);
        assert_eq!(connected_channel_24ghz(Some(5955), Some(1), dot11_phy_type_he), None); // 6 GHz ch 1
        // Not listed: the channel number, with a PHY that is never on 6 GHz
        assert_eq!(connected_channel_24ghz(None, Some(6), dot11_phy_type_ht), Some(6));
        assert_eq!(connected_channel_24ghz(None, Some(1), dot11_phy_type_erp), Some(1));
        assert_eq!(connected_channel_24ghz(None, Some(36), dot11_phy_type_ht), None);
        // 802.11ax / be on channel 1-13 may be 6 GHz: not taken
        assert_eq!(connected_channel_24ghz(None, Some(5), dot11_phy_type_he), None);
        assert_eq!(connected_channel_24ghz(None, Some(5), dot11_phy_type_eht), None);
        assert_eq!(connected_channel_24ghz(None, Some(149), dot11_phy_type_vht), None);
        assert_eq!(connected_channel_24ghz(None, None, dot11_phy_type_ht), None);
    }

    #[test]
    fn connected_channels_come_from_connected_interfaces_only() {
        let interface = |connected, channel_24ghz| WifiInterface {
            description: String::new(),
            connected,
            connected_bssid: None,
            channel_24ghz,
        };
        let loud_ap_on_11 = WifiBss {
            ssid: String::new(),
            bssid: [0; 6],
            rssi_dbm: -30,
            frequency_mhz: 2462,
            channel_24ghz: Some(11),
            connected: false,
        };
        // Disconnected (2026-10-17 case), or connected on 5 GHz, with a loud
        // access point on 11 heard: nothing
        for intf in [interface(false, None), interface(true, None)] {
            let s = WifiSurvey { interfaces: vec![intf], bss: vec![loud_ap_on_11.clone()] };
            assert!(s.connected_channels_24ghz().is_empty());
        }
        // Connected on 6; two interfaces on the same channel count once
        let s = WifiSurvey {
            interfaces: vec![interface(true, Some(6)), interface(true, Some(6)), interface(false, Some(1))],
            bss: vec![loud_ap_on_11],
        };
        assert_eq!(s.connected_channels_24ghz(), [6]);
    }

    #[test]
    fn survey_runs() {
        // Read-only: the background scan results; empty without Wi-Fi
        let s = survey().unwrap();
        for b in &s.bss {
            assert_eq!(b.channel_24ghz, channel_from_mhz(b.frequency_mhz));
        }
        eprintln!(
            "{} interface(s), {} BSS, connected 2.4 GHz {:?}",
            s.interfaces.len(),
            s.bss.len(),
            s.connected_channels_24ghz()
        );
    }
}
