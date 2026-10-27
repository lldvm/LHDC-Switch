// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! [`Stack`]: the safe BTstack API, usable on the run loop thread only

use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_int};
use std::marker::PhantomData;

use crate::{LINK_KEYS, LOGGER, LinkKeyStore, PacketLogger, ffi, transport};

/// HCI power state (BTstack's HCI_STATE)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HciState {
    Off,
    Initializing,
    Working,
    Halting,
    Sleeping,
    FallingAsleep,
}

/// State of the ACL link to a device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclState {
    None,
    /// Create Connection not finished (no connection handle yet)
    Paging,
    Open(u16),
    Other,
}

/// State of the AVDTP connection to a device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvdtpState {
    None,
    /// Signaling channel open: a disconnect is answered by
    /// A2DP_SUBEVENT_SIGNALING_CONNECTION_RELEASED
    Open,
    /// Not open yet: a disconnect frees it at once
    Pending,
}

const INIT_HCI: u32 = 1;
const INIT_L2CAP: u32 = 2;
const INIT_SDP: u32 = 4;
const INIT_A2DP: u32 = 8;
const INIT_AVRCP: u32 = 16;
/// LE: SM, GATT client, TLV store (Stack::le_init)
pub(crate) const INIT_LE: u32 = 32;
/// LE: the ATT server (Stack::att_server_init)
pub(crate) const INIT_ATT: u32 = 64;

thread_local! {
    static INITIALIZED: Cell<u32> = const { Cell::new(0) };
    /// Paths handed to BTstack, which keeps the pointers
    static PATHS: RefCell<Vec<CString>> = const { RefCell::new(Vec::new()) };
}

/// Proof of being on the BTstack thread inside [`crate::run`]. Not `Send`:
/// it cannot reach another thread.
pub struct Stack {
    _not_send: PhantomData<*mut ()>,
}

fn keep_path(path: &str) -> *const std::ffi::c_char {
    let c = CString::new(path.replace('\0', "")).unwrap_or_default();
    let p = c.as_ptr();
    PATHS.with(|v| v.borrow_mut().push(c));
    p
}

pub(crate) fn mark(flag: u32) {
    INITIALIZED.with(|i| i.set(i.get() | flag));
}

pub(crate) fn initialized(flag: u32) -> bool {
    INITIALIZED.with(|i| i.get() & flag != 0)
}

impl Stack {
    pub(crate) fn new() -> Stack {
        Stack { _not_send: PhantomData }
    }

    // ---- Setup (in the init closure of `run`) ----

    /// Installs the HCI transport and initializes HCI on it
    pub fn hci_init(&self, backend: Box<dyn crate::HciBackend>) {
        let is_async = transport::install(backend);
        log_le_hidden();
        // SAFETY: on the BTstack thread; the backend is installed, so the
        // transport table's callbacks have something to call.
        unsafe { ffi::a2dpwb_hci_init(is_async as c_int) };
        mark(INIT_HCI);
    }

    /// Installs the transport without initializing HCI (for a firmware
    /// loader that runs first, see [`Stack::intel_download_firmware`])
    pub fn install_transport(&self, backend: Box<dyn crate::HciBackend>) {
        transport::install(backend);
        log_le_hidden();
    }

    /// Initializes HCI on the transport installed with
    /// [`Stack::install_transport`]
    pub fn hci_init_installed(&self) {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dpwb_hci_init(transport::is_async() as c_int) };
        mark(INIT_HCI);
    }

    /// Link keys in `store` (None: in memory only)
    pub fn set_link_key_store(&self, store: Option<Box<dyn LinkKeyStore>>) {
        let rust = store.is_some();
        LINK_KEYS.with(|s| *s.borrow_mut() = store);
        // SAFETY: on the BTstack thread, after hci_init.
        unsafe { ffi::a2dpwb_set_link_key_db(rust as c_int) };
    }

    pub fn set_packet_logger(&self, logger: Option<PacketLogger>) {
        LOGGER.with(|l| *l.borrow_mut() = logger);
    }

    /// SSP Just Works, General Bonding, name "A2DPWB", role switch and
    /// sniff allowed
    pub fn gap_setup(&self, class_of_device: u32, page_timeout: u16) {
        // SAFETY: on the BTstack thread, after hci_init.
        unsafe { ffi::a2dpwb_gap_setup(class_of_device, page_timeout) };
    }

    pub fn l2cap_init(&self) {
        // SAFETY: on the BTstack thread, after hci_init.
        unsafe { ffi::l2cap_init() };
        mark(INIT_L2CAP);
    }

    pub fn sdp_init(&self) {
        // SAFETY: on the BTstack thread, after l2cap_init.
        unsafe { ffi::sdp_init() };
        mark(INIT_SDP);
    }

    /// HCI events go to the app ([`crate::Source::Hci`])
    pub fn add_hci_event_handler(&self) {
        // SAFETY: on the BTstack thread; the registration is static in C.
        unsafe { ffi::a2dpwb_add_hci_event_handler() };
    }

    pub fn a2dp_source_init(&self) {
        // SAFETY: on the BTstack thread, after l2cap / sdp init.
        unsafe { ffi::a2dpwb_a2dp_source_init() };
        mark(INIT_A2DP);
    }

    pub fn avrcp_init(&self) {
        // SAFETY: on the BTstack thread, after l2cap / sdp init.
        unsafe { ffi::a2dpwb_avrcp_init() };
        mark(INIT_AVRCP);
    }

    /// SDP records: A2DP source (player), AVRCP controller and target with
    /// these supported features
    pub fn sdp_register_records(&self, controller_features: u16, target_features: u16) {
        // SAFETY: on the BTstack thread after sdp_init; buffers are static in C.
        unsafe { ffi::a2dpwb_sdp_register_records(controller_features, target_features) };
    }

    /// A2DP source stream endpoint in slot 0..7. Returns the local SEID.
    pub fn create_stream_endpoint(&self, slot: usize, codec_type: u8, caps: &[u8], config: &[u8]) -> Option<u8> {
        if caps.len() > 32 || config.len() > 32 {
            return None;
        }
        // SAFETY: on the BTstack thread after a2dp_source_init; the shim
        // copies caps / config into static buffers of 32 bytes.
        let seid = unsafe {
            ffi::a2dpwb_create_stream_endpoint(
                slot as c_int,
                codec_type,
                caps.as_ptr(),
                caps.len() as u16,
                config.as_ptr(),
                config.len() as u16,
            )
        };
        (seid != 0).then_some(seid)
    }

    /// Realtek vendor commands before HCI Reset (firmware download)
    pub fn enable_custom_pre_init(&self) {
        // SAFETY: on the BTstack thread, after hci_init.
        unsafe { ffi::hci_enable_custom_pre_init() };
    }

    pub fn realtek_setup(&self, product_id: u16, folder: &str) {
        let p = keep_path(folder);
        // SAFETY: on the BTstack thread; the path outlives BTstack (PATHS,
        // cleared only after deinit); BTstack copies nothing and keeps p.
        unsafe {
            ffi::btstack_chipset_realtek_set_product_id(product_id);
            ffi::btstack_chipset_realtek_set_firmware_folder_path(p);
            ffi::btstack_chipset_realtek_set_config_folder_path(p);
        }
    }

    /// Registers the Realtek chipset driver (runs its init at once)
    pub fn set_chipset_realtek(&self) {
        // SAFETY: on the BTstack thread after hci_init; the instance is static.
        unsafe { ffi::hci_set_chipset(ffi::btstack_chipset_realtek_instance()) };
    }

    /// Registers the Broadcom chipset driver with a PatchRAM (.hcd) file
    pub fn set_chipset_bcm(&self, hcd_path: &str) {
        let p = keep_path(hcd_path);
        // SAFETY: on the BTstack thread after hci_init; p outlives BTstack.
        unsafe {
            ffi::btstack_chipset_bcm_set_hcd_file_path(p);
            ffi::hci_set_chipset(ffi::btstack_chipset_bcm_instance());
        }
    }

    /// Intel bootloader firmware download on the installed transport, before
    /// hci_init; ends with [`crate::App::on_intel_firmware`]
    pub fn intel_download_firmware(&self, firmware_folder: &str) {
        let p = keep_path(firmware_folder);
        // SAFETY: on the BTstack thread; transport installed; p outlives it.
        unsafe { ffi::a2dpwb_intel_download_firmware(transport::is_async() as c_int, p) };
    }

    // ---- HCI ----

    /// HCI_POWER_ON: opens the transport. Err if it could not be opened.
    pub fn power_on(&self) -> Result<(), i32> {
        // SAFETY: on the BTstack thread, after hci_init.
        let r = unsafe { ffi::hci_power_control(1) };
        if r == 0 { Ok(()) } else { Err(r) }
    }

    pub fn power_off(&self) {
        // SAFETY: on the BTstack thread, after hci_init.
        unsafe { ffi::hci_power_control(0) };
    }

    pub fn hci_initialized(&self) -> bool {
        INITIALIZED.with(|i| i.get() & INIT_HCI != 0)
    }

    pub fn hci_state(&self) -> HciState {
        if !self.hci_initialized() {
            return HciState::Off;
        }
        // SAFETY: on the BTstack thread, after hci_init.
        match unsafe { ffi::a2dpwb_hci_get_state() } {
            1 => HciState::Initializing,
            2 => HciState::Working,
            3 => HciState::Halting,
            4 => HciState::Sleeping,
            5 => HciState::FallingAsleep,
            _ => HciState::Off,
        }
    }

    pub fn can_send_command_now(&self) -> bool {
        // SAFETY: on the BTstack thread, after hci_init.
        unsafe { ffi::a2dpwb_hci_can_send_command_now() != 0 }
    }

    /// The HCI transport reported a permanent failure ([`crate::Upstream::fail`])
    pub fn transport_failed(&self) -> bool {
        transport::failed()
    }

    pub fn cmd_set_afh_classification(&self, map: &[u8; 10]) -> u8 {
        // SAFETY: on the BTstack thread; map has the 10 bytes the shim reads.
        unsafe { ffi::a2dpwb_cmd_set_afh_classification(map.as_ptr()) }
    }

    pub fn cmd_read_afh_channel_map(&self, con_handle: u16) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dpwb_cmd_read_afh_channel_map(con_handle) }
    }

    pub fn cmd_read_rssi(&self, con_handle: u16) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dpwb_cmd_read_rssi(con_handle) }
    }

    pub fn cmd_read_bd_addr(&self) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dpwb_cmd_read_bd_addr() }
    }

    pub fn radio_read(&self, handle: u16, kind: u8) -> u8 {
        // SAFETY: BTstack thread, scalar arguments validated in the C shim.
        unsafe { ffi::a2dpwb_radio_read(handle, kind) }
    }

    pub fn link_mode(&self, handle: u16) -> Option<u8> {
        // SAFETY: BTstack thread; connection pointer stays within the shim.
        let mode = unsafe { ffi::a2dpwb_link_mode(handle) };
        (0..=3).contains(&mode).then_some(mode as u8)
    }

    pub fn exit_sniff(&self, handle: u16) -> u8 {
        // SAFETY: BTstack thread; GAP validates the connection handle.
        unsafe { ffi::a2dpwb_exit_sniff(handle) }
    }

    pub fn cmd_read_local_version(&self) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dpwb_cmd_read_local_version() }
    }

    pub fn acl_state(&self, addr: &[u8; 6]) -> AclState {
        let mut handle = 0u16;
        // SAFETY: on the BTstack thread; addr is 6 bytes, handle writable.
        match unsafe { ffi::a2dpwb_acl_state(addr.as_ptr(), &mut handle) } {
            -1 => AclState::None,
            0 => AclState::Paging,
            1 => AclState::Open(handle),
            _ => AclState::Other,
        }
    }

    // ---- GAP ----

    pub fn inquiry_start(&self, duration_1280ms_units: u8) -> i32 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::gap_inquiry_start(duration_1280ms_units) }
    }

    pub fn inquiry_stop(&self) -> i32 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::gap_inquiry_stop() }
    }

    pub fn ssp_confirmation_response(&self, addr: &[u8; 6]) {
        // SAFETY: on the BTstack thread; addr is 6 bytes.
        unsafe { ffi::gap_ssp_confirmation_response(addr.as_ptr()) };
    }

    /// Legacy pairing PIN (BTstack copies it)
    pub fn pin_code_response(&self, addr: &[u8; 6], pin: &str) {
        let pin = CString::new(pin).unwrap_or_default();
        // SAFETY: on the BTstack thread; both pointers valid during the call.
        unsafe { ffi::gap_pin_code_response(addr.as_ptr(), pin.as_ptr()) };
    }

    pub fn disconnect(&self, con_handle: u16) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::gap_disconnect(con_handle) }
    }

    pub fn drop_link_key(&self, addr: &[u8; 6]) {
        let mut a = *addr;
        // SAFETY: on the BTstack thread; a is a writable copy of 6 bytes.
        unsafe { ffi::gap_drop_link_key_for_bd_addr(a.as_mut_ptr()) };
    }

    // ---- A2DP source ----

    /// Returns the A2DP connection id, or BTstack's status
    pub fn a2dp_establish_stream(&self, addr: &[u8; 6]) -> Result<u16, u8> {
        let mut a = *addr;
        let mut cid = 0u16;
        // SAFETY: on the BTstack thread; pointers valid during the call.
        let status = unsafe { ffi::a2dp_source_establish_stream(a.as_mut_ptr(), &mut cid) };
        if status == 0 { Ok(cid) } else { Err(status) }
    }

    pub fn a2dp_disconnect(&self, a2dp_cid: u16) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dp_source_disconnect(a2dp_cid) }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn a2dp_set_config_sbc(
        &self,
        a2dp_cid: u16,
        local_seid: u8,
        remote_seid: u8,
        sampling_frequency: u16,
        channel_mode: i32,
        block_length: i32,
        subbands: i32,
        allocation_method: i32,
        min_bitpool: u8,
        max_bitpool: u8,
    ) -> u8 {
        // SAFETY: on the BTstack thread; plain values.
        unsafe {
            ffi::a2dpwb_set_config_sbc(
                a2dp_cid,
                local_seid,
                remote_seid,
                sampling_frequency,
                channel_mode,
                block_length,
                subbands,
                allocation_method,
                min_bitpool,
                max_bitpool,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn a2dp_set_config_aac(
        &self,
        a2dp_cid: u16,
        local_seid: u8,
        remote_seid: u8,
        object_type: i32,
        sampling_frequency: u32,
        channels: u8,
        bit_rate: u32,
        vbr: bool,
    ) -> u8 {
        // SAFETY: on the BTstack thread; plain values.
        unsafe {
            ffi::a2dpwb_set_config_aac(
                a2dp_cid,
                local_seid,
                remote_seid,
                object_type,
                sampling_frequency,
                channels,
                bit_rate,
                vbr as u8,
            )
        }
    }

    pub fn a2dp_set_config_other(&self, a2dp_cid: u16, local_seid: u8, remote_seid: u8, info: &[u8]) -> u8 {
        if info.len() > 32 {
            return crate::consts::ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
        }
        // SAFETY: on the BTstack thread; the shim copies info (<= 32 bytes)
        // into a persistent buffer.
        unsafe { ffi::a2dpwb_set_config_other(a2dp_cid, local_seid, remote_seid, info.as_ptr(), info.len() as u8) }
    }

    pub fn a2dp_start_stream(&self, a2dp_cid: u16, local_seid: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dp_source_start_stream(a2dp_cid, local_seid) }
    }

    pub fn a2dp_pause_stream(&self, a2dp_cid: u16, local_seid: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dp_source_pause_stream(a2dp_cid, local_seid) }
    }

    pub fn a2dp_request_can_send_now(&self, a2dp_cid: u16, local_seid: u8) {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dp_source_stream_endpoint_request_can_send_now(a2dp_cid, local_seid) }
    }

    /// Largest media payload after the 12-byte RTP header, or None
    pub fn a2dp_max_media_payload_size(&self, a2dp_cid: u16, local_seid: u8) -> Option<u16> {
        // SAFETY: on the BTstack thread.
        let n = unsafe { ffi::a2dp_max_media_payload_size(a2dp_cid, local_seid) };
        (n > 0).then_some(n as u16)
    }

    pub fn a2dp_send_media_payload_rtp(
        &self,
        a2dp_cid: u16,
        local_seid: u8,
        marker: u8,
        timestamp: u32,
        payload: &[u8],
    ) -> u8 {
        if payload.len() > u16::MAX as usize {
            return crate::consts::ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
        }
        // SAFETY: on the BTstack thread; payload valid during the call
        // (BTstack copies it into its outgoing buffer).
        unsafe {
            ffi::a2dp_source_stream_send_media_payload_rtp(
                a2dp_cid,
                local_seid,
                marker,
                timestamp,
                payload.as_ptr(),
                payload.len() as u16,
            )
        }
    }

    /// Media packet sent as is (no RTP header: aptX, aptX LL)
    pub fn a2dp_send_media_packet(&self, a2dp_cid: u16, local_seid: u8, packet: &[u8]) -> u8 {
        if packet.len() > u16::MAX as usize {
            return crate::consts::ERROR_CODE_INVALID_HCI_COMMAND_PARAMETERS;
        }
        // SAFETY: on the BTstack thread; packet valid during the call.
        unsafe { ffi::a2dp_source_stream_send_media_packet(a2dp_cid, local_seid, packet.as_ptr(), packet.len() as u16) }
    }

    pub fn avdtp_state_for_addr(&self, addr: &[u8; 6]) -> (AvdtpState, u16) {
        let mut cid = 0u16;
        // SAFETY: on the BTstack thread; addr 6 bytes, cid writable.
        let s = unsafe { ffi::a2dpwb_avdtp_state_for_addr(addr.as_ptr(), &mut cid) };
        (avdtp_state(s), cid)
    }

    pub fn avdtp_state_for_cid(&self, avdtp_cid: u16) -> AvdtpState {
        // SAFETY: on the BTstack thread.
        avdtp_state(unsafe { ffi::a2dpwb_avdtp_state_for_cid(avdtp_cid) })
    }

    // ---- AVRCP ----

    pub fn avrcp_connect(&self, addr: &[u8; 6]) -> Result<u16, u8> {
        let mut a = *addr;
        let mut cid = 0u16;
        // SAFETY: on the BTstack thread; pointers valid during the call.
        let status = unsafe { ffi::avrcp_connect(a.as_mut_ptr(), &mut cid) };
        if status == 0 { Ok(cid) } else { Err(status) }
    }

    pub fn avrcp_disconnect(&self, avrcp_cid: u16) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::avrcp_disconnect(avrcp_cid) }
    }

    pub fn avrcp_set_absolute_volume(&self, avrcp_cid: u16, volume: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::avrcp_controller_set_absolute_volume(avrcp_cid, volume) }
    }

    pub fn avrcp_enable_notification(&self, avrcp_cid: u16, event_id: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::avrcp_controller_enable_notification(avrcp_cid, event_id as c_int) }
    }

    pub fn avrcp_target_support_event(&self, avrcp_cid: u16, event_id: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::avrcp_target_support_event(avrcp_cid, event_id as c_int) }
    }

    pub fn avrcp_target_set_playback_status(&self, avrcp_cid: u16, status: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::avrcp_target_set_playback_status(avrcp_cid, status as c_int) }
    }

    pub fn avrcp_target_play_status(&self, avrcp_cid: u16, length_ms: u32, position_ms: u32, status: u8) -> u8 {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::avrcp_target_play_status(avrcp_cid, length_ms, position_ms, status as c_int) }
    }

    // ---- Run loop ----

    /// Timer `id` (0..16) fires once after `timeout_ms` ([`crate::App::on_timer`]);
    /// starting a running timer restarts it
    pub fn timer_start(&self, id: u32, timeout_ms: u32) -> bool {
        // SAFETY: on the BTstack thread; timers are static in C.
        unsafe { ffi::a2dpwb_timer_start(id, timeout_ms) != 0 }
    }

    pub fn timer_stop(&self, id: u32) {
        // SAFETY: on the BTstack thread.
        unsafe { ffi::a2dpwb_timer_stop(id) }
    }

    // ---- Teardown (crate::run) ----

    /// Closes what HCI left open and deinitializes every profile and HCI
    pub(crate) fn deinit(&self) {
        let init = INITIALIZED.with(|i| i.replace(0));
        // SAFETY: on the BTstack thread after the run loop has stopped; each
        // deinit only for what was initialized.
        unsafe {
            if init & INIT_HCI != 0 && ffi::a2dpwb_hci_get_state() != 0 {
                // Power off did not finish (or was skipped for an adapter
                // that went away): release the device anyway, or the next
                // open in this process finds it busy
                if transport::loss_expected() {
                    tracing::info!(target: "btstack", "HCI not off at exit, closing the transport");
                } else {
                    tracing::warn!(target: "btstack", "HCI not off at exit, closing the transport");
                }
                transport::close();
            }
            if init & INIT_LE != 0 {
                self.le_deinit(init & INIT_ATT != 0);
            }
            if init & INIT_AVRCP != 0 {
                ffi::avrcp_target_deinit();
                ffi::avrcp_controller_deinit();
                ffi::avrcp_deinit();
            }
            if init & INIT_A2DP != 0 {
                ffi::a2dp_source_deinit();
            }
            if init & INIT_SDP != 0 {
                ffi::sdp_client_deinit();
                ffi::sdp_deinit();
            }
            if init & INIT_L2CAP != 0 {
                ffi::l2cap_deinit();
            }
            if init & INIT_HCI != 0 {
                ffi::hci_deinit();
            }
        }
        transport::uninstall();
        PATHS.with(|p| p.borrow_mut().clear());
    }
}

fn log_le_hidden() {
    if transport::le_hidden() {
        tracing::info!(target: "hci", "LE hidden: not using the controller's LE support (LE Audio off)");
    }
}

fn avdtp_state(raw: c_int) -> AvdtpState {
    match raw {
        -1 => AvdtpState::None,
        1 => AvdtpState::Open,
        _ => AvdtpState::Pending,
    }
}
