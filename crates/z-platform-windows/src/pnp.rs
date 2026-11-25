use std::mem::{offset_of, size_of};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_DRP_HARDWAREID, CM_DRP_SERVICE, CM_Get_DevNode_Registry_PropertyW,
    CM_Get_Device_Interface_PropertyW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL,
    CM_Locate_DevNodeW, CR_SUCCESS, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList,
    SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HIDD_ATTRIBUTES, HIDP_CAPS, HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid,
    HidD_GetPreparsedData, HidP_GetCaps,
};
use windows_sys::Win32::Devices::Properties::DEVPKEY_Device_InstanceId;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, GENERIC_READ, GetLastError, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::core::GUID;

#[derive(Debug, Clone)]
pub struct HidDeviceInfo {
    pub interface_path: String,
    pub instance_id: String,
    pub open_status: HidOpenStatus,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub is_vigem_virtual: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidOpenStatus {
    Ok,
    AccessDenied,
    Error(u32),
}

pub fn interface_path_to_instance_id(interface_path: &str) -> Option<String> {
    let wide: Vec<u16> = interface_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut prop_type = 0u32;
    let mut bytes_needed = 0u32;
    let cr = unsafe {
        CM_Get_Device_Interface_PropertyW(
            wide.as_ptr(),
            &DEVPKEY_Device_InstanceId,
            &mut prop_type,
            null_mut(),
            &mut bytes_needed,
            0,
        )
    };
    if bytes_needed >= 2 {
        let mut buf = vec![0u16; (bytes_needed as usize).div_ceil(2)];
        let cr2 = unsafe {
            CM_Get_Device_Interface_PropertyW(
                wide.as_ptr(),
                &DEVPKEY_Device_InstanceId,
                &mut prop_type,
                buf.as_mut_ptr().cast::<u8>(),
                &mut bytes_needed,
                0,
            )
        };
        if cr2 == CR_SUCCESS {
            let nul = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let s = String::from_utf16_lossy(&buf[..nul]);
            if !s.is_empty() {
                return Some(s.to_ascii_uppercase());
            }
        }
    } else if cr == CR_SUCCESS {
        // Unreachable with 0-byte probe unless empty
    }

    // Fallback: parse `\\?\hid#vid_...#instance#{guid}`
    let trimmed = interface_path
        .strip_prefix("\\\\?\\")
        .or_else(|| interface_path.strip_prefix("\\??\\"))
        .unwrap_or(interface_path);
    let mut parts = trimmed.split('#').collect::<Vec<_>>();
    if parts.len() >= 4 {
        parts.pop(); // interface GUID
        return Some(parts.join("\\").to_ascii_uppercase());
    }
    None
}

fn read_devnode_string_prop(devinst: u32, prop: u32) -> Vec<String> {
    let mut reg_type = 0u32;
    let mut bytes_needed = 0u32;
    unsafe {
        CM_Get_DevNode_Registry_PropertyW(
            devinst,
            prop,
            &mut reg_type,
            null_mut(),
            &mut bytes_needed,
            0,
        );
    }
    if bytes_needed < 2 {
        return Vec::new();
    }
    let mut buf = vec![0u16; (bytes_needed as usize).div_ceil(2)];
    let cr = unsafe {
        CM_Get_DevNode_Registry_PropertyW(
            devinst,
            prop,
            &mut reg_type,
            buf.as_mut_ptr().cast(),
            &mut bytes_needed,
            0,
        )
    };
    if cr != CR_SUCCESS {
        return Vec::new();
    }
    let mut out = Vec::new();
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
            out.push(s);
        }
        start = end + 1;
    }
    out
}

pub fn is_vigem_virtual_instance(instance_id: &str) -> bool {
    let upper = instance_id.to_ascii_uppercase();
    if upper.contains("VIGEM") || upper.contains("IG_00") {
        // Check ancestor tree directly when possible; IG_00 can also be a physical Xbox pad,
        // so only mark ViGEm when an ancestor node is owned by ViGEmBus.
    }
    let wide: Vec<u16> = instance_id
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut devinst = 0u32;
    let cr = unsafe { CM_Locate_DevNodeW(&mut devinst, wide.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) };
    if cr != CR_SUCCESS {
        return false;
    }

    for _ in 0..8 {
        for prop in [CM_DRP_SERVICE, CM_DRP_HARDWAREID] {
            for val in read_devnode_string_prop(devinst, prop) {
                let v = val.to_ascii_lowercase();
                if v.contains("vigem") || v.contains("nefarius\\vigem") {
                    return true;
                }
            }
        }
        let mut parent = 0u32;
        if unsafe { CM_Get_Parent(&mut parent, devinst, 0) } != CR_SUCCESS {
            break;
        }
        if parent == devinst {
            break;
        }
        devinst = parent;
    }
    false
}

pub fn is_vigem_virtual_device_path(interface_path: Option<&str>) -> bool {
    let Some(path) = interface_path else {
        return false;
    };
    let Some(instance_id) = interface_path_to_instance_id(path) else {
        return false;
    };
    is_vigem_virtual_instance(&instance_id)
}

/// Returns `true` when at least one ViGEm virtual Xbox 360 HID node (`045E:028E`) is present
/// and zero non-ViGEm physical `045E:028E` HID nodes exist, used to suppress pathless SDL XInput loops.
pub fn only_vigem_x360_hid_present() -> bool {
    let hids = enumerate_hid_devices();
    let mut saw_vigem = false;
    let mut saw_non_vigem = false;
    for d in hids {
        if d.vendor_id == 0x045E && d.product_id == 0x028E {
            if d.is_vigem_virtual {
                saw_vigem = true;
            } else {
                saw_non_vigem = true;
            }
        }
    }
    saw_vigem && !saw_non_vigem
}

struct DevInfoGuard(HDEVINFO);
impl Drop for DevInfoGuard {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

pub fn enumerate_hid_devices() -> Vec<HidDeviceInfo> {
    let mut hid_guid: GUID = unsafe { std::mem::zeroed() };
    unsafe { HidD_GetHidGuid(&mut hid_guid) };

    let devices = unsafe {
        SetupDiGetClassDevsW(
            &hid_guid,
            null(),
            null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    if devices == INVALID_HANDLE_VALUE as HDEVINFO {
        return Vec::new();
    }
    let _guard = DevInfoGuard(devices);

    let mut results = Vec::new();
    let mut index = 0u32;
    loop {
        let mut iface: SP_DEVICE_INTERFACE_DATA = unsafe { std::mem::zeroed() };
        iface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
        let ok = unsafe {
            SetupDiEnumDeviceInterfaces(devices, null(), &hid_guid, index, &mut iface)
        };
        if ok == 0 {
            break;
        }
        index += 1;

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
        let wide_path = path_slice[..nul_pos].to_vec();
        let interface_path = String::from_utf16_lossy(&wide_path);
        let instance_id = interface_path_to_instance_id(&interface_path).unwrap_or_default();
        let is_vigem_virtual = is_vigem_virtual_instance(&instance_id);

        let mut wide_nul = wide_path;
        wide_nul.push(0);
        let handle = unsafe {
            CreateFileW(
                wide_nul.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            let status = if err == ERROR_ACCESS_DENIED {
                HidOpenStatus::AccessDenied
            } else {
                HidOpenStatus::Error(err)
            };
            let (vid, pid) = parse_vid_pid_from_id(&instance_id);
            results.push(HidDeviceInfo {
                interface_path,
                instance_id,
                open_status: status,
                vendor_id: vid,
                product_id: pid,
                usage_page: 0,
                usage: 0,
                is_vigem_virtual,
            });
            continue;
        }

        let mut attrs: HIDD_ATTRIBUTES = unsafe { std::mem::zeroed() };
        attrs.Size = size_of::<HIDD_ATTRIBUTES>() as u32;
        unsafe { HidD_GetAttributes(handle, &mut attrs) };

        let mut usage_page = 0u16;
        let mut usage = 0u16;
        let mut preparsed: isize = 0;
        if unsafe { HidD_GetPreparsedData(handle, &mut preparsed) } != 0 && preparsed != 0 {
            let mut caps: HIDP_CAPS = unsafe { std::mem::zeroed() };
            if unsafe { HidP_GetCaps(preparsed, &mut caps) } >= 0 {
                usage_page = caps.UsagePage;
                usage = caps.Usage;
            }
            unsafe { HidD_FreePreparsedData(preparsed) };
        }

        unsafe { CloseHandle(handle) };

        results.push(HidDeviceInfo {
            interface_path,
            instance_id,
            open_status: HidOpenStatus::Ok,
            vendor_id: attrs.VendorID,
            product_id: attrs.ProductID,
            usage_page,
            usage,
            is_vigem_virtual,
        });
    }

    results
}

fn parse_vid_pid_from_id(id: &str) -> (u16, u16) {
    let upper = id.to_ascii_uppercase();
    let vid = extract_hex_after(&upper, "VID_").or_else(|| extract_hex_after(&upper, "VID&0002"))
        .unwrap_or(0);
    let pid = extract_hex_after(&upper, "PID_").or_else(|| extract_hex_after(&upper, "PID&"))
        .unwrap_or(0);
    (vid, pid)
}

fn extract_hex_after(haystack: &str, prefix: &str) -> Option<u16> {
    let pos = haystack.find(prefix)?;
    let rest = &haystack[pos + prefix.len()..];
    if rest.len() < 4 {
        return None;
    }
    u16::from_str_radix(&rest[..4], 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_instance_id_parses_hid_interface_path() {
        let path = r"\\?\HID#{00001124-0000-1000-8000-00805f9b34fb}_VID&0002057e_PID&2009#9&1a2dbc92&1&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";
        let id = interface_path_to_instance_id(path).unwrap();
        assert_eq!(
            id,
            r"HID\{00001124-0000-1000-8000-00805F9B34FB}_VID&0002057E_PID&2009\9&1A2DBC92&1&0000"
        );
        assert_eq!(parse_vid_pid_from_id(&id), (0x057E, 0x2009));
    }
}
