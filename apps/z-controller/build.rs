use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc_exe() -> Option<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let cand = dir.join("rc.exe");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    let kits_bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if let Ok(entries) = std::fs::read_dir(kits_bin) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| p.join("x64").join("rc.exe").is_file())
            .collect();
        versions.sort();
        if let Some(latest) = versions.pop() {
            return Some(latest.join("x64").join("rc.exe"));
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=z-controller.manifest");
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=assets/z-controller.ico");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let manifest_dir =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let manifest = manifest_dir.join("z-controller.manifest");

        println!("cargo:rustc-link-arg-bin=z-controller=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=z-controller=/MANIFESTINPUT:{}",
            manifest.display()
        );

        let rc_exe = find_rc_exe().expect("Windows SDK rc.exe not found to compile app.rc");
        let res_path = out_dir.join("app.res");
        let status = Command::new(&rc_exe)
            .arg("/nologo")
            .arg("/fo")
            .arg(&res_path)
            .arg(manifest_dir.join("app.rc"))
            .status()
            .expect("failed to execute rc.exe");
        assert!(status.success(), "rc.exe failed with status {status}");
        println!("cargo:rustc-link-arg-bin=z-controller={}", res_path.display());
    }
}
