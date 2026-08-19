use std::{env, fs, path::PathBuf};

fn main() {
    ensure_windows_icon();
    tauri_build::build()
}

fn ensure_windows_icon() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Missing CARGO_MANIFEST_DIR"));
    let icon_path = manifest_dir.join("icons").join("icon.ico");
    let source_icon_path = manifest_dir
        .join("..")
        .join("..")
        .join("..")
        .join("assets")
        .join("icon.ico");

    println!("cargo:rerun-if-changed={}", source_icon_path.display());

    if icon_path.exists() {
        return;
    }

    let icon_dir = icon_path.parent().expect("Icon path should have a parent");
    fs::create_dir_all(icon_dir).expect("Create Tauri icon directory failed");
    fs::copy(&source_icon_path, &icon_path).expect("Copy Tauri Windows icon failed");
}
