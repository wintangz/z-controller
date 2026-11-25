use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex, Once};
use std::time::Instant;

use egui::{
    Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Vec2, ViewportId,
};
use windows_sys::Win32::Foundation::{FreeLibrary, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos,
    GetMessageW, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, HICON, IDC_ARROW, IMAGE_ICON,
    LR_DEFAULTCOLOR, LoadCursorW, LoadImageW, MF_STRING, MSG, PM_REMOVE, PeekMessageW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SM_CXICON, SM_CXSMICON, SM_CYICON,
    SM_CYSMICON, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, TrackPopupMenu, TranslateMessage, WM_CHAR, WM_CLOSE, WM_COMMAND, WM_DESTROY,
    WM_ENDSESSION, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEWHEEL, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE, WNDCLASSEXW,
    WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_OVERLAPPEDWINDOW, WS_SYSMENU,
    WS_THICKFRAME,
};

use crate::engine::{ControllerSnapshot, EngineHandle, EngineSnapshot, WM_APP_STATE_CHANGED};
use crate::sw_painter::SoftwarePainter;
use crate::ui::{
    AppUiState, UiAction, configure_neon_theme_and_fonts, draw_compact_main_snapshot,
    draw_profile_editor_snapshot, static_ui_hash,
};

const WM_TRAYICON: u32 = 0x8000 + 2;
const WM_APP_IMMEDIATE_REPAINT: u32 = 0x8000 + 3;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_ENTERSIZEMOVE: u32 = 0x0231;
const WM_EXITSIZEMOVE: u32 = 0x0232;
const WM_GETMINMAXINFO: u32 = 0x0024;
const WM_NCHITTEST: u32 = 0x0084;
const WM_SIZING: u32 = 0x0214;
const CMD_OPEN: usize = 1001;
const CMD_QUIT: usize = 1002;
const CLEAR_BG_RGB: u32 = 0x000A_0A10;

// Logical client dimensions at 96 DPI (exact same UI geometry at 100%, scaled cleanly per monitor DPI).
const COMPACT_CLIENT_LOGICAL_W: i32 = 396;
const COMPACT_CLIENT_LOGICAL_H: i32 = 601;
const COMPACT_CLIENT_MIN_LOGICAL_H: i32 = 150;
const COMPACT_CLIENT_MAX_AUTO_LOGICAL_H: i32 = 760;
const EDITOR_CLIENT_LOGICAL_W: i32 = 1024;
const EDITOR_CLIENT_LOGICAL_H: i32 = 641;
// Main compact window locks horizontal width (no Maximize, vertical resize only).
const MAIN_WINDOW_STYLE: u32 =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_THICKFRAME;
const EDITOR_WINDOW_STYLE: u32 =
    WS_OVERLAPPEDWINDOW | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;

#[repr(C)]
struct MinMaxInfo {
    pt_reserved: POINT,
    pt_max_size: POINT,
    pt_max_position: POINT,
    pt_min_track_size: POINT,
    pt_max_track_size: POINT,
}

#[inline]
fn main_fixed_outer_width(dpi: u32) -> i32 {
    outer_size_for_dpi(
        COMPACT_CLIENT_LOGICAL_W,
        COMPACT_CLIENT_LOGICAL_H,
        MAIN_WINDOW_STYLE,
        dpi,
    )
    .0
}

fn max_auto_logical_height(dpi: u32) -> i32 {
    let screen_phys_h = unsafe { GetSystemMetrics(1 /* SM_CYSCREEN */) };
    if screen_phys_h > 200 {
        let ppp = dpi_to_ppp(dpi);
        let work_logical = ((screen_phys_h - 90) as f32 / ppp).floor() as i32;
        work_logical.clamp(240, COMPACT_CLIENT_MAX_AUTO_LOGICAL_H)
    } else {
        COMPACT_CLIENT_MAX_AUTO_LOGICAL_H
    }
}

static REGISTER_EDITOR_CLASS: Once = Once::new();

#[inline]
fn dpi_to_ppp(dpi: u32) -> f32 {
    (dpi.max(96) as f32) / 96.0
}

#[inline]
fn physical_to_logical_pos(x: f32, y: f32, pixels_per_point: f32) -> Pos2 {
    let scale = pixels_per_point.max(0.5);
    Pos2::new(x / scale, y / scale)
}

fn get_dpi_for_system() -> u32 {
    type FnGetDpiForSystem = unsafe extern "system" fn() -> u32;
    let dll: Vec<u16> = "user32.dll\0".encode_utf16().collect();
    let module = unsafe { GetModuleHandleW(dll.as_ptr()) };
    if module.is_null() {
        return 96;
    }
    if let Some(proc) = unsafe { GetProcAddress(module, b"GetDpiForSystem\0".as_ptr()) } {
        let f: FnGetDpiForSystem = unsafe { std::mem::transmute_copy(&proc) };
        let dpi = unsafe { f() };
        if dpi >= 72 {
            return dpi;
        }
    }
    96
}

fn get_dpi_for_window(hwnd: HWND) -> u32 {
    if hwnd.is_null() {
        return get_dpi_for_system();
    }
    type FnGetDpiForWindow = unsafe extern "system" fn(HWND) -> u32;
    let dll: Vec<u16> = "user32.dll\0".encode_utf16().collect();
    let module = unsafe { GetModuleHandleW(dll.as_ptr()) };
    if !module.is_null() {
        if let Some(proc) = unsafe { GetProcAddress(module, b"GetDpiForWindow\0".as_ptr()) } {
            let f: FnGetDpiForWindow = unsafe { std::mem::transmute_copy(&proc) };
            let dpi = unsafe { f(hwnd) };
            if dpi >= 72 {
                return dpi;
            }
        }
    }
    get_dpi_for_system()
}

fn outer_size_for_dpi(
    client_logical_w: i32,
    client_logical_h: i32,
    style: u32,
    dpi: u32,
) -> (i32, i32) {
    let ppp = dpi_to_ppp(dpi);
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: (client_logical_w as f32 * ppp).round() as i32,
        bottom: (client_logical_h as f32 * ppp).round() as i32,
    };
    type FnAdjustForDpi = unsafe extern "system" fn(*mut RECT, u32, i32, u32, u32) -> i32;
    let dll: Vec<u16> = "user32.dll\0".encode_utf16().collect();
    let module = unsafe { GetModuleHandleW(dll.as_ptr()) };
    let adjusted = if !module.is_null() {
        if let Some(proc) = unsafe { GetProcAddress(module, b"AdjustWindowRectExForDpi\0".as_ptr()) }
        {
            let f: FnAdjustForDpi = unsafe { std::mem::transmute_copy(&proc) };
            unsafe { f(&mut rc, style, 0, 0, dpi.max(96)) != 0 }
        } else {
            unsafe { AdjustWindowRectEx(&mut rc, style, 0, 0) != 0 }
        }
    } else {
        unsafe { AdjustWindowRectEx(&mut rc, style, 0, 0) != 0 }
    };
    if adjusted {
        (rc.right - rc.left, rc.bottom - rc.top)
    } else {
        (
            (client_logical_w as f32 * ppp).round() as i32 + 16,
            (client_logical_h as f32 * ppp).round() as i32 + 39,
        )
    }
}

/// Computes outer window size for an existing HWND using its desired logical client size
/// (`logical_dpi`) plus the OS non-client frame size at the window's actual monitor DPI.
fn outer_size_for_hwnd(
    hwnd: HWND,
    client_logical_w: i32,
    client_logical_h: i32,
    style: u32,
    logical_dpi: u32,
) -> (i32, i32) {
    let ppp = dpi_to_ppp(logical_dpi);
    let client_w = (client_logical_w as f32 * ppp).round() as i32;
    let client_h = (client_logical_h as f32 * ppp).round() as i32;
    let nc_dpi = get_dpi_for_window(hwnd);
    let (nc_outer_w, nc_outer_h) = outer_size_for_dpi(0, 0, style, nc_dpi);
    (client_w + nc_outer_w, client_h + nc_outer_h)
}

fn sync_window_dpi_and_bounds(
    hwnd: HWND,
    style: u32,
    client_logical_w: i32,
    client_logical_h: i32,
) -> (u32, usize, usize) {
    let dpi = get_dpi_for_window(hwnd);
    let (outer_w, outer_h) = outer_size_for_dpi(client_logical_w, client_logical_h, style, dpi);
    unsafe {
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            outer_w,
            outer_h,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    let mut crc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(hwnd, &mut crc) };
    (
        dpi,
        (crc.right - crc.left).max(0) as usize,
        (crc.bottom - crc.top).max(0) as usize,
    )
}

fn send_test_dpi_change(
    hwnd: HWND,
    style: u32,
    dpi: u32,
    client_logical_w: i32,
    client_logical_h: i32,
) {
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetWindowRect(hwnd, &mut rc) };
    let (outer_w, outer_h) = outer_size_for_hwnd(hwnd, client_logical_w, client_logical_h, style, dpi);
    let suggested = RECT {
        left: rc.left,
        top: rc.top,
        right: rc.left + outer_w,
        bottom: rc.top + outer_h,
    };
    let wp = ((dpi & 0xFFFF) << 16) | (dpi & 0xFFFF);
    unsafe {
        SendMessageW(
            hwnd,
            WM_DPICHANGED,
            wp as WPARAM,
            (&suggested as *const RECT) as LPARAM,
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowRole {
    Main,
    Editor,
}

struct AppShared {
    engine: Arc<Mutex<EngineHandle>>,
    ui_state: Mutex<AppUiState>,
    preview_controllers: Option<Vec<ControllerSnapshot>>,
    start_time: Instant,
    hinstance: HINSTANCE,
    large_icon: HICON,
    small_icon: HICON,
    main_hwnd: Mutex<usize>,
    editor_hwnd: Mutex<usize>,
}

struct WindowHostState {
    shared: Arc<AppShared>,
    role: WindowRole,
    ui_session: Option<(egui::Context, SoftwarePainter)>,
    dark_titlebar_applied: bool,
    events: Vec<Event>,
    width: usize,
    height: usize,
    dpi: u32,
    pixels_per_point: f32,
    visible: bool,
    followup_paint_budget: u8,
    in_size_move: bool,
    pending_state_refresh: bool,
    needs_redraw: bool,
    dirty_only_rect: Option<RECT>,
    last_auto_logical_h: Option<i32>,
}

fn load_app_icons(hinstance: HINSTANCE) -> (HICON, HICON) {
    let cx_lg = unsafe { GetSystemMetrics(SM_CXICON) }.max(32);
    let cy_lg = unsafe { GetSystemMetrics(SM_CYICON) }.max(32);
    let cx_sm = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    let cy_sm = unsafe { GetSystemMetrics(SM_CYSMICON) }.max(16);

    let res_id = 1usize as *const u16;
    let large = unsafe { LoadImageW(hinstance, res_id, IMAGE_ICON, cx_lg, cy_lg, LR_DEFAULTCOLOR) }
        as HICON;
    let small = unsafe { LoadImageW(hinstance, res_id, IMAGE_ICON, cx_sm, cy_sm, LR_DEFAULTCOLOR) }
        as HICON;

    if !large.is_null() && !small.is_null() {
        (large, small)
    } else {
        let fb = unsafe { create_gamepad_icon() };
        (
            if large.is_null() { fb } else { large },
            if small.is_null() { fb } else { small },
        )
    }
}

pub fn run_tray_app(
    engine: EngineHandle,
    open_immediately: bool,
    initial_editor_profile: Option<String>,
    preview_controllers: Option<Vec<ControllerSnapshot>>,
    smoke_exit_after_ms: Option<u64>,
    test_dpi: Option<u32>,
) -> Result<(), String> {
    let hinstance = unsafe { GetModuleHandleW(null()) } as HINSTANCE;
    let class_name: Vec<u16> = "ZControllerTrayHost\0".encode_utf16().collect();
    let title: Vec<u16> = "z-controller\0".encode_utf16().collect();

    let (large_icon, small_icon) = load_app_icons(hinstance);

    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: large_icon,
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: small_icon,
    };
    if unsafe { RegisterClassExW(&wc) } == 0 {
        return Err("RegisterClassExW failed".into());
    }

    let shared = Arc::new(AppShared {
        engine: Arc::new(Mutex::new(engine)),
        ui_state: Mutex::new(AppUiState::default()),
        preview_controllers,
        start_time: Instant::now(),
        hinstance,
        large_icon,
        small_icon,
        main_hwnd: Mutex::new(0),
        editor_hwnd: Mutex::new(0),
    });

    let show_main = open_immediately && initial_editor_profile.is_none();
    let init_dpi = get_dpi_for_system();
    let (init_outer_w, init_outer_h) = outer_size_for_dpi(
        COMPACT_CLIENT_LOGICAL_W,
        COMPACT_CLIENT_LOGICAL_H,
        MAIN_WINDOW_STYLE,
        init_dpi,
    );

    let main_state = Box::new(WindowHostState {
        shared: Arc::clone(&shared),
        role: WindowRole::Main,
        ui_session: None,
        dark_titlebar_applied: show_main,
        events: Vec::new(),
        width: (COMPACT_CLIENT_LOGICAL_W as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        height: (COMPACT_CLIENT_LOGICAL_H as f32 * dpi_to_ppp(init_dpi)).round() as usize,
        dpi: init_dpi,
        pixels_per_point: dpi_to_ppp(init_dpi),
        visible: show_main,
        followup_paint_budget: 0,
        in_size_move: false,
        pending_state_refresh: false,
        needs_redraw: true,
        dirty_only_rect: None,
        last_auto_logical_h: None,
    });
    let main_state_ptr = Box::into_raw(main_state);

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            MAIN_WINDOW_STYLE,
            120,
            100,
            init_outer_w,
            init_outer_h,
            null_mut(),
            null_mut(),
            hinstance,
            main_state_ptr.cast(),
        )
    };
    if hwnd.is_null() {
        unsafe { drop(Box::from_raw(main_state_ptr)) };
        return Err("CreateWindowExW failed".into());
    }

    {
        let (actual_dpi, cw, ch) = sync_window_dpi_and_bounds(
            hwnd,
            MAIN_WINDOW_STYLE,
            COMPACT_CLIENT_LOGICAL_W,
            COMPACT_CLIENT_LOGICAL_H,
        );
        if let Some(st) = unsafe { host_state_mut(hwnd) } {
            st.dpi = actual_dpi;
            st.pixels_per_point = dpi_to_ppp(actual_dpi);
            st.width = cw;
            st.height = ch;
        }
    }

    *shared.main_hwnd.lock().unwrap_or_else(|e| e.into_inner()) = hwnd as usize;
    {
        let eng = shared.engine.lock().unwrap_or_else(|e| e.into_inner());
        eng.set_ui_hwnd(hwnd);
        eng.set_window_visible(show_main);
    }

    add_tray_icon(hwnd, small_icon);

    if show_main {
        apply_dark_titlebar(hwnd);
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
        if let Some(tdpi) = test_dpi {
            send_test_dpi_change(
                hwnd,
                MAIN_WINDOW_STYLE,
                tdpi,
                COMPACT_CLIENT_LOGICAL_W,
                COMPACT_CLIENT_LOGICAL_H,
            );
        }
        unsafe {
            InvalidateRect(hwnd, null(), 0);
            UpdateWindow(hwnd);
        }
    } else if let Some(profile_id) = initial_editor_profile {
        open_editor_window(&shared, None, &profile_id);
        let ed_hwnd = *shared.editor_hwnd.lock().unwrap_or_else(|e| e.into_inner()) as HWND;
        if !ed_hwnd.is_null() {
            if let Some(tdpi) = test_dpi {
                send_test_dpi_change(
                    ed_hwnd,
                    EDITOR_WINDOW_STYLE,
                    tdpi,
                    EDITOR_CLIENT_LOGICAL_W,
                    EDITOR_CLIENT_LOGICAL_H,
                );
            }
            unsafe {
                InvalidateRect(ed_hwnd, null(), 0);
                UpdateWindow(ed_hwnd);
            }
        }
        let ed_hwnd = *shared.editor_hwnd.lock().unwrap_or_else(|e| e.into_inner()) as HWND;
        if !ed_hwnd.is_null() {
            unsafe {
                UpdateWindow(ed_hwnd);
            }
        }
    } else {
        z_platform_windows::drivers::trim_process_working_set();
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    if let Some(smoke_ms) = smoke_exit_after_ms {
        let deadline = Instant::now() + std::time::Duration::from_millis(smoke_ms);
        while Instant::now() < deadline {
            while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {
                if msg.message == 0x0012 /* WM_QUIT */ {
                    break;
                }
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let ed = *shared.editor_hwnd.lock().unwrap_or_else(|e| e.into_inner()) as HWND;
        if !ed.is_null() {
            unsafe { DestroyWindow(ed) };
        }
        unsafe { DestroyWindow(hwnd) };
    } else {
        while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    remove_tray_icon(hwnd);
    unsafe {
        if !shared.small_icon.is_null() {
            DestroyIcon(shared.small_icon);
        }
        if !shared.large_icon.is_null() {
            DestroyIcon(shared.large_icon);
        }
        shared
            .engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .shutdown();
    }
    Ok(())
}

fn ensure_editor_class_registered(
    hinstance: HINSTANCE,
    large_icon: HICON,
    small_icon: HICON,
) -> Vec<u16> {
    let class_name: Vec<u16> = "ZControllerEditorHost\0".encode_utf16().collect();
    REGISTER_EDITOR_CLASS.call_once(|| {
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: large_icon,
            hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: small_icon,
        };
        unsafe { RegisterClassExW(&wc) };
    });
    class_name
}

fn open_editor_window(shared: &Arc<AppShared>, controller_id: Option<&str>, profile_id: &str) {
    {
        let eng = shared.engine.lock().unwrap_or_else(|e| e.into_inner());
        let snap = eng.snapshot();
        let target_profile = snap.config.profile_by_id(profile_id).clone();
        let mut ui = shared.ui_state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cid) = controller_id {
            ui.selected_controller = Some(cid.to_string());
        } else if ui.selected_controller.is_none() {
            ui.selected_controller = snap
                .controllers
                .iter()
                .find(|c| {
                    c.physical_state.buttons_mask != 0
                        || c.physical_state.left_x != 0
                        || c.physical_state.left_y != 0
                })
                .or_else(|| snap.controllers.first())
                .map(|c| c.canonical_id.clone());
        }
        ui.editing_profile_id = Some(target_profile.id.clone());
        ui.draft_profile = Some(target_profile);
        ui.draft_dirty = false;
    }

    let mut ed_hwnd_guard = shared.editor_hwnd.lock().unwrap_or_else(|e| e.into_inner());
    let ed_hwnd = if *ed_hwnd_guard == 0 {
        let class_name = ensure_editor_class_registered(
            shared.hinstance,
            shared.large_icon,
            shared.small_icon,
        );
        let title: Vec<u16> = "z-controller - Profile Editor\0".encode_utf16().collect();
        let main_hwnd_val = *shared.main_hwnd.lock().unwrap_or_else(|e| e.into_inner()) as HWND;
        let init_dpi = get_dpi_for_window(main_hwnd_val);
        let (init_w, init_h) = outer_size_for_dpi(
            EDITOR_CLIENT_LOGICAL_W,
            EDITOR_CLIENT_LOGICAL_H,
            EDITOR_WINDOW_STYLE,
            init_dpi,
        );
        let ed_state = Box::new(WindowHostState {
            shared: Arc::clone(shared),
            role: WindowRole::Editor,
            ui_session: None,
            dark_titlebar_applied: true,
            events: Vec::new(),
            width: (EDITOR_CLIENT_LOGICAL_W as f32 * dpi_to_ppp(init_dpi)).round() as usize,
            height: (EDITOR_CLIENT_LOGICAL_H as f32 * dpi_to_ppp(init_dpi)).round() as usize,
            dpi: init_dpi,
            pixels_per_point: dpi_to_ppp(init_dpi),
            visible: true,
            followup_paint_budget: 2,
            in_size_move: false,
            pending_state_refresh: false,
            needs_redraw: true,
            dirty_only_rect: None,
            last_auto_logical_h: None,
        });
        let ed_ptr = Box::into_raw(ed_state);
        let created = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                EDITOR_WINDOW_STYLE,
                160,
                80,
                init_w,
                init_h,
                null_mut(),
                null_mut(),
                shared.hinstance,
                ed_ptr.cast(),
            )
        };
        if created.is_null() {
            unsafe { drop(Box::from_raw(ed_ptr)) };
            return;
        }
        apply_dark_titlebar(created);
        let (actual_dpi, cw, ch) = sync_window_dpi_and_bounds(
            created,
            EDITOR_WINDOW_STYLE,
            EDITOR_CLIENT_LOGICAL_W,
            EDITOR_CLIENT_LOGICAL_H,
        );
        if let Some(st) = unsafe { host_state_mut(created) } {
            st.dpi = actual_dpi;
            st.pixels_per_point = dpi_to_ppp(actual_dpi);
            st.width = cw;
            st.height = ch;
        }
        *ed_hwnd_guard = created as usize;
        created
    } else {
        *ed_hwnd_guard as HWND
    };
    drop(ed_hwnd_guard);

    if let Some(st) = unsafe { host_state_mut(ed_hwnd) } {
        if !st.dark_titlebar_applied {
            apply_dark_titlebar(ed_hwnd);
            st.dark_titlebar_applied = true;
        }
        let main_hwnd_val = *shared.main_hwnd.lock().unwrap_or_else(|e| e.into_inner()) as HWND;
        let target_dpi = if !main_hwnd_val.is_null() {
            get_dpi_for_window(main_hwnd_val)
        } else {
            get_dpi_for_window(ed_hwnd)
        };
        if st.dpi != target_dpi {
            let (outer_w, outer_h) = outer_size_for_dpi(
                EDITOR_CLIENT_LOGICAL_W,
                EDITOR_CLIENT_LOGICAL_H,
                EDITOR_WINDOW_STYLE,
                target_dpi,
            );
            st.dpi = target_dpi;
            st.pixels_per_point = dpi_to_ppp(target_dpi);
            unsafe {
                SetWindowPos(
                    ed_hwnd,
                    null_mut(),
                    0,
                    0,
                    outer_w,
                    outer_h,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        st.visible = true;
        st.followup_paint_budget = 2;
        st.needs_redraw = true;
        st.dirty_only_rect = None;
    }

    {
        let eng = shared.engine.lock().unwrap_or_else(|e| e.into_inner());
        eng.set_editor_hwnd(ed_hwnd);
        eng.set_editor_visible(true);
    }

    unsafe {
        ShowWindow(ed_hwnd, SW_SHOW);
        SetForegroundWindow(ed_hwnd);
        InvalidateRect(ed_hwnd, null(), 0);
    }
}

const GWLP_USERDATA: i32 = -21;

unsafe fn host_state_mut<'a>(hwnd: HWND) -> Option<&'a mut WindowHostState> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowHostState;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

fn apply_dark_titlebar(hwnd: HWND) {
    let dll: Vec<u16> = "dwmapi.dll\0".encode_utf16().collect();
    let module = unsafe { LoadLibraryW(dll.as_ptr()) };
    if module.is_null() {
        return;
    }
    type DwmSetAttrFn =
        unsafe extern "system" fn(HWND, u32, *const std::ffi::c_void, u32) -> i32;
    if let Some(proc) = unsafe { GetProcAddress(module, b"DwmSetWindowAttribute\0".as_ptr()) } {
        let set_attr: DwmSetAttrFn = unsafe { std::mem::transmute_copy(&proc) };
        let dark_mode: i32 = 1;
        unsafe {
            set_attr(
                hwnd,
                20,
                (&dark_mode as *const i32).cast(),
                size_of::<i32>() as u32,
            );
        }
        let caption_color: u32 = 0x0010_0A0A;
        unsafe {
            set_attr(
                hwnd,
                35,
                (&caption_color as *const u32).cast(),
                size_of::<u32>() as u32,
            );
        }
    }
    unsafe {
        FreeLibrary(module);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == 0x0081 /* WM_NCCREATE */ {
        let cs = lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
        let ptr = unsafe { (*cs).lpCreateParams } as isize;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr) };
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowHostState;
    if state_ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    match msg {
        WM_ERASEBKGND => 1,
        WM_GETMINMAXINFO => {
            let st = unsafe { &*state_ptr };
            if st.role == WindowRole::Main && lparam != 0 {
                let mmi = unsafe { &mut *(lparam as *mut MinMaxInfo) };
                let fixed_w = main_fixed_outer_width(st.dpi);
                let min_h = outer_size_for_dpi(
                    COMPACT_CLIENT_LOGICAL_W,
                    COMPACT_CLIENT_MIN_LOGICAL_H,
                    MAIN_WINDOW_STYLE,
                    st.dpi,
                )
                .1;
                mmi.pt_max_size.x = fixed_w;
                mmi.pt_min_track_size.x = fixed_w;
                mmi.pt_max_track_size.x = fixed_w;
                mmi.pt_min_track_size.y = min_h;
                mmi.pt_max_track_size.y = 2400;
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_SIZING => {
            let st = unsafe { &*state_ptr };
            if st.role == WindowRole::Main && lparam != 0 {
                let rc = unsafe { &mut *(lparam as *mut RECT) };
                let fixed_w = main_fixed_outer_width(st.dpi);
                let edge = wparam as u32;
                // Anchor right edge when dragging left border (WMSZ_LEFT=1, TOPLEFT=4, BOTTOMLEFT=7).
                if matches!(edge, 1 | 4 | 7) {
                    rc.left = rc.right - fixed_w;
                } else {
                    rc.right = rc.left + fixed_w;
                }
                return 1;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_NCHITTEST => {
            let hit = unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            let st = unsafe { &*state_ptr };
            if st.role == WindowRole::Main {
                // Disable horizontal resize hit regions while keeping vertical top/bottom sizing active.
                return match hit {
                    10 | 11 => 18, // HTLEFT | HTRIGHT -> HTBORDER
                    13 | 14 => 12, // HTTOPLEFT | HTTOPRIGHT -> HTTOP
                    16 | 17 => 15, // HTBOTTOMLEFT | HTBOTTOMRIGHT -> HTBOTTOM
                    other => other,
                };
            }
            hit
        }
        WM_ENTERSIZEMOVE => {
            let st = unsafe { &mut *state_ptr };
            st.in_size_move = true;
            0
        }
        WM_EXITSIZEMOVE => {
            let st = unsafe { &mut *state_ptr };
            st.in_size_move = false;
            if st.pending_state_refresh {
                st.pending_state_refresh = false;
                st.needs_redraw = true;
                let dirty = st.dirty_only_rect;
                if let Some(ref r) = dirty {
                    unsafe { InvalidateRect(hwnd, r, 0) };
                } else {
                    unsafe { InvalidateRect(hwnd, null(), 0) };
                }
            }
            0
        }
        WM_SIZE => {
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            let st = unsafe { &mut *state_ptr };
            let new_w = (rc.right - rc.left).max(0) as usize;
            let new_h = (rc.bottom - rc.top).max(0) as usize;
            if new_w != st.width || new_h != st.height {
                st.width = new_w;
                st.height = new_h;
                st.needs_redraw = true;
                st.dirty_only_rect = None;
                unsafe { InvalidateRect(hwnd, null(), 0) };
            }
            0
        }
        WM_DPICHANGED => {
            let new_dpi = ((wparam & 0xFFFF) as u32).max(96);
            let new_ppp = dpi_to_ppp(new_dpi);
            let role = {
                let st = unsafe { &mut *state_ptr };
                st.dpi = new_dpi;
                st.pixels_per_point = new_ppp;
                st.followup_paint_budget = 2;
                st.needs_redraw = true;
                st.dirty_only_rect = None;
                st.role
            };
            // Drop &mut before SetWindowPos so any synchronous WM_SIZE does not alias state_ptr.
            if lparam != 0 {
                let mut rect = unsafe { *(lparam as *const RECT) };
                if role == WindowRole::Main {
                    rect.right = rect.left + main_fixed_outer_width(new_dpi);
                }
                unsafe {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            } else {
                let (style, lw, lh) = match role {
                    WindowRole::Main => (
                        MAIN_WINDOW_STYLE,
                        COMPACT_CLIENT_LOGICAL_W,
                        COMPACT_CLIENT_LOGICAL_H,
                    ),
                    WindowRole::Editor => (
                        EDITOR_WINDOW_STYLE,
                        EDITOR_CLIENT_LOGICAL_W,
                        EDITOR_CLIENT_LOGICAL_H,
                    ),
                };
                let (ow, oh) = outer_size_for_dpi(lw, lh, style, new_dpi);
                unsafe {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        0,
                        0,
                        ow,
                        oh,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            let mut crc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut crc) };
            {
                let st = unsafe { &mut *state_ptr };
                st.width = (crc.right - crc.left).max(0) as usize;
                st.height = (crc.bottom - crc.top).max(0) as usize;
                st.needs_redraw = true;
                st.dirty_only_rect = None;
                st.last_auto_logical_h = None;
            }
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_APP_STATE_CHANGED => {
            let st = unsafe { &mut *state_ptr };
            if !st.visible {
                return 0;
            }
            if st.in_size_move {
                // Defer live-highlight updates during window drags so dragging stays zero-jank.
                st.pending_state_refresh = true;
                return 0;
            }
            let shared = Arc::clone(&st.shared);
            let eng = shared.engine.lock().unwrap_or_else(|e| e.into_inner());
            let ui_state = shared.ui_state.lock().unwrap_or_else(|e| e.into_inner());
            let snap = eng.snapshot();
            let static_key = static_ui_hash(&snap, &ui_state);
            let dyn_rect_opt = match st.role {
                WindowRole::Main => ui_state.last_dynamic_rect_main,
                WindowRole::Editor => ui_state.last_dynamic_rect_editor,
            };
            drop(ui_state);
            drop(eng);

            st.needs_redraw = true;
            st.followup_paint_budget = 1;

            let can_partial = st
                .ui_session
                .as_ref()
                .is_some_and(|(_, p)| p.has_static_cache(st.width, st.height, static_key));
            if can_partial {
                if let Some(lr) = dyn_rect_opt {
                    let ppp = st.pixels_per_point.max(0.5);
                    let phys = RECT {
                        left: ((lr.left() * ppp).floor() as i32 - 2).max(0),
                        top: ((lr.top() * ppp).floor() as i32 - 2).max(0),
                        right: ((lr.right() * ppp).ceil() as i32 + 2).min(st.width as i32),
                        bottom: ((lr.bottom() * ppp).ceil() as i32 + 2).min(st.height as i32),
                    };
                    if phys.right > phys.left && phys.bottom > phys.top {
                        st.dirty_only_rect = Some(phys);
                        unsafe { InvalidateRect(hwnd, &phys, 0) };
                        return 0;
                    }
                }
            }
            st.dirty_only_rect = None;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_APP_IMMEDIATE_REPAINT => {
            let st = unsafe { &mut *state_ptr };
            if st.visible {
                st.needs_redraw = true;
                unsafe {
                    InvalidateRect(hwnd, null(), 0);
                    UpdateWindow(hwnd);
                }
            }
            0
        }
        WM_PAINT => {
            let st = unsafe { &mut *state_ptr };
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut actions: Vec<UiAction> = Vec::new();
            let mut request_followup = false;
            let mut auto_resize_main_h: Option<i32> = None;

            if st.visible && st.width > 0 && st.height > 0 {
                let backbuffer_ready = st
                    .ui_session
                    .as_ref()
                    .is_some_and(|(_, p)| p.has_ready_backbuffer(st.width, st.height));

                // Fast path: during drag or OS expose repaints when nothing UI-side changed,
                // blit the cached DIB back buffer in one BitBlt without invoking egui or rasterizing.
                if backbuffer_ready && (st.in_size_move || (!st.needs_redraw && st.events.is_empty())) {
                    if let Some((_, painter)) = st.ui_session.as_ref() {
                        painter.present_rect(hdc, Some(ps.rcPaint));
                    }
                    unsafe { EndPaint(hwnd, &ps) };
                    return 0;
                }

                let shared = Arc::clone(&st.shared);
                let eng = shared.engine.lock().unwrap_or_else(|e| e.into_inner());
                let mut ui_state = shared.ui_state.lock().unwrap_or_else(|e| e.into_inner());
                let width = st.width;
                let height = st.height;
                let ppp = st.pixels_per_point.max(0.5);
                let logical_w = (width as f32 / ppp).max(1.0);
                let logical_h = (height as f32 / ppp).max(1.0);
                let events = std::mem::take(&mut st.events);
                let elapsed = shared.start_time.elapsed().as_secs_f64();

                let (egui_ctx, painter) = st.ui_session.get_or_insert_with(|| {
                    let ctx = egui::Context::default();
                    configure_neon_theme_and_fonts(&ctx);
                    (ctx, SoftwarePainter::new())
                });
                let mut raw = RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(logical_w, logical_h),
                    )),
                    time: Some(elapsed),
                    modifiers: current_modifiers(),
                    events,
                    focused: true,
                    ..Default::default()
                };
                raw.viewports
                    .entry(ViewportId::ROOT)
                    .or_default()
                    .native_pixels_per_point = Some(ppp);
                let preview_list = shared.preview_controllers.as_ref();
                let role = st.role;
                let snap_for_hash = eng.snapshot();
                let full_out = egui_ctx.run(raw, |ctx| match (role, preview_list) {
                    (WindowRole::Main, Some(preview)) => {
                        let snap = EngineSnapshot {
                            controllers: preview.clone(),
                            config: snap_for_hash.config.clone(),
                            driver_status: snap_for_hash.driver_status.clone(),
                            last_status_message: snap_for_hash.last_status_message.clone(),
                        };
                        actions = draw_compact_main_snapshot(ctx, &mut ui_state, &snap, |cmd| {
                            eng.send(cmd)
                        });
                    }
                    (WindowRole::Main, None) => {
                        actions = draw_compact_main_snapshot(ctx, &mut ui_state, &snap_for_hash, |cmd| {
                            eng.send(cmd)
                        });
                    }
                    (WindowRole::Editor, Some(preview)) => {
                        let snap = EngineSnapshot {
                            controllers: preview.clone(),
                            config: snap_for_hash.config.clone(),
                            driver_status: snap_for_hash.driver_status.clone(),
                            last_status_message: snap_for_hash.last_status_message.clone(),
                        };
                        draw_profile_editor_snapshot(ctx, &mut ui_state, &snap, |cmd| {
                            eng.send(cmd)
                        });
                    }
                    (WindowRole::Editor, None) => {
                        draw_profile_editor_snapshot(ctx, &mut ui_state, &snap_for_hash, |cmd| {
                            eng.send(cmd)
                        });
                    }
                });
                let static_key = static_ui_hash(&snap_for_hash, &ui_state);
                if role == WindowRole::Main && !st.in_size_move {
                    if let Some(raw_h) = ui_state.desired_compact_logical_h {
                        let clamped_h = (raw_h.ceil() as i32)
                            .clamp(COMPACT_CLIENT_MIN_LOGICAL_H, max_auto_logical_height(st.dpi));
                        if st.last_auto_logical_h != Some(clamped_h) {
                            auto_resize_main_h = Some(clamped_h);
                        }
                    }
                }
                drop(ui_state);
                drop(eng);

                let wants_zero_delay = full_out
                    .viewport_output
                    .values()
                    .any(|v| v.repaint_delay.is_zero());
                if wants_zero_delay && st.followup_paint_budget > 0 {
                    st.followup_paint_budget -= 1;
                    request_followup = true;
                } else {
                    st.followup_paint_budget = 0;
                }

                painter.apply_textures(&full_out.textures_delta);
                let render_ppp = full_out.pixels_per_point;
                let clipped = egui_ctx.tessellate(full_out.shapes, render_ppp);

                let dirty_px = st.dirty_only_rect.take().and_then(|r| {
                    if painter.has_static_cache(width, height, static_key)
                        && r.right > r.left
                        && r.bottom > r.top
                    {
                        Some([
                            r.left.max(0) as usize,
                            r.top.max(0) as usize,
                            (r.right as usize).min(width),
                            (r.bottom as usize).min(height),
                        ])
                    } else {
                        None
                    }
                });
                let is_full_paint = dirty_px.is_none();
                painter.paint_and_blit(
                    hdc,
                    width,
                    height,
                    render_ppp,
                    &clipped,
                    CLEAR_BG_RGB,
                    dirty_px,
                    Some(ps.rcPaint),
                );
                if is_full_paint {
                    painter.save_static_cache(static_key);
                }
                st.needs_redraw = false;
            }
            unsafe { EndPaint(hwnd, &ps) };

            if let Some(target_h) = auto_resize_main_h {
                let dpi = {
                    let st = unsafe { &mut *state_ptr };
                    st.last_auto_logical_h = Some(target_h);
                    st.needs_redraw = true;
                    st.dirty_only_rect = None;
                    st.dpi
                };
                let (fixed_w, target_outer_h) = outer_size_for_hwnd(
                    hwnd,
                    COMPACT_CLIENT_LOGICAL_W,
                    target_h,
                    MAIN_WINDOW_STYLE,
                    dpi,
                );
                unsafe {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        0,
                        0,
                        fixed_w,
                        target_outer_h,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    InvalidateRect(hwnd, null(), 0);
                    UpdateWindow(hwnd);
                }
            }

            if request_followup {
                unsafe {
                    PostMessageW(
                        hwnd,
                        WM_APP_IMMEDIATE_REPAINT,
                        WPARAM::default(),
                        LPARAM::default(),
                    );
                }
            }
            for act in actions {
                match act {
                    UiAction::OpenProfileEditor {
                        controller_id,
                        profile_id,
                    } => {
                        let shared = unsafe { Arc::clone(&(*state_ptr).shared) };
                        open_editor_window(&shared, Some(&controller_id), &profile_id);
                    }
                }
            }
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = lp_xy(lparam);
            let st = unsafe { &mut *state_ptr };
            let pos = physical_to_logical_pos(x, y, st.pixels_per_point);
            st.events.push(Event::PointerMoved(pos));
            st.needs_redraw = true;
            st.dirty_only_rect = None;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP => {
            let (x, y) = lp_xy(lparam);
            let button = if msg == WM_LBUTTONDOWN || msg == WM_LBUTTONUP {
                PointerButton::Primary
            } else {
                PointerButton::Secondary
            };
            let pressed = msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN;
            unsafe {
                if pressed {
                    SetCapture(hwnd);
                } else {
                    ReleaseCapture();
                }
                let st = &mut *state_ptr;
                let pos = physical_to_logical_pos(x, y, st.pixels_per_point);
                st.followup_paint_budget = 2;
                st.needs_redraw = true;
                st.dirty_only_rect = None;
                st.events.push(Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: current_modifiers(),
                });
            }
            // No &mut ref alive across synchronous UpdateWindow(hwnd) -> WM_PAINT.
            unsafe {
                InvalidateRect(hwnd, null(), 0);
                UpdateWindow(hwnd);
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta_raw = ((wparam >> 16) as i16) as f32 / 120.0;
            let st = unsafe { &mut *state_ptr };
            st.needs_redraw = true;
            st.dirty_only_rect = None;
            st.events.push(Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                delta: Vec2::new(0.0, delta_raw * 2.5),
                modifiers: current_modifiers(),
            });
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_KEYDOWN | WM_KEYUP => {
            if let Some(key) = vk_to_egui_key(wparam as u32) {
                let st = unsafe { &mut *state_ptr };
                st.followup_paint_budget = 2;
                st.needs_redraw = true;
                st.dirty_only_rect = None;
                st.events.push(Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: msg == WM_KEYDOWN,
                    repeat: false,
                    modifiers: current_modifiers(),
                });
                unsafe { InvalidateRect(hwnd, null(), 0) };
            }
            0
        }
        WM_CHAR => {
            if let Some(ch) = char::from_u32(wparam as u32) {
                if !ch.is_control() {
                    let st = unsafe { &mut *state_ptr };
                    st.needs_redraw = true;
                    st.dirty_only_rect = None;
                    st.events.push(Event::Text(ch.to_string()));
                    unsafe { InvalidateRect(hwnd, null(), 0) };
                }
            }
            0
        }
        WM_TRAYICON => {
            let st = unsafe { &mut *state_ptr };
            let event_code = (lparam & 0xFFFF) as u32;
            if event_code == WM_LBUTTONUP || event_code == WM_LBUTTONDBLCLK {
                show_main_window(hwnd, st);
            } else if event_code == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            }
            0
        }
        WM_COMMAND => {
            let st = unsafe { &mut *state_ptr };
            match wparam & 0xFFFF {
                CMD_OPEN => show_main_window(hwnd, st),
                CMD_QUIT => unsafe {
                    let ed = *st.shared.editor_hwnd.lock().unwrap_or_else(|e| e.into_inner())
                        as HWND;
                    if !ed.is_null() {
                        DestroyWindow(ed);
                    }
                    DestroyWindow(hwnd);
                },
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            let st = unsafe { &mut *state_ptr };
            st.visible = false;
            if let Some((_, mut painter)) = st.ui_session.take() {
                painter.release_buffers();
            }
            st.events = Vec::new();
            st.followup_paint_budget = 0;
            st.last_auto_logical_h = None;

            match st.role {
                WindowRole::Editor => {
                    st.shared
                        .engine
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .set_editor_visible(false);
                    unsafe { ShowWindow(hwnd, SW_HIDE) };
                    z_platform_windows::drivers::trim_process_working_set();
                    let main_hwnd =
                        *st.shared.main_hwnd.lock().unwrap_or_else(|e| e.into_inner()) as HWND;
                    if !main_hwnd.is_null() {
                        if let Some(main_st) = unsafe { host_state_mut(main_hwnd) } {
                            if main_st.visible {
                                unsafe { SetForegroundWindow(main_hwnd) };
                            }
                        }
                    }
                }
                WindowRole::Main => {
                    // Hide to tray and release UI session + working set so idle RAM remains <= 25 MB.
                    st.shared
                        .engine
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .set_window_visible(false);
                    unsafe { ShowWindow(hwnd, SW_HIDE) };
                    z_platform_windows::drivers::trim_process_working_set();
                }
            }
            0
        }
        WM_ENDSESSION => {
            if wparam != 0 {
                let st = unsafe { &mut *state_ptr };
                st.shared
                    .engine
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .shutdown();
            }
            0
        }
        WM_DESTROY => {
            let boxed = unsafe { Box::from_raw(state_ptr) };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            match boxed.role {
                WindowRole::Main => {
                    *boxed.shared.main_hwnd.lock().unwrap_or_else(|e| e.into_inner()) = 0;
                    unsafe { PostQuitMessage(0) };
                }
                WindowRole::Editor => {
                    *boxed.shared.editor_hwnd.lock().unwrap_or_else(|e| e.into_inner()) = 0;
                    boxed
                        .shared
                        .engine
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .set_editor_visible(false);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn show_main_window(hwnd: HWND, st: &mut WindowHostState) {
    if !st.dark_titlebar_applied {
        apply_dark_titlebar(hwnd);
        st.dark_titlebar_applied = true;
    }
    st.visible = true;
    st.followup_paint_budget = 1;
    st.needs_redraw = true;
    st.dirty_only_rect = None;
    st.shared
        .engine
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .set_window_visible(true);
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        InvalidateRect(hwnd, null(), 0);
    }
}

fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        let open_txt: Vec<u16> = "Open z-controller\0".encode_utf16().collect();
        let quit_txt: Vec<u16> = "Quit\0".encode_utf16().collect();
        AppendMenuW(menu, MF_STRING, CMD_OPEN, open_txt.as_ptr());
        AppendMenuW(menu, MF_STRING, CMD_QUIT, quit_txt.as_ptr());

        let mut pt: windows_sys::Win32::Foundation::POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            null(),
        );
        DestroyMenu(menu);
    }
}

fn add_tray_icon(hwnd: HWND, icon: HICON) {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    // STRICT NO-POPUP RULE: never set NIF_INFO
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = icon;
    let tip: Vec<u16> = "z-controller".encode_utf16().collect();
    let len = tip.len().min(nid.szTip.len() - 1);
    nid.szTip[..len].copy_from_slice(&tip[..len]);
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

fn remove_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

unsafe fn create_gamepad_icon() -> HICON {
    const SIZE: usize = 16;
    let mut color = [0u32; SIZE * SIZE];
    let and_mask = [0u8; SIZE * 2];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let inside = (3..13).contains(&y)
                && (1..15).contains(&x)
                && !((y == 3 || y == 12) && (x < 3 || x > 12));
            if inside {
                let left_accent = y == 7 && (x == 4 || x == 5);
                let right_accent = (y == 7 && x == 11) || (y == 8 && x == 10);
                color[y * SIZE + x] = if left_accent {
                    0xFFA9_70FF
                } else if right_accent {
                    0xFF3D_FF7A
                } else {
                    0xFF14_141F
                };
            }
        }
    }
    unsafe {
        CreateIcon(
            null_mut(),
            SIZE as i32,
            SIZE as i32,
            1,
            32,
            and_mask.as_ptr(),
            color.as_ptr().cast(),
        )
    }
}

fn lp_xy(lparam: LPARAM) -> (f32, f32) {
    let x = (lparam & 0xFFFF) as i16 as f32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as f32;
    (x, y)
}

fn current_modifiers() -> Modifiers {
    let shift = unsafe { GetKeyState(VK_SHIFT as i32) } < 0;
    let ctrl = unsafe { GetKeyState(VK_CONTROL as i32) } < 0;
    let alt = unsafe { GetKeyState(VK_MENU as i32) } < 0;
    Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: false,
        command: ctrl,
    }
}

fn vk_to_egui_key(vk: u32) -> Option<Key> {
    match vk {
        0x08 => Some(Key::Backspace),
        0x09 => Some(Key::Tab),
        0x0D => Some(Key::Enter),
        0x1B => Some(Key::Escape),
        0x25 => Some(Key::ArrowLeft),
        0x26 => Some(Key::ArrowUp),
        0x27 => Some(Key::ArrowRight),
        0x28 => Some(Key::ArrowDown),
        0x2E => Some(Key::Delete),
        0x24 => Some(Key::Home),
        0x23 => Some(Key::End),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_scaling_math_and_outer_window_size_scale_monotonically() {
        assert!((dpi_to_ppp(96) - 1.0).abs() < 1e-4);
        assert!((dpi_to_ppp(120) - 1.25).abs() < 1e-4);
        assert!((dpi_to_ppp(144) - 1.5).abs() < 1e-4);
        assert!((dpi_to_ppp(192) - 2.0).abs() < 1e-4);

        let logical = physical_to_logical_pos(150.0, 300.0, dpi_to_ppp(144));
        assert!((logical.x - 100.0).abs() < 1e-4);
        assert!((logical.y - 200.0).abs() < 1e-4);

        let (w100, h100) = outer_size_for_dpi(
            COMPACT_CLIENT_LOGICAL_W,
            COMPACT_CLIENT_LOGICAL_H,
            MAIN_WINDOW_STYLE,
            96,
        );
        let (w150, h150) = outer_size_for_dpi(
            COMPACT_CLIENT_LOGICAL_W,
            COMPACT_CLIENT_LOGICAL_H,
            MAIN_WINDOW_STYLE,
            144,
        );
        assert!(w150 > w100 + 180);
        assert!(h150 > h100 + 280);
    }
}
