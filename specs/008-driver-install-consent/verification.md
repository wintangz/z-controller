# Verification Report: `driver-install-consent`

**Date**: 2026-11-26
**Slug**: `driver-install-consent`

## Testing Limits Respected

- Zero games launched or tested.
- Zero drivers, services, SDKs, crates, or tools installed, uninstalled, stopped, disabled, or restarted on the dev machine.
- Bundled installers were never executed on the dev machine; installer launch command lines were verified via unit tests and `--dry-run-install` logging.

## Acceptance Checks (Requirements 1–8)

1. **No Auto-Install**: `install_missing_drivers()` removed; startup, controller plug-in, and toggling `Active` never spawn any installer (`missing_vigem_toggle_explains_and_offers_install_without_auto_installing`).
2. **Per-Driver Presentation**: `Drivers` section renders independent `ViGEmBus` (`REQUIRED`) and `HidHide` (`OPTIONAL`) cards with `by nefarius`, plain-language purpose lines, functional state, and per-driver actions (`driver_cards_render_separately_with_no_combined_install_button`).
3. **Confirmation Before Launch**: Clicking `Install…` / `Reinstall…` opens an in-app confirmation modal naming the driver, publisher (`nefarius`), kernel driver impact, and wizard/UAC notice; cancelling emits `EngineCommand::CancelInstallDriver` (`AppLog` cancellation only) and launches nothing (`install_confirmation_dialog_names_driver_publisher_uac_and_cancel_logs_without_installing`).
4. **No Silent Install (`dry-run-install.log`)**:
   - `ViGEmBus`: `verb=runas parameters="" contains_silent_flags=false show=SW_SHOWNORMAL`
   - `HidHide`: `verb=runas parameters="" contains_silent_flags=false show=SW_SHOWNORMAL`
   - Unit test `installer_launch_spec_has_no_silent_or_quiet_flags` passed.
5. **Detection by Function, Not File Location**:
   - Live dev-machine probe (`smoke-verify.json`):
     - `ViGEmBus`: `Working` (`v1.17.333.0`)
     - `HidHide`: `Working` (`v1.5.230`)
   - Simulated state-machine tests passed: `state_machine_reports_working_when_functional_probe_succeeds`, `state_machine_reports_stopped_never_not_installed`, `state_machine_reports_disabled_never_not_installed`, `state_machine_reports_pnp_device_error_with_problem_code`, `state_machine_reports_reboot_pending_and_absent`.
6. **Remedies Before Reinstall**: `remedy_buttons_match_stopped_disabled_device_error_and_reboot_pending` passed; preview screenshot `screenshot-drivers-remedies-preview.png` confirms `Start Service` is primary ahead of `Reinstall (Later Option)…`.
7. **Manual Locate Fallback**: Folder picker (`SHBrowseForFolderW`) + `Win32SignatureVerifier` (`WinVerifyTrust` + `CryptQueryObject` + PE `CompanyName` check for WHQL `.sys`) persists accepted folders to `%APPDATA%\z-controller\config.toml` and re-runs functional probes without executing any binary (`manual_locate_rejects_missing_and_wrong_signature_and_accepts_nefarius_with_reprobe`, `win32_signature_verifier_accepts_nefarius_and_rejects_wrong_publisher_without_executing`).
8. **Missing-Driver Behaviour**: Turning `Active` ON without working `ViGEmBus` keeps the virtual pad OFF, shows inline explanation + install/remedy offer; missing `HidHide` disables only cloaking (`Cloak off (no HidHide)` and disabled `Cloak` checkbox in Profile Editor) while virtual pads & mapping stay active.

## Build, Tests & Memory

- **`cargo test --workspace`**: `41` passed, `0` failed (`17` in `z-controller`, `18` in `z-core`, `6` in `z-platform-windows`), `0` warnings.
- **Release binary**: `21,621,248` bytes (`target\release\z-controller.exe` and `Y:\Apps\z-controller\z-controller.exe`).
- **Tray idle WorkingSet**: `679,936` bytes (`0.65 MB`), Private Bytes `4,034,560` bytes (`3.85 MB`), within the `<= 25 MB` budget.

## Screenshots

- `screenshot-drivers-working.png`
- `screenshot-confirm-dialog.png`
- `screenshot-drivers-remedies-preview.png`
