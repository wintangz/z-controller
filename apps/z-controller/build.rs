use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "src/logo_png.rs"]
mod logo_png;

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
    if let Ok(entries) = fs::read_dir(kits_bin) {
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

fn encode_ico_bmp_entry(size: usize, rgba: &[u8]) -> Vec<u8> {
    // 40-byte BITMAPINFOHEADER + bottom-up BGRA pixels + 1-bit AND mask (transparent bit = 1)
    let mut data = Vec::new();
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(size as i32).to_le_bytes());
    data.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&32u16.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&((size * size * 4) as u32).to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..size).rev() {
        for x in 0..size {
            let off = (y * size + x) * 4;
            data.push(rgba[off + 2]);
            data.push(rgba[off + 1]);
            data.push(rgba[off]);
            data.push(rgba[off + 3]);
        }
    }

    let mask_row_bytes = ((size + 31) / 32) * 4;
    for y in (0..size).rev() {
        let row_start = data.len();
        data.resize(row_start + mask_row_bytes, 0u8);
        for x in 0..size {
            let off = (y * size + x) * 4;
            if rgba[off + 3] == 0 {
                data[row_start + (x / 8)] |= 0x80 >> (x & 7);
            }
        }
    }
    data
}

fn build_multi_size_ico_from_png(logo_png_path: &Path) -> Vec<u8> {
    let png_bytes = fs::read(logo_png_path).unwrap_or_else(|e| {
        panic!(
            "failed to read canonical WT artwork at '{}': {e}",
            logo_png_path.display()
        )
    });
    let (src_w, src_h, src_rgba) = logo_png::decode_png_rgba(&png_bytes).unwrap_or_else(|e| {
        panic!(
            "failed to decode WT artwork '{}': {e}",
            logo_png_path.display()
        )
    });

    let sizes = [16usize, 20, 24, 32, 40, 48, 64, 128, 256];
    let mut entries = Vec::with_capacity(sizes.len());
    for s in sizes {
        let rgba = logo_png::downsample_trimmed_logo_rgba(src_w, src_h, &src_rgba, s);
        entries.push((s, encode_ico_bmp_entry(s, &rgba)));
    }

    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&(entries.len() as u16).to_le_bytes());

    let mut offset = 6 + entries.len() * 16;
    for (s, payload) in &entries {
        let dim = if *s >= 256 { 0u8 } else { *s as u8 };
        ico.push(dim);
        ico.push(dim);
        ico.push(0);
        ico.push(0);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += payload.len();
    }
    for (_, payload) in entries {
        ico.extend_from_slice(&payload);
    }
    ico
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=z-controller.manifest");
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=src/logo_png.rs");
    println!("cargo:rerun-if-changed=../../assets/logo-mark.png");

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let logo_png_path = manifest_dir.join("../../assets/logo-mark.png");
    let assets_dir = manifest_dir.join("assets");
    let _ = fs::create_dir_all(&assets_dir);
    let ico_path = assets_dir.join("wt-app.ico");
    let ico_bytes = build_multi_size_ico_from_png(&logo_png_path);
    if fs::read(&ico_path).ok().as_deref() != Some(ico_bytes.as_slice()) {
        fs::write(&ico_path, &ico_bytes).expect("write wt-app.ico");
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
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
