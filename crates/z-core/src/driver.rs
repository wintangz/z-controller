use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Supported kernel-level companion drivers managed independently by z-controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DriverId {
    ViGEmBus,
    HidHide,
}

impl DriverId {
    pub const ALL: [DriverId; 2] = [DriverId::ViGEmBus, DriverId::HidHide];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::ViGEmBus => "ViGEmBus",
            Self::HidHide => "HidHide",
        }
    }

    pub fn service_name(self) -> &'static str {
        match self {
            Self::ViGEmBus => "ViGEmBus",
            Self::HidHide => "HidHide",
        }
    }

    pub fn publisher_short(self) -> &'static str {
        "nefarius"
    }

    pub fn publisher_full(self) -> &'static str {
        "Nefarius Software Solutions e.U. (nefarius)"
    }

    pub fn requirement_badge(self) -> &'static str {
        match self {
            Self::ViGEmBus => "REQUIRED",
            Self::HidHide => "OPTIONAL",
        }
    }

    pub fn is_required(self) -> bool {
        matches!(self, Self::ViGEmBus)
    }

    pub fn purpose_line(self) -> &'static str {
        match self {
            Self::ViGEmBus => "REQUIRED to create virtual Xbox 360 / DualShock 4 pads.",
            Self::HidHide => "OPTIONAL, only needed to hide (cloak) the physical pad from games.",
        }
    }

    pub fn bundled_installer_filename(self) -> &'static str {
        match self {
            Self::ViGEmBus => "ViGEmBus_Setup.exe",
            Self::HidHide => "HidHide_Setup.exe",
        }
    }

    pub fn bundled_version_hint(self) -> &'static str {
        match self {
            Self::ViGEmBus => "1.22.0",
            Self::HidHide => "1.5.230",
        }
    }

    pub fn install_impact(self) -> &'static str {
        match self {
            Self::ViGEmBus => {
                "Installs the Nefarius Virtual Gamepad Emulation Bus kernel driver (ViGEmBus.sys) and Windows service so z-controller can create virtual Xbox 360 and DualShock 4 controllers."
            }
            Self::HidHide => {
                "Installs the Nefarius HidHide HID filter kernel driver (HidHide.sys), control service, and HidHide tools so z-controller can hide physical pads from games. A Windows reboot may be required after installation."
            }
        }
    }

    pub fn expected_locate_filenames(self) -> &'static [&'static str] {
        match self {
            Self::ViGEmBus => &["vigembus.sys", "vigembus_setup.exe"],
            Self::HidHide => &[
                "hidhidecli.exe",
                "hidhideclient.exe",
                "hidhide.sys",
                "hidhide_setup.exe",
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceProbeState {
    Running,
    Stopped,
    Disabled,
    #[default]
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PnpProbeState {
    Ok {
        instance_id: String,
    },
    Problem {
        instance_id: String,
        problem_code: u32,
    },
    RebootPending {
        instance_id: String,
    },
    #[default]
    Absent,
}

impl PnpProbeState {
    pub fn instance_id(&self) -> Option<&str> {
        match self {
            Self::Ok { instance_id }
            | Self::Problem { instance_id, .. }
            | Self::RebootPending { instance_id } => Some(instance_id.as_str()),
            Self::Absent => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DriverProbeInputs {
    pub functional_working: bool,
    pub functional_access_denied: bool,
    pub functional_detail: Option<String>,
    pub service: ServiceProbeState,
    pub service_delete_pending: bool,
    pub pnp: PnpProbeState,
    pub session_reboot_pending: bool,
    pub uninstall_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotRunningReason {
    ServiceStopped,
    ServiceDisabled,
    DeviceError { problem_code: u32 },
    RebootPending,
}

impl NotRunningReason {
    pub fn label(self) -> String {
        match self {
            Self::ServiceStopped => "service stopped".to_string(),
            Self::ServiceDisabled => "service disabled".to_string(),
            Self::DeviceError { problem_code } => {
                if problem_code == 0 {
                    "device error".to_string()
                } else {
                    format!("device error, problem code {problem_code}")
                }
            }
            Self::RebootPending => "reboot pending".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DriverState {
    Working,
    InstalledNotRunning(NotRunningReason),
    #[default]
    NotInstalled,
}

impl DriverState {
    pub fn is_working(self) -> bool {
        matches!(self, Self::Working)
    }

    pub fn is_not_installed(self) -> bool {
        matches!(self, Self::NotInstalled)
    }

    pub fn not_running_reason(self) -> Option<NotRunningReason> {
        match self {
            Self::InstalledNotRunning(r) => Some(r),
            _ => None,
        }
    }

    pub fn status_label(self) -> String {
        match self {
            Self::Working => "Working".to_string(),
            Self::InstalledNotRunning(r) => format!("Installed but not running ({})", r.label()),
            Self::NotInstalled => "Not installed".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverEntryStatus {
    pub id: DriverId,
    pub state: DriverState,
    pub version: Option<String>,
    pub pnp_instance_id: Option<String>,
    pub detail_note: Option<String>,
}

impl DriverEntryStatus {
    pub fn not_installed(id: DriverId) -> Self {
        Self {
            id,
            state: DriverState::NotInstalled,
            version: None,
            pnp_instance_id: None,
            detail_note: None,
        }
    }

    pub fn working(id: DriverId, version: Option<String>) -> Self {
        Self {
            id,
            state: DriverState::Working,
            version,
            pnp_instance_id: None,
            detail_note: None,
        }
    }
}

/// Evaluates one driver's exact operational state in probe order:
/// (a) functional probe (ground truth) -> (b) service state -> (c) PnP device/problem code ->
/// (d) uninstall registry version metadata only. Never reports `NotInstalled` when service/PnP
/// indicates the driver is present on the machine.
pub fn evaluate_driver_state(id: DriverId, inputs: &DriverProbeInputs) -> DriverEntryStatus {
    let pnp_instance_id = inputs.pnp.instance_id().map(ToOwned::to_owned);
    let version = inputs.uninstall_version.clone();

    if inputs.functional_working {
        return DriverEntryStatus {
            id,
            state: DriverState::Working,
            version,
            pnp_instance_id,
            detail_note: inputs.functional_detail.clone(),
        };
    }

    let any_installed_signal = inputs.service != ServiceProbeState::Absent
        || !matches!(inputs.pnp, PnpProbeState::Absent)
        || inputs.session_reboot_pending
        || inputs.functional_access_denied;

    if !any_installed_signal {
        return DriverEntryStatus {
            id,
            state: DriverState::NotInstalled,
            version,
            pnp_instance_id: None,
            detail_note: inputs.functional_detail.clone(),
        };
    }

    let reason = if inputs.session_reboot_pending
        || inputs.service_delete_pending
        || matches!(inputs.pnp, PnpProbeState::RebootPending { .. })
    {
        NotRunningReason::RebootPending
    } else if inputs.service == ServiceProbeState::Disabled {
        NotRunningReason::ServiceDisabled
    } else if let PnpProbeState::Problem { problem_code, .. } = inputs.pnp {
        NotRunningReason::DeviceError { problem_code }
    } else if inputs.functional_access_denied {
        NotRunningReason::DeviceError { problem_code: 5 }
    } else if inputs.service == ServiceProbeState::Stopped {
        NotRunningReason::ServiceStopped
    } else {
        NotRunningReason::DeviceError { problem_code: 0 }
    };

    let detail_note = inputs.functional_detail.clone().or_else(|| {
        if reason == (NotRunningReason::DeviceError { problem_code: 5 }) && id == DriverId::HidHide {
            Some("HidHide control device locked by another client (close HidHideClient).".to_string())
        } else {
            None
        }
    });

    DriverEntryStatus {
        id,
        state: DriverState::InstalledNotRunning(reason),
        version,
        pnp_instance_id,
        detail_note,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureCheck {
    ValidNefarius { signer: String },
    WrongPublisher { signer: String },
    UnsignedOrInvalid { reason: String },
}

pub trait SignatureVerifier {
    fn verify_nefarius(&self, path: &Path) -> SignatureCheck;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateOutcome {
    AcceptedWorking {
        folder: String,
        matched_binary: String,
        signer: String,
    },
    AcceptedFoundButNotRunning {
        folder: String,
        matched_binary: String,
        state: DriverState,
    },
    NoMatchingBinary {
        folder: String,
    },
    WrongSignature {
        folder: String,
        matched_binary: String,
        reason: String,
    },
}

impl LocateOutcome {
    pub fn is_accepted(&self) -> bool {
        matches!(
            self,
            Self::AcceptedWorking { .. } | Self::AcceptedFoundButNotRunning { .. }
        )
    }

    pub fn summary_message(&self) -> String {
        match self {
            Self::AcceptedWorking {
                matched_binary,
                signer,
                ..
            } => format!(
                "Verified {matched_binary} (signed by {signer}); driver probe confirmed Working."
            ),
            Self::AcceptedFoundButNotRunning {
                matched_binary,
                state,
                ..
            } => format!(
                "Found valid binary ({matched_binary}), but not running: {}.",
                state.status_label()
            ),
            Self::NoMatchingBinary { .. } => {
                "No matching binary found in the selected folder.".to_string()
            }
            Self::WrongSignature {
                matched_binary,
                reason,
                ..
            } => format!("Wrong signature on {matched_binary}: {reason}."),
        }
    }
}

fn collect_matching_binaries(folder: &Path, expected_names: &[&str]) -> Vec<PathBuf> {
    let mut matched = Vec::new();
    let expected: BTreeSet<&str> = expected_names.iter().copied().collect();
    let mut stack: Vec<(PathBuf, usize)> = vec![(folder.to_path_buf(), 0)];
    let mut visited = 0usize;

    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > 256 {
                break;
            }
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                if depth < 2 {
                    stack.push((path, depth + 1));
                }
            } else if meta.is_file() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    let lower = name.to_ascii_lowercase();
                    if expected.contains(lower.as_str()) {
                        matched.push(path);
                    }
                }
            }
        }
        if visited > 256 {
            break;
        }
    }
    matched.sort();
    matched
}

/// Inspects a user-selected folder for expected binaries of `driver`, verifies their digital
/// signature against Nefarius without ever executing them, and re-runs functional detection when valid.
pub fn inspect_driver_folder_with(
    driver: DriverId,
    folder: &Path,
    verifier: &dyn SignatureVerifier,
    reprobe_after_valid: impl FnOnce() -> DriverState,
) -> LocateOutcome {
    let folder_str = folder.display().to_string();
    let matches = collect_matching_binaries(folder, driver.expected_locate_filenames());
    if matches.is_empty() {
        return LocateOutcome::NoMatchingBinary { folder: folder_str };
    }

    let mut first_failure: Option<(String, String)> = None;
    for path in matches {
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        match verifier.verify_nefarius(&path) {
            SignatureCheck::ValidNefarius { signer } => {
                let state = reprobe_after_valid();
                if state.is_working() {
                    return LocateOutcome::AcceptedWorking {
                        folder: folder_str,
                        matched_binary: fname,
                        signer,
                    };
                }
                return LocateOutcome::AcceptedFoundButNotRunning {
                    folder: folder_str,
                    matched_binary: fname,
                    state,
                };
            }
            SignatureCheck::WrongPublisher { signer } => {
                if first_failure.is_none() {
                    first_failure = Some((fname, format!("publisher \"{signer}\" is not nefarius")));
                }
            }
            SignatureCheck::UnsignedOrInvalid { reason } => {
                if first_failure.is_none() {
                    first_failure = Some((fname, reason));
                }
            }
        }
    }

    let (matched_binary, reason) =
        first_failure.unwrap_or_else(|| ("".to_string(), "verification failed".to_string()));
    LocateOutcome::WrongSignature {
        folder: folder_str,
        matched_binary,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_reports_working_when_functional_probe_succeeds() {
        let out = evaluate_driver_state(
            DriverId::ViGEmBus,
            &DriverProbeInputs {
                functional_working: true,
                service: ServiceProbeState::Running,
                pnp: PnpProbeState::Ok {
                    instance_id: "ROOT\\SYSTEM\\0003".to_string(),
                },
                uninstall_version: Some("1.17.333.0".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(out.state, DriverState::Working);
        assert_eq!(out.version.as_deref(), Some("1.17.333.0"));
        assert_eq!(out.state.status_label(), "Working");
    }

    #[test]
    fn state_machine_reports_stopped_never_not_installed() {
        let out = evaluate_driver_state(
            DriverId::ViGEmBus,
            &DriverProbeInputs {
                functional_working: false,
                service: ServiceProbeState::Stopped,
                pnp: PnpProbeState::Absent,
                ..Default::default()
            },
        );
        assert_eq!(
            out.state,
            DriverState::InstalledNotRunning(NotRunningReason::ServiceStopped)
        );
        assert!(!out.state.is_not_installed());
    }

    #[test]
    fn state_machine_reports_disabled_never_not_installed() {
        let out = evaluate_driver_state(
            DriverId::HidHide,
            &DriverProbeInputs {
                functional_working: false,
                service: ServiceProbeState::Disabled,
                pnp: PnpProbeState::Ok {
                    instance_id: "ROOT\\SYSTEM\\0004".to_string(),
                },
                ..Default::default()
            },
        );
        assert_eq!(
            out.state,
            DriverState::InstalledNotRunning(NotRunningReason::ServiceDisabled)
        );
    }

    #[test]
    fn state_machine_reports_pnp_device_error_with_problem_code() {
        let out = evaluate_driver_state(
            DriverId::ViGEmBus,
            &DriverProbeInputs {
                functional_working: false,
                service: ServiceProbeState::Stopped,
                pnp: PnpProbeState::Problem {
                    instance_id: "ROOT\\SYSTEM\\0003".to_string(),
                    problem_code: 22,
                },
                ..Default::default()
            },
        );
        assert_eq!(
            out.state,
            DriverState::InstalledNotRunning(NotRunningReason::DeviceError { problem_code: 22 })
        );
        assert!(out.state.status_label().contains("problem code 22"));
    }

    #[test]
    fn state_machine_reports_reboot_pending_and_absent() {
        let reboot = evaluate_driver_state(
            DriverId::HidHide,
            &DriverProbeInputs {
                functional_working: false,
                service: ServiceProbeState::Stopped,
                session_reboot_pending: true,
                ..Default::default()
            },
        );
        assert_eq!(
            reboot.state,
            DriverState::InstalledNotRunning(NotRunningReason::RebootPending)
        );

        let absent = evaluate_driver_state(
            DriverId::ViGEmBus,
            &DriverProbeInputs {
                functional_working: false,
                service: ServiceProbeState::Absent,
                pnp: PnpProbeState::Absent,
                uninstall_version: Some("1.22.0".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(absent.state, DriverState::NotInstalled);
    }

    struct MockVerifier<'a> {
        check: fn(&Path) -> SignatureCheck,
        _marker: std::marker::PhantomData<&'a ()>,
    }

    impl<'a> SignatureVerifier for MockVerifier<'a> {
        fn verify_nefarius(&self, path: &Path) -> SignatureCheck {
            (self.check)(path)
        }
    }

    #[test]
    fn manual_locate_rejects_missing_and_wrong_signature_and_accepts_nefarius_with_reprobe() {
        let base = std::env::temp_dir().join(format!("z_ctrl_locate_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        let empty_dir = base.join("empty");
        std::fs::create_dir_all(&empty_dir).unwrap();
        let bad_sig = base.join("bad_sig");
        std::fs::create_dir_all(&bad_sig).unwrap();
        std::fs::write(bad_sig.join("HidHideCLI.exe"), b"not really signed").unwrap();

        let wrong_v = MockVerifier {
            check: |_| SignatureCheck::WrongPublisher {
                signer: "Untrusted Corp".to_string(),
            },
            _marker: Default::default(),
        };

        assert!(matches!(
            inspect_driver_folder_with(DriverId::HidHide, &empty_dir, &wrong_v, || {
                DriverState::NotInstalled
            }),
            LocateOutcome::NoMatchingBinary { .. }
        ));

        let wrong_out = inspect_driver_folder_with(DriverId::HidHide, &bad_sig, &wrong_v, || {
            DriverState::Working
        });
        assert!(matches!(wrong_out, LocateOutcome::WrongSignature { .. }));
        assert!(!wrong_out.is_accepted());

        let valid_v = MockVerifier {
            check: |_| SignatureCheck::ValidNefarius {
                signer: "Nefarius Software Solutions e.U.".to_string(),
            },
            _marker: Default::default(),
        };

        let found_stopped =
            inspect_driver_folder_with(DriverId::HidHide, &bad_sig, &valid_v, || {
                DriverState::InstalledNotRunning(NotRunningReason::ServiceStopped)
            });
        assert!(matches!(
            found_stopped,
            LocateOutcome::AcceptedFoundButNotRunning { .. }
        ));

        let found_working =
            inspect_driver_folder_with(DriverId::HidHide, &bad_sig, &valid_v, || {
                DriverState::Working
            });
        assert!(matches!(
            found_working,
            LocateOutcome::AcceptedWorking { .. }
        ));

        let _ = std::fs::remove_dir_all(&base);
    }
}
