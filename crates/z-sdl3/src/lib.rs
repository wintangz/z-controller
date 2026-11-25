use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
#[cfg(windows)]
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
    LoadLibraryExW,
};

const EMBEDDED_GAMECONTROLLERDB: &[u8] = include_bytes!("../../../assets/gamecontrollerdb.txt");

#[cfg(windows)]
const EMBEDDED_SDL3_DLL: &[u8] = include_bytes!("../../../vendor/sdl3-win64/SDL3.dll");

#[derive(Debug, Error)]
pub enum SdlError {
    #[error("failed to prepare embedded SDL3 runtime: {0}")]
    RuntimeSetup(String),
    #[error("failed to load SDL3 library: {0}")]
    LoadLibrary(String),
    #[error("missing SDL3 export symbol: {0}")]
    MissingSymbol(&'static str),
    #[error("SDL3 error: {0}")]
    Sdl(String),
}

pub type SdlJoystickId = u32;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdlPowerState {
    Error = -1,
    Unknown = 0,
    OnBattery = 1,
    NoBattery = 2,
    Charging = 3,
    Charged = 4,
}

impl SdlPowerState {
    fn from_raw(v: i32) -> Self {
        match v {
            0 => Self::Unknown,
            1 => Self::OnBattery,
            2 => Self::NoBattery,
            3 => Self::Charging,
            4 => Self::Charged,
            _ => Self::Error,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdlConnectionState {
    Invalid = -1,
    Unknown = 0,
    Wired = 1,
    Wireless = 2,
}

impl SdlConnectionState {
    fn from_raw(v: i32) -> Self {
        match v {
            0 => Self::Unknown,
            1 => Self::Wired,
            2 => Self::Wireless,
            _ => Self::Invalid,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdlGamepadType {
    Unknown = 0,
    Standard = 1,
    Xbox360 = 2,
    XboxOne = 3,
    Ps3 = 4,
    Ps4 = 5,
    Ps5 = 6,
    NintendoSwitchPro = 7,
    NintendoSwitchJoyconLeft = 8,
    NintendoSwitchJoyconRight = 9,
    NintendoSwitchJoyconPair = 10,
    GameCube = 11,
    Steam = 12,
}

impl SdlGamepadType {
    fn from_raw(v: i32) -> Self {
        match v {
            1 => Self::Standard,
            2 => Self::Xbox360,
            3 => Self::XboxOne,
            4 => Self::Ps3,
            5 => Self::Ps4,
            6 => Self::Ps5,
            7 => Self::NintendoSwitchPro,
            8 => Self::NintendoSwitchJoyconLeft,
            9 => Self::NintendoSwitchJoyconRight,
            10 => Self::NintendoSwitchJoyconPair,
            11 => Self::GameCube,
            12 => Self::Steam,
            _ => Self::Unknown,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdlGamepadAxis {
    LeftX = 0,
    LeftY = 1,
    RightX = 2,
    RightY = 3,
    LeftTrigger = 4,
    RightTrigger = 5,
}

impl SdlGamepadAxis {
    pub fn from_raw(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::LeftX),
            1 => Some(Self::LeftY),
            2 => Some(Self::RightX),
            3 => Some(Self::RightY),
            4 => Some(Self::LeftTrigger),
            5 => Some(Self::RightTrigger),
            _ => None,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdlGamepadButton {
    South = 0,
    East = 1,
    West = 2,
    North = 3,
    Back = 4,
    Guide = 5,
    Start = 6,
    LeftStick = 7,
    RightStick = 8,
    LeftShoulder = 9,
    RightShoulder = 10,
    DpadUp = 11,
    DpadDown = 12,
    DpadLeft = 13,
    DpadRight = 14,
    Misc1 = 15,
    RightPaddle1 = 16,
    LeftPaddle1 = 17,
    RightPaddle2 = 18,
    LeftPaddle2 = 19,
    Touchpad = 20,
}

impl SdlGamepadButton {
    pub fn from_raw(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::South),
            1 => Some(Self::East),
            2 => Some(Self::West),
            3 => Some(Self::North),
            4 => Some(Self::Back),
            5 => Some(Self::Guide),
            6 => Some(Self::Start),
            7 => Some(Self::LeftStick),
            8 => Some(Self::RightStick),
            9 => Some(Self::LeftShoulder),
            10 => Some(Self::RightShoulder),
            11 => Some(Self::DpadUp),
            12 => Some(Self::DpadDown),
            13 => Some(Self::DpadLeft),
            14 => Some(Self::DpadRight),
            15 => Some(Self::Misc1),
            16 => Some(Self::RightPaddle1),
            17 => Some(Self::LeftPaddle1),
            18 => Some(Self::RightPaddle2),
            19 => Some(Self::LeftPaddle2),
            20 => Some(Self::Touchpad),
            _ => None,
        }
    }
}

pub const SDL_INIT_JOYSTICK: u32 = 0x0000_0200;
pub const SDL_INIT_HAPTIC: u32 = 0x0000_1000;
pub const SDL_INIT_GAMEPAD: u32 = 0x0000_2000;
pub const SDL_INIT_EVENTS: u32 = 0x0000_4000;

pub const SDL_EVENT_QUIT: u32 = 0x100;
pub const SDL_EVENT_JOYSTICK_BATTERY_UPDATED: u32 = 0x607;
pub const SDL_EVENT_GAMEPAD_AXIS_MOTION: u32 = 0x650;
pub const SDL_EVENT_GAMEPAD_BUTTON_DOWN: u32 = 0x651;
pub const SDL_EVENT_GAMEPAD_BUTTON_UP: u32 = 0x652;
pub const SDL_EVENT_GAMEPAD_ADDED: u32 = 0x653;
pub const SDL_EVENT_GAMEPAD_REMOVED: u32 = 0x654;
pub const SDL_EVENT_GAMEPAD_REMAPPED: u32 = 0x655;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SdlJoyBatteryEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub which: SdlJoystickId,
    pub state: i32,
    pub percent: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SdlGamepadAxisEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub which: SdlJoystickId,
    pub axis: u8,
    pub padding1: u8,
    pub padding2: u8,
    pub padding3: u8,
    pub value: i16,
    pub padding4: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SdlGamepadButtonEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub which: SdlJoystickId,
    pub button: u8,
    pub down: bool,
    pub padding1: u8,
    pub padding2: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SdlGamepadDeviceEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub which: SdlJoystickId,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SdlUserEvent {
    pub r#type: u32,
    pub reserved: u32,
    pub timestamp: u64,
    pub window_id: u32,
    pub code: i32,
    pub data1: *mut c_void,
    pub data2: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union RawSdlEvent {
    pub r#type: u32,
    pub jbattery: SdlJoyBatteryEvent,
    pub gaxis: SdlGamepadAxisEvent,
    pub gbutton: SdlGamepadButtonEvent,
    pub gdevice: SdlGamepadDeviceEvent,
    pub user: SdlUserEvent,
    pub padding: [u8; 128],
}

#[derive(Debug, Clone)]
pub enum SdlEvent {
    Quit,
    GamepadAdded(SdlJoystickId),
    GamepadRemoved(SdlJoystickId),
    GamepadRemapped(SdlJoystickId),
    GamepadAxisMotion {
        which: SdlJoystickId,
        axis: SdlGamepadAxis,
        value: i16,
    },
    GamepadButton {
        which: SdlJoystickId,
        button: SdlGamepadButton,
        down: bool,
    },
    BatteryUpdated {
        which: SdlJoystickId,
        state: SdlPowerState,
        percent: Option<u8>,
    },
    Wakeup {
        code: i32,
    },
    Other(u32),
}

#[repr(C)]
pub struct SdlGamepadOpaque {
    _private: [u8; 0],
}

#[repr(C)]
pub struct SdlIoStreamOpaque {
    _private: [u8; 0],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdlPowerInfo {
    pub state: SdlPowerState,
    pub percent: Option<u8>,
}

struct SdlApi {
    #[cfg(windows)]
    _module: HMODULE,
    sdl_init: unsafe extern "C" fn(u32) -> bool,
    sdl_quit: unsafe extern "C" fn(),
    sdl_get_error: unsafe extern "C" fn() -> *const c_char,
    sdl_set_hint: unsafe extern "C" fn(*const c_char, *const c_char) -> bool,
    sdl_free: unsafe extern "C" fn(*mut c_void),
    sdl_io_from_const_mem: unsafe extern "C" fn(*const c_void, usize) -> *mut SdlIoStreamOpaque,
    sdl_add_gamepad_mappings_from_io: unsafe extern "C" fn(*mut SdlIoStreamOpaque, bool) -> c_int,
    sdl_get_gamepads: unsafe extern "C" fn(*mut c_int) -> *mut SdlJoystickId,
    sdl_open_gamepad: unsafe extern "C" fn(SdlJoystickId) -> *mut SdlGamepadOpaque,
    sdl_close_gamepad: unsafe extern "C" fn(*mut SdlGamepadOpaque),
    sdl_get_gamepad_id: unsafe extern "C" fn(*mut SdlGamepadOpaque) -> SdlJoystickId,
    sdl_get_gamepad_name: unsafe extern "C" fn(*mut SdlGamepadOpaque) -> *const c_char,
    sdl_get_gamepad_path: unsafe extern "C" fn(*mut SdlGamepadOpaque) -> *const c_char,
    sdl_get_gamepad_type: unsafe extern "C" fn(*mut SdlGamepadOpaque) -> i32,
    sdl_get_gamepad_vendor: unsafe extern "C" fn(*mut SdlGamepadOpaque) -> u16,
    sdl_get_gamepad_product: unsafe extern "C" fn(*mut SdlGamepadOpaque) -> u16,
    sdl_get_gamepad_serial: unsafe extern "C" fn(*mut SdlGamepadOpaque) -> *const c_char,
    sdl_get_gamepad_connection_state: unsafe extern "C" fn(*mut SdlGamepadOpaque) -> i32,
    sdl_get_gamepad_power_info: unsafe extern "C" fn(*mut SdlGamepadOpaque, *mut c_int) -> i32,
    sdl_get_gamepad_axis: unsafe extern "C" fn(*mut SdlGamepadOpaque, i32) -> i16,
    sdl_get_gamepad_button: unsafe extern "C" fn(*mut SdlGamepadOpaque, i32) -> bool,
    sdl_rumble_gamepad: unsafe extern "C" fn(*mut SdlGamepadOpaque, u16, u16, u32) -> bool,
    sdl_register_events: unsafe extern "C" fn(c_int) -> u32,
    sdl_push_event: unsafe extern "C" fn(*mut RawSdlEvent) -> bool,
    sdl_poll_event: unsafe extern "C" fn(*mut RawSdlEvent) -> bool,
    sdl_wait_event: unsafe extern "C" fn(*mut RawSdlEvent) -> bool,
    sdl_wait_event_timeout: unsafe extern "C" fn(*mut RawSdlEvent, i32) -> bool,
}

unsafe impl Send for SdlApi {}
unsafe impl Sync for SdlApi {}

static SDL_API: OnceLock<Result<SdlApi, String>> = OnceLock::new();

#[cfg(windows)]
fn extract_embedded_sdl3_dll() -> Result<PathBuf, SdlError> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let runtime_dir = base.join("z-controller").join("runtime");
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| SdlError::RuntimeSetup(format!("create runtime dir: {e}")))?;

    let dll_path = runtime_dir.join("SDL3-3.4.20.dll");
    let needs_write = match std::fs::metadata(&dll_path) {
        Ok(meta) => meta.len() != EMBEDDED_SDL3_DLL.len() as u64,
        Err(_) => true,
    };

    if needs_write {
        let tmp_path = runtime_dir.join(format!("SDL3-3.4.20.dll.tmp.{}", std::process::id()));
        std::fs::write(&tmp_path, EMBEDDED_SDL3_DLL)
            .map_err(|e| SdlError::RuntimeSetup(format!("write SDL3.dll: {e}")))?;
        if let Err(err) = std::fs::rename(&tmp_path, &dll_path) {
            let _ = std::fs::remove_file(&tmp_path);
            if !dll_path.exists() {
                return Err(SdlError::RuntimeSetup(format!("rename SDL3.dll: {err}")));
            }
        }
    }

    Ok(dll_path)
}

#[cfg(windows)]
unsafe fn resolve_sym<T>(module: HMODULE, name: &'static str) -> Result<T, SdlError> {
    let cname = CString::new(name).map_err(|_| SdlError::MissingSymbol(name))?;
    let proc = unsafe { GetProcAddress(module, cname.as_ptr().cast()) }
        .ok_or(SdlError::MissingSymbol(name))?;
    Ok(unsafe { std::mem::transmute_copy(&proc) })
}

#[cfg(windows)]
fn load_sdl_api() -> Result<&'static SdlApi, SdlError> {
    let res = SDL_API.get_or_init(|| {
        let dll_path = extract_embedded_sdl3_dll().map_err(|e| e.to_string())?;
        let wide: Vec<u16> = dll_path
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let module = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        };
        if module.is_null() {
            let err = std::io::Error::last_os_error();
            return Err(format!("LoadLibraryExW({}): {err}", dll_path.display()));
        }

        let load_res: Result<SdlApi, String> = (|| -> Result<SdlApi, SdlError> {
            unsafe {
                Ok(SdlApi {
                    _module: module,
                    sdl_init: resolve_sym(module, "SDL_Init")?,
                    sdl_quit: resolve_sym(module, "SDL_Quit")?,
                    sdl_get_error: resolve_sym(module, "SDL_GetError")?,
                    sdl_set_hint: resolve_sym(module, "SDL_SetHint")?,
                    sdl_free: resolve_sym(module, "SDL_free")?,
                    sdl_io_from_const_mem: resolve_sym(module, "SDL_IOFromConstMem")?,
                    sdl_add_gamepad_mappings_from_io: resolve_sym(
                        module,
                        "SDL_AddGamepadMappingsFromIO",
                    )?,
                    sdl_get_gamepads: resolve_sym(module, "SDL_GetGamepads")?,
                    sdl_open_gamepad: resolve_sym(module, "SDL_OpenGamepad")?,
                    sdl_close_gamepad: resolve_sym(module, "SDL_CloseGamepad")?,
                    sdl_get_gamepad_id: resolve_sym(module, "SDL_GetGamepadID")?,
                    sdl_get_gamepad_name: resolve_sym(module, "SDL_GetGamepadName")?,
                    sdl_get_gamepad_path: resolve_sym(module, "SDL_GetGamepadPath")?,
                    sdl_get_gamepad_type: resolve_sym(module, "SDL_GetGamepadType")?,
                    sdl_get_gamepad_vendor: resolve_sym(module, "SDL_GetGamepadVendor")?,
                    sdl_get_gamepad_product: resolve_sym(module, "SDL_GetGamepadProduct")?,
                    sdl_get_gamepad_serial: resolve_sym(module, "SDL_GetGamepadSerial")?,
                    sdl_get_gamepad_connection_state: resolve_sym(
                        module,
                        "SDL_GetGamepadConnectionState",
                    )?,
                    sdl_get_gamepad_power_info: resolve_sym(module, "SDL_GetGamepadPowerInfo")?,
                    sdl_get_gamepad_axis: resolve_sym(module, "SDL_GetGamepadAxis")?,
                    sdl_get_gamepad_button: resolve_sym(module, "SDL_GetGamepadButton")?,
                    sdl_rumble_gamepad: resolve_sym(module, "SDL_RumbleGamepad")?,
                    sdl_register_events: resolve_sym(module, "SDL_RegisterEvents")?,
                    sdl_push_event: resolve_sym(module, "SDL_PushEvent")?,
                    sdl_poll_event: resolve_sym(module, "SDL_PollEvent")?,
                    sdl_wait_event: resolve_sym(module, "SDL_WaitEvent")?,
                    sdl_wait_event_timeout: resolve_sym(module, "SDL_WaitEventTimeout")?,
                })
            }
        })()
        .map_err(|e| e.to_string());

        match load_res {
            Ok(api) => Ok(api),
            Err(e) => {
                unsafe { FreeLibrary(module) };
                Err(e.to_string())
            }
        }
    });

    match res {
        Ok(api) => Ok(api),
        Err(msg) => Err(SdlError::LoadLibrary(msg.clone())),
    }
}

fn cstr_opt(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let s = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    if s.is_empty() { None } else { Some(s) }
}

pub struct SdlContext {
    api: &'static SdlApi,
    wakeup_event_type: u32,
}

#[derive(Clone)]
pub struct SdlEventWaker {
    api: &'static SdlApi,
    wakeup_event_type: u32,
}

impl SdlEventWaker {
    pub fn wake(&self, code: i32) {
        let mut ev = RawSdlEvent {
            padding: [0u8; 128],
        };
        ev.user = SdlUserEvent {
            r#type: self.wakeup_event_type,
            reserved: 0,
            timestamp: 0,
            window_id: 0,
            code,
            data1: std::ptr::null_mut(),
            data2: std::ptr::null_mut(),
        };
        unsafe {
            (self.api.sdl_push_event)(&mut ev);
        }
    }
}

impl SdlContext {
    pub fn init() -> Result<Self, SdlError> {
        #[cfg(windows)]
        let api = load_sdl_api()?;
        #[cfg(not(windows))]
        return Err(SdlError::LoadLibrary(
            "non-Windows dynamic SDL3 loader not wired in v1".into(),
        ));

        // Hints MUST be configured before SDL_Init so background input and HIDAPI drivers engage.
        for (name, val) in [
            ("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1"),
            ("SDL_JOYSTICK_HIDAPI", "1"),
            ("SDL_JOYSTICK_HIDAPI_SWITCH", "1"),
            ("SDL_JOYSTICK_HIDAPI_PS4", "1"),
            ("SDL_JOYSTICK_HIDAPI_PS5", "1"),
            ("SDL_JOYSTICK_HIDAPI_XBOX", "1"),
            ("SDL_JOYSTICK_HIDAPI_SWITCH_HOME_LED", "0"),
            ("SDL_GAMECONTROLLER_USE_BUTTON_LABELS", "0"),
        ] {
            let cname = CString::new(name).unwrap();
            let cval = CString::new(val).unwrap();
            unsafe {
                (api.sdl_set_hint)(cname.as_ptr(), cval.as_ptr());
            }
        }

        let ok = unsafe {
            (api.sdl_init)(SDL_INIT_EVENTS | SDL_INIT_JOYSTICK | SDL_INIT_GAMEPAD)
        };
        if !ok {
            return Err(SdlError::Sdl(last_sdl_error(api)));
        }

        let wakeup_event_type = unsafe { (api.sdl_register_events)(1) };
        if wakeup_event_type == 0 {
            unsafe { (api.sdl_quit)() };
            return Err(SdlError::Sdl("SDL_RegisterEvents returned 0".into()));
        }

        let ctx = Self {
            api,
            wakeup_event_type,
        };
        let _ = ctx.add_mappings_from_bytes(EMBEDDED_GAMECONTROLLERDB)?;
        Ok(ctx)
    }

    pub fn waker(&self) -> SdlEventWaker {
        SdlEventWaker {
            api: self.api,
            wakeup_event_type: self.wakeup_event_type,
        }
    }

    pub fn add_mappings_from_bytes(&self, data: &[u8]) -> Result<i32, SdlError> {
        let io = unsafe { (self.api.sdl_io_from_const_mem)(data.as_ptr().cast(), data.len()) };
        if io.is_null() {
            return Err(SdlError::Sdl(last_sdl_error(self.api)));
        }
        let added = unsafe { (self.api.sdl_add_gamepad_mappings_from_io)(io, true) };
        if added < 0 {
            return Err(SdlError::Sdl(last_sdl_error(self.api)));
        }
        Ok(added)
    }

    pub fn add_mappings_from_file(&self, path: &Path) -> Result<i32, SdlError> {
        let bytes = std::fs::read(path)
            .map_err(|e| SdlError::RuntimeSetup(format!("read {}: {e}", path.display())))?;
        self.add_mappings_from_bytes(&bytes)
    }

    pub fn gamepad_ids(&self) -> Result<Vec<SdlJoystickId>, SdlError> {
        let mut count: c_int = 0;
        let ptr = unsafe { (self.api.sdl_get_gamepads)(&mut count) };
        if ptr.is_null() {
            return Err(SdlError::Sdl(last_sdl_error(self.api)));
        }
        let ids = if count > 0 {
            unsafe { std::slice::from_raw_parts(ptr, count as usize) }.to_vec()
        } else {
            Vec::new()
        };
        unsafe { (self.api.sdl_free)(ptr.cast()) };
        Ok(ids)
    }

    pub fn open_gamepad(&self, id: SdlJoystickId) -> Result<SdlGamepad, SdlError> {
        let ptr = unsafe { (self.api.sdl_open_gamepad)(id) };
        if ptr.is_null() {
            return Err(SdlError::Sdl(last_sdl_error(self.api)));
        }
        Ok(SdlGamepad {
            api: self.api,
            ptr,
        })
    }

    pub fn poll_event(&self) -> Option<SdlEvent> {
        let mut raw = RawSdlEvent {
            padding: [0u8; 128],
        };
        let ok = unsafe { (self.api.sdl_poll_event)(&mut raw) };
        if ok {
            Some(self.decode_event(&raw))
        } else {
            None
        }
    }

    pub fn wait_event(&self) -> Result<SdlEvent, SdlError> {
        let mut raw = RawSdlEvent {
            padding: [0u8; 128],
        };
        let ok = unsafe { (self.api.sdl_wait_event)(&mut raw) };
        if !ok {
            return Err(SdlError::Sdl(last_sdl_error(self.api)));
        }
        Ok(self.decode_event(&raw))
    }

    pub fn wait_event_timeout(&self, timeout_ms: i32) -> Option<SdlEvent> {
        let mut raw = RawSdlEvent {
            padding: [0u8; 128],
        };
        let ok = unsafe { (self.api.sdl_wait_event_timeout)(&mut raw, timeout_ms) };
        if ok {
            Some(self.decode_event(&raw))
        } else {
            None
        }
    }

    fn decode_event(&self, raw: &RawSdlEvent) -> SdlEvent {
        let ev_type = unsafe { raw.r#type };
        if ev_type == self.wakeup_event_type {
            let user = unsafe { raw.user };
            return SdlEvent::Wakeup { code: user.code };
        }
        match ev_type {
            SDL_EVENT_QUIT => SdlEvent::Quit,
            SDL_EVENT_GAMEPAD_ADDED => {
                let d = unsafe { raw.gdevice };
                SdlEvent::GamepadAdded(d.which)
            }
            SDL_EVENT_GAMEPAD_REMOVED => {
                let d = unsafe { raw.gdevice };
                SdlEvent::GamepadRemoved(d.which)
            }
            SDL_EVENT_GAMEPAD_REMAPPED => {
                let d = unsafe { raw.gdevice };
                SdlEvent::GamepadRemapped(d.which)
            }
            SDL_EVENT_GAMEPAD_AXIS_MOTION => {
                let a = unsafe { raw.gaxis };
                if let Some(axis) = SdlGamepadAxis::from_raw(a.axis) {
                    SdlEvent::GamepadAxisMotion {
                        which: a.which,
                        axis,
                        value: a.value,
                    }
                } else {
                    SdlEvent::Other(ev_type)
                }
            }
            SDL_EVENT_GAMEPAD_BUTTON_DOWN | SDL_EVENT_GAMEPAD_BUTTON_UP => {
                let b = unsafe { raw.gbutton };
                if let Some(button) = SdlGamepadButton::from_raw(b.button) {
                    SdlEvent::GamepadButton {
                        which: b.which,
                        button,
                        down: b.down,
                    }
                } else {
                    SdlEvent::Other(ev_type)
                }
            }
            SDL_EVENT_JOYSTICK_BATTERY_UPDATED => {
                let b = unsafe { raw.jbattery };
                SdlEvent::BatteryUpdated {
                    which: b.which,
                    state: SdlPowerState::from_raw(b.state),
                    percent: if (0..=100).contains(&b.percent) {
                        Some(b.percent as u8)
                    } else {
                        None
                    },
                }
            }
            other => SdlEvent::Other(other),
        }
    }
}

impl Drop for SdlContext {
    fn drop(&mut self) {
        unsafe {
            (self.api.sdl_quit)();
        }
    }
}

pub struct SdlGamepad {
    api: &'static SdlApi,
    ptr: *mut SdlGamepadOpaque,
}

impl SdlGamepad {
    pub fn id(&self) -> SdlJoystickId {
        unsafe { (self.api.sdl_get_gamepad_id)(self.ptr) }
    }

    pub fn name(&self) -> String {
        cstr_opt(unsafe { (self.api.sdl_get_gamepad_name)(self.ptr) })
            .unwrap_or_else(|| "Unknown Gamepad".to_string())
    }

    pub fn path(&self) -> Option<String> {
        cstr_opt(unsafe { (self.api.sdl_get_gamepad_path)(self.ptr) })
    }

    pub fn serial(&self) -> Option<String> {
        cstr_opt(unsafe { (self.api.sdl_get_gamepad_serial)(self.ptr) })
    }

    pub fn vendor(&self) -> u16 {
        unsafe { (self.api.sdl_get_gamepad_vendor)(self.ptr) }
    }

    pub fn product(&self) -> u16 {
        unsafe { (self.api.sdl_get_gamepad_product)(self.ptr) }
    }

    pub fn gamepad_type(&self) -> SdlGamepadType {
        SdlGamepadType::from_raw(unsafe { (self.api.sdl_get_gamepad_type)(self.ptr) })
    }

    pub fn connection_state(&self) -> SdlConnectionState {
        SdlConnectionState::from_raw(unsafe { (self.api.sdl_get_gamepad_connection_state)(self.ptr) })
    }

    pub fn power_info(&self) -> SdlPowerInfo {
        let mut pct: c_int = -1;
        let st = unsafe { (self.api.sdl_get_gamepad_power_info)(self.ptr, &mut pct) };
        SdlPowerInfo {
            state: SdlPowerState::from_raw(st),
            percent: if (0..=100).contains(&pct) {
                Some(pct as u8)
            } else {
                None
            },
        }
    }

    pub fn axis(&self, axis: SdlGamepadAxis) -> i16 {
        unsafe { (self.api.sdl_get_gamepad_axis)(self.ptr, axis as i32) }
    }

    pub fn button(&self, button: SdlGamepadButton) -> bool {
        unsafe { (self.api.sdl_get_gamepad_button)(self.ptr, button as i32) }
    }

    pub fn rumble(&self, low_freq: u16, high_freq: u16, duration_ms: u32) -> bool {
        unsafe { (self.api.sdl_rumble_gamepad)(self.ptr, low_freq, high_freq, duration_ms) }
    }
}

impl Drop for SdlGamepad {
    fn drop(&mut self) {
        unsafe {
            (self.api.sdl_close_gamepad)(self.ptr);
        }
    }
}

fn last_sdl_error(api: &SdlApi) -> String {
    cstr_opt(unsafe { (api.sdl_get_error)() }).unwrap_or_else(|| "unknown SDL error".to_string())
}
