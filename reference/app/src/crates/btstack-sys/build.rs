// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! Builds BTstack (classic: HCI, L2CAP, SDP, AVDTP / A2DP source, AVRCP; LE:
//! SM, ATT / GATT, LE device DB in a TLV file, ISO), the Windows run loop and
//! TLV store, micro-ecc, the chipset drivers and the C glue in csrc/shim.c
//! with the MSVC compiler found by `cc`.
//!
//! The BTstack sources come from the git submodule extern/btstack of the
//! repository (override with the BTSTACK_DIR environment variable).

use std::env;
use std::path::{Path, PathBuf};

/// src/*.c not needed by A2DPWB (audio playback helpers, LC3 - LE Audio
/// encodes with crate lc3-sys -, other HCI transports, HID)
const CORE_EXCLUDE: &[&str] = &[
    "btstack_audio.c",
    "btstack_audio_generator.c",
    "btstack_audio_player.c",
    "btstack_audio_song.c",
    "btstack_lc3.c",
    "btstack_lc3_google.c",
    "btstack_lc3plus_fraunhofer.c",
    "btstack_resample.c",
    "btstack_sample_rate_compensation.c",
    "btstack_slip.c",
    "btstack_uart_slip_wrapper.c",
    "hci_transport_em9304_spi.c",
    "hci_transport_h4.c",
    "hci_transport_h5.c",
    "btstack_hid.c",
    "btstack_hid_parser.c",
    "btstack_base64_decoder.c",
    "btstack_base64_encoder.c",
];

const CLASSIC: &[&str] = &[
    "a2dp.c",
    "a2dp_source.c",
    "a2dp_sink.c",
    "avdtp.c",
    "avdtp_acceptor.c",
    "avdtp_initiator.c",
    "avdtp_sink.c",
    "avdtp_source.c",
    "avdtp_util.c",
    "avrcp.c",
    "avrcp_controller.c",
    "avrcp_target.c",
    "sdp_client.c",
    "sdp_server.c",
    "sdp_util.c",
    "btstack_link_key_db_memory.c",
];

/// src/ble/*.c not built: LE bonds are kept in a TLV file
/// (le_device_db_tlv.c), as in v1
const BLE_EXCLUDE: &[&str] = &["le_device_db_memory.c"];

/// Bluedroid SBC encoder (Apache-2.0), feature "sbc-reference"
const SBC_ENCODER: &[&str] = &[
    "sbc_analysis.c",
    "sbc_dct.c",
    "sbc_dct_coeffs.c",
    "sbc_enc_bit_alloc_mono.c",
    "sbc_enc_bit_alloc_ste.c",
    "sbc_enc_coeffs.c",
    "sbc_encoder.c",
    "sbc_packing.c",
];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let btstack =
        env::var_os("BTSTACK_DIR").map(PathBuf::from).unwrap_or_else(|| manifest.join("../../../../extern/btstack"));
    // std::path::absolute, not canonicalize: MSVC does not take \\?\ paths
    let btstack = std::path::absolute(&btstack).unwrap();
    assert!(
        btstack.join("src/hci.c").exists(),
        "BTstack not found at {} (git submodule update --init extern/btstack)",
        btstack.display()
    );
    let csrc = manifest.join("csrc");

    let mut build = cc::Build::new();
    build
        .include(&csrc)
        .include(btstack.join("src"))
        .include(btstack.join("platform/windows"))
        .include(btstack.join("platform/posix"))
        .include(btstack.join("chipset/realtek"))
        .include(btstack.join("chipset/bcm"))
        .include(btstack.join("chipset/intel"))
        .include(btstack.join("3rd-party/tinydir"))
        .include(btstack.join("3rd-party/rijndael"))
        .include(btstack.join("3rd-party/micro-ecc"))
        .define("_WIN32_WINNT", "0x0A00")
        .define("NOMINMAX", None)
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .warnings(false)
        .flag_if_supported("/utf-8")
        .flag_if_supported("/W1")
        // A function called without a declaration (C4013) links against
        // nothing when it is a static inline of a header: an error
        .flag_if_supported("/we4013");

    // The Realtek driver prints why it loads which firmware ("Realtek:
    // Using firmware ...", "Firmware already exists") with printf, to a
    // stdout the resident process does not have: BTstack's
    // ENABLE_PRINTF_TO_LOG sends those lines to the HCI dump log instead
    // (HCI_DUMP_LOG_LEVEL_PRINT), for this file only
    let mut realtek = build.clone();
    realtek.define("ENABLE_PRINTF_TO_LOG", None).file(btstack.join("chipset/realtek/btstack_chipset_realtek.c"));
    realtek.compile("btstack_realtek");

    let mut core: Vec<PathBuf> = std::fs::read_dir(btstack.join("src"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c"))
        .filter(|p| !CORE_EXCLUDE.contains(&p.file_name().unwrap().to_str().unwrap()))
        .collect();
    core.sort();
    build.files(&core);
    let mut ble: Vec<PathBuf> = std::fs::read_dir(btstack.join("src/ble"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c"))
        .filter(|p| !BLE_EXCLUDE.contains(&p.file_name().unwrap().to_str().unwrap()))
        .collect();
    ble.sort();
    build.files(&ble);
    build.files(CLASSIC.iter().map(|f| btstack.join("src/classic").join(f)));
    // Pinned Windows run-loop derivative: monotonic QPC + high-resolution
    // waitable timer. Copyright/licence retained in this source.
    build.file(csrc.join("media_run_loop_windows.c"));
    // LE bonds (le_device_db_tlv) and the GATT server's client
    // configurations in a TLV file, readable by v1's le_audio_bonds.tlv
    build.file(btstack.join("platform/windows/btstack_tlv_windows.c"));
    // Software P-256 for LE Secure Connections (ENABLE_MICRO_ECC_P256)
    build.file(btstack.join("3rd-party/micro-ecc/uECC.c"));
    build.file(btstack.join("chipset/bcm/btstack_chipset_bcm.c"));
    build.file(btstack.join("chipset/intel/btstack_chipset_intel_firmware.c"));
    build.file(btstack.join("3rd-party/rijndael/rijndael.c"));
    build.file(csrc.join("shim.c"));

    if env::var_os("CARGO_FEATURE_SBC_REFERENCE").is_some() {
        let enc = btstack.join("3rd-party/bluedroid/encoder");
        build.include(enc.join("include"));
        build.files(SBC_ENCODER.iter().map(|f| enc.join("srce").join(f)));
        build.file(csrc.join("sbc_reference.c"));
        build.define("A2DPWB_SBC_REFERENCE", None);
    }

    build.compile("btstack");

    for lib in ["setupapi", "winusb", "ws2_32", "advapi32", "user32"] {
        println!("cargo:rustc-link-lib={lib}");
    }
    println!("cargo:rerun-if-changed=csrc");
    println!("cargo:rerun-if-env-changed=BTSTACK_DIR");
    rerun_dir(&btstack.join("src"));
    rerun_dir(&btstack.join("src/ble"));
    // For dependents (e.g. tests that need the include paths)
    println!("cargo:root={}", btstack.display());
}

fn rerun_dir(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
}
