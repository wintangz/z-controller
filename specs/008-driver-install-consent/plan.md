# Implementation Plan: `driver-install-consent`

**Slug**: `driver-install-consent` | **Date**: 2026-11-26 | **Spec**: [spec.md](spec.md)

## Summary

Replace silent combined driver installation with an explicit, per-driver consent and diagnostics architecture spanning `z-core`, `z-platform-windows`, and `z-controller`. Keep `z-core` OS-agnostic with a pure 4-layer driver evaluation state machine (`evaluate_driver_state`) and folder-locate signature trait (`SignatureVerifier` + `inspect_driver_folder_with`). Implement Windows SCM/PnP/Uninstall/Authenticode (`WinVerifyTrust` + `CryptQueryObject`) inspection, single-driver zero-flag `ShellExecuteExW("runas", ..., lpParameters = null())` wizard launcher, targeted UAC remedies, and `AppLog` persistence in `z-platform-windows`. Render independent `ViGEmBus` (`REQUIRED`) and `HidHide` (`OPTIONAL`) driver cards, confirmation modals, manual locate controls, and graceful missing-driver behaviour in `apps/z-controller` using the existing `4/8/12/16` software-painter layout and `<= 25 MB` tray memory budget.

## Project Changes

- `crates/z-core/src/driver.rs`: `DriverId`, `ServiceProbeState`, `PnpProbeState`, `DriverProbeInputs`, `NotRunningReason`, `DriverState`, `DriverEntryStatus`, `evaluate_driver_state`, `SignatureCheck`, `SignatureVerifier`, `LocateOutcome`, `inspect_driver_folder_with`, and comprehensive unit tests.
- `crates/z-core/src/backend.rs` & `crates/z-core/src/profile.rs`: Update `DriverStatus` to hold `vigem: DriverEntryStatus` + `hidhide: DriverEntryStatus`; persist `vigem_located_dir` / `hidhide_located_dir` in `AppConfig`.
- `crates/z-platform-windows/src/drivers.rs`: Real functional + SCM service + PnP devnode + Uninstall registry probing; single-driver zero-flag `InstallerLaunchSpec` + `run_driver_installer_wizard`; UAC remedies (`StartService`, `EnableAndStartService`, `ReenableDevice`, `RestartMachine`); `Win32SignatureVerifier`; thread-safe `AppLog`.
- `apps/z-controller/src/{engine.rs,ui.rs,tray_win32.rs,main.rs}`: Per-driver install/cancel/remedy/locate commands, inline missing-driver guards on virtual-pad toggle & cloak checkbox, in-app consent modal, `SHBrowseForFolderW` picker, `--dry-run-install` + `--preview-confirm-driver` CLI hooks.
- `README.md`: Update first-run driver setup instructions to document explicit per-driver confirmation and interactive wizard installs.
