use std::collections::BTreeSet;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::{Mutex, OnceLock};

use thiserror::Error;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER,
    ERROR_PATH_NOT_FOUND, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    QueryDosDeviceW,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

// Shared/HidHideIoctlContract.h: IoControlDeviceType = 32769, METHOD_BUFFERED = 0, FILE_READ_DATA = 1
const IO_CONTROL_DEVICE_TYPE: u32 = 32769;
const METHOD_BUFFERED: u32 = 0;
const FILE_READ_DATA: u32 = 1;

const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

pub const IOCTL_GET_WHITELIST: u32 =
    ctl_code(IO_CONTROL_DEVICE_TYPE, 2048, METHOD_BUFFERED, FILE_READ_DATA);
pub const IOCTL_SET_WHITELIST: u32 =
    ctl_code(IO_CONTROL_DEVICE_TYPE, 2049, METHOD_BUFFERED, FILE_READ_DATA);
pub const IOCTL_GET_BLACKLIST: u32 =
    ctl_code(IO_CONTROL_DEVICE_TYPE, 2050, METHOD_BUFFERED, FILE_READ_DATA);
pub const IOCTL_SET_BLACKLIST: u32 =
    ctl_code(IO_CONTROL_DEVICE_TYPE, 2051, METHOD_BUFFERED, FILE_READ_DATA);
pub const IOCTL_GET_ACTIVE: u32 =
    ctl_code(IO_CONTROL_DEVICE_TYPE, 2052, METHOD_BUFFERED, FILE_READ_DATA);
pub const IOCTL_SET_ACTIVE: u32 =
    ctl_code(IO_CONTROL_DEVICE_TYPE, 2053, METHOD_BUFFERED, FILE_READ_DATA);
pub const IOCTL_GET_WLINVERSE: u32 =
    ctl_code(IO_CONTROL_DEVICE_TYPE, 2054, METHOD_BUFFERED, FILE_READ_DATA);
pub const IOCTL_SET_WLINVERSE: u32 =
    ctl_code(IO_CONTROL_DEVICE_TYPE, 2055, METHOD_BUFFERED, FILE_READ_DATA);

const _: () = {
    assert!(IOCTL_GET_WHITELIST == 0x8001_6000);
    assert!(IOCTL_SET_WHITELIST == 0x8001_6004);
    assert!(IOCTL_GET_BLACKLIST == 0x8001_6008);
    assert!(IOCTL_SET_BLACKLIST == 0x8001_600C);
    assert!(IOCTL_GET_ACTIVE == 0x8001_6010);
    assert!(IOCTL_SET_ACTIVE == 0x8001_6014);
    assert!(IOCTL_GET_WLINVERSE == 0x8001_6018);
    assert!(IOCTL_SET_WLINVERSE == 0x8001_601C);
};

#[derive(Debug, Error)]
pub enum HidHideError {
    #[error("HidHide driver is not installed or unavailable")]
    NotInstalled,
    #[error("HidHide control device is locked by another client (close HidHideClient)")]
    AccessDenied,
    #[error("HidHide IOCTL failed ({op}): Win32 error {code}")]
    Ioctl { op: &'static str, code: u32 },
    #[error("cannot convert executable path to NT device path: {0}")]
    PathConversion(String),
}

static OWNED_CLOAKS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();

fn owned_cloaks() -> &'static Mutex<BTreeSet<String>> {
    OWNED_CLOAKS.get_or_init(|| Mutex::new(BTreeSet::new()))
}

pub fn is_hidhide_installed() -> bool {
    let dev_name: Vec<u16> = "HidHide\0".encode_utf16().collect();
    let mut target = [0u16; 512];
    let len = unsafe { QueryDosDeviceW(dev_name.as_ptr(), target.as_mut_ptr(), target.len() as u32) };
    len > 0
}

struct ControlHandle(HANDLE);

impl ControlHandle {
    fn open() -> Result<Self, HidHideError> {
        let path: Vec<u16> = "\\\\.\\HidHide\0".encode_utf16().collect();
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            let code = unsafe { GetLastError() };
            return match code {
                ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Err(HidHideError::NotInstalled),
                ERROR_ACCESS_DENIED => Err(HidHideError::AccessDenied),
                _ => Err(HidHideError::Ioctl {
                    op: "CreateFileW(\\\\.\\HidHide)",
                    code,
                }),
            };
        }
        Ok(Self(handle))
    }

    fn get_bool(&self, ioctl: u32, op: &'static str) -> Result<bool, HidHideError> {
        let mut val: u8 = 0;
        let mut returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                self.0,
                ioctl,
                null(),
                0,
                (&mut val as *mut u8).cast::<c_void>(),
                1,
                &mut returned,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(HidHideError::Ioctl {
                op,
                code: unsafe { GetLastError() },
            });
        }
        Ok(val != 0)
    }

    fn set_bool(&self, ioctl: u32, val: bool, op: &'static str) -> Result<(), HidHideError> {
        let byte: u8 = if val { 1 } else { 0 };
        let mut returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                self.0,
                ioctl,
                (&byte as *const u8).cast::<c_void>(),
                1,
                null_mut(),
                0,
                &mut returned,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(HidHideError::Ioctl {
                op,
                code: unsafe { GetLastError() },
            });
        }
        Ok(())
    }

    fn get_multi_sz(&self, ioctl: u32, op: &'static str) -> Result<Vec<String>, HidHideError> {
        let mut needed: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                self.0,
                ioctl,
                null(),
                0,
                null_mut(),
                0,
                &mut needed,
                null_mut(),
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            if err != ERROR_INSUFFICIENT_BUFFER && needed == 0 {
                return Err(HidHideError::Ioctl { op, code: err });
            }
        }
        if needed == 0 {
            return Ok(Vec::new());
        }

        let u16_len = (needed as usize).div_ceil(2);
        let mut buf = vec![0u16; u16_len];
        let mut returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                self.0,
                ioctl,
                null(),
                0,
                buf.as_mut_ptr().cast::<c_void>(),
                (buf.len() * 2) as u32,
                &mut returned,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(HidHideError::Ioctl {
                op,
                code: unsafe { GetLastError() },
            });
        }

        Ok(decode_multi_sz(&buf[..(returned as usize / 2)]))
    }

    fn set_multi_sz(
        &self,
        ioctl: u32,
        items: &[String],
        op: &'static str,
    ) -> Result<(), HidHideError> {
        let mut returned: u32 = 0;
        if items.is_empty() {
            // Logic.c accepts inputBufferLength == 0 to clear list; outputBufferLength must be 0.
            let ok = unsafe {
                DeviceIoControl(
                    self.0,
                    ioctl,
                    null(),
                    0,
                    null_mut(),
                    0,
                    &mut returned,
                    null_mut(),
                )
            };
            if ok == 0 {
                return Err(HidHideError::Ioctl {
                    op,
                    code: unsafe { GetLastError() },
                });
            }
            return Ok(());
        }

        let wide = encode_multi_sz(items);
        let ok = unsafe {
            DeviceIoControl(
                self.0,
                ioctl,
                wide.as_ptr().cast::<c_void>(),
                (wide.len() * 2) as u32,
                null_mut(),
                0,
                &mut returned,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(HidHideError::Ioctl {
                op,
                code: unsafe { GetLastError() },
            });
        }
        Ok(())
    }
}

impl Drop for ControlHandle {
    fn drop(&mut self) {
        if self.0 != INVALID_HANDLE_VALUE && !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

pub fn encode_multi_sz(items: &[String]) -> Vec<u16> {
    let mut out = Vec::new();
    for item in items {
        if item.is_empty() {
            continue;
        }
        out.extend(item.encode_utf16());
        out.push(0);
    }
    if out.is_empty() {
        out.push(0);
    }
    out.push(0);
    out
}

pub fn decode_multi_sz(buf: &[u16]) -> Vec<String> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < buf.len() {
        if buf[start] == 0 {
            break;
        }
        let end = buf[start..]
            .iter()
            .position(|&c| c == 0)
            .map(|p| start + p)
            .unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[start..end]);
        if !s.is_empty() {
            result.push(s);
        }
        start = end + 1;
    }
    result
}

pub fn win32_path_to_nt_device_path(path: &Path) -> Result<String, HidHideError> {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut raw = canonical.to_string_lossy().into_owned();
    if let Some(stripped) = raw.strip_prefix("\\\\?\\") {
        raw = stripped.to_string();
    }
    if raw.len() >= 2 && raw.as_bytes()[1] == b':' {
        let drive = &raw[..2];
        let rest = &raw[2..];
        let drive_w: Vec<u16> = drive.encode_utf16().chain(std::iter::once(0)).collect();
        let mut dev_buf = [0u16; 512];
        let len = unsafe {
            QueryDosDeviceW(drive_w.as_ptr(), dev_buf.as_mut_ptr(), dev_buf.len() as u32)
        };
        if len == 0 {
            return Err(HidHideError::PathConversion(format!(
                "QueryDosDeviceW({drive}) failed: {}",
                unsafe { GetLastError() }
            )));
        }
        let nul = dev_buf.iter().position(|&c| c == 0).unwrap_or(len as usize);
        let nt_prefix = String::from_utf16_lossy(&dev_buf[..nul]);
        Ok(format!("{nt_prefix}{rest}"))
    } else if raw.starts_with("\\Device\\") {
        Ok(raw)
    } else {
        Err(HidHideError::PathConversion(format!(
            "unsupported executable path format: {raw}"
        )))
    }
}

#[derive(Debug, Clone)]
pub struct HidHideState {
    pub active: bool,
    pub inverse_whitelist: bool,
    pub whitelist: Vec<String>,
    pub blacklist: Vec<String>,
}

pub fn query_state() -> Result<HidHideState, HidHideError> {
    let dev = ControlHandle::open()?;
    Ok(HidHideState {
        active: dev.get_bool(IOCTL_GET_ACTIVE, "IOCTL_GET_ACTIVE")?,
        inverse_whitelist: dev.get_bool(IOCTL_GET_WLINVERSE, "IOCTL_GET_WLINVERSE")?,
        whitelist: dev.get_multi_sz(IOCTL_GET_WHITELIST, "IOCTL_GET_WHITELIST")?,
        blacklist: dev.get_multi_sz(IOCTL_GET_BLACKLIST, "IOCTL_GET_BLACKLIST")?,
    })
}

pub fn whitelist_current_exe() -> Result<String, HidHideError> {
    let exe = std::env::current_exe()
        .map_err(|e| HidHideError::PathConversion(format!("current_exe: {e}")))?;
    let nt_path = win32_path_to_nt_device_path(&exe)?;
    let dev = ControlHandle::open()?;
    let mut wl = dev.get_multi_sz(IOCTL_GET_WHITELIST, "IOCTL_GET_WHITELIST")?;
    if !wl.iter().any(|p| p.eq_ignore_ascii_case(&nt_path)) {
        wl.push(nt_path.clone());
        dev.set_multi_sz(IOCTL_SET_WHITELIST, &wl, "IOCTL_SET_WHITELIST")?;
    }
    Ok(nt_path)
}

pub fn remove_exe_from_whitelist(nt_path: &str) -> Result<(), HidHideError> {
    let dev = ControlHandle::open()?;
    let mut wl = dev.get_multi_sz(IOCTL_GET_WHITELIST, "IOCTL_GET_WHITELIST")?;
    let before = wl.len();
    wl.retain(|p| !p.eq_ignore_ascii_case(nt_path));
    if wl.len() != before {
        dev.set_multi_sz(IOCTL_SET_WHITELIST, &wl, "IOCTL_SET_WHITELIST")?;
    }
    Ok(())
}

pub fn set_cloak_instances(
    desired_owned: &BTreeSet<String>,
) -> Result<HidHideState, HidHideError> {
    let exe = std::env::current_exe()
        .map_err(|e| HidHideError::PathConversion(format!("current_exe: {e}")))?;
    let nt_path = win32_path_to_nt_device_path(&exe)?;

    let dev = ControlHandle::open()?;
    let mut wl = dev.get_multi_sz(IOCTL_GET_WHITELIST, "IOCTL_GET_WHITELIST")?;
    if !wl.iter().any(|p| p.eq_ignore_ascii_case(&nt_path)) {
        wl.push(nt_path);
        dev.set_multi_sz(IOCTL_SET_WHITELIST, &wl, "IOCTL_SET_WHITELIST")?;
    }

    if dev.get_bool(IOCTL_GET_WLINVERSE, "IOCTL_GET_WLINVERSE")? {
        dev.set_bool(IOCTL_SET_WLINVERSE, false, "IOCTL_SET_WLINVERSE")?;
    }

    let mut owned_guard = owned_cloaks().lock().unwrap_or_else(|e| e.into_inner());
    let current_bl = dev.get_multi_sz(IOCTL_GET_BLACKLIST, "IOCTL_GET_BLACKLIST")?;

    // Preserve any user-configured entries not managed by this session, then apply desired_owned.
    let mut next_bl: Vec<String> = current_bl
        .into_iter()
        .filter(|id| {
            let upper = id.to_ascii_uppercase();
            !owned_guard.contains(&upper) && !desired_owned.contains(&upper)
        })
        .collect();
    for id in desired_owned {
        next_bl.push(id.clone());
    }

    dev.set_multi_sz(IOCTL_SET_BLACKLIST, &next_bl, "IOCTL_SET_BLACKLIST")?;
    let active = !next_bl.is_empty();
    dev.set_bool(IOCTL_SET_ACTIVE, active, "IOCTL_SET_ACTIVE")?;

    *owned_guard = desired_owned.clone();

    Ok(HidHideState {
        active,
        inverse_whitelist: false,
        whitelist: wl,
        blacklist: next_bl,
    })
}

pub fn cloak_instance(instance_id: &str) -> Result<HidHideState, HidHideError> {
    let mut next = owned_cloaks()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    next.insert(instance_id.to_ascii_uppercase());
    set_cloak_instances(&next)
}

pub fn uncloak_instance(instance_id: &str) -> Result<HidHideState, HidHideError> {
    let mut next = owned_cloaks()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    next.remove(&instance_id.to_ascii_uppercase());
    set_cloak_instances(&next)
}

pub fn emergency_uncloak() -> Result<(), HidHideError> {
    let owned_snapshot = {
        let guard = owned_cloaks().lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_empty() {
            return Ok(());
        }
        guard.clone()
    };

    let dev = ControlHandle::open()?;
    let current_bl = dev.get_multi_sz(IOCTL_GET_BLACKLIST, "IOCTL_GET_BLACKLIST")?;
    let next_bl: Vec<String> = current_bl
        .into_iter()
        .filter(|id| !owned_snapshot.contains(&id.to_ascii_uppercase()))
        .collect();
    dev.set_multi_sz(IOCTL_SET_BLACKLIST, &next_bl, "IOCTL_SET_BLACKLIST")?;
    dev.set_bool(IOCTL_SET_ACTIVE, !next_bl.is_empty(), "IOCTL_SET_ACTIVE")?;
    owned_cloaks()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    Ok(())
}
