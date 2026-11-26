use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::Mutex;

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_DRP_HARDWAREID, CM_DRP_SERVICE, CM_Get_DevNode_Status, CM_Get_Device_IDW, CR_SUCCESS,
    DIGCF_ALLCLASSES, DN_HAS_PROBLEM, DN_NEED_RESTART, HDEVINFO, SP_DEVINFO_DATA,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_SUCCESS, GetLastError, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Cryptography::{
    CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED, CERT_QUERY_FORMAT_FLAG_BINARY,
    CERT_QUERY_OBJECT_FILE, CertCloseStore, CertEnumCertificatesInStore,
    CertFreeCertificateContext, CertGetNameStringW, CryptMsgClose, CryptQueryObject,
};
use windows_sys::Win32::Security::WinTrust::{
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
    WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY,
    WTD_UI_NONE, WinVerifyTrust,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD, REG_SZ, RegCloseKey, RegEnumKeyExW,
    RegOpenKeyExW, RegQueryValueExW,
};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_MANAGER_CONNECT,
    SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_STATUS,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, INFINITE, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL};
use z_core::backend::DriverStatus;
use z_core::driver::{
    DriverEntryStatus, DriverId, DriverProbeInputs, DriverState, LocateOutcome, NotRunningReason,
    PnpProbeState, ServiceProbeState, SignatureCheck, SignatureVerifier, evaluate_driver_state,
    inspect_driver_folder_with,
};

use crate::hidhide::{self, HidHideError};
use crate::pnp::read_devnode_string_prop;
use crate::vigem::VigemClient;

// Demand-paged in PE `.rdata` — zero heap/private RAM until explicitly extracted for user-confirmed setup.
const VIGEMBUS_INSTALLER_EXE: &[u8] =
    include_bytes!("../../../assets/drivers/ViGEmBus_Setup.exe");
const HIDHIDE_INSTALLER_EXE: &[u8] =
    include_bytes!("../../../assets/drivers/HidHide_Setup.exe");

const CERT_NAME_SIMPLE_DISPLAY_TYPE: u32 = 4;
const CM_PROB_NEED_RESTART: u32 = 14;
const SERVICE_DISABLED_START: u32 = 4;

static LOG_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone)]
pub struct AppLog {
    path: PathBuf,
}

impl AppLog {
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join("z-controller").join("z-controller.log")
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, level: &str, message: &str) {
        let _guard = LOG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let (yr, mo, dy, hr, mn, sc) = utc_ymdhms(secs);
        let line = format!("[{yr:04}-{mo:02}-{dy:02} {hr:02}:{mn:02}:{sc:02}Z] [{level}] {message}\n");
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

fn utc_ymdhms(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = y + if m <= 2 { 1 } else { 0 };
    let rem = (secs % 86_400) as u32;
    (yr, m as u32, d as u32, rem / 3600, (rem % 3600) / 60, rem % 60)
}

pub fn bundled_installer_sizes() -> (usize, usize) {
    (VIGEMBUS_INSTALLER_EXE.len(), HIDHIDE_INSTALLER_EXE.len())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerLaunchSpec {
    pub driver: DriverId,
    pub exe_path: PathBuf,
    pub verb: &'static str,
    pub parameters: String,
    pub show_cmd: i32,
}

impl InstallerLaunchSpec {
    pub fn contains_silent_flags(&self) -> bool {
        let lower = self.parameters.to_ascii_lowercase();
        ["/quiet", "/qn", "/qb", "/silent", "/verysilent", "/s", "/passive", "/norestart", "-s", "-q"]
            .iter()
            .any(|flag| lower.contains(flag))
    }
}

pub fn build_installer_launch_spec(driver: DriverId, setup_dir: &Path) -> InstallerLaunchSpec {
    InstallerLaunchSpec {
        driver,
        exe_path: setup_dir.join(driver.bundled_installer_filename()),
        verb: "runas",
        // Strict Requirement 4: never pass silent/quiet/no-wizard flags.
        parameters: String::new(),
        show_cmd: SW_SHOWNORMAL,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallerRunReport {
    InstalledAndWorking { exit_code: u32 },
    InstalledRebootPending { exit_code: u32 },
    InstalledNotRunning { exit_code: u32, reason: NotRunningReason },
    StillNotInstalled { exit_code: u32 },
    InstallerFailed { exit_code: u32 },
    LaunchError(String),
}

impl InstallerRunReport {
    pub fn summary(&self, driver: DriverId) -> String {
        let name = driver.display_name();
        match self {
            Self::InstalledAndWorking { .. } => {
                format!("{name}: installed and working.")
            }
            Self::InstalledRebootPending { .. } => {
                format!("{name}: installed (reboot pending to finish driver setup).")
            }
            Self::InstalledNotRunning { reason, .. } => {
                format!("{name}: installed, but not running ({}).", reason.label())
            }
            Self::StillNotInstalled { .. } => {
                format!("{name}: still not installed (setup closed without installing).")
            }
            Self::InstallerFailed { exit_code } => {
                format!("{name}: installer failed (exit code {exit_code}).")
            }
            Self::LaunchError(err) => format!("{name}: installer launch error: {err}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemedyKind {
    StartService,
    EnableAndStartService,
    ReenableDevice { instance_id: String },
    RestartMachine,
}

impl RemedyKind {
    pub fn button_label(&self) -> &'static str {
        match self {
            Self::StartService => "Start Service",
            Self::EnableAndStartService => "Enable & Start Service",
            Self::ReenableDevice { .. } => "Re-enable Device",
            Self::RestartMachine => "Restart Windows…",
        }
    }

    pub fn requires_reboot_confirm(&self) -> bool {
        matches!(self, Self::RestartMachine)
    }
}

pub fn remedy_for_entry(entry: &DriverEntryStatus) -> Option<RemedyKind> {
    let DriverState::InstalledNotRunning(reason) = entry.state else {
        return None;
    };
    match reason {
        NotRunningReason::ServiceStopped => Some(RemedyKind::StartService),
        NotRunningReason::ServiceDisabled => Some(RemedyKind::EnableAndStartService),
        NotRunningReason::DeviceError { problem_code } => {
            if problem_code == 5 {
                // AccessDenied (e.g. HidHideClient holding exclusive control handle): no UAC action needed.
                None
            } else if let Some(inst) = &entry.pnp_instance_id {
                Some(RemedyKind::ReenableDevice {
                    instance_id: inst.clone(),
                })
            } else {
                Some(RemedyKind::StartService)
            }
        }
        NotRunningReason::RebootPending => Some(RemedyKind::RestartMachine),
    }
}

fn to_wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

struct RegKeyGuard(HKEY);
impl Drop for RegKeyGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { RegCloseKey(self.0) };
        }
    }
}

fn reg_query_dword(hkey: HKEY, value_name: &str) -> Option<u32> {
    let name_w = to_wide_z(value_name);
    let mut ty: u32 = 0;
    let mut data: u32 = 0;
    let mut size: u32 = size_of::<u32>() as u32;
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            name_w.as_ptr(),
            null(),
            &mut ty,
            (&mut data as *mut u32).cast::<u8>(),
            &mut size,
        )
    };
    if rc == ERROR_SUCCESS && ty == REG_DWORD {
        Some(data)
    } else {
        None
    }
}

fn reg_query_sz(hkey: HKEY, value_name: &str) -> Option<String> {
    let name_w = to_wide_z(value_name);
    let mut ty: u32 = 0;
    let mut bytes: u32 = 0;
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            name_w.as_ptr(),
            null(),
            &mut ty,
            null_mut(),
            &mut bytes,
        )
    };
    if rc != ERROR_SUCCESS || ty != REG_SZ || bytes < 2 {
        return None;
    }
    let mut buf = vec![0u16; (bytes as usize).div_ceil(2)];
    let rc2 = unsafe {
        RegQueryValueExW(
            hkey,
            name_w.as_ptr(),
            null(),
            &mut ty,
            buf.as_mut_ptr().cast::<u8>(),
            &mut bytes,
        )
    };
    if rc2 != ERROR_SUCCESS {
        return None;
    }
    let nul = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..nul]).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn query_service_probe(service_name: &str) -> (ServiceProbeState, bool) {
    let subkey = format!("SYSTEM\\CurrentControlSet\\Services\\{service_name}");
    let subkey_w = to_wide_z(&subkey);
    let mut hkey: HKEY = null_mut();
    let rc = unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, subkey_w.as_ptr(), 0, KEY_READ, &mut hkey) };
    if rc != ERROR_SUCCESS || hkey.is_null() {
        return (ServiceProbeState::Absent, false);
    }
    let _reg_guard = RegKeyGuard(hkey);
    let start_type = reg_query_dword(hkey, "Start");
    let delete_flag = reg_query_dword(hkey, "DeleteFlag").unwrap_or(0) != 0;

    let scm = unsafe { OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT) };
    if !scm.is_null() {
        let svc_w = to_wide_z(service_name);
        let svc = unsafe { OpenServiceW(scm, svc_w.as_ptr(), SERVICE_QUERY_STATUS) };
        if !svc.is_null() {
            let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
            let ok = unsafe { QueryServiceStatus(svc, &mut status) };
            unsafe {
                CloseServiceHandle(svc);
                CloseServiceHandle(scm);
            }
            if ok != 0 && status.dwCurrentState == SERVICE_RUNNING {
                return (ServiceProbeState::Running, delete_flag);
            }
        } else {
            unsafe { CloseServiceHandle(scm) };
        }
    }

    if start_type == Some(SERVICE_DISABLED_START) {
        (ServiceProbeState::Disabled, delete_flag)
    } else {
        (ServiceProbeState::Stopped, delete_flag)
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

fn query_pnp_probe(driver: DriverId) -> PnpProbeState {
    let devs = unsafe { SetupDiGetClassDevsW(null(), null(), null_mut(), DIGCF_ALLCLASSES) };
    if devs == INVALID_HANDLE_VALUE as HDEVINFO {
        return PnpProbeState::Absent;
    }
    let _guard = DevInfoGuard(devs);

    let mut idx = 0u32;
    let mut best: Option<PnpProbeState> = None;

    loop {
        let mut dev_info: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
        dev_info.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;
        if unsafe { SetupDiEnumDeviceInfo(devs, idx, &mut dev_info) } == 0 {
            break;
        }
        idx += 1;

        let mut matches = false;
        for s in read_devnode_string_prop(dev_info.DevInst, CM_DRP_SERVICE) {
            let low = s.to_ascii_lowercase();
            match driver {
                DriverId::ViGEmBus if low == "vigembus" => matches = true,
                DriverId::HidHide if low == "hidhide" => matches = true,
                _ => {}
            }
        }
        if !matches {
            for hw in read_devnode_string_prop(dev_info.DevInst, CM_DRP_HARDWAREID) {
                let low = hw.to_ascii_lowercase();
                match driver {
                    DriverId::ViGEmBus
                        if low == "root\\vigembus" || low.contains("nefarius\\vigembus") =>
                    {
                        matches = true
                    }
                    DriverId::HidHide
                        if low == "root\\hidhide" || low.contains("nefarius\\hidhide") =>
                    {
                        matches = true
                    }
                    _ => {}
                }
            }
        }
        if !matches {
            continue;
        }

        let mut status: u32 = 0;
        let mut problem: u32 = 0;
        let cr = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, dev_info.DevInst, 0) };
        if cr != CR_SUCCESS {
            continue;
        }

        let mut id_buf = [0u16; 260];
        let instance_id = if unsafe {
            CM_Get_Device_IDW(dev_info.DevInst, id_buf.as_mut_ptr(), id_buf.len() as u32, 0)
        } == CR_SUCCESS
        {
            let nul = id_buf.iter().position(|&c| c == 0).unwrap_or(id_buf.len());
            String::from_utf16_lossy(&id_buf[..nul])
        } else {
            String::new()
        };

        let cand = if (status & DN_NEED_RESTART) != 0 || problem == CM_PROB_NEED_RESTART {
            PnpProbeState::RebootPending { instance_id }
        } else if (status & DN_HAS_PROBLEM) != 0 || problem != 0 {
            PnpProbeState::Problem {
                instance_id,
                problem_code: problem,
            }
        } else {
            return PnpProbeState::Ok { instance_id };
        };
        best = Some(cand);
    }

    best.unwrap_or(PnpProbeState::Absent)
}

fn query_uninstall_version(driver: DriverId) -> Option<String> {
    let roots = [
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
    ];
    for root in roots {
        let root_w = to_wide_z(root);
        let mut hroot: HKEY = null_mut();
        if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, root_w.as_ptr(), 0, KEY_READ, &mut hroot) }
            != ERROR_SUCCESS
            || hroot.is_null()
        {
            continue;
        }
        let _root_guard = RegKeyGuard(hroot);

        let mut index = 0u32;
        loop {
            let mut name_buf = [0u16; 260];
            let mut name_len = name_buf.len() as u32;
            let rc = unsafe {
                RegEnumKeyExW(
                    hroot,
                    index,
                    name_buf.as_mut_ptr(),
                    &mut name_len,
                    null(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if rc != ERROR_SUCCESS {
                break;
            }
            index += 1;

            let mut hsub: HKEY = null_mut();
            if unsafe { RegOpenKeyExW(hroot, name_buf.as_ptr(), 0, KEY_READ, &mut hsub) }
                != ERROR_SUCCESS
                || hsub.is_null()
            {
                continue;
            }
            let _sub_guard = RegKeyGuard(hsub);
            let Some(disp) = reg_query_sz(hsub, "DisplayName") else {
                continue;
            };
            let low = disp.to_ascii_lowercase();
            let is_match = match driver {
                DriverId::ViGEmBus => {
                    low.contains("vigembus") || low.contains("virtual gamepad emulation bus")
                }
                DriverId::HidHide => low.contains("hidhide"),
            };
            if is_match {
                if let Some(ver) = reg_query_sz(hsub, "DisplayVersion") {
                    return Some(ver);
                }
                return None;
            }
        }
    }
    None
}

pub fn detect_single_driver(driver: DriverId, session_reboot_pending: bool) -> DriverEntryStatus {
    let (functional_working, functional_access_denied, functional_detail) = match driver {
        DriverId::ViGEmBus => match VigemClient::connect() {
            Ok(_) => (true, false, None),
            Err(e) => (false, false, Some(e.to_string())),
        },
        DriverId::HidHide => match hidhide::query_state() {
            Ok(_) => (true, false, None),
            Err(HidHideError::AccessDenied) => (
                false,
                true,
                Some(
                    "HidHideClient is open and holds \\\\.\\HidHide — close it to toggle cloaking"
                        .to_string(),
                ),
            ),
            Err(e) => (false, false, Some(e.to_string())),
        },
    };

    let (service, service_delete_pending) = query_service_probe(driver.service_name());
    let pnp = query_pnp_probe(driver);
    let uninstall_version = query_uninstall_version(driver);

    let inputs = DriverProbeInputs {
        functional_working,
        functional_access_denied,
        functional_detail,
        service,
        service_delete_pending,
        pnp,
        session_reboot_pending,
        uninstall_version,
    };
    evaluate_driver_state(driver, &inputs)
}

pub fn detect_driver_status(
    vigem_reboot_pending: bool,
    hidhide_reboot_pending: bool,
) -> DriverStatus {
    DriverStatus {
        vigem: detect_single_driver(DriverId::ViGEmBus, vigem_reboot_pending),
        hidhide: detect_single_driver(DriverId::HidHide, hidhide_reboot_pending),
    }
}

fn run_elevated_process(
    exe_path: &Path,
    parameters: &str,
    show_cmd: i32,
) -> Result<u32, String> {
    let verb = to_wide_z("runas");
    let file = to_wide_z(&exe_path.to_string_lossy());
    let params_w = if parameters.is_empty() {
        None
    } else {
        Some(to_wide_z(parameters))
    };

    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params_w.as_ref().map_or(null(), |v| v.as_ptr());
    info.nShow = show_cmd;

    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("UAC elevation or launch failed (Win32 error {err})"));
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

pub fn run_driver_installer_wizard(
    driver: DriverId,
    already_reboot_pending: bool,
    log: &AppLog,
) -> (InstallerRunReport, bool) {
    let setup_dir = std::env::temp_dir().join("z-controller-setup");
    if let Err(e) = std::fs::create_dir_all(&setup_dir) {
        let msg = format!("create setup dir {}: {e}", setup_dir.display());
        log.append("ERROR", &format!("Install {} failed: {msg}", driver.display_name()));
        return (InstallerRunReport::LaunchError(msg), already_reboot_pending);
    }

    let spec = build_installer_launch_spec(driver, &setup_dir);
    debug_assert!(spec.parameters.is_empty());
    debug_assert!(!spec.contains_silent_flags());

    let bytes = match driver {
        DriverId::ViGEmBus => VIGEMBUS_INSTALLER_EXE,
        DriverId::HidHide => HIDHIDE_INSTALLER_EXE,
    };
    if let Err(e) = std::fs::write(&spec.exe_path, bytes) {
        let msg = format!("extract {}: {e}", spec.exe_path.display());
        log.append("ERROR", &format!("Install {} failed: {msg}", driver.display_name()));
        return (InstallerRunReport::LaunchError(msg), already_reboot_pending);
    }

    log.append(
        "INFO",
        &format!(
            "Launching installer wizard: driver={} publisher={} exe=\"{}\" verb={} parameters=\"{}\" show=SW_SHOWNORMAL",
            driver.display_name(),
            driver.publisher_short(),
            spec.exe_path.display(),
            spec.verb,
            spec.parameters
        ),
    );

    let exit_code = match run_elevated_process(&spec.exe_path, &spec.parameters, spec.show_cmd) {
        Ok(c) => c,
        Err(e) => {
            log.append(
                "ERROR",
                &format!("Installer launch failed: driver={} error={}", driver.display_name(), e),
            );
            return (InstallerRunReport::LaunchError(e), already_reboot_pending);
        }
    };

    let reboot_flag = already_reboot_pending || exit_code == 3010;
    let post = detect_single_driver(driver, reboot_flag);
    let report = if exit_code == 0 || exit_code == 3010 {
        match post.state {
            DriverState::Working => InstallerRunReport::InstalledAndWorking { exit_code },
            DriverState::InstalledNotRunning(NotRunningReason::RebootPending) => {
                InstallerRunReport::InstalledRebootPending { exit_code }
            }
            DriverState::InstalledNotRunning(reason) => {
                InstallerRunReport::InstalledNotRunning { exit_code, reason }
            }
            DriverState::NotInstalled => InstallerRunReport::StillNotInstalled { exit_code },
        }
    } else if post.state.is_working() {
        InstallerRunReport::InstalledAndWorking { exit_code }
    } else {
        InstallerRunReport::InstallerFailed { exit_code }
    };

    log.append(
        "INFO",
        &format!(
            "Installer finished: driver={} exit_code={} post_state=\"{}\" outcome=\"{}\"",
            driver.display_name(),
            exit_code,
            post.state.status_label(),
            report.summary(driver)
        ),
    );

    let new_reboot_pending = reboot_flag
        || matches!(
            post.state,
            DriverState::InstalledNotRunning(NotRunningReason::RebootPending)
        );
    (report, new_reboot_pending)
}

pub fn dry_run_installer_spec(driver: DriverId, log: &AppLog) -> InstallerLaunchSpec {
    let setup_dir = std::env::temp_dir().join("z-controller-setup");
    let spec = build_installer_launch_spec(driver, &setup_dir);
    log.append(
        "DRY-RUN",
        &format!(
            "driver={} publisher={} exe=\"{}\" verb={} parameters=\"{}\" contains_silent_flags={} show=SW_SHOWNORMAL",
            driver.display_name(),
            driver.publisher_short(),
            spec.exe_path.display(),
            spec.verb,
            spec.parameters,
            spec.contains_silent_flags()
        ),
    );
    spec
}

pub fn run_driver_remedy(driver: DriverId, remedy: &RemedyKind, log: &AppLog) -> Result<String, String> {
    let svc = driver.service_name();
    log.append(
        "INFO",
        &format!("Running remedy: driver={} action={:?}", driver.display_name(), remedy),
    );

    let res = match remedy {
        RemedyKind::StartService => {
            let exe = PathBuf::from(r"C:\Windows\System32\sc.exe");
            let args = format!("start {svc}");
            let code = run_elevated_process(&exe, &args, SW_HIDE)?;
            if code == 0 || code == 1056 {
                Ok(())
            } else {
                Err(format!("sc start {svc} exited with code {code}"))
            }
        }
        RemedyKind::EnableAndStartService => {
            let exe = PathBuf::from(r"C:\Windows\System32\cmd.exe");
            let start_mode = match driver {
                DriverId::ViGEmBus => "system",
                DriverId::HidHide => "demand",
            };
            let args = format!("/C sc.exe config {svc} start= {start_mode} && sc.exe start {svc}");
            let code = run_elevated_process(&exe, &args, SW_HIDE)?;
            if code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "Enable & start service {svc} exited with code {code}"
                ))
            }
        }
        RemedyKind::ReenableDevice { instance_id } => {
            let exe = PathBuf::from(r"C:\Windows\System32\pnputil.exe");
            let clean_id = instance_id.replace('"', "");
            let args = format!("/enable-device \"{clean_id}\"");
            let code = run_elevated_process(&exe, &args, SW_HIDE)?;
            if code == 0 || code == 3010 {
                Ok(())
            } else {
                Err(format!("pnputil /enable-device exited with code {code}"))
            }
        }
        RemedyKind::RestartMachine => {
            let exe = PathBuf::from(r"C:\Windows\System32\shutdown.exe");
            let args = "/r /t 5 /c \"z-controller: restarting Windows to finish loading driver\"";
            let code = run_elevated_process(&exe, args, SW_HIDE)?;
            if code == 0 {
                return Ok("Windows restart scheduled in 5 seconds.".to_string());
            } else {
                Err(format!("shutdown.exe /r exited with code {code}"))
            }
        }
    };

    match res {
        Ok(()) => {
            let post = detect_single_driver(driver, false);
            let msg = format!(
                "{}: remedy completed -> {}.",
                driver.display_name(),
                post.state.status_label()
            );
            log.append("INFO", &msg);
            Ok(msg)
        }
        Err(e) => {
            log.append("ERROR", &format!("Remedy failed for {}: {e}", driver.display_name()));
            Err(e)
        }
    }
}

fn read_pe_company_name(path: &Path) -> Option<String> {
    let wide = to_wide_z(&path.to_string_lossy());
    let mut handle: u32 = 0;
    let size = unsafe { GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(wide.as_ptr(), 0, size, buf.as_mut_ptr().cast()) } == 0 {
        return None;
    }

    let trans_q = to_wide_z("\\VarFileInfo\\Translation");
    let mut ptr: *mut c_void = null_mut();
    let mut len: u32 = 0;
    let (lang, cp) = if unsafe {
        VerQueryValueW(buf.as_ptr().cast(), trans_q.as_ptr(), &mut ptr, &mut len)
    } != 0
        && !ptr.is_null()
        && len >= 4
    {
        let pair = unsafe { *(ptr as *const [u16; 2]) };
        (pair[0], pair[1])
    } else {
        (0x0409, 0x04B0)
    };

    let sub_q = to_wide_z(&format!("\\StringFileInfo\\{lang:04X}{cp:04X}\\CompanyName"));
    let mut str_ptr: *mut c_void = null_mut();
    let mut str_len: u32 = 0;
    if unsafe { VerQueryValueW(buf.as_ptr().cast(), sub_q.as_ptr(), &mut str_ptr, &mut str_len) }
        != 0
        && !str_ptr.is_null()
        && str_len > 0
    {
        let slice = unsafe { std::slice::from_raw_parts(str_ptr as *const u16, str_len as usize) };
        let nul = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
        let s = String::from_utf16_lossy(&slice[..nul]).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

pub struct Win32SignatureVerifier;

impl SignatureVerifier for Win32SignatureVerifier {
    fn verify_nefarius(&self, path: &Path) -> SignatureCheck {
        let wide = to_wide_z(&path.to_string_lossy());
        let mut file_info: WINTRUST_FILE_INFO = unsafe { std::mem::zeroed() };
        file_info.cbStruct = size_of::<WINTRUST_FILE_INFO>() as u32;
        file_info.pcwszFilePath = wide.as_ptr();

        let mut wtd: WINTRUST_DATA = unsafe { std::mem::zeroed() };
        wtd.cbStruct = size_of::<WINTRUST_DATA>() as u32;
        wtd.dwUIChoice = WTD_UI_NONE;
        wtd.fdwRevocationChecks = WTD_REVOKE_NONE;
        wtd.dwUnionChoice = WTD_CHOICE_FILE;
        wtd.Anonymous = WINTRUST_DATA_0 {
            pFile: &mut file_info,
        };
        wtd.dwStateAction = WTD_STATEACTION_VERIFY;

        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let trust_rc = unsafe {
            WinVerifyTrust(
                INVALID_HANDLE_VALUE,
                &mut action,
                (&mut wtd as *mut WINTRUST_DATA).cast(),
            )
        };
        wtd.dwStateAction = WTD_STATEACTION_CLOSE;
        unsafe {
            WinVerifyTrust(
                INVALID_HANDLE_VALUE,
                &mut action,
                (&mut wtd as *mut WINTRUST_DATA).cast(),
            );
        }

        if trust_rc != 0 {
            return SignatureCheck::UnsignedOrInvalid {
                reason: format!("WinVerifyTrust rejected signature (0x{trust_rc:08X})"),
            };
        }

        let mut store: *mut c_void = null_mut();
        let mut msg: *mut c_void = null_mut();
        let queried = unsafe {
            CryptQueryObject(
                CERT_QUERY_OBJECT_FILE,
                wide.as_ptr().cast(),
                CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
                CERT_QUERY_FORMAT_FLAG_BINARY,
                0,
                null_mut(),
                null_mut(),
                null_mut(),
                &mut store,
                &mut msg,
                null_mut(),
            )
        };
        if queried == 0 || store.is_null() {
            if !msg.is_null() {
                unsafe { CryptMsgClose(msg) };
            }
            return SignatureCheck::UnsignedOrInvalid {
                reason: "no embedded PKCS7 Authenticode certificate found".to_string(),
            };
        }

        let mut signers = Vec::new();
        let mut prev = null();
        loop {
            let ctx = unsafe { CertEnumCertificatesInStore(store, prev) };
            if ctx.is_null() {
                break;
            }
            prev = ctx;
            let mut name_buf = [0u16; 256];
            let len = unsafe {
                CertGetNameStringW(
                    ctx,
                    CERT_NAME_SIMPLE_DISPLAY_TYPE,
                    0,
                    null(),
                    name_buf.as_mut_ptr(),
                    name_buf.len() as u32,
                )
            };
            if len > 1 {
                let name = String::from_utf16_lossy(&name_buf[..(len as usize - 1)]);
                signers.push(name);
            }
        }
        if !prev.is_null() {
            unsafe { CertFreeCertificateContext(prev) };
        }
        unsafe {
            CertCloseStore(store, 0);
            if !msg.is_null() {
                CryptMsgClose(msg);
            }
        }

        if let Some(nef) = signers
            .iter()
            .find(|s| s.to_ascii_lowercase().contains("nefarius"))
        {
            return SignatureCheck::ValidNefarius {
                signer: nef.clone(),
            };
        }

        let is_sys = path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sys"));
        let whql = signers.iter().any(|s| {
            s.to_ascii_lowercase()
                .contains("microsoft windows hardware compatibility publisher")
        });
        if is_sys && whql {
            if let Some(company) = read_pe_company_name(path) {
                if company.to_ascii_lowercase().contains("nefarius") {
                    return SignatureCheck::ValidNefarius {
                        signer: format!("WHQL ({company})"),
                    };
                }
            }
        }

        let first_signer = signers
            .into_iter()
            .next()
            .unwrap_or_else(|| "unknown signer".to_string());
        SignatureCheck::WrongPublisher {
            signer: first_signer,
        }
    }
}

pub fn locate_and_verify_driver_folder(
    driver: DriverId,
    folder: &Path,
    reboot_pending: bool,
    log: &AppLog,
) -> LocateOutcome {
    let verifier = Win32SignatureVerifier;
    let outcome = inspect_driver_folder_with(driver, folder, &verifier, || {
        detect_single_driver(driver, reboot_pending).state
    });
    log.append(
        "INFO",
        &format!(
            "Manual locate check: driver={} folder=\"{}\" outcome=\"{}\"",
            driver.display_name(),
            folder.display(),
            outcome.summary_message()
        ),
    );
    outcome
}

pub fn query_process_memory_bytes() -> (u64, u64) {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    let mut pmc: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    pmc.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
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

    #[test]
    fn installer_launch_spec_has_no_silent_or_quiet_flags() {
        let setup_dir = PathBuf::from(r"C:\Temp\z-controller-setup");
        for driver in DriverId::ALL {
            let spec = build_installer_launch_spec(driver, &setup_dir);
            assert_eq!(spec.verb, "runas");
            assert_eq!(spec.parameters, "");
            assert!(!spec.contains_silent_flags());
            assert_eq!(spec.show_cmd, SW_SHOWNORMAL);
            assert_eq!(
                spec.exe_path.file_name().and_then(|s| s.to_str()),
                Some(driver.bundled_installer_filename())
            );
        }
    }

    #[test]
    fn win32_signature_verifier_accepts_nefarius_and_rejects_wrong_publisher_without_executing() {
        let base = std::env::temp_dir().join(format!("z_ctrl_sig_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        // Wrong publisher check: copy notepad.exe disguised as HidHideCLI.exe
        let notepad = Path::new(r"C:\Windows\System32\notepad.exe");
        if notepad.is_file() {
            let wrong_dir = base.join("wrong_pub");
            std::fs::create_dir_all(&wrong_dir).unwrap();
            std::fs::copy(notepad, wrong_dir.join("HidHideCLI.exe")).unwrap();

            let log = AppLog::new(base.join("test.log"));
            let out = locate_and_verify_driver_folder(DriverId::HidHide, &wrong_dir, false, &log);
            assert!(
                matches!(out, LocateOutcome::WrongSignature { .. }),
                "Microsoft notepad.exe disguised as HidHideCLI.exe must fail Nefarius publisher check, got {out:?}"
            );
        }

        // Valid publisher check against bundled signed Nefarius installer written to temp dir (never executed).
        let ok_dir = base.join("nefarius_ok");
        std::fs::create_dir_all(&ok_dir).unwrap();
        std::fs::write(ok_dir.join("ViGEmBus_Setup.exe"), VIGEMBUS_INSTALLER_EXE).unwrap();
        let verifier = Win32SignatureVerifier;
        let check = verifier.verify_nefarius(&ok_dir.join("ViGEmBus_Setup.exe"));
        assert!(
            matches!(check, SignatureCheck::ValidNefarius { .. }),
            "expected bundled Nefarius installer to pass Authenticode verification, got {check:?}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
