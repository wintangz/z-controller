use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use serde::Serialize;
use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

use z_core::backend::{DriverStatus, OutputBackend, SlotCoordinator};
use z_core::battery::{BatteryReading, BatteryStatus, reconcile_battery};
use z_core::driver::DriverId;
use z_core::mapping::map_physical_to_virtual;
use z_core::model::{
    ControllerFamily, DisconnectPolicy, HardwareKey, LogicalButton, PhysicalGamepadState,
    Transport, VirtualOutputReport, VirtualTargetKind,
};
use z_core::profile::{AppConfig, Profile, ThemeMode};
use z_platform_windows::backend_impl::WindowsOutputBackend;
use z_platform_windows::drivers::{self, AppLog, RemedyKind};
use z_platform_windows::hidhide;
use z_platform_windows::pnp::{interface_path_to_instance_id, is_vigem_virtual_device_path};
use z_platform_windows::raw_battery::{read_switch_hid_battery, read_xinput_battery};
use z_sdl3::{
    SdlConnectionState, SdlContext, SdlEvent, SdlEventWaker, SdlGamepad, SdlGamepadAxis,
    SdlGamepadButton, SdlGamepadType, SdlJoystickId, SdlPowerState,
};

pub const WM_APP_STATE_CHANGED: u32 = 0x8000 + 1;

#[derive(Debug, Clone, Serialize)]
pub struct ControllerSnapshot {
    pub key: HardwareKey,
    pub canonical_id: String,
    pub sdl_id: Option<u32>,
    pub name: String,
    pub family: ControllerFamily,
    pub transport: Transport,
    pub connected: bool,
    pub enabled: bool,
    pub show_live_input: bool,
    pub profile_id: String,
    pub profile_name: String,
    pub target_kind: VirtualTargetKind,
    pub cloaked: bool,
    pub virtual_active: bool,
    pub virtual_serial: Option<u32>,
    pub battery: BatteryReading,
    pub present_buttons_mask: u32,
    pub physical_state: PhysicalSnapshot,
    pub virtual_output: VirtualOutputSnapshot,
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PhysicalSnapshot {
    pub buttons_mask: u32,
    pub left_x: i16,
    pub left_y: i16,
    pub right_x: i16,
    pub right_y: i16,
    pub left_trigger: i16,
    pub right_trigger: i16,
}

impl From<&PhysicalGamepadState> for PhysicalSnapshot {
    fn from(s: &PhysicalGamepadState) -> Self {
        Self {
            buttons_mask: s.buttons_mask,
            left_x: s.left_x,
            left_y: s.left_y,
            right_x: s.right_x,
            right_y: s.right_y,
            left_trigger: s.left_trigger,
            right_trigger: s.right_trigger,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum VirtualOutputSnapshot {
    None,
    Xbox360 {
        buttons: u16,
        left_trigger: u8,
        right_trigger: u8,
        thumb_lx: i16,
        thumb_ly: i16,
        thumb_rx: i16,
        thumb_ry: i16,
    },
    DualShock4 {
        thumb_lx: u8,
        thumb_ly: u8,
        thumb_rx: u8,
        thumb_ry: u8,
        buttons: u16,
        special: u8,
        trigger_left: u8,
        trigger_right: u8,
    },
}

impl From<&VirtualOutputReport> for VirtualOutputSnapshot {
    fn from(r: &VirtualOutputReport) -> Self {
        match r {
            VirtualOutputReport::Xbox360(x) => Self::Xbox360 {
                buttons: x.buttons,
                left_trigger: x.left_trigger,
                right_trigger: x.right_trigger,
                thumb_lx: x.thumb_lx,
                thumb_ly: x.thumb_ly,
                thumb_rx: x.thumb_rx,
                thumb_ry: x.thumb_ry,
            },
            VirtualOutputReport::DualShock4(d) => Self::DualShock4 {
                thumb_lx: d.thumb_lx,
                thumb_ly: d.thumb_ly,
                thumb_rx: d.thumb_rx,
                thumb_ry: d.thumb_ry,
                buttons: d.buttons,
                special: d.special,
                trigger_left: d.trigger_left,
                trigger_right: d.trigger_right,
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EngineSnapshot {
    pub controllers: Vec<ControllerSnapshot>,
    pub config: AppConfig,
    pub driver_status: DriverStatus,
    pub driver_messages: BTreeMap<DriverId, String>,
    pub busy_driver: Option<DriverId>,
    pub last_status_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineCommand {
    SelectDeviceProfile {
        key: HardwareKey,
        profile_id: String,
    },
    SetDeviceEnabled {
        key: HardwareKey,
        enabled: bool,
    },
    SetDeviceShowLiveInput {
        key: HardwareKey,
        show_live_input: bool,
    },
    SetThemeMode(ThemeMode),
    UpsertProfile(Profile),
    DeleteProfile(String),
    CancelInstallDriver(DriverId),
    ConfirmInstallDriver(DriverId),
    RunDriverRemedy(DriverId),
    ConfirmSystemReboot(DriverId),
    LocateDriverFolder {
        driver: DriverId,
        folder: PathBuf,
    },
    ClearDriverLocation(DriverId),
    #[doc(hidden)]
    InternalDriverTaskDone {
        driver: DriverId,
        reboot_pending: bool,
        message: String,
    },
    RefreshStatus,
    Shutdown,
}

pub struct EngineHandle {
    cmd_tx: Sender<EngineCommand>,
    waker: SdlEventWaker,
    snapshot: Arc<Mutex<Arc<EngineSnapshot>>>,
    window_visible: Arc<AtomicBool>,
    ui_hwnd: Arc<Mutex<usize>>,
    editor_visible: Arc<AtomicBool>,
    editor_hwnd: Arc<Mutex<usize>>,
    join: Option<JoinHandle<()>>,
}

impl EngineHandle {
    pub fn start(config_path: PathBuf, log_path: PathBuf) -> Result<Self, String> {
        let sdl = SdlContext::init().map_err(|e| format!("SDL3 init failed: {e}"))?;
        let waker = sdl.waker();
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(Arc::new(EngineSnapshot::default())));
        let window_visible = Arc::new(AtomicBool::new(false));
        let ui_hwnd = Arc::new(Mutex::new(0usize));
        let editor_visible = Arc::new(AtomicBool::new(false));
        let editor_hwnd = Arc::new(Mutex::new(0usize));

        let snap_clone = Arc::clone(&snapshot);
        let vis_clone = Arc::clone(&window_visible);
        let hwnd_clone = Arc::clone(&ui_hwnd);
        let ed_vis_clone = Arc::clone(&editor_visible);
        let ed_hwnd_clone = Arc::clone(&editor_hwnd);
        let waker_clone = waker.clone();
        let worker_tx = cmd_tx.clone();

        let (ready_tx, ready_rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("z-controller-engine".to_string())
            .spawn(move || {
                run_engine_loop(
                    sdl,
                    waker_clone,
                    config_path,
                    log_path,
                    worker_tx,
                    cmd_rx,
                    snap_clone,
                    vis_clone,
                    hwnd_clone,
                    ed_vis_clone,
                    ed_hwnd_clone,
                    ready_tx,
                );
            })
            .map_err(|e| format!("spawn engine thread: {e}"))?;

        let _ = ready_rx.recv();

        Ok(Self {
            cmd_tx,
            waker,
            snapshot,
            window_visible,
            ui_hwnd,
            editor_visible,
            editor_hwnd,
            join: Some(join),
        })
    }

    pub fn set_ui_hwnd(&self, hwnd: HWND) {
        *self.ui_hwnd.lock().unwrap_or_else(|e| e.into_inner()) = hwnd as usize;
    }

    pub fn set_window_visible(&self, visible: bool) {
        self.window_visible.store(visible, Ordering::Release);
    }

    pub fn set_editor_hwnd(&self, hwnd: HWND) {
        *self.editor_hwnd.lock().unwrap_or_else(|e| e.into_inner()) = hwnd as usize;
    }

    pub fn set_editor_visible(&self, visible: bool) {
        self.editor_visible.store(visible, Ordering::Release);
    }

    pub fn snapshot(&self) -> Arc<EngineSnapshot> {
        Arc::clone(&self.snapshot.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn send(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.send(cmd);
        self.waker.wake(1);
    }

    pub fn wake(&self) {
        self.waker.wake(1);
    }

    pub fn shutdown(&mut self) {
        let _ = self.cmd_tx.send(EngineCommand::Shutdown);
        self.waker.wake(99);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct OpenPad {
    gamepad: SdlGamepad,
    key: HardwareKey,
    name: String,
    family: ControllerFamily,
    transport: Transport,
    instance_id: Option<String>,
    present_buttons_mask: u32,
    state: PhysicalGamepadState,
    battery: BatteryReading,
    opened_at: Instant,
}

fn map_sdl_button(btn: SdlGamepadButton) -> Option<LogicalButton> {
    match btn {
        SdlGamepadButton::South => Some(LogicalButton::South),
        SdlGamepadButton::East => Some(LogicalButton::East),
        SdlGamepadButton::West => Some(LogicalButton::West),
        SdlGamepadButton::North => Some(LogicalButton::North),
        SdlGamepadButton::LeftShoulder => Some(LogicalButton::LeftShoulder),
        SdlGamepadButton::RightShoulder => Some(LogicalButton::RightShoulder),
        SdlGamepadButton::Back => Some(LogicalButton::Back),
        SdlGamepadButton::Start => Some(LogicalButton::Start),
        SdlGamepadButton::Guide => Some(LogicalButton::Guide),
        SdlGamepadButton::LeftStick => Some(LogicalButton::LeftStick),
        SdlGamepadButton::RightStick => Some(LogicalButton::RightStick),
        SdlGamepadButton::DpadUp => Some(LogicalButton::DpadUp),
        SdlGamepadButton::DpadDown => Some(LogicalButton::DpadDown),
        SdlGamepadButton::DpadLeft => Some(LogicalButton::DpadLeft),
        SdlGamepadButton::DpadRight => Some(LogicalButton::DpadRight),
        SdlGamepadButton::Misc1 => Some(LogicalButton::Misc1),
        SdlGamepadButton::RightPaddle1 => Some(LogicalButton::RightPaddle1),
        SdlGamepadButton::LeftPaddle1 => Some(LogicalButton::LeftPaddle1),
        SdlGamepadButton::RightPaddle2 => Some(LogicalButton::RightPaddle2),
        SdlGamepadButton::LeftPaddle2 => Some(LogicalButton::LeftPaddle2),
        SdlGamepadButton::Touchpad => Some(LogicalButton::Touchpad),
        SdlGamepadButton::Misc2 => Some(LogicalButton::Misc2),
        SdlGamepadButton::Misc3 => Some(LogicalButton::Misc3),
        SdlGamepadButton::Misc4 => Some(LogicalButton::Misc4),
        SdlGamepadButton::Misc5 => Some(LogicalButton::Misc5),
        SdlGamepadButton::Misc6 => Some(LogicalButton::Misc6),
    }
}

fn detect_present_buttons_mask(gp: &SdlGamepad) -> u32 {
    let mut mask = 0u32;
    for btn in SdlGamepadButton::ALL {
        if gp.has_button(btn) {
            if let Some(lb) = map_sdl_button(btn) {
                mask |= lb.bit();
            }
        }
    }
    mask
}

fn read_sdl_battery(gp: &SdlGamepad, family: ControllerFamily, path: Option<&str>) -> BatteryReading {
    let pwr = gp.power_info();
    let status = match pwr.state {
        SdlPowerState::OnBattery => BatteryStatus::OnBattery,
        SdlPowerState::Charging => BatteryStatus::Charging,
        SdlPowerState::Charged => BatteryStatus::Charged,
        SdlPowerState::NoBattery => BatteryStatus::Wired,
        _ => BatteryStatus::Unknown,
    };
    let sdl_reading = BatteryReading {
        percent: pwr.percent,
        status,
        source: "sdl3",
    };

    let override_reading = if sdl_reading.percent.is_none() && status == BatteryStatus::Unknown {
        match family {
            ControllerFamily::Nintendo => path.and_then(read_switch_hid_battery).map(|r| {
                BatteryReading {
                    percent: r.percent,
                    status: match r.status {
                        z_platform_windows::raw_battery::RawBatteryStatus::OnBattery => {
                            BatteryStatus::OnBattery
                        }
                        z_platform_windows::raw_battery::RawBatteryStatus::Charging => {
                            BatteryStatus::Charging
                        }
                        z_platform_windows::raw_battery::RawBatteryStatus::Charged => {
                            BatteryStatus::Charged
                        }
                        z_platform_windows::raw_battery::RawBatteryStatus::Wired => {
                            BatteryStatus::Wired
                        }
                        z_platform_windows::raw_battery::RawBatteryStatus::Unknown => {
                            BatteryStatus::Unknown
                        }
                    },
                    source: r.source,
                }
            }),
            ControllerFamily::Xbox => read_xinput_battery(0).map(|r| BatteryReading {
                percent: r.percent,
                status: match r.status {
                    z_platform_windows::raw_battery::RawBatteryStatus::Wired => BatteryStatus::Wired,
                    z_platform_windows::raw_battery::RawBatteryStatus::OnBattery => {
                        BatteryStatus::OnBattery
                    }
                    _ => BatteryStatus::Unknown,
                },
                source: r.source,
            }),
            _ => None,
        }
    } else {
        None
    };

    reconcile_battery(family, sdl_reading, override_reading)
}

fn infer_family(gp: &SdlGamepad, vid: u16, pid: u16, name: &str) -> ControllerFamily {
    match gp.gamepad_type() {
        SdlGamepadType::NintendoSwitchPro
        | SdlGamepadType::NintendoSwitchJoyconLeft
        | SdlGamepadType::NintendoSwitchJoyconRight
        | SdlGamepadType::NintendoSwitchJoyconPair => ControllerFamily::Nintendo,
        SdlGamepadType::Ps3 | SdlGamepadType::Ps4 | SdlGamepadType::Ps5 => ControllerFamily::Sony,
        SdlGamepadType::Xbox360 | SdlGamepadType::XboxOne => ControllerFamily::Xbox,
        _ => ControllerFamily::detect(vid, pid, name),
    }
}

fn infer_transport(gp: &SdlGamepad, path: Option<&str>) -> Transport {
    match gp.connection_state() {
        SdlConnectionState::Wireless => Transport::Bluetooth,
        SdlConnectionState::Wired => Transport::Usb,
        _ => {
            if let Some(p) = path {
                let u = p.to_ascii_uppercase();
                if u.contains("{00001124-") || u.contains("BTHENUM") {
                    Transport::Bluetooth
                } else if u.contains("USB") {
                    Transport::Usb
                } else {
                    Transport::Unknown
                }
            } else {
                Transport::Unknown
            }
        }
    }
}

fn snapshot_physical_state(gp: &SdlGamepad) -> PhysicalGamepadState {
    let mut state = PhysicalGamepadState {
        buttons_mask: 0,
        left_x: gp.axis(SdlGamepadAxis::LeftX),
        left_y: gp.axis(SdlGamepadAxis::LeftY),
        right_x: gp.axis(SdlGamepadAxis::RightX),
        right_y: gp.axis(SdlGamepadAxis::RightY),
        left_trigger: gp.axis(SdlGamepadAxis::LeftTrigger),
        right_trigger: gp.axis(SdlGamepadAxis::RightTrigger),
    };
    for btn in SdlGamepadButton::ALL {
        if let Some(lb) = map_sdl_button(btn) {
            state.set_button(lb, gp.button(btn));
        }
    }
    state
}

#[allow(clippy::too_many_arguments)]
fn run_engine_loop(
    sdl: SdlContext,
    waker: SdlEventWaker,
    config_path: PathBuf,
    log_path: PathBuf,
    worker_tx: Sender<EngineCommand>,
    cmd_rx: Receiver<EngineCommand>,
    snapshot_out: Arc<Mutex<Arc<EngineSnapshot>>>,
    window_visible: Arc<AtomicBool>,
    ui_hwnd: Arc<Mutex<usize>>,
    editor_visible: Arc<AtomicBool>,
    editor_hwnd: Arc<Mutex<usize>>,
    ready_tx: Sender<()>,
) {
    let app_log = AppLog::new(log_path);
    let mut config = AppConfig::load_or_default(&config_path);
    let mut backend = WindowsOutputBackend::new();
    let mut coord = SlotCoordinator::new();
    let mut pads: BTreeMap<SdlJoystickId, OpenPad> = BTreeMap::new();
    let mut cloaked_ids: BTreeSet<String> = BTreeSet::new();
    let mut disconnected_meta: BTreeMap<String, (String, ControllerFamily, Transport, BatteryReading)> =
        BTreeMap::new();
    let mut driver_messages: BTreeMap<DriverId, String> = BTreeMap::new();
    let mut busy_driver: Option<DriverId> = None;
    let mut last_status_message: Option<String> = None;

    // Initial enumeration
    if let Ok(ids) = sdl.gamepad_ids() {
        for id in ids {
            try_open_pad(
                &sdl,
                id,
                &mut pads,
                &config,
                &mut backend,
                &mut coord,
                &waker,
                &mut last_status_message,
            );
        }
    }
    // Brief settle pass so initial Switch/Sony HIDAPI battery packets arrive before entering wait_event
    if !pads.is_empty() {
        let settle_start = Instant::now();
        while settle_start.elapsed().as_millis() < 160 {
            if let Some(ev) = sdl.wait_event_timeout(40) {
                if matches!(ev, SdlEvent::Quit) {
                    break;
                }
                handle_sdl_event(
                    ev,
                    &sdl,
                    &mut pads,
                    &config,
                    &mut backend,
                    &mut coord,
                    &waker,
                    &mut disconnected_meta,
                    &mut last_status_message,
                );
            }
        }
        for pad in pads.values_mut() {
            let p = pad.gamepad.path();
            pad.battery = read_sdl_battery(&pad.gamepad, pad.family, p.as_deref());
        }
    }

    sync_cloaks(&pads, &config, &mut backend, &mut cloaked_ids, &mut last_status_message);
    publish_snapshot(
        &pads,
        &coord,
        &config,
        &backend,
        &cloaked_ids,
        &disconnected_meta,
        &driver_messages,
        busy_driver,
        &last_status_message,
        &snapshot_out,
        &window_visible,
        &ui_hwnd,
        &editor_visible,
        &editor_hwnd,
    );
    let _ = ready_tx.send(());

    let mut running = true;
    while running {
        let needs_initial_batt = pads
            .values()
            .any(|p| p.battery.percent.is_none() && p.opened_at.elapsed().as_millis() < 600);
        let ev = if needs_initial_batt {
            sdl.wait_event_timeout(120)
        } else {
            match sdl.wait_event() {
                Ok(e) => Some(e),
                Err(_) => break,
            }
        };

        let mut dirty = false;
        if let Some(first_ev) = ev {
            let mut batch = vec![first_ev];
            while let Some(more) = sdl.poll_event() {
                batch.push(more);
            }
            for e in batch {
                match e {
                    SdlEvent::Quit => {
                        running = false;
                        break;
                    }
                    SdlEvent::Wakeup { .. } => {
                        while let Ok(cmd) = cmd_rx.try_recv() {
                            match cmd {
                                EngineCommand::Shutdown => {
                                    running = false;
                                }
                                other => {
                                    handle_ui_command(
                                        other,
                                        &config_path,
                                        &app_log,
                                        &worker_tx,
                                        &mut config,
                                        &mut pads,
                                        &mut backend,
                                        &mut coord,
                                        &waker,
                                        &mut driver_messages,
                                        &mut busy_driver,
                                        &mut last_status_message,
                                    );
                                    dirty = true;
                                }
                            }
                        }
                        for pad in pads.values() {
                            if let Some(cmd) = coord.drain_rumble(&pad.key) {
                                let (_, profile) = config.profile_for_device(&pad.key);
                                if profile.rumble_enabled {
                                    let _ = pad.gamepad.rumble(
                                        (cmd.large_motor as u16) * 257,
                                        (cmd.small_motor as u16) * 257,
                                        1500,
                                    );
                                }
                            }
                        }
                    }
                    other => {
                        if handle_sdl_event(
                            other,
                            &sdl,
                            &mut pads,
                            &config,
                            &mut backend,
                            &mut coord,
                            &waker,
                            &mut disconnected_meta,
                            &mut last_status_message,
                        ) {
                            dirty = true;
                        }
                    }
                }
            }
        } else if needs_initial_batt {
            for pad in pads.values_mut() {
                if pad.battery.percent.is_none() {
                    let p = pad.gamepad.path();
                    let next = read_sdl_battery(&pad.gamepad, pad.family, p.as_deref());
                    if next != pad.battery {
                        pad.battery = next;
                        dirty = true;
                    }
                }
            }
        }

        if !running {
            break;
        }

        if dirty {
            sync_cloaks(
                &pads,
                &config,
                &mut backend,
                &mut cloaked_ids,
                &mut last_status_message,
            );
            publish_snapshot(
                &pads,
                &coord,
                &config,
                &backend,
                &cloaked_ids,
                &disconnected_meta,
                &driver_messages,
                busy_driver,
                &last_status_message,
                &snapshot_out,
                &window_visible,
                &ui_hwnd,
                &editor_visible,
                &editor_hwnd,
            );
        }
    }

    let _ = backend.uncloak_all();
    drop(coord);
}

#[allow(clippy::too_many_arguments)]
fn try_open_pad(
    sdl: &SdlContext,
    id: SdlJoystickId,
    pads: &mut BTreeMap<SdlJoystickId, OpenPad>,
    config: &AppConfig,
    backend: &mut WindowsOutputBackend,
    coord: &mut SlotCoordinator,
    waker: &SdlEventWaker,
    last_status_message: &mut Option<String>,
) -> bool {
    if pads.contains_key(&id) {
        return false;
    }
    let Ok(gp) = sdl.open_gamepad(id) else {
        return false;
    };
    let path = gp.path();
    if is_vigem_virtual_device_path(path.as_deref()) {
        return false;
    }
    if !gp.is_virtual() && path.is_none() {
        if let Some(pidx) = gp.player_index() {
            if coord.owns_xinput_user_index(pidx) {
                return false;
            }
        }
        if gp.vendor() == 0x045E
            && gp.product() == 0x028E
            && (coord.has_active_x360() || z_platform_windows::pnp::only_vigem_x360_hid_present())
        {
            return false;
        }
    }

    let vid = gp.vendor();
    let pid = gp.product();
    let name = gp.name();
    let serial = gp.serial().unwrap_or_default();
    let instance_id = path.as_deref().and_then(interface_path_to_instance_id);
    let discriminator = if !serial.is_empty() {
        serial
    } else if let Some(inst) = &instance_id {
        inst.clone()
    } else {
        format!("sdl-{id}")
    };
    let key = HardwareKey::new(vid, pid, discriminator);
    let family = infer_family(&gp, vid, pid, &name);
    let transport = infer_transport(&gp, path.as_deref());
    let present_buttons_mask = detect_present_buttons_mask(&gp);
    let state = snapshot_physical_state(&gp);
    let battery = read_sdl_battery(&gp, family, path.as_deref());

    // Never auto-install any driver if ViGEmBus is absent/not running; only spawn virtual pad if working.
    let (enabled, profile) = config.profile_for_device(&key);
    if enabled && backend.driver_status().vigem_working() {
        let wake_waker = waker.clone();
        if let Err(e) = coord.ensure_pad(
            &key,
            profile.target,
            instance_id.clone(),
            backend,
            Box::new(move || wake_waker.wake(2)),
        ) {
            *last_status_message = Some(format!("Virtual pad init failed: {e}"));
        } else {
            let out = map_physical_to_virtual(&state, profile);
            let _ = coord.submit_dedup(&key, &out);
        }
    }

    pads.insert(
        id,
        OpenPad {
            gamepad: gp,
            key,
            name,
            family,
            transport,
            instance_id,
            present_buttons_mask,
            state,
            battery,
            opened_at: Instant::now(),
        },
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn handle_sdl_event(
    ev: SdlEvent,
    sdl: &SdlContext,
    pads: &mut BTreeMap<SdlJoystickId, OpenPad>,
    config: &AppConfig,
    backend: &mut WindowsOutputBackend,
    coord: &mut SlotCoordinator,
    waker: &SdlEventWaker,
    disconnected_meta: &mut BTreeMap<String, (String, ControllerFamily, Transport, BatteryReading)>,
    last_status_message: &mut Option<String>,
) -> bool {
    match ev {
        SdlEvent::GamepadAdded(id) => {
            try_open_pad(sdl, id, pads, config, backend, coord, waker, last_status_message)
        }
        SdlEvent::GamepadRemoved(id) => {
            if let Some(old) = pads.remove(&id) {
                let (_, profile) = config.profile_for_device(&old.key);
                if profile.disconnect_policy == DisconnectPolicy::KeepNeutral {
                    disconnected_meta.insert(
                        old.key.canonical_id(),
                        (old.name, old.family, old.transport, old.battery),
                    );
                } else {
                    disconnected_meta.remove(&old.key.canonical_id());
                }
                let _ = coord.on_disconnect(&old.key, profile.disconnect_policy);
                true
            } else {
                false
            }
        }
        SdlEvent::GamepadAxisMotion { which, axis, value } => {
            if let Some(pad) = pads.get_mut(&which) {
                let changed = match axis {
                    SdlGamepadAxis::LeftX => {
                        let c = pad.state.left_x != value;
                        pad.state.left_x = value;
                        c
                    }
                    SdlGamepadAxis::LeftY => {
                        let c = pad.state.left_y != value;
                        pad.state.left_y = value;
                        c
                    }
                    SdlGamepadAxis::RightX => {
                        let c = pad.state.right_x != value;
                        pad.state.right_x = value;
                        c
                    }
                    SdlGamepadAxis::RightY => {
                        let c = pad.state.right_y != value;
                        pad.state.right_y = value;
                        c
                    }
                    SdlGamepadAxis::LeftTrigger => {
                        let c = pad.state.left_trigger != value;
                        pad.state.left_trigger = value;
                        c
                    }
                    SdlGamepadAxis::RightTrigger => {
                        let c = pad.state.right_trigger != value;
                        pad.state.right_trigger = value;
                        c
                    }
                };
                if !changed {
                    return false;
                }
                let (enabled, profile) = config.profile_for_device(&pad.key);
                if enabled {
                    let report = map_physical_to_virtual(&pad.state, profile);
                    let _ = coord.submit_dedup(&pad.key, &report);
                }
                true
            } else {
                false
            }
        }
        SdlEvent::GamepadButton {
            which,
            button,
            down,
        } => {
            if let (Some(pad), Some(lb)) = (pads.get_mut(&which), map_sdl_button(button)) {
                if pad.state.is_pressed(lb) == down {
                    return false;
                }
                pad.state.set_button(lb, down);
                let (enabled, profile) = config.profile_for_device(&pad.key);
                if enabled {
                    let report = map_physical_to_virtual(&pad.state, profile);
                    let _ = coord.submit_dedup(&pad.key, &report);
                }
                true
            } else {
                false
            }
        }
        SdlEvent::BatteryUpdated {
            which,
            state,
            percent,
        } => {
            if let Some(pad) = pads.get_mut(&which) {
                let mapped_status = match state {
                    SdlPowerState::OnBattery => BatteryStatus::OnBattery,
                    SdlPowerState::Charging => BatteryStatus::Charging,
                    SdlPowerState::Charged => BatteryStatus::Charged,
                    SdlPowerState::NoBattery => BatteryStatus::Wired,
                    _ => BatteryStatus::Unknown,
                };
                pad.battery = BatteryReading {
                    percent,
                    status: mapped_status,
                    source: "sdl3-event",
                };
                true
            } else {
                false
            }
        }
        SdlEvent::GamepadRemapped(which) => {
            if let Some(pad) = pads.get_mut(&which) {
                pad.state = snapshot_physical_state(&pad.gamepad);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_ui_command(
    cmd: EngineCommand,
    config_path: &PathBuf,
    app_log: &AppLog,
    worker_tx: &Sender<EngineCommand>,
    config: &mut AppConfig,
    pads: &mut BTreeMap<SdlJoystickId, OpenPad>,
    backend: &mut WindowsOutputBackend,
    coord: &mut SlotCoordinator,
    waker: &SdlEventWaker,
    driver_messages: &mut BTreeMap<DriverId, String>,
    busy_driver: &mut Option<DriverId>,
    last_status_message: &mut Option<String>,
) {
    match cmd {
        EngineCommand::SelectDeviceProfile { key, profile_id } => {
            let (cur_enabled, _) = config.profile_for_device(&key);
            config.set_device_binding(&key, profile_id, cur_enabled);
            let _ = config.save_atomic(config_path);
            reconcile_slots(pads, config, backend, coord, waker, last_status_message);
        }
        EngineCommand::SetDeviceEnabled { key, enabled } => {
            if enabled && !backend.driver_status().vigem_working() {
                // Requirement 1 & 8: virtual-pad toggle NEVER triggers driver install; keep OFF when ViGEmBus is not working.
                let pid = config.profile_for_device(&key).1.id.clone();
                config.set_device_binding(&key, pid, false);
                let _ = config.save_atomic(config_path);
                *last_status_message = Some(
                    "Virtual pad needs ViGEmBus — install or start ViGEmBus under Drivers."
                        .to_string(),
                );
                return;
            }
            let pid = config.profile_for_device(&key).1.id.clone();
            config.set_device_binding(&key, pid, enabled);
            let _ = config.save_atomic(config_path);
            reconcile_slots(pads, config, backend, coord, waker, last_status_message);
        }
        EngineCommand::SetDeviceShowLiveInput {
            key,
            show_live_input,
        } => {
            config.set_device_show_live_input(&key, show_live_input);
            let _ = config.save_atomic(config_path);
        }
        EngineCommand::SetThemeMode(mode) => {
            config.theme_mode = mode;
            let _ = config.save_atomic(config_path);
        }
        EngineCommand::UpsertProfile(profile) => {
            if let Some(existing) = config.profiles.iter_mut().find(|p| p.id == profile.id) {
                *existing = profile;
            } else {
                config.profiles.push(profile);
            }
            config.ensure_valid();
            let _ = config.save_atomic(config_path);
            reconcile_slots(pads, config, backend, coord, waker, last_status_message);
        }
        EngineCommand::DeleteProfile(profile_id) => {
            if config.profiles.len() > 1 {
                config.profiles.retain(|p| p.id != profile_id);
                config.ensure_valid();
                let fallback = config.default_profile_id.clone();
                for binding in config.bindings.values_mut() {
                    if binding.profile_id == profile_id {
                        binding.profile_id = fallback.clone();
                    }
                }
                let _ = config.save_atomic(config_path);
                reconcile_slots(pads, config, backend, coord, waker, last_status_message);
            }
        }
        EngineCommand::CancelInstallDriver(driver) => {
            // Requirement 3: cancelling launches nothing and logs nothing beyond the cancellation.
            app_log.append(
                "INFO",
                &format!(
                    "Install cancelled in confirmation dialog: driver={} publisher={}",
                    driver.display_name(),
                    driver.publisher_short()
                ),
            );
        }
        EngineCommand::ConfirmInstallDriver(driver) => {
            if busy_driver.is_some() {
                return;
            }
            *busy_driver = Some(driver);
            driver_messages.insert(
                driver,
                format!("Installer wizard open for {}...", driver.display_name()),
            );
            let already_reboot = backend.reboot_pending_drivers.contains(&driver);
            let log_clone = app_log.clone();
            let tx_clone = worker_tx.clone();
            let waker_clone = waker.clone();
            thread::Builder::new()
                .name(format!("z-ctrl-inst-{}", driver.service_name()))
                .spawn(move || {
                    let (report, reboot_pending) =
                        drivers::run_driver_installer_wizard(driver, already_reboot, &log_clone);
                    let _ = tx_clone.send(EngineCommand::InternalDriverTaskDone {
                        driver,
                        reboot_pending,
                        message: report.summary(driver),
                    });
                    waker_clone.wake(1);
                })
                .ok();
        }
        EngineCommand::RunDriverRemedy(driver) => {
            if busy_driver.is_some() {
                return;
            }
            let reboot_flag = backend.reboot_pending_drivers.contains(&driver);
            let entry = drivers::detect_single_driver(driver, reboot_flag);
            let Some(remedy) = drivers::remedy_for_entry(&entry) else {
                driver_messages.insert(
                    driver,
                    entry
                        .detail_note
                        .unwrap_or_else(|| "Re-checked status.".to_string()),
                );
                return;
            };
            if remedy.requires_reboot_confirm() {
                return;
            }
            *busy_driver = Some(driver);
            driver_messages.insert(driver, format!("Running remedy: {}...", remedy.button_label()));
            let log_clone = app_log.clone();
            let tx_clone = worker_tx.clone();
            let waker_clone = waker.clone();
            thread::Builder::new()
                .name(format!("z-ctrl-remedy-{}", driver.service_name()))
                .spawn(move || {
                    let message = match drivers::run_driver_remedy(driver, &remedy, &log_clone) {
                        Ok(msg) => msg,
                        Err(err) => err,
                    };
                    let _ = tx_clone.send(EngineCommand::InternalDriverTaskDone {
                        driver,
                        reboot_pending: false,
                        message,
                    });
                    waker_clone.wake(1);
                })
                .ok();
        }
        EngineCommand::ConfirmSystemReboot(driver) => {
            app_log.append(
                "INFO",
                &format!(
                    "User confirmed system reboot for {} driver setup.",
                    driver.display_name()
                ),
            );
            match drivers::run_driver_remedy(driver, &RemedyKind::RestartMachine, app_log) {
                Ok(msg) => {
                    driver_messages.insert(driver, msg.clone());
                    *last_status_message = Some(msg);
                }
                Err(err) => {
                    driver_messages.insert(driver, err.clone());
                    *last_status_message = Some(err);
                }
            }
        }
        EngineCommand::LocateDriverFolder { driver, folder } => {
            let reboot_flag = backend.reboot_pending_drivers.contains(&driver);
            let outcome = drivers::locate_and_verify_driver_folder(
                driver,
                &folder,
                reboot_flag,
                app_log,
            );
            if outcome.is_accepted() {
                config.set_located_dir(driver, Some(folder.display().to_string()));
                let _ = config.save_atomic(config_path);
            }
            let msg = outcome.summary_message();
            driver_messages.insert(driver, msg.clone());
            *last_status_message = Some(msg);
            reconcile_slots(pads, config, backend, coord, waker, last_status_message);
        }
        EngineCommand::ClearDriverLocation(driver) => {
            config.set_located_dir(driver, None);
            let _ = config.save_atomic(config_path);
            let msg = format!("{}: cleared custom located folder.", driver.display_name());
            app_log.append("INFO", &msg);
            driver_messages.insert(driver, msg);
        }
        EngineCommand::InternalDriverTaskDone {
            driver,
            reboot_pending,
            message,
        } => {
            *busy_driver = None;
            if reboot_pending {
                backend.reboot_pending_drivers.insert(driver);
            } else {
                backend.reboot_pending_drivers.remove(&driver);
            }
            driver_messages.insert(driver, message.clone());
            *last_status_message = Some(message);
            reconcile_slots(pads, config, backend, coord, waker, last_status_message);
        }
        EngineCommand::RefreshStatus => {
            reconcile_slots(pads, config, backend, coord, waker, last_status_message);
        }
        EngineCommand::Shutdown => {}
    }
}

fn reconcile_slots(
    pads: &BTreeMap<SdlJoystickId, OpenPad>,
    config: &AppConfig,
    backend: &mut WindowsOutputBackend,
    coord: &mut SlotCoordinator,
    waker: &SdlEventWaker,
    last_status_message: &mut Option<String>,
) {
    let vigem_ok = backend.driver_status().vigem_working();
    for pad in pads.values() {
        let (enabled, profile) = config.profile_for_device(&pad.key);
        if enabled && vigem_ok {
            let wake_waker = waker.clone();
            if let Err(e) = coord.ensure_pad(
                &pad.key,
                profile.target,
                pad.instance_id.clone(),
                backend,
                Box::new(move || wake_waker.wake(2)),
            ) {
                *last_status_message = Some(format!("Virtual pad error: {e}"));
            } else {
                let report = map_physical_to_virtual(&pad.state, profile);
                let _ = coord.submit_dedup(&pad.key, &report);
            }
        } else {
            coord.remove(&pad.key);
        }
    }
}

fn sync_cloaks(
    pads: &BTreeMap<SdlJoystickId, OpenPad>,
    config: &AppConfig,
    backend: &mut WindowsOutputBackend,
    cloaked_ids: &mut BTreeSet<String>,
    last_status_message: &mut Option<String>,
) {
    if !backend.driver_status().hidhide_working() {
        // Requirement 8: when HidHide is not working, only cloaking is unavailable; mapping & virtual pads continue.
        cloaked_ids.clear();
        return;
    }
    let mut desired = BTreeSet::new();
    for pad in pads.values() {
        let (enabled, profile) = config.profile_for_device(&pad.key);
        if enabled && profile.cloak_physical {
            if let Some(inst) = &pad.instance_id {
                desired.insert(inst.to_ascii_uppercase());
            }
        }
    }
    match backend.sync_cloaked_instances(&desired) {
        Ok(()) => {
            *cloaked_ids = desired;
        }
        Err(e) => {
            *last_status_message = Some(format!("HidHide cloak warning: {e}"));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn publish_snapshot(
    pads: &BTreeMap<SdlJoystickId, OpenPad>,
    coord: &SlotCoordinator,
    config: &AppConfig,
    backend: &WindowsOutputBackend,
    cloaked_ids: &BTreeSet<String>,
    disconnected_meta: &BTreeMap<String, (String, ControllerFamily, Transport, BatteryReading)>,
    driver_messages: &BTreeMap<DriverId, String>,
    busy_driver: Option<DriverId>,
    last_status_message: &Option<String>,
    snapshot_out: &Arc<Mutex<Arc<EngineSnapshot>>>,
    window_visible: &Arc<AtomicBool>,
    ui_hwnd: &Arc<Mutex<usize>>,
    editor_visible: &Arc<AtomicBool>,
    editor_hwnd: &Arc<Mutex<usize>>,
) {
    let mut controllers = Vec::new();
    let mut seen_ids = BTreeSet::new();

    for pad in pads.values() {
        let canonical = pad.key.canonical_id();
        seen_ids.insert(canonical.clone());
        let (enabled, profile) = config.profile_for_device(&pad.key);
        let slot = coord.get(&pad.key);
        let cloaked = pad
            .instance_id
            .as_ref()
            .is_some_and(|i| cloaked_ids.contains(&i.to_ascii_uppercase()));
        let virt_out = slot
            .and_then(|s| s.last_sent.as_ref())
            .map(VirtualOutputSnapshot::from)
            .unwrap_or(VirtualOutputSnapshot::None);
        controllers.push(ControllerSnapshot {
            key: pad.key.clone(),
            canonical_id: canonical,
            sdl_id: Some(pad.gamepad.id()),
            name: pad.name.clone(),
            family: pad.family,
            transport: pad.transport,
            connected: true,
            enabled,
            show_live_input: config.show_live_input_for_device(&pad.key),
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            target_kind: profile.target,
            cloaked,
            virtual_active: slot.is_some(),
            virtual_serial: slot.map(|s| s.pad.serial_no()),
            battery: pad.battery,
            present_buttons_mask: pad.present_buttons_mask,
            physical_state: PhysicalSnapshot::from(&pad.state),
            virtual_output: virt_out,
            instance_id: pad.instance_id.clone(),
        });
    }

    for (canonical, (name, family, transport, batt)) in disconnected_meta {
        if seen_ids.contains(canonical) {
            continue;
        }
        if let Some(binding_key) = parse_canonical_key(canonical) {
            if let Some(slot) = coord.get(&binding_key) {
                let (enabled, profile) = config.profile_for_device(&binding_key);
                let show_live_input = config.show_live_input_for_device(&binding_key);
                let virt_out = slot
                    .last_sent
                    .as_ref()
                    .map(VirtualOutputSnapshot::from)
                    .unwrap_or(VirtualOutputSnapshot::None);
                controllers.push(ControllerSnapshot {
                    key: binding_key,
                    canonical_id: canonical.clone(),
                    sdl_id: None,
                    name: format!("{name} (Virtual Held Neutral)"),
                    family: *family,
                    transport: *transport,
                    connected: false,
                    enabled,
                    show_live_input,
                    profile_id: profile.id.clone(),
                    profile_name: profile.name.clone(),
                    target_kind: profile.target,
                    cloaked: false,
                    virtual_active: true,
                    virtual_serial: Some(slot.pad.serial_no()),
                    battery: *batt,
                    present_buttons_mask: 0,
                    physical_state: PhysicalSnapshot::default(),
                    virtual_output: virt_out,
                    instance_id: None,
                });
            }
        }
    }

    let snap = Arc::new(EngineSnapshot {
        controllers,
        config: config.clone(),
        driver_status: backend.driver_status(),
        driver_messages: driver_messages.clone(),
        busy_driver,
        last_status_message: last_status_message.clone(),
    });
    *snapshot_out.lock().unwrap_or_else(|e| e.into_inner()) = snap;

    if window_visible.load(Ordering::Acquire) {
        let hwnd_val = *ui_hwnd.lock().unwrap_or_else(|e| e.into_inner());
        if hwnd_val != 0 {
            unsafe {
                PostMessageW(
                    hwnd_val as HWND,
                    WM_APP_STATE_CHANGED,
                    WPARAM::default(),
                    LPARAM::default(),
                );
            }
        }
    }
    if editor_visible.load(Ordering::Acquire) {
        let hwnd_val = *editor_hwnd.lock().unwrap_or_else(|e| e.into_inner());
        if hwnd_val != 0 {
            unsafe {
                PostMessageW(
                    hwnd_val as HWND,
                    WM_APP_STATE_CHANGED,
                    WPARAM::default(),
                    LPARAM::default(),
                );
            }
        }
    }
}

fn parse_canonical_key(canonical: &str) -> Option<HardwareKey> {
    let mut parts = canonical.splitn(3, ':');
    let vid = u16::from_str_radix(parts.next()?, 16).ok()?;
    let pid = u16::from_str_radix(parts.next()?, 16).ok()?;
    let disc = parts.next()?.to_string();
    Some(HardwareKey::new(vid, pid, disc))
}

pub fn install_emergency_uncloak_hooks() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = hidhide::emergency_uncloak();
        prev(info);
    }));

    unsafe extern "system" fn console_ctrl(_ctrl_type: u32) -> i32 {
        let _ = hidhide::emergency_uncloak();
        0
    }
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console_ctrl), 1);
    }
}
