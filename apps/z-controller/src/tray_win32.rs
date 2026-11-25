use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex, Once};
use std::time::Instant;

use egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Vec2};
use windows_sys::Win32::Foundation::{FreeLibrary, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
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
    AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos,
    GetMessageW, GetWindowLongPtrW, HICON, IDC_ARROW, LoadCursorW, MF_STRING, MSG, PM_REMOVE,
    PeekMessageW, PostMessageW, PostQuitMessage, RegisterClassExW, SW_HIDE, SW_SHOW,
    SetForegroundWindow, SetWindowLongPtrW, ShowWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    TrackPopupMenu, TranslateMessage, WM_CHAR, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_ENDSESSION,
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEWHEEL,
    WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE, WNDCLASSEXW, WS_CAPTION,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_SYSMENU,
};

use crate::engine::{ControllerSnapshot, EngineHandle, EngineSnapshot, WM_APP_STATE_CHANGED};
use crate::sw_painter::SoftwarePainter;
use crate::ui::{
    AppUiState, UiAction, configure_neon_theme_and_fonts, draw_compact_main_snapshot,
    draw_compact_main_ui, draw_profile_editor_snapshot, draw_profile_editor_ui,
};

const WM_TRAYICON: u32 = 0x8000 + 2;
const WM_APP_IMMEDIATE_REPAINT: u32 = 0x8000 + 3;
const CMD_OPEN: usize = 1001;
const CMD_QUIT: usize = 1002;
const CLEAR_BG_RGB: u32 = 0x000A_0A10;

const COMPACT_WIN_W: i32 = 412;
const COMPACT_WIN_H: i32 = 640;
const EDITOR_WIN_W: i32 = 1040;
const EDITOR_WIN_H: i32 = 680;

static REGISTER_EDITOR_CLASS: Once = Once::new();

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
    tray_icon: HICON,
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
    visible: bool,
    followup_paint_budget: u8,
}

pub fn run_tray_app(
    engine: EngineHandle,
    open_immediately: bool,
    initial_editor_profile: Option<String>,
    preview_controllers: Option<Vec<ControllerSnapshot>>,
    smoke_exit_after_ms: Option<u64>,
) -> Result<(), String> {
    let hinstance = unsafe { GetModuleHandleW(null()) } as HINSTANCE;
    let class_name: Vec<u16> = "ZControllerTrayHost\0".encode_utf16().collect();
    let title: Vec<u16> = "z-controller\0".encode_utf16().collect();

    let tray_icon = unsafe { create_gamepad_icon() };

    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: tray_icon,
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: tray_icon,
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
        tray_icon,
        main_hwnd: Mutex::new(0),
        editor_hwnd: Mutex::new(0),
    });

    let show_main = open_immediately && initial_editor_profile.is_none();

    let main_state = Box::new(WindowHostState {
        shared: Arc::clone(&shared),
        role: WindowRole::Main,
        ui_session: None,
        dark_titlebar_applied: show_main,
        events: Vec::new(),
        width: (COMPACT_WIN_W - 16) as usize,
        height: (COMPACT_WIN_H - 39) as usize,
        visible: show_main,
        followup_paint_budget: 0,
    });
    let main_state_ptr = Box::into_raw(main_state);

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX,
            120,
            100,
            COMPACT_WIN_W,
            COMPACT_WIN_H,
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

    *shared.main_hwnd.lock().unwrap_or_else(|e| e.into_inner()) = hwnd as usize;
    {
        let eng = shared.engine.lock().unwrap_or_else(|e| e.into_inner());
        eng.set_ui_hwnd(hwnd);
        eng.set_window_visible(show_main);
    }

    add_tray_icon(hwnd, tray_icon);

    if show_main {
        apply_dark_titlebar(hwnd);
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            InvalidateRect(hwnd, null(), 0);
            UpdateWindow(hwnd);
        }
    } else if let Some(profile_id) = initial_editor_profile {
        open_editor_window(&shared, None, &profile_id);
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
        if !shared.tray_icon.is_null() {
            DestroyIcon(shared.tray_icon);
        }
        shared
            .engine
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .shutdown();
    }
    Ok(())
}

fn ensure_editor_class_registered(hinstance: HINSTANCE, icon: HICON) -> Vec<u16> {
    let class_name: Vec<u16> = "ZControllerEditorHost\0".encode_utf16().collect();
    REGISTER_EDITOR_CLASS.call_once(|| {
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: icon,
            hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: icon,
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
        let class_name = ensure_editor_class_registered(shared.hinstance, shared.tray_icon);
        let title: Vec<u16> = "z-controller - Profile Editor\0".encode_utf16().collect();
        let ed_state = Box::new(WindowHostState {
            shared: Arc::clone(shared),
            role: WindowRole::Editor,
            ui_session: None,
            dark_titlebar_applied: true,
            events: Vec::new(),
            width: (EDITOR_WIN_W - 16) as usize,
            height: (EDITOR_WIN_H - 39) as usize,
            visible: true,
            followup_paint_budget: 2,
        });
        let ed_ptr = Box::into_raw(ed_state);
        let created = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX,
                160,
                80,
                EDITOR_WIN_W,
                EDITOR_WIN_H,
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
        st.visible = true;
        st.followup_paint_budget = 2;
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
        WM_SIZE => {
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            let st = unsafe { &mut *state_ptr };
            st.width = (rc.right - rc.left).max(0) as usize;
            st.height = (rc.bottom - rc.top).max(0) as usize;
            unsafe { InvalidateRect(hwnd, null(), 0) };
            0
        }
        WM_APP_STATE_CHANGED => {
            let st = unsafe { &mut *state_ptr };
            if st.visible {
                st.followup_paint_budget = 1;
                unsafe { InvalidateRect(hwnd, null(), 0) };
            }
            0
        }
        WM_APP_IMMEDIATE_REPAINT => {
            let visible = unsafe { (*state_ptr).visible };
            if visible {
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

            if st.visible && st.width > 0 && st.height > 0 {
                let shared = Arc::clone(&st.shared);
                let eng = shared.engine.lock().unwrap_or_else(|e| e.into_inner());
                let mut ui_state = shared.ui_state.lock().unwrap_or_else(|e| e.into_inner());
                let width = st.width;
                let height = st.height;
                let events = std::mem::take(&mut st.events);
                let elapsed = shared.start_time.elapsed().as_secs_f64();

                let (egui_ctx, painter) = st.ui_session.get_or_insert_with(|| {
                    let ctx = egui::Context::default();
                    configure_neon_theme_and_fonts(&ctx);
                    (ctx, SoftwarePainter::new())
                });
                let raw = RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(width as f32, height as f32),
                    )),
                    time: Some(elapsed),
                    modifiers: current_modifiers(),
                    events,
                    focused: true,
                    ..Default::default()
                };
                let preview_list = shared.preview_controllers.as_ref();
                let role = st.role;
                let full_out = egui_ctx.run(raw, |ctx| match (role, preview_list) {
                    (WindowRole::Main, Some(preview)) => {
                        let base = eng.snapshot();
                        let snap = EngineSnapshot {
                            controllers: preview.clone(),
                            config: base.config.clone(),
                            driver_status: base.driver_status.clone(),
                            last_status_message: base.last_status_message.clone(),
                        };
                        actions = draw_compact_main_snapshot(ctx, &mut ui_state, &snap, |cmd| {
                            eng.send(cmd)
                        });
                    }
                    (WindowRole::Main, None) => {
                        actions = draw_compact_main_ui(ctx, &mut ui_state, &eng);
                    }
                    (WindowRole::Editor, Some(preview)) => {
                        let base = eng.snapshot();
                        let snap = EngineSnapshot {
                            controllers: preview.clone(),
                            config: base.config.clone(),
                            driver_status: base.driver_status.clone(),
                            last_status_message: base.last_status_message.clone(),
                        };
                        draw_profile_editor_snapshot(ctx, &mut ui_state, &snap, |cmd| {
                            eng.send(cmd)
                        });
                    }
                    (WindowRole::Editor, None) => {
                        draw_profile_editor_ui(ctx, &mut ui_state, &eng);
                    }
                });
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
                let clipped = egui_ctx.tessellate(full_out.shapes, 1.0);
                painter.paint_and_blit(hdc, width, height, 1.0, &clipped, CLEAR_BG_RGB);
            }
            unsafe { EndPaint(hwnd, &ps) };

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
            st.events.push(Event::PointerMoved(Pos2::new(x, y)));
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
                st.followup_paint_budget = 2;
                st.events.push(Event::PointerButton {
                    pos: Pos2::new(x, y),
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
