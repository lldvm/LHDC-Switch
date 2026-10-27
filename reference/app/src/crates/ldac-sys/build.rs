//! Builds libldac (AOSP, Apache-2.0) with its ABR module, as v1's
//! CMakeLists.txt does: only src/ldaclib.c and src/ldacBT.c are compiled (the
//! other .c files are #included by them with DECLFUNC static linkage, so
//! compiling them separately breaks the link), plus abr/src/ldacBT_abr.c.
//!
//! The sources come from the git submodule extern/libldac of the repository
//! (override with LIBLDAC_DIR). With MSVC the compat shims of compat/msvc
//! (override with LDAC_COMPAT_DIR) supply <unistd.h> and are force-included
//! to hide the GCC `__attribute__`s.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ldac = dir_from_env("LIBLDAC_DIR", &manifest, "../../../../extern/libldac");
    assert!(
        ldac.join("src/ldaclib.c").exists(),
        "libldac not found at {} (git submodule update --init extern/libldac)",
        ldac.display()
    );

    let mut build = cc::Build::new();
    build
        .file(ldac.join("src/ldaclib.c"))
        .file(ldac.join("src/ldacBT.c"))
        .file(ldac.join("abr/src/ldacBT_abr.c"))
        .include(ldac.join("inc"))
        .include(ldac.join("src"))
        .include(ldac.join("abr/inc"))
        .warnings(false);

    if build.get_compiler().is_like_msvc() {
        let compat = dir_from_env("LDAC_COMPAT_DIR", &manifest, "../../../../compat/msvc");
        let shim = compat.join("ldac_compat.h");
        assert!(shim.exists(), "MSVC compat header not found at {}", shim.display());
        build.include(&compat).flag("/W3").flag(format!("/FI{}", shim.display()));
        // Third-party code: its /W3 warnings (int conversions) are not ours
        // to fix, so keep them out of every cargo build
        build.cargo_warnings(false);
        rerun(&compat);
    }
    build.compile("ldac");

    println!("cargo:rerun-if-env-changed=LIBLDAC_DIR");
    println!("cargo:rerun-if-env-changed=LDAC_COMPAT_DIR");
    rerun(&ldac.join("src"));
    rerun(&ldac.join("inc"));
    rerun(&ldac.join("abr"));
    // For dependents that need the headers
    println!("cargo:include={}", ldac.join("inc").display());
}

/// The directory in `var`, or `default` relative to the manifest
fn dir_from_env(var: &str, manifest: &Path, default: &str) -> PathBuf {
    let dir = env::var_os(var).map(PathBuf::from).unwrap_or_else(|| manifest.join(default));
    // std::path::absolute, not canonicalize: MSVC does not take \\?\ paths
    std::path::absolute(&dir).unwrap()
}

fn rerun(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
}
