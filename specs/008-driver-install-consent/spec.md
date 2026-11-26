# Feature Specification: `driver-install-consent`

**Slug**: `driver-install-consent`
**Created**: 2026-11-26
**Status**: Approved (Autonomous Spec Kit Run)

## Problem Statement

Previously, `z-controller` presented a single combined driver installer action that silently executed both `ViGEmBus` and `HidHide` installers with `/quiet /norestart`, without per-driver explanations, explicit confirmation, or nuanced installed-vs-running diagnostics. Because kernel drivers affect OS-level input stacks, every driver installation must be an explicit, informed, per-driver, user-driven wizard workflow backed by functional detection and non-destructive remedies.

## Functional Requirements

- **FR-001 (No Auto-Install, Ever)**: The application MUST NEVER automatically install a driver on first run, when toggling a controller's virtual-pad switch, or on any other automatic trigger. An installation may begin only when the user clicks that specific driver's `Install` (or `Reinstall`) button AND explicitly confirms in the confirmation dialog.
- **FR-002 (Per-Driver Presentation)**: `ViGEmBus` and `HidHide` MUST appear as two separate driver entries in the UI. Each entry displays:
  - Driver name (`ViGEmBus` / `HidHide`)
  - Publisher (`nefarius`)
  - One plain-language purpose line:
    - `ViGEmBus`: `REQUIRED — creates virtual Xbox 360 / DualShock 4 pads.`
    - `HidHide`: `OPTIONAL — only needed to hide (cloak) the physical pad from games.`
  - Current detected state from FR-005
  - Its own `Install` / remedy / `I already have this — Locate…` actions
  - ZERO combined "Install Drivers" actions anywhere in the UI.
- **FR-003 (Confirmation Before Launch)**: Clicking a driver's `Install` / `Reinstall` button first opens an in-app modal dialog naming that single driver, its publisher (`nefarius`), what it will install/change on the machine, and explicitly stating that the official signed installer wizard will now open and Windows may show a UAC prompt. Only explicit confirmation launches the installer; cancelling launches nothing and logs only the cancellation.
- **FR-004 (No Silent Install)**: On confirmation, extract ONLY that driver's bundled official signed installer and launch it elevated (`runas`, `SW_SHOWNORMAL`) with **no silent/quiet/no-wizard flags** (`parameters = ""`), so the installer's own wizard is fully visible for the user to step through. Wait for exit, re-run detection for that driver, surface the outcome (`Installed and working` / `Still not installed` / `Installer failed (exit code N)` / `Reboot pending`), and record every attempt and outcome in the app log.
- **FR-005 (Detection by Function, Not File Path)**: Detect each driver in order:
  1. Functional probe (ground truth): `ViGEmBus` is `Working` only if `VigemClient::connect()` succeeds; `HidHide` is `Working` only if `hidhide::query_state()` responds.
  2. Service state (`Running`, `Stopped`, `Disabled`, `Absent`) from SCM + `HKLM\SYSTEM\CurrentControlSet\Services\<svc>`.
  3. PnP device status & problem code (`CM_Get_DevNode_Status`, `DN_NEED_RESTART`, `DN_HAS_PROBLEM`, problem code).
  4. Uninstall registry (`DisplayVersion`) as secondary metadata only.
  Report exactly one state per driver:
  - `Working`
  - `InstalledNotRunning(ServiceStopped | ServiceDisabled | DeviceError(code) | RebootPending)`
  - `NotInstalled`
  A driver that is installed but not running MUST NEVER be reported as `NotInstalled`.
- **FR-006 (Remedies for Installed-But-Not-Running)**: Surface the sub-reason plainly and offer the fitting primary remedy (explicit user-clicked action via UAC: start service, enable & start service, re-enable PnP device, or confirm PC restart) before offering `Reinstall (Later Option)…`.
- **FR-007 (Manual Locate Fallback)**: Each driver entry offers `"I already have this — Locate…"` opening a folder picker (`SHBrowseForFolderW`). Inspect the picked folder (max depth 2) without trusting it blindly or executing any binary from it: locate expected binaries, verify Authenticode trust + Nefarius publisher (`WinVerifyTrust` + `CryptQueryObject`), record accepted folder paths per driver in `%APPDATA%\z-controller\config.toml` (with `Change…` / `Clear`), re-run functional detection from FR-005, and report exact failure reasons (`No matching binary found` / `Wrong signature` / `Found, but not running`).
- **FR-008 (Missing-Driver Behaviour)**: If `ViGEmBus` is not `Working` and the user clicks a controller's `Active` toggle ON, explain inline that virtual pads need `ViGEmBus` and offer that driver's install/remedy flow without toggling ON or auto-installing. If `HidHide` is not `Working`, disable only the cloak UI with its reason and `HidHide` install/remedy link while virtual pads and controller mapping continue functioning normally.

## Logged Assumptions

- **A1 (Window Open Policy)**: First-run tray startup never triggers any driver install. Opening the main window on startup happens only on `--show` / `--open` or when required `ViGEmBus` is not `Working` so users can inspect its status directly; optional `HidHide` absence never forces the window open.
- **A2 (WHQL Driver Signature Check in Manual Locate)**: Nefarius user-mode binaries (`HidHideCLI.exe`, `HidHideClient.exe`, `ViGEmBus_Setup.exe`, `HidHide_Setup.exe`, `devcon.exe`) carry an Authenticode leaf signer certificate with subject `CN=Nefarius Software Solutions e.U.`, while installed WHQL kernel `.sys` drivers (`ViGEmBus.sys`, `HidHide.sys`) carry `CN=Microsoft Windows Hardware Compatibility Publisher` on a PE whose version `CompanyName` is `Nefarius Software Solutions e.U.`. `Win32SignatureVerifier` requires `WinVerifyTrust == 0` AND (`signer` contains `"nefarius"`, OR for `.sys` drivers: `signer` contains `"Microsoft Windows Hardware Compatibility Publisher"` AND PE `CompanyName` contains `"nefarius"`).
- **A3 (App Log Path)**: Driver install, cancel, remedy, and locate events are appended with timestamps to `%APPDATA%\z-controller\z-controller.log` (overridable via `--log-path` for dry-run verification).
- **A4 (Modal Overlays Without OS Popups)**: Consistent with `tray-ui` rules (`MessageBoxW` and `NIF_INFO` forbidden), install confirmation and restart confirmation dialogs render as in-app modal overlays.
