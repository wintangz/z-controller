use std::path::PathBuf;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, INFINITE, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use z_core::backend::DriverStatus;

use crate::hidhide::{self, HidHideError};
use crate::vigem::VigemClient;

// Demand-paged in PE `.rdata` — zero heap/private RAM until explicitly extracted for setup.
const VIGEMBUS_INSTALLER_EXE: &[u8] =
    include_bytes!("../../../assets/drivers/ViGEmBus_Setup.exe");
const HIDHIDE_INSTALLER_EXE: &[u8] =
    include_bytes!("../../../assets/drivers/HidHide_Setup.exe");

#[derive(Debug, Clone, Default)]
pub struct DriverInstallOutcome {
    pub vigem_installed_now: bool,
    pub hidhide_installed_now: bool,
    pub reboot_recommended: bool,
    pub message: String,
}

pub fn bundled_installer_sizes() -> (usize, usize) {
    (VIGEMBUS_INSTALLER_EXE.len(), HIDHIDE_INSTALLER_EXE.len())
}

pub fn detect_driver_status(reboot_recommended: bool) -> DriverStatus {
    let vigem_installed = VigemClient::connect().is_ok();
    let hidhide_installed = hidhide::is_hidhide_installed();
    let (hidhide_accessible, hidhide_error) = if !hidhide_installed {
        (false, Some("HidHide driver is not installed".to_string()))
    } else {
        match hidhide::query_state() {
            Ok(_) => (true, None),
            Err(HidHideError::AccessDenied) => (
                false,
                Some("HidHideClient is open and holds \\\\.\\HidHide — close it to toggle cloaking".to_string()),
            ),
            Err(e) => (false, Some(e.to_string())),
        }
    };

    DriverStatus {
        vigem_installed,
        hidhide_installed,
        hidhide_accessible,
        hidhide_error,
        reboot_recommended,
    }
}

pub fn install_missing_drivers() -> Result<DriverInstallOutcome, String> {
    let current = detect_driver_status(false);
    if current.vigem_installed && current.hidhide_installed {
        return Ok(DriverInstallOutcome {
            message: "All required drivers are already installed.".to_string(),
            ..Default::default()
        });
    }

    let setup_dir = std::env::temp_dir().join("z-controller-setup");
    std::fs::create_dir_all(&setup_dir)
        .map_err(|e| format!("create setup dir {}: {e}", setup_dir.display()))?;

    let mut out = DriverInstallOutcome::default();
    let mut notes = Vec::new();

    if !current.vigem_installed {
        let exe = setup_dir.join("ViGEmBus_Setup.exe");
        std::fs::write(&exe, VIGEMBUS_INSTALLER_EXE)
            .map_err(|e| format!("extract ViGEmBus installer: {e}"))?;
        let code = run_elevated_installer(&exe)?;
        if code == 0 || code == 3010 {
            out.vigem_installed_now = true;
            notes.push("ViGEmBus installed");
        } else {
            return Err(format!("ViGEmBus installer exited with code {code}"));
        }
    }

    if !current.hidhide_installed {
        let exe = setup_dir.join("HidHide_Setup.exe");
        std::fs::write(&exe, HIDHIDE_INSTALLER_EXE)
            .map_err(|e| format!("extract HidHide installer: {e}"))?;
        let code = run_elevated_installer(&exe)?;
        if code == 0 || code == 3010 {
            out.hidhide_installed_now = true;
            out.reboot_recommended = true;
            notes.push("HidHide installed (reboot recommended to finish driver load)");
        } else {
            return Err(format!("HidHide installer exited with code {code}"));
        }
    }

    out.message = if notes.is_empty() {
        "No changes made.".to_string()
    } else {
        notes.join("; ")
    };
    Ok(out)
}

fn run_elevated_installer(exe_path: &PathBuf) -> Result<u32, String> {
    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = exe_path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let params: Vec<u16> = "/quiet /norestart\0".encode_utf16().collect();

    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_SHOWNORMAL;

    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("elevation or installer launch failed (Win32 error {err})"));
    }

    let mut exit_code = 0u32;
    if !info.hProcess.is_null() {
        unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            GetExitCodeProcess(info.hProcess, &mut exit_code);
            CloseHandle(info.hProcess);
        }
    }
    Ok(exit_code)
}

pub fn query_process_memory_bytes() -> (u64, u64) {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    let mut pmc: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut pmc as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            pmc.cb,
        )
    };
    if ok == 0 {
        return (0, 0);
    }
    (pmc.WorkingSetSize as u64, pmc.PrivateUsage as u64)
}

/// Asks Windows to trim non-essential faulted pages after closing the UI window back to tray.
pub fn trim_process_working_set() {
    unsafe {
        windows_sys::Win32::System::ProcessStatus::EmptyWorkingSet(GetCurrentProcess());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_installers_are_non_empty() {
        let (v_len, h_len) = bundled_installer_sizes();
        assert!(v_len > 1_000_000);
        assert!(h_len > 1_000_000);
    }

    #[test]
    fn process_memory_query_returns_nonzero() {
        let (ws, priv_bytes) = query_process_memory_bytes();
        assert!(ws > 0);
        assert!(priv_bytes > 0);
    }
}
