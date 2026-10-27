// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! Builds the LHDC V5 encoder (portable C port of the AOSP encoder,
//! Apache-2.0) as v1's CMakeLists.txt does: its 12 sources as one static
//! library, the default 192 kHz context size (~117 KB, heap-allocated by
//! lhdcv5_enc_new), warnings off.
//!
//! The sources come from the git submodule extern/lhdcv5-enc of the
//! repository (override with LHDCV5_ENC_DIR).

use std::env;
use std::path::{Path, PathBuf};

const SOURCES: &[&str] = &[
    "lhdcv5_math.c",
    "lhdcv5_arith.c",
    "lhdcv5_bitout.c",
    "lhdcv5_fft.c",
    "lhdcv5_level.c",
    "lhdcv5_header.c",
    "lhdcv5_freq.c",
    "lhdcv5_tables.c",
    "lhdcv5_workspace.c",
    "lhdcv5_process.c",
    "lhdcv5_cirbuf.c",
    "lhdcv5_api.c",
];

fn main() {
    if env::var_os("CARGO_FEATURE_AOSP_RUST").is_some() { return; }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dir = env::var_os("LHDCV5_ENC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../../extern/lhdcv5-enc"));
    // std::path::absolute, not canonicalize: MSVC does not take \\?\ paths
    let dir = std::path::absolute(&dir).unwrap();
    assert!(
        dir.join("src/lhdcv5_api.c").exists(),
        "LHDC V5 encoder not found at {} (git submodule update --init extern/lhdcv5-enc)",
        dir.display()
    );

    let mut build = cc::Build::new();
    build
        .files(SOURCES.iter().map(|f| dir.join("src").join(f)))
        .include(dir.join("include"))
        .include(dir.join("src"))
        // Third-party code: keep its warnings out of the build log
        .warnings(false)
        .cargo_warnings(false);
    if build.get_compiler().is_like_msvc() {
        build.flag("/W1").flag("/utf-8");
    } else {
        build.flag("-w");
    }
    build.compile("lhdcv5_enc");

    println!("cargo:rerun-if-env-changed=LHDCV5_ENC_DIR");
    rerun(&dir.join("src"));
    rerun(&dir.join("include"));
    // For dependents that need the header
    println!("cargo:include={}", dir.join("include").display());
}

fn rerun(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
}
