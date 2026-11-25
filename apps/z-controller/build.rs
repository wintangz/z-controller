use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=z-controller.manifest");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let manifest_dir =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
        let manifest = manifest_dir.join("z-controller.manifest");
        println!("cargo:rustc-link-arg-bin=z-controller=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=z-controller=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
