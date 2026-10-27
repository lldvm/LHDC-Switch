fn main() {
    // tauri-build reruns only for tauri.conf.json and capabilities; without
    // this, a new icon.ico leaves the old icon in a2dpwb.exe
    println!("cargo:rerun-if-changed=icons/icon.ico");
    tauri_build::build();
}
