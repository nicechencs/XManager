fn main() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico = manifest_dir.join("assets/logo.ico");
    println!("cargo:rerun-if-changed={}", ico.display());

    #[cfg(target_os = "windows")]
    embed_windows_icon(&ico);
}

#[cfg(target_os = "windows")]
fn embed_windows_icon(ico: &std::path::Path) {
    let rc_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("app.rc");
    let ico_path = ico.display().to_string().replace('\\', "/");
    std::fs::write(&rc_path, format!("1 ICON \"{ico_path}\"\n"))
        .expect("write Windows icon resource script");
    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_optional()
        .expect("embed Windows application icon");
}
