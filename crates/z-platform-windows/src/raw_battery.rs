use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile,
};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawBatteryStatus {
    OnBattery,
    Charging,
    Charged,
    Wired,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawBatteryReading {
    pub percent: Option<u8>,
    pub status: RawBatteryStatus,
    pub raw_byte: Option<u8>,
    pub source: &'static str,
}

/// Nintendo Switch Pro / Joy-Con report IDs `0x30` and `0x21` battery nibble formula
/// matching Valve/SDL3 `SDL_hidapi_switch.c`.
pub fn parse_switch_report_battery(report: &[u8]) -> Option<RawBatteryReading> {
    if report.len() < 3 {
        return None;
    }
    if report[0] != 0x30 && report[0] != 0x21 {
        return None;
    }
    let b2 = report[2];
    let charging = (b2 & 0x10) != 0;
    let level = ((b2 & 0xE0) >> 4).min(8);
    let percent = ((level as f32 / 8.0) * 100.0).round() as u8;
    let status = if charging {
        if level >= 8 {
            RawBatteryStatus::Charged
        } else {
            RawBatteryStatus::Charging
        }
    } else {
        RawBatteryStatus::OnBattery
    };
    Some(RawBatteryReading {
        percent: Some(percent),
        status,
        raw_byte: Some(b2),
        source: "switch-hid-0x30",
    })
}

/// Read one input report directly from a Switch HID path (shared mode, non-blocking attempt).
pub fn read_switch_hid_battery(interface_path: &str) -> Option<RawBatteryReading> {
    let wide: Vec<u16> = interface_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut buf = [0u8; 64];
    let mut read_bytes = 0u32;
    let mut found = None;
    for _ in 0..4 {
        let ok = unsafe {
            ReadFile(
                handle,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut read_bytes,
                null_mut(),
            )
        };
        if ok == 0 {
            break;
        }
        if let Some(reading) = parse_switch_report_battery(&buf[..read_bytes as usize]) {
            found = Some(reading);
            break;
        }
    }

    unsafe { CloseHandle(handle) };
    found
}

#[repr(C)]
#[derive(Default)]
struct XInputBatteryInfo {
    battery_type: u8,
    battery_level: u8,
}

pub fn read_xinput_battery(user_index: u32) -> Option<RawBatteryReading> {
    type FnGetBatt = unsafe extern "system" fn(u32, u8, *mut XInputBatteryInfo) -> u32;
    let dll: Vec<u16> = "xinput1_4.dll\0".encode_utf16().collect();
    let module = unsafe { LoadLibraryW(dll.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let proc = unsafe { GetProcAddress(module, b"XInputGetBatteryInformation\0".as_ptr()) }?;
    let func: FnGetBatt = unsafe { std::mem::transmute_copy(&proc) };

    let mut info = XInputBatteryInfo::default();
    let res = unsafe { func(user_index, 0 /* BATTERY_DEVTYPE_GAMEPAD */, &mut info) };
    if res != 0 {
        return None;
    }

    let (status, percent) = match info.battery_type {
        0x01 /* WIRED */ => (RawBatteryStatus::Wired, Some(100)),
        0x02 | 0x03 /* ALKALINE | NIMH */ => {
            let pct = match info.battery_level {
                0 => 0,
                1 => 25,
                2 => 50,
                _ => 100,
            };
            (RawBatteryStatus::OnBattery, Some(pct))
        }
        _ => (RawBatteryStatus::Unknown, None),
    };
    Some(RawBatteryReading {
        percent,
        status,
        raw_byte: Some(info.battery_level),
        source: "xinput1_4",
    })
}
