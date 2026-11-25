use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use bitflags::bitflags;
use thiserror::Error;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_IO_PENDING, ERROR_NOT_SUPPORTED, GENERIC_READ,
    GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_NO_BUFFERING, FILE_FLAG_OVERLAPPED,
    FILE_FLAG_WRITE_THROUGH, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::{CancelIoEx, DeviceIoControl, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
use windows_sys::core::GUID;

const VIGEM_BUS_GUID: GUID = GUID {
    data1: 0x96E4_2B22,
    data2: 0xF5E9,
    data3: 0x42F8,
    data4: [0xB0, 0x43, 0xED, 0x0F, 0x93, 0x2F, 0x01, 0x4F],
};

const VIGEM_COMMON_VERSION: u32 = 0x0001;
const FILE_DEVICE_BUSENUM: u32 = 0x0000_002A;
const METHOD_BUFFERED: u32 = 0;
const FILE_READ_ACCESS: u32 = 1;
const FILE_WRITE_ACCESS: u32 = 2;

const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

const IOCTL_VIGEM_PLUGIN_TARGET: u32 =
    ctl_code(FILE_DEVICE_BUSENUM, 0x801, METHOD_BUFFERED, FILE_WRITE_ACCESS);
const IOCTL_VIGEM_UNPLUG_TARGET: u32 =
    ctl_code(FILE_DEVICE_BUSENUM, 0x802, METHOD_BUFFERED, FILE_WRITE_ACCESS);
const IOCTL_VIGEM_CHECK_VERSION: u32 =
    ctl_code(FILE_DEVICE_BUSENUM, 0x803, METHOD_BUFFERED, FILE_WRITE_ACCESS);
const IOCTL_VIGEM_WAIT_DEVICE_READY: u32 =
    ctl_code(FILE_DEVICE_BUSENUM, 0x804, METHOD_BUFFERED, FILE_WRITE_ACCESS);

const IOCTL_XUSB_REQUEST_NOTIFICATION: u32 = ctl_code(
    FILE_DEVICE_BUSENUM,
    0xA01,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);
const IOCTL_XUSB_SUBMIT_REPORT: u32 =
    ctl_code(FILE_DEVICE_BUSENUM, 0xA02, METHOD_BUFFERED, FILE_WRITE_ACCESS);
const IOCTL_DS4_SUBMIT_REPORT: u32 =
    ctl_code(FILE_DEVICE_BUSENUM, 0xA03, METHOD_BUFFERED, FILE_WRITE_ACCESS);
const IOCTL_DS4_REQUEST_NOTIFICATION: u32 =
    ctl_code(FILE_DEVICE_BUSENUM, 0xA04, METHOD_BUFFERED, FILE_WRITE_ACCESS);
const IOCTL_XUSB_GET_USER_INDEX: u32 = ctl_code(
    FILE_DEVICE_BUSENUM,
    0xA07,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

#[derive(Debug, Error)]
pub enum VigemError {
    #[error("ViGEmBus driver not found or not running")]
    BusNotFound,
    #[error("ViGEmBus access denied")]
    AccessDenied,
    #[error("ViGEmBus version mismatch")]
    VersionMismatch,
    #[error("virtual target {serial_no} is not ready yet")]
    TargetNotReady { serial_no: u32 },
    #[error("Win32 IOCTL failure ({op}): code {code}")]
    Ioctl { op: &'static str, code: u32 },
    #[error("timed out waiting for ViGEmBus ({op})")]
    Timeout { op: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VigemTargetKind {
    Xbox360 = 0,
    DualShock4 = 2,
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct X360Buttons: u16 {
        const DPAD_UP        = 0x0001;
        const DPAD_DOWN      = 0x0002;
        const DPAD_LEFT      = 0x0004;
        const DPAD_RIGHT     = 0x0008;
        const START          = 0x0010;
        const BACK           = 0x0020;
        const LEFT_THUMB     = 0x0040;
        const RIGHT_THUMB    = 0x0080;
        const LEFT_SHOULDER  = 0x0100;
        const RIGHT_SHOULDER = 0x0200;
        const GUIDE          = 0x0400;
        const A              = 0x1000;
        const B              = 0x2000;
        const X              = 0x4000;
        const Y              = 0x8000;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct X360Report {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ds4Dpad {
    North = 0,
    NorthEast = 1,
    East = 2,
    SouthEast = 3,
    South = 4,
    SouthWest = 5,
    West = 6,
    NorthWest = 7,
    #[default]
    Neutral = 8,
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Ds4ButtonFlags: u16 {
        const SQUARE         = 1 << 4;
        const CROSS          = 1 << 5;
        const CIRCLE         = 1 << 6;
        const TRIANGLE       = 1 << 7;
        const SHOULDER_LEFT  = 1 << 8;
        const SHOULDER_RIGHT = 1 << 9;
        const TRIGGER_LEFT   = 1 << 10;
        const TRIGGER_RIGHT  = 1 << 11;
        const SHARE          = 1 << 12;
        const OPTIONS        = 1 << 13;
        const THUMB_LEFT     = 1 << 14;
        const THUMB_RIGHT    = 1 << 15;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ds4Report {
    pub thumb_lx: u8,
    pub thumb_ly: u8,
    pub thumb_rx: u8,
    pub thumb_ry: u8,
    pub buttons: u16,
    pub special: u8,
    pub trigger_left: u8,
    pub trigger_right: u8,
}

impl Default for Ds4Report {
    fn default() -> Self {
        Self {
            thumb_lx: 128,
            thumb_ly: 128,
            thumb_rx: 128,
            thumb_ry: 128,
            buttons: Ds4Dpad::Neutral as u16,
            special: 0,
            trigger_left: 0,
            trigger_right: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RumbleFeedback {
    pub large_motor: u8,
    pub small_motor: u8,
    pub led_number: u8,
    pub lightbar_rgb: [u8; 3],
}

#[repr(C)]
struct CheckVersion {
    size: u32,
    version: u32,
}

#[repr(C)]
struct PluginTarget {
    size: u32,
    serial_no: u32,
    target_type: u32,
    vendor_id: u16,
    product_id: u16,
}

#[repr(C)]
struct UnPlugTarget {
    size: u32,
    serial_no: u32,
}

#[repr(C)]
struct WaitDeviceReady {
    size: u32,
    serial_no: u32,
}

#[repr(C)]
struct XusbGetUserIndex {
    size: u32,
    serial_no: u32,
    user_index: u32,
}

#[repr(C)]
struct XusbSubmitReport {
    size: u32,
    serial_no: u32,
    report: X360Report,
}

#[repr(C)]
struct Ds4SubmitReport {
    size: u32,
    serial_no: u32,
    report: Ds4Report,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XusbRequestNotification {
    size: u32,
    serial_no: u32,
    large_motor: u8,
    small_motor: u8,
    led_number: u8,
    _pad: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Ds4RequestNotification {
    size: u32,
    serial_no: u32,
    small_motor: u8,
    large_motor: u8,
    red: u8,
    green: u8,
    blue: u8,
    _pad: [u8; 3],
}

const _: () = {
    assert!(size_of::<CheckVersion>() == 8);
    assert!(size_of::<PluginTarget>() == 16);
    assert!(offset_of!(PluginTarget, vendor_id) == 12);
    assert!(size_of::<UnPlugTarget>() == 8);
    assert!(size_of::<WaitDeviceReady>() == 8);
    assert!(size_of::<X360Report>() == 12 && align_of::<X360Report>() == 2);
    assert!(size_of::<XusbSubmitReport>() == 20 && offset_of!(XusbSubmitReport, report) == 8);
    assert!(size_of::<Ds4Report>() == 10 && align_of::<Ds4Report>() == 2);
    assert!(size_of::<Ds4SubmitReport>() == 20 && offset_of!(Ds4SubmitReport, report) == 8);
    assert!(size_of::<XusbRequestNotification>() == 12);
    assert!(size_of::<Ds4RequestNotification>() == 16);
    assert!(offset_of!(Ds4RequestNotification, small_motor) == 8);
    assert!(offset_of!(Ds4RequestNotification, large_motor) == 9);
};

struct BusHandle {
    handle: HANDLE,
    next_serial: AtomicU32,
}

unsafe impl Send for BusHandle {}
unsafe impl Sync for BusHandle {}

impl Drop for BusHandle {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE && !self.handle.is_null() {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

struct DevInfoGuard(HDEVINFO);
impl Drop for DevInfoGuard {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

unsafe fn ioctl_overlapped(
    handle: HANDLE,
    code: u32,
    in_ptr: *const c_void,
    in_len: u32,
    out_ptr: *mut c_void,
    out_len: u32,
    timeout_ms: u32,
    op: &'static str,
    cancel_slot: Option<&Mutex<Option<usize>>>,
) -> Result<u32, VigemError> {
    let event = unsafe { CreateEventW(null(), 1, 0, null()) };
    if event.is_null() {
        return Err(VigemError::Ioctl {
            op,
            code: unsafe { GetLastError() },
        });
    }

    let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
    ov.hEvent = event;
    if let Some(slot) = cancel_slot {
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(&ov as *const OVERLAPPED as usize);
    }

    let mut bytes: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            code,
            in_ptr,
            in_len,
            out_ptr,
            out_len,
            &mut bytes,
            &mut ov,
        )
    };

    let res = if ok != 0 {
        Ok(bytes)
    } else {
        let err = unsafe { GetLastError() };
        if err == ERROR_IO_PENDING {
            let wait = unsafe { WaitForSingleObject(event, timeout_ms) };
            if wait == WAIT_OBJECT_0 {
                let mut transferred = 0u32;
                let g = unsafe { GetOverlappedResult(handle, &ov, &mut transferred, 0) };
                if g != 0 {
                    Ok(transferred)
                } else {
                    Err(VigemError::Ioctl {
                        op,
                        code: unsafe { GetLastError() },
                    })
                }
            } else {
                unsafe {
                    CancelIoEx(handle, &ov);
                    let mut dummy = 0u32;
                    GetOverlappedResult(handle, &ov, &mut dummy, 1);
                }
                if wait == WAIT_TIMEOUT {
                    Err(VigemError::Timeout { op })
                } else {
                    Err(VigemError::Ioctl {
                        op,
                        code: unsafe { GetLastError() },
                    })
                }
            }
        } else {
            Err(VigemError::Ioctl { op, code: err })
        }
    };

    if let Some(slot) = cancel_slot {
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    unsafe {
        CloseHandle(event);
    }
    res
}

#[derive(Clone)]
pub struct VigemClient {
    bus: Arc<BusHandle>,
}

impl VigemClient {
    pub fn connect() -> Result<Self, VigemError> {
        let devices = unsafe {
            SetupDiGetClassDevsW(
                &VIGEM_BUS_GUID,
                null(),
                null_mut(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        };
        if devices == INVALID_HANDLE_VALUE as HDEVINFO {
            return Err(VigemError::BusNotFound);
        }
        let _guard = DevInfoGuard(devices);

        let mut saw_iface = false;
        let mut last_err = 0u32;
        let mut index = 0u32;

        loop {
            let mut iface: SP_DEVICE_INTERFACE_DATA = unsafe { std::mem::zeroed() };
            iface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            let ok = unsafe {
                SetupDiEnumDeviceInterfaces(devices, null(), &VIGEM_BUS_GUID, index, &mut iface)
            };
            if ok == 0 {
                break;
            }
            index += 1;
            saw_iface = true;

            let mut needed = 0u32;
            unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    devices,
                    &iface,
                    null_mut(),
                    0,
                    &mut needed,
                    null_mut(),
                );
            }
            if needed == 0 {
                continue;
            }

            let words = (needed as usize).div_ceil(size_of::<u32>());
            let mut buf = vec![0u32; words];
            let detail_ptr = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            unsafe {
                (*detail_ptr).cbSize = if cfg!(target_pointer_width = "64") { 8 } else { 6 };
            }

            let ok = unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    devices,
                    &iface,
                    detail_ptr,
                    needed,
                    &mut needed,
                    null_mut(),
                )
            };
            if ok == 0 {
                last_err = unsafe { GetLastError() };
                continue;
            }

            let path_offset = offset_of!(SP_DEVICE_INTERFACE_DETAIL_DATA_W, DevicePath);
            let max_u16 = (buf.len() * size_of::<u32>() - path_offset) / size_of::<u16>();
            let path_slice = unsafe {
                std::slice::from_raw_parts(
                    buf.as_ptr().cast::<u8>().add(path_offset).cast::<u16>(),
                    max_u16,
                )
            };
            let nul_pos = path_slice.iter().position(|&c| c == 0).unwrap_or(max_u16);
            let mut wide_path = path_slice[..nul_pos].to_vec();
            wide_path.push(0);

            let handle = unsafe {
                CreateFileW(
                    wide_path.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL
                        | FILE_FLAG_NO_BUFFERING
                        | FILE_FLAG_WRITE_THROUGH
                        | FILE_FLAG_OVERLAPPED,
                    null_mut(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                last_err = unsafe { GetLastError() };
                continue;
            }

            let check = CheckVersion {
                size: size_of::<CheckVersion>() as u32,
                version: VIGEM_COMMON_VERSION,
            };
            let ver_res = unsafe {
                ioctl_overlapped(
                    handle,
                    IOCTL_VIGEM_CHECK_VERSION,
                    (&check as *const CheckVersion).cast(),
                    check.size,
                    null_mut(),
                    0,
                    5000,
                    "CHECK_VERSION",
                    None,
                )
            };
            match ver_res {
                Ok(_) => {
                    return Ok(Self {
                        bus: Arc::new(BusHandle {
                            handle,
                            next_serial: AtomicU32::new(1),
                        }),
                    });
                }
                Err(VigemError::Ioctl { code, .. }) if code == ERROR_NOT_SUPPORTED => {
                    unsafe { CloseHandle(handle) };
                    return Err(VigemError::VersionMismatch);
                }
                Err(e) => {
                    unsafe { CloseHandle(handle) };
                    if let VigemError::Ioctl { code, .. } = e {
                        last_err = code;
                    }
                }
            }
        }

        if saw_iface && last_err == ERROR_ACCESS_DENIED {
            Err(VigemError::AccessDenied)
        } else {
            Err(VigemError::BusNotFound)
        }
    }

    pub fn plug_x360(&self) -> Result<VigemTarget, VigemError> {
        self.plug(VigemTargetKind::Xbox360, 0x045E, 0x028E)
    }

    pub fn plug_ds4(&self) -> Result<VigemTarget, VigemError> {
        self.plug(VigemTargetKind::DualShock4, 0x054C, 0x05C4)
    }

    pub fn plug(
        &self,
        kind: VigemTargetKind,
        vendor_id: u16,
        product_id: u16,
    ) -> Result<VigemTarget, VigemError> {
        let serial_no = self.bus.next_serial.fetch_add(1, Ordering::Relaxed);
        let req = PluginTarget {
            size: size_of::<PluginTarget>() as u32,
            serial_no,
            target_type: kind as u32,
            vendor_id,
            product_id,
        };
        unsafe {
            ioctl_overlapped(
                self.bus.handle,
                IOCTL_VIGEM_PLUGIN_TARGET,
                (&req as *const PluginTarget).cast(),
                req.size,
                null_mut(),
                0,
                5000,
                "PLUGIN_TARGET",
                None,
            )?;
        }

        let wait_req = WaitDeviceReady {
            size: size_of::<WaitDeviceReady>() as u32,
            serial_no,
        };
        let wait_res = unsafe {
            ioctl_overlapped(
                self.bus.handle,
                IOCTL_VIGEM_WAIT_DEVICE_READY,
                (&wait_req as *const WaitDeviceReady).cast(),
                wait_req.size,
                null_mut(),
                0,
                2500,
                "WAIT_DEVICE_READY",
                None,
            )
        };
        let mut confirmed = wait_res.is_ok();
        if let Err(err) = wait_res {
            match err {
                VigemError::Timeout { .. } => {}
                // 1=INVALID_FUNCTION, 50=NOT_SUPPORTED, 87=INVALID_PARAMETER, 483=DEVICE_HARDWARE_ERROR
                VigemError::Ioctl { code, .. }
                    if matches!(code, 1 | ERROR_NOT_SUPPORTED | 87 | 483) => {}
                other => {
                    let _ = self.unplug_serial(serial_no);
                    return Err(other);
                }
            }
        }

        let mut xinput_user_index = None;
        let mut target = VigemTarget {
            bus: Arc::clone(&self.bus),
            serial_no,
            kind,
            xinput_user_index: None,
            alive: Arc::new(AtomicBool::new(true)),
            in_flight_ov: Arc::new(Mutex::new(None)),
            notify_thread: None,
        };

        if kind == VigemTargetKind::Xbox360 {
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(if confirmed { 600 } else { 3000 });
            while std::time::Instant::now() < deadline {
                let req = XusbGetUserIndex {
                    size: size_of::<XusbGetUserIndex>() as u32,
                    serial_no,
                    user_index: 0,
                };
                let mut resp = XusbGetUserIndex {
                    size: size_of::<XusbGetUserIndex>() as u32,
                    serial_no,
                    user_index: 0,
                };
                let ok = unsafe {
                    ioctl_overlapped(
                        self.bus.handle,
                        IOCTL_XUSB_GET_USER_INDEX,
                        (&req as *const XusbGetUserIndex).cast(),
                        req.size,
                        (&mut resp as *mut XusbGetUserIndex).cast(),
                        resp.size,
                        500,
                        "XUSB_GET_USER_INDEX",
                        None,
                    )
                    .is_ok()
                };
                if ok {
                    xinput_user_index = Some(resp.user_index);
                    confirmed = true;
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(15));
            }
        } else if !confirmed {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                if target.update_ds4(&Ds4Report::default()).is_ok() {
                    confirmed = true;
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(15));
            }
        }
        let _ = confirmed;
        target.xinput_user_index = xinput_user_index;

        Ok(target)
    }

    fn unplug_serial(&self, serial_no: u32) -> Result<(), VigemError> {
        let req = UnPlugTarget {
            size: size_of::<UnPlugTarget>() as u32,
            serial_no,
        };
        unsafe {
            ioctl_overlapped(
                self.bus.handle,
                IOCTL_VIGEM_UNPLUG_TARGET,
                (&req as *const UnPlugTarget).cast(),
                req.size,
                null_mut(),
                0,
                2500,
                "UNPLUG_TARGET",
                None,
            )?;
        }
        Ok(())
    }
}

pub struct VigemTarget {
    bus: Arc<BusHandle>,
    serial_no: u32,
    kind: VigemTargetKind,
    xinput_user_index: Option<u32>,
    alive: Arc<AtomicBool>,
    in_flight_ov: Arc<Mutex<Option<usize>>>,
    notify_thread: Option<JoinHandle<()>>,
}

impl VigemTarget {
    pub fn kind(&self) -> VigemTargetKind {
        self.kind
    }

    pub fn serial_no(&self) -> u32 {
        self.serial_no
    }

    pub fn xinput_user_index(&self) -> Option<u32> {
        if let Some(idx) = self.xinput_user_index {
            return Some(idx);
        }
        if self.kind != VigemTargetKind::Xbox360 {
            return None;
        }
        let req = XusbGetUserIndex {
            size: size_of::<XusbGetUserIndex>() as u32,
            serial_no: self.serial_no,
            user_index: 0,
        };
        let mut resp = XusbGetUserIndex {
            size: size_of::<XusbGetUserIndex>() as u32,
            serial_no: self.serial_no,
            user_index: 0,
        };
        let ok = unsafe {
            ioctl_overlapped(
                self.bus.handle,
                IOCTL_XUSB_GET_USER_INDEX,
                (&req as *const XusbGetUserIndex).cast(),
                req.size,
                (&mut resp as *mut XusbGetUserIndex).cast(),
                resp.size,
                250,
                "XUSB_GET_USER_INDEX",
                None,
            )
            .is_ok()
        };
        if ok { Some(resp.user_index) } else { None }
    }

    pub fn update_x360(&self, report: &X360Report) -> Result<(), VigemError> {
        let submit = XusbSubmitReport {
            size: size_of::<XusbSubmitReport>() as u32,
            serial_no: self.serial_no,
            report: *report,
        };
        let res = unsafe {
            ioctl_overlapped(
                self.bus.handle,
                IOCTL_XUSB_SUBMIT_REPORT,
                (&submit as *const XusbSubmitReport).cast(),
                submit.size,
                null_mut(),
                0,
                1000,
                "XUSB_SUBMIT_REPORT",
                None,
            )
        };
        match res {
            Ok(_) => Ok(()),
            // 259=ERROR_NO_MORE_ITEMS, 170=ERROR_BUSY, 433=ERROR_NO_SUCH_DEVICE
            Err(VigemError::Ioctl { code, .. }) if matches!(code, 259 | 170 | 433) => {
                Err(VigemError::TargetNotReady {
                    serial_no: self.serial_no,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn update_ds4(&self, report: &Ds4Report) -> Result<(), VigemError> {
        let mut submit: Ds4SubmitReport = unsafe { std::mem::zeroed() };
        submit.size = size_of::<Ds4SubmitReport>() as u32;
        submit.serial_no = self.serial_no;
        submit.report = *report;
        let res = unsafe {
            ioctl_overlapped(
                self.bus.handle,
                IOCTL_DS4_SUBMIT_REPORT,
                (&submit as *const Ds4SubmitReport).cast(),
                submit.size,
                null_mut(),
                0,
                1000,
                "DS4_SUBMIT_REPORT",
                None,
            )
        };
        match res {
            Ok(_) => Ok(()),
            Err(VigemError::Ioctl { code, .. }) if matches!(code, 259 | 170 | 433) => {
                Err(VigemError::TargetNotReady {
                    serial_no: self.serial_no,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn start_rumble_listener<F>(
        &mut self,
        on_feedback: F,
    ) -> Result<mpsc::Receiver<RumbleFeedback>, VigemError>
    where
        F: Fn(RumbleFeedback) + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let bus = Arc::clone(&self.bus);
        let serial_no = self.serial_no;
        let kind = self.kind;
        let alive = Arc::clone(&self.alive);
        let cancel_slot = Arc::clone(&self.in_flight_ov);

        let handle = thread::Builder::new()
            .name(format!("z-vigem-fb-{serial_no}"))
            .spawn(move || {
                while alive.load(Ordering::Acquire) {
                    let fb = match kind {
                        VigemTargetKind::Xbox360 => {
                            let mut req: XusbRequestNotification = unsafe { std::mem::zeroed() };
                            req.size = size_of::<XusbRequestNotification>() as u32;
                            req.serial_no = serial_no;
                            let mut resp: XusbRequestNotification = unsafe { std::mem::zeroed() };
                            let r = unsafe {
                                ioctl_overlapped(
                                    bus.handle,
                                    IOCTL_XUSB_REQUEST_NOTIFICATION,
                                    (&req as *const XusbRequestNotification).cast(),
                                    req.size,
                                    (&mut resp as *mut XusbRequestNotification).cast(),
                                    size_of::<XusbRequestNotification>() as u32,
                                    INFINITE,
                                    "XUSB_REQUEST_NOTIFICATION",
                                    Some(&cancel_slot),
                                )
                            };
                            match r {
                                Ok(_) => RumbleFeedback {
                                    large_motor: resp.large_motor,
                                    small_motor: resp.small_motor,
                                    led_number: resp.led_number,
                                    lightbar_rgb: [0, 0, 0],
                                },
                                Err(_) => break,
                            }
                        }
                        VigemTargetKind::DualShock4 => {
                            let mut req: Ds4RequestNotification = unsafe { std::mem::zeroed() };
                            req.size = size_of::<Ds4RequestNotification>() as u32;
                            req.serial_no = serial_no;
                            let mut resp: Ds4RequestNotification = unsafe { std::mem::zeroed() };
                            let r = unsafe {
                                ioctl_overlapped(
                                    bus.handle,
                                    IOCTL_DS4_REQUEST_NOTIFICATION,
                                    (&req as *const Ds4RequestNotification).cast(),
                                    req.size,
                                    (&mut resp as *mut Ds4RequestNotification).cast(),
                                    size_of::<Ds4RequestNotification>() as u32,
                                    INFINITE,
                                    "DS4_REQUEST_NOTIFICATION",
                                    Some(&cancel_slot),
                                )
                            };
                            match r {
                                Ok(_) => RumbleFeedback {
                                    large_motor: resp.large_motor,
                                    small_motor: resp.small_motor,
                                    led_number: 0,
                                    lightbar_rgb: [resp.red, resp.green, resp.blue],
                                },
                                Err(_) => break,
                            }
                        }
                    };

                    if !alive.load(Ordering::Acquire) {
                        break;
                    }
                    on_feedback(fb);
                    let _ = tx.send(fb);
                }
            })
            .map_err(|_| VigemError::Ioctl {
                op: "spawn_rumble_listener",
                code: 1,
            })?;

        self.notify_thread = Some(handle);
        Ok(rx)
    }
}

impl Drop for VigemTarget {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        if let Some(ov_addr) = *self.in_flight_ov.lock().unwrap_or_else(|e| e.into_inner()) {
            unsafe {
                CancelIoEx(self.bus.handle, ov_addr as *const OVERLAPPED);
            }
        }
        if let Some(h) = self.notify_thread.take() {
            let _ = h.join();
        }
        let unplug = UnPlugTarget {
            size: size_of::<UnPlugTarget>() as u32,
            serial_no: self.serial_no,
        };
        unsafe {
            let _ = ioctl_overlapped(
                self.bus.handle,
                IOCTL_VIGEM_UNPLUG_TARGET,
                (&unplug as *const UnPlugTarget).cast(),
                unplug.size,
                null_mut(),
                0,
                2000,
                "UNPLUG_TARGET",
                None,
            );
        }
    }
}
