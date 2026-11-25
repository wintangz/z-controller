#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod sw_painter;
mod tray_win32;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use z_core::profile::AppConfig;
use z_platform_windows::drivers::query_process_memory_bytes;
use z_sdl3::{SdlGamepadAxis, SdlGamepadButton, SdlVirtualGamepad};

use crate::engine::{ControllerSnapshot, EngineHandle, install_emergency_uncloak_hooks};

#[derive(Serialize)]
struct SmokeReport {
    controllers: Vec<ControllerSnapshot>,
    driver_vigem_installed: bool,
    driver_hidhide_installed: bool,
    driver_hidhide_accessible: bool,
    driver_hidhide_error: Option<String>,
    working_set_bytes: u64,
    private_bytes: u64,
}

fn main() {
    install_emergency_uncloak_hooks();

    let args: Vec<String> = std::env::args().collect();
    let mut open_immediately = false;
    let mut smoke_json_path: Option<PathBuf> = None;
    let mut smoke_hold_ms: u64 = 1200;
    let mut smoke_dual_pad = false;
    let mut tray_smoke_ms: Option<u64> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--show" | "--open" => open_immediately = true,
            "--smoke-verify" => {
                if i + 1 < args.len() {
                    smoke_json_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--smoke-hold-ms" => {
                if i + 1 < args.len() {
                    smoke_hold_ms = args[i + 1].parse().unwrap_or(1200);
                    i += 1;
                }
            }
            "--smoke-dual-pad" => smoke_dual_pad = true,
            "--tray-smoke-ms" => {
                if i + 1 < args.len() {
                    tray_smoke_ms = Some(args[i + 1].parse().unwrap_or(4000));
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let config_path = AppConfig::default_path();
    let mut engine = match EngineHandle::start(config_path) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("z-controller startup failed: {err}");
            std::process::exit(1);
        }
    };

    if let Some(out_path) = smoke_json_path {
        let mut vpads: Vec<SdlVirtualGamepad> = Vec::new();
        if smoke_dual_pad {
            let initial = engine.snapshot().controllers.len();
            if initial == 0 {
                match (
                    SdlVirtualGamepad::attach(0x057E, 0x2009, "Pad 1 Pro Sim"),
                    SdlVirtualGamepad::attach(0x054C, 0x09CC, "Pad 2 Second Pad"),
                ) {
                    (Ok(vp1), Ok(vp2)) => {
                        engine.wake();
                        std::thread::sleep(Duration::from_millis(220));
                        vp1.set_axis(SdlGamepadAxis::LeftY, i16::MIN);
                        vp1.set_axis(SdlGamepadAxis::RightY, i16::MAX);
                        vp1.set_button(SdlGamepadButton::South, true);
                        vp2.set_axis(SdlGamepadAxis::LeftX, -16384);
                        vp2.set_button(SdlGamepadButton::East, true);
                        vp1.pump();
                        engine.wake();
                        std::thread::sleep(Duration::from_millis(60));
                        engine.wake();
                        vpads.push(vp1);
                        vpads.push(vp2);
                    }
                    (e1, e2) => {
                        eprintln!(
                            "smoke dual pad attach failed: {:?} / {:?}",
                            e1.err(),
                            e2.err()
                        );
                    }
                }
            } else {
                match SdlVirtualGamepad::attach(0x054C, 0x09CC, "Second Test Pad") {
                    Ok(vp) => {
                        engine.wake();
                        std::thread::sleep(Duration::from_millis(220));
                        vp.set_axis(SdlGamepadAxis::LeftY, i16::MIN);
                        vp.set_axis(SdlGamepadAxis::RightY, i16::MAX);
                        vp.set_button(SdlGamepadButton::South, true);
                        vp.pump();
                        engine.wake();
                        std::thread::sleep(Duration::from_millis(60));
                        engine.wake();
                        vpads.push(vp);
                    }
                    Err(e) => {
                        eprintln!("smoke dual pad attach failed: {e}");
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(smoke_hold_ms));
        let snap = engine.snapshot();
        let (ws, priv_b) = query_process_memory_bytes();
        let report = SmokeReport {
            controllers: snap.controllers.clone(),
            driver_vigem_installed: snap.driver_status.vigem_installed,
            driver_hidhide_installed: snap.driver_status.hidhide_installed,
            driver_hidhide_accessible: snap.driver_status.hidhide_accessible,
            driver_hidhide_error: snap.driver_status.hidhide_error.clone(),
            working_set_bytes: ws,
            private_bytes: priv_b,
        };
        if let Ok(json) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(&out_path, json);
        }
        drop(vpads);
        engine.shutdown();
        return;
    }

    // First-run convenience: automatically show main window if drivers are missing so user
    // can trigger the inline UAC driver bootstrap without any popup dialog.
    let snap = engine.snapshot();
    if !snap.driver_status.vigem_installed || !snap.driver_status.hidhide_installed {
        open_immediately = true;
    }

    if let Err(err) = tray_win32::run_tray_app(engine, open_immediately, tray_smoke_ms) {
        eprintln!("z-controller tray error: {err}");
        std::process::exit(1);
    }
}
