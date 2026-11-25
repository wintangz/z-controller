#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod diagram;
mod engine;
mod sw_painter;
mod tray_win32;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use serde::Serialize;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use z_core::battery::{BatteryReading, BatteryStatus};
use z_core::model::{ControllerFamily, HardwareKey, LogicalButton, Transport, VirtualTargetKind};
use z_core::profile::{AppConfig, Profile};
use z_platform_windows::drivers::query_process_memory_bytes;
use z_sdl3::{SdlGamepadAxis, SdlGamepadButton, SdlVirtualGamepad};

use crate::engine::{
    ControllerSnapshot, EngineCommand, EngineHandle, PhysicalSnapshot, VirtualOutputSnapshot,
    install_emergency_uncloak_hooks,
};
use crate::ui::{AppUiState, draw_compact_main_snapshot};

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
    let mut preview_cards = false;
    let mut preview_one_collapsed = false;
    let mut live_demo_pad = false;
    let mut tray_smoke_ms: Option<u64> = None;
    let mut show_editor_profile: Option<String> = None;
    let mut probe_profile_switch_json: Option<PathBuf> = None;
    let mut test_dpi: Option<u32> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--show" | "--open" => open_immediately = true,
            "--show-editor" => {
                if i + 1 < args.len() {
                    show_editor_profile = Some(args[i + 1].clone());
                    open_immediately = true;
                    i += 1;
                } else {
                    show_editor_profile = Some("xbox-360".to_string());
                    open_immediately = true;
                }
            }
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
            "--preview-cards" => preview_cards = true,
            "--preview-one-collapsed" => preview_one_collapsed = true,
            "--live-demo-pad" => live_demo_pad = true,
            "--tray-smoke-ms" => {
                if i + 1 < args.len() {
                    tray_smoke_ms = Some(args[i + 1].parse().unwrap_or(4000));
                    i += 1;
                }
            }
            "--test-dpi" => {
                if i + 1 < args.len() {
                    test_dpi = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            "--probe-profile-switch" => {
                if i + 1 < args.len() {
                    probe_profile_switch_json = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(probe_out) = probe_profile_switch_json {
        if let Err(err) = run_profile_switch_probe(&probe_out) {
            eprintln!("profile switch probe failed: {err}");
            std::process::exit(1);
        }
        return;
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

    let preview = if preview_one_collapsed {
        Some(vec![build_preview_cards().into_iter().next().unwrap()])
    } else if preview_cards {
        Some(build_preview_cards())
    } else {
        None
    };

    let demo_pad = if live_demo_pad {
        match SdlVirtualGamepad::attach(0x057E, 0x2009, "Nintendo Switch Pro Controller") {
            Ok(vp) => {
                engine.wake();
                std::thread::sleep(Duration::from_millis(260));
                vp.set_axis(SdlGamepadAxis::LeftX, 24000);
                vp.set_axis(SdlGamepadAxis::LeftY, -21000);
                vp.set_axis(SdlGamepadAxis::RightX, 0);
                vp.set_axis(SdlGamepadAxis::RightY, 0);
                vp.set_axis(SdlGamepadAxis::LeftTrigger, i16::MIN);
                vp.set_axis(SdlGamepadAxis::RightTrigger, 12000);
                vp.set_button(SdlGamepadButton::South, true);
                vp.set_button(SdlGamepadButton::Misc1, true);
                vp.pump();
                engine.wake();
                std::thread::sleep(Duration::from_millis(60));
                engine.wake();
                Some(vp)
            }
            Err(e) => {
                eprintln!("live demo pad attach failed: {e}");
                None
            }
        }
    } else {
        None
    };

    if let Err(err) = tray_win32::run_tray_app(
        engine,
        open_immediately,
        show_editor_profile,
        preview,
        tray_smoke_ms,
        test_dpi,
    ) {
        eprintln!("z-controller tray error: {err}");
        std::process::exit(1);
    }
    drop(demo_pad);
}

#[repr(C)]
#[derive(Default)]
struct ProbeXInputGamepad {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    thumb_rx: i16,
    thumb_ry: i16,
}

#[repr(C)]
#[derive(Default)]
struct ProbeXInputState {
    packet_number: u32,
    gamepad: ProbeXInputGamepad,
}

fn read_xinput_buttons() -> Option<(u32, u16)> {
    type FnXInputGetState = unsafe extern "system" fn(u32, *mut ProbeXInputState) -> u32;
    let dll: Vec<u16> = "xinput1_4.dll\0".encode_utf16().collect();
    let module = unsafe { LoadLibraryW(dll.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let proc = unsafe { GetProcAddress(module, b"XInputGetState\0".as_ptr()) }?;
    let func: FnXInputGetState = unsafe { std::mem::transmute_copy(&proc) };
    for user in 0..4 {
        let mut st = ProbeXInputState::default();
        if unsafe { func(user, &mut st) } == 0 {
            return Some((user, st.gamepad.buttons));
        }
    }
    None
}

fn find_text_below_y(
    shapes: &[egui::epaint::ClippedShape],
    needle: &str,
    min_y: f32,
) -> Option<Pos2> {
    fn scan(s: &egui::Shape, needle: &str, min_y: f32, out: &mut Vec<Pos2>) {
        match s {
            egui::Shape::Vec(list) => {
                for sub in list {
                    scan(sub, needle, min_y, out);
                }
            }
            egui::Shape::Text(ts) => {
                if ts.galley.text() == needle {
                    let c = ts.pos + ts.galley.size() * 0.5;
                    if c.y >= min_y {
                        out.push(c);
                    }
                }
            }
            _ => {}
        }
    }
    let mut hits = Vec::new();
    for clipped in shapes {
        scan(&clipped.shape, needle, min_y, &mut hits);
    }
    hits.into_iter()
        .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
}

fn find_text_rect_center(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<Pos2> {
    for clipped in shapes.iter().rev() {
        if let Some(pos) = find_text_in_shape(&clipped.shape, needle) {
            return Some(pos);
        }
    }
    None
}

fn find_text_in_shape(shape: &egui::Shape, needle: &str) -> Option<Pos2> {
    match shape {
        egui::Shape::Vec(list) => list.iter().rev().find_map(|s| find_text_in_shape(s, needle)),
        egui::Shape::Text(ts) => {
            if ts.galley.text() == needle {
                Some(ts.pos + ts.galley.size() * 0.5)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn run_egui_click_step(
    ctx: &egui::Context,
    ui_state: &mut AppUiState,
    snap: &crate::engine::EngineSnapshot,
    time_sec: &mut f64,
    pos: Option<Pos2>,
    click: bool,
    commands: &mut Vec<EngineCommand>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut events = Vec::new();
    if let Some(p) = pos {
        events.push(Event::PointerMoved(p));
        if click {
            events.push(Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            });
        }
    }
    let mut run_once = |ev: Vec<Event>| {
        *time_sec += 0.05;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 600.0))),
            time: Some(*time_sec),
            events: ev,
            focused: true,
            ..Default::default()
        };
        let out = ctx.run(raw, |c| {
            draw_compact_main_snapshot(c, ui_state, snap, |cmd| commands.push(cmd));
        });
        out.shapes
    };

    let shapes = run_once(events);
    if click {
        if let Some(p) = pos {
            return run_once(vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }]);
        }
    }
    shapes
}

#[derive(Serialize)]
struct ProfileSwitchProbeReport {
    passed: bool,
    controller_canonical_id: String,
    before_profile_id: String,
    after_profile_id: String,
    xinput_slot: u32,
    before_xinput_buttons: String,
    after_xinput_buttons: String,
    after_collapse_show_live_input: bool,
    persisted_show_live_input_after_reload: bool,
    after_collapse_xinput_buttons: String,
    emitted_commands_count: usize,
    combo_clicked_xy: [f32; 2],
    item_clicked_xy: [f32; 2],
}

fn run_profile_switch_probe(out_path: &std::path::Path) -> Result<(), String> {
    let cfg_path = std::env::temp_dir().join("z-controller-probe-switch.toml");
    let _ = std::fs::remove_file(&cfg_path);

    let mut cfg = AppConfig::default();
    let mut probe_swap = Profile::builtin_xbox360();
    probe_swap.id = "probe-swap".to_string();
    probe_swap.name = "Probe Swap A->B".to_string();
    probe_swap
        .button_remap
        .insert(LogicalButton::South, Some(LogicalButton::East));
    cfg.profiles.push(probe_swap);
    cfg.save_atomic(&cfg_path).map_err(|e| e.to_string())?;

    let mut engine = EngineHandle::start(cfg_path.clone())?;
    let vp = SdlVirtualGamepad::attach(0x045E, 0x028E, "ZCtrl Probe Pad")
        .map_err(|e| format!("virtual pad attach failed: {e}"))?;

    for _ in 0..30 {
        vp.pump();
        engine.wake();
        std::thread::sleep(Duration::from_millis(80));
        let probe_snap = engine.snapshot();
        if probe_snap
            .controllers
            .iter()
            .any(|c| c.connected && c.key.vid == 0x045E && c.key.pid == 0x028E && c.virtual_active)
        {
            break;
        }
    }

    // Disable any unrelated physical pads plugged into the host so XInput slot 0 belongs solely to ZCtrl Probe Pad.
    for c in &engine.snapshot().controllers {
        if c.key.vid != 0x045E || c.key.pid != 0x028E {
            engine.send(EngineCommand::SetDeviceEnabled {
                key: c.key.clone(),
                enabled: false,
            });
        }
    }
    std::thread::sleep(Duration::from_millis(120));

    vp.set_button(SdlGamepadButton::South, true);
    vp.pump();
    engine.wake();
    std::thread::sleep(Duration::from_millis(240));
    let before_snap = engine.snapshot();

    let ctrl = before_snap
        .controllers
        .iter()
        .find(|c| c.key.vid == 0x045E && c.key.pid == 0x028E)
        .cloned()
        .ok_or_else(|| "probe pad did not enumerate".to_string())?;
    let (slot_idx, before_xi) =
        read_xinput_buttons().ok_or_else(|| "no active XInput slot before switch".to_string())?;

    // Drive the actual compact UI ComboBox via egui RawInput click coordinates.
    let ctx = egui::Context::default();
    let mut ui_state = AppUiState::default();
    let mut emitted = Vec::new();
    let mut time_sec = 0.0_f64;

    let shapes_idle = run_egui_click_step(
        &ctx,
        &mut ui_state,
        &before_snap,
        &mut time_sec,
        None,
        false,
        &mut emitted,
    );
    let header_y = find_text_rect_center(&shapes_idle, &ctrl.name)
        .map_or(0.0, |p| p.y);
    let combo_pos = find_text_below_y(&shapes_idle, &ctrl.profile_name, header_y)
        .ok_or_else(|| format!("ComboBox text '{}' not found in shapes", ctrl.profile_name))?;
    let shapes_open = run_egui_click_step(
        &ctx,
        &mut ui_state,
        &before_snap,
        &mut time_sec,
        Some(combo_pos),
        true,
        &mut emitted,
    );
    // Run one extra popup layout frame so egui finishes popup Area sizing.
    let shapes_popup = run_egui_click_step(
        &ctx,
        &mut ui_state,
        &before_snap,
        &mut time_sec,
        None,
        false,
        &mut emitted,
    );
    let item_pos = find_text_rect_center(&shapes_popup, "Probe Swap A->B")
        .or_else(|| find_text_rect_center(&shapes_open, "Probe Swap A->B"))
        .ok_or_else(|| "ComboBox popup item 'Probe Swap A->B' not found".to_string())?;
    let _ = run_egui_click_step(
        &ctx,
        &mut ui_state,
        &before_snap,
        &mut time_sec,
        Some(item_pos),
        true,
        &mut emitted,
    );

    let cmd_count = emitted.len();
    for cmd in emitted {
        engine.send(cmd);
    }
    vp.set_button(SdlGamepadButton::South, true);
    vp.pump();
    engine.wake();
    std::thread::sleep(Duration::from_millis(260));

    let after_snap = engine.snapshot();
    let after_ctrl = after_snap
        .controllers
        .iter()
        .find(|c| c.canonical_id == ctrl.canonical_id)
        .cloned()
        .ok_or_else(|| "controller missing after switch".to_string())?;
    let (_, after_xi) =
        read_xinput_buttons().ok_or_else(|| "no active XInput slot after switch".to_string())?;

    // Collapse live input section for this controller and verify persistence + uninterrupted virtual output.
    engine.send(EngineCommand::SetDeviceShowLiveInput {
        key: ctrl.key.clone(),
        show_live_input: false,
    });
    vp.set_button(SdlGamepadButton::South, true);
    vp.pump();
    engine.wake();
    std::thread::sleep(Duration::from_millis(180));
    let collapsed_snap = engine.snapshot();
    let collapsed_ctrl = collapsed_snap
        .controllers
        .iter()
        .find(|c| c.canonical_id == ctrl.canonical_id)
        .cloned()
        .ok_or_else(|| "controller missing after collapse".to_string())?;
    let (_, collapsed_xi) =
        read_xinput_buttons().ok_or_else(|| "no active XInput slot after collapse".to_string())?;
    let persisted_show_live = AppConfig::load_or_default(&cfg_path).show_live_input_for_device(&ctrl.key);

    let passed = cmd_count >= 1
        && ctrl.profile_id == "xbox-360"
        && after_ctrl.profile_id == "probe-swap"
        && before_xi == 0x1000
        && after_xi == 0x2000
        && !collapsed_ctrl.show_live_input
        && !persisted_show_live
        && collapsed_xi == 0x2000;

    let report = ProfileSwitchProbeReport {
        passed,
        controller_canonical_id: ctrl.canonical_id,
        before_profile_id: ctrl.profile_id,
        after_profile_id: after_ctrl.profile_id,
        xinput_slot: slot_idx,
        before_xinput_buttons: format!("0x{before_xi:04X}"),
        after_xinput_buttons: format!("0x{after_xi:04X}"),
        after_collapse_show_live_input: collapsed_ctrl.show_live_input,
        persisted_show_live_input_after_reload: persisted_show_live,
        after_collapse_xinput_buttons: format!("0x{collapsed_xi:04X}"),
        emitted_commands_count: cmd_count,
        combo_clicked_xy: [combo_pos.x, combo_pos.y],
        item_clicked_xy: [item_pos.x, item_pos.y],
    };
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(out_path, json);
    }

    drop(vp);
    engine.shutdown();
    let _ = std::fs::remove_file(&cfg_path);

    if !passed {
        return Err(format!(
            "probe assertions failed: before=0x{before_xi:04X} after=0x{after_xi:04X} profile={}->{}" ,
            report.before_profile_id, report.after_profile_id
        ));
    }
    Ok(())
}

fn build_preview_cards() -> Vec<ControllerSnapshot> {
    vec![
        ControllerSnapshot {
            key: HardwareKey::new(0x057E, 0x2009, "NSP98b6b0d12a2c"),
            canonical_id: "057e:2009:NSP98b6b0d12a2c".to_string(),
            sdl_id: Some(1),
            name: "Nintendo Switch Pro Controller".to_string(),
            family: ControllerFamily::Nintendo,
            transport: Transport::Bluetooth,
            connected: true,
            enabled: true,
            show_live_input: false,
            profile_id: "switch-pro".to_string(),
            profile_name: "Nintendo Switch Pro Controller".to_string(),
            target_kind: VirtualTargetKind::Xbox360,
            cloaked: true,
            virtual_active: true,
            virtual_serial: Some(1),
            battery: BatteryReading {
                percent: Some(86),
                status: BatteryStatus::OnBattery,
                source: "sdl3",
            },
            present_buttons_mask: 0x0001_FFFF,
            physical_state: PhysicalSnapshot::default(),
            virtual_output: VirtualOutputSnapshot::None,
            instance_id: None,
        },
        ControllerSnapshot {
            key: HardwareKey::new(0x054C, 0x0CE6, "DS5-USB-02"),
            canonical_id: "054c:0ce6:DS5-USB-02".to_string(),
            sdl_id: Some(2),
            name: "DualSense Wireless Controller".to_string(),
            family: ControllerFamily::Sony,
            transport: Transport::Usb,
            connected: true,
            enabled: false,
            show_live_input: true,
            profile_id: "dualsense".to_string(),
            profile_name: "DualSense".to_string(),
            target_kind: VirtualTargetKind::DualShock4,
            cloaked: false,
            virtual_active: false,
            virtual_serial: None,
            battery: BatteryReading {
                percent: Some(18),
                status: BatteryStatus::OnBattery,
                source: "sony-hid",
            },
            present_buttons_mask: 0x001F_FFFF,
            physical_state: PhysicalSnapshot {
                buttons_mask: LogicalButton::South.bit() | LogicalButton::Touchpad.bit(),
                left_x: 18000,
                left_y: -15000,
                right_x: 0,
                right_y: 0,
                left_trigger: 0,
                right_trigger: 16000,
            },
            virtual_output: VirtualOutputSnapshot::None,
            instance_id: None,
        },
        ControllerSnapshot {
            key: HardwareKey::new(0x045E, 0x0B12, "XBOX-SX-03"),
            canonical_id: "045e:0b12:XBOX-SX-03".to_string(),
            sdl_id: Some(3),
            name: "Xbox Series Wireless Controller".to_string(),
            family: ControllerFamily::Xbox,
            transport: Transport::Bluetooth,
            connected: true,
            enabled: true,
            show_live_input: false,
            profile_id: "xbox-360".to_string(),
            profile_name: "Xbox 360".to_string(),
            target_kind: VirtualTargetKind::Xbox360,
            cloaked: true,
            virtual_active: true,
            virtual_serial: Some(2),
            battery: BatteryReading {
                percent: Some(74),
                status: BatteryStatus::OnBattery,
                source: "xinput",
            },
            present_buttons_mask: LogicalButton::Misc1.bit(),
            physical_state: PhysicalSnapshot::default(),
            virtual_output: VirtualOutputSnapshot::None,
            instance_id: None,
        },
    ]
}
