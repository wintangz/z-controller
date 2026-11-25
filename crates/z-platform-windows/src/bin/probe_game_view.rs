use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    DI8DEVCLASS_GAMECTRL, DIDEVICEINSTANCEW, DIEDFL_ATTACHEDONLY, DIRECTINPUT_VERSION,
    DirectInput8Create,
};
use windows_sys::Win32::Media::Multimedia::{
    JOYCAPSW, JOYINFOEX, joyGetDevCapsW, joyGetNumDevs, joyGetPosEx,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::core::GUID;
use z_platform_windows::pnp::{HidOpenStatus, enumerate_hid_devices};

const IID_IDIRECTINPUT8W: GUID = GUID {
    data1: 0xBF79_8031,
    data2: 0x483A,
    data3: 0x4DA2,
    data4: [0xAA, 0x99, 0x5D, 0x64, 0xED, 0x36, 0x97, 0x00],
};

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    thumb_rx: i16,
    thumb_ry: i16,
}

#[repr(C)]
struct XInputState {
    packet_number: u32,
    gamepad: XInputGamepad,
}

#[repr(C)]
struct IDirectInput8WVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    create_device:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void, *mut c_void) -> i32,
    enum_devices: unsafe extern "system" fn(
        *mut c_void,
        u32,
        unsafe extern "system" fn(*const DIDEVICEINSTANCEW, *mut c_void) -> i32,
        *mut c_void,
        u32,
    ) -> i32,
}

#[repr(C)]
struct IDirectInput8W {
    vtbl: *const IDirectInput8WVtbl,
}

#[derive(Default)]
struct DiDeviceRecord {
    instance_name: String,
    product_name: String,
    usage_page: u16,
    usage: u16,
}

unsafe extern "system" fn di_enum_cb(
    inst: *const DIDEVICEINSTANCEW,
    ctx: *mut c_void,
) -> i32 {
    if inst.is_null() || ctx.is_null() {
        return 1;
    }
    let list = unsafe { &mut *(ctx.cast::<Vec<DiDeviceRecord>>()) };
    let r = unsafe { &*inst };
    let iname_len = r
        .tszInstanceName
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(r.tszInstanceName.len());
    let pname_len = r
        .tszProductName
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(r.tszProductName.len());
    list.push(DiDeviceRecord {
        instance_name: String::from_utf16_lossy(&r.tszInstanceName[..iname_len]),
        product_name: String::from_utf16_lossy(&r.tszProductName[..pname_len]),
        usage_page: r.wUsagePage,
        usage: r.wUsage,
    });
    1 // DIENUM_CONTINUE
}

fn enum_directinput8() -> Result<Vec<DiDeviceRecord>, i32> {
    let hinst = unsafe { GetModuleHandleW(null()) };
    let mut di_ptr: *mut c_void = null_mut();
    let hr = unsafe {
        DirectInput8Create(
            hinst,
            DIRECTINPUT_VERSION,
            &IID_IDIRECTINPUT8W,
            &mut di_ptr,
            null_mut(),
        )
    };
    if hr < 0 || di_ptr.is_null() {
        return Err(hr);
    }

    let di = di_ptr.cast::<IDirectInput8W>();
    let mut devices = Vec::<DiDeviceRecord>::new();
    let hr_enum = unsafe {
        let vtbl = &*(*di).vtbl;
        (vtbl.enum_devices)(
            di_ptr,
            DI8DEVCLASS_GAMECTRL,
            di_enum_cb,
            (&mut devices as *mut Vec<DiDeviceRecord>).cast(),
            DIEDFL_ATTACHEDONLY,
        )
    };
    unsafe {
        let vtbl = &*(*di).vtbl;
        (vtbl.release)(di_ptr);
    }
    if hr_enum < 0 {
        return Err(hr_enum);
    }
    Ok(devices)
}

fn probe_winmm() {
    let num = unsafe { joyGetNumDevs() };
    let mut active = 0usize;
    for slot in 0..num.min(16) {
        let mut caps: JOYCAPSW = unsafe { std::mem::zeroed() };
        if unsafe { joyGetDevCapsW(slot as usize, &mut caps, size_of::<JOYCAPSW>() as u32) } == 0 {
            let mut info: JOYINFOEX = unsafe { std::mem::zeroed() };
            info.dwSize = size_of::<JOYINFOEX>() as u32;
            info.dwFlags = 0xFF; // JOY_RETURNALL
            let pos_rc = unsafe { joyGetPosEx(slot, &mut info) };
            let pname = caps.szPname;
            let mid = caps.wMid;
            let pid = caps.wPid;
            let nul = pname.iter().position(|&c| c == 0).unwrap_or(pname.len());
            let name = String::from_utf16_lossy(&pname[..nul]);
            if pos_rc == 0 {
                active += 1;
                let xpos = info.dwXpos;
                let ypos = info.dwYpos;
                let btns = info.dwButtons;
                println!(
                    "  [WinMM] slot={slot} VID={mid:04X} PID={pid:04X} name='{name}' X={xpos} Y={ypos} buttons=0x{btns:X}"
                );
            }
        }
    }
    println!("  [WinMM] active_slots={active}");
}

fn probe_xinput() {
    type FnXInputGetState = unsafe extern "system" fn(u32, *mut XInputState) -> u32;
    let dll: Vec<u16> = "xinput1_4.dll\0".encode_utf16().collect();
    let module = unsafe { LoadLibraryW(dll.as_ptr()) };
    if module.is_null() {
        println!("  [XInput] failed to load xinput1_4.dll");
        return;
    }
    let Some(proc) = (unsafe { GetProcAddress(module, b"XInputGetState\0".as_ptr()) }) else {
        println!("  [XInput] missing XInputGetState");
        return;
    };
    let func: FnXInputGetState = unsafe { std::mem::transmute_copy(&proc) };
    let mut active = 0usize;
    for user in 0..4 {
        let mut st: XInputState = unsafe { std::mem::zeroed() };
        let rc = unsafe { func(user, &mut st) };
        if rc == 0 {
            active += 1;
            println!(
                "  [XInput] slot={} pkt={} wButtons=0x{:04X} LT={} RT={} sThumbLX={} sThumbLY={} sThumbRX={} sThumbRY={}",
                user,
                st.packet_number,
                st.gamepad.buttons,
                st.gamepad.left_trigger,
                st.gamepad.right_trigger,
                st.gamepad.thumb_lx,
                st.gamepad.thumb_ly,
                st.gamepad.thumb_rx,
                st.gamepad.thumb_ry
            );
        }
    }
    println!("  [XInput] connected_slots={active}");
}

fn main() {
    probe_winmm();

    match enum_directinput8() {
        Ok(list) => {
            println!("  [DirectInput8] count={}", list.len());
            for d in &list {
                println!(
                    "  [DirectInput8] DI8 name='{}' product='{}' usage={:02X}:{:02X}",
                    d.instance_name, d.product_name, d.usage_page, d.usage
                );
            }
        }
        Err(hr) => println!("  [DirectInput8] error HRESULT=0x{hr:08X}"),
    }

    probe_xinput();

    let hids = enumerate_hid_devices();
    let game_hids: Vec<_> = hids
        .into_iter()
        .filter(|d| {
            (d.usage_page == 0x01 && (d.usage == 0x04 || d.usage == 0x05))
                || d.open_status == HidOpenStatus::AccessDenied
                || d.vendor_id == 0x057E
                || d.vendor_id == 0x045E
        })
        .collect();
    println!("  [HID] relevant_devices={}", game_hids.len());
    for d in game_hids {
        println!(
            "  [HID] VID={:04X} PID={:04X} status={:?} usage={:02X}:{:02X} vigem={} instance='{}'",
            d.vendor_id,
            d.product_id,
            d.open_status,
            d.usage_page,
            d.usage,
            d.is_vigem_virtual,
            d.instance_id
        );
    }
}
