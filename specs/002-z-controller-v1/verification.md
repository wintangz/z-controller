# Verification Report: `z-controller` V1

**Date**: 2026-11-25  
**Toolchain**: `rustc 1.100.0` (`stable-x86_64-pc-windows-gnu`, `windows-sys = "=0.59.0"`, zero external SDK/MSVC dependencies)  
**Release Artifact**: `target/release/z-controller.exe` (`21,086,208` bytes single portable binary)

---

## Milestone Results Summary

| Milestone / Feature Slug | Result | Proving Number / Evidence |
|---|---|---|
| `feasibility-spike` — Item 1: SDL3 reads Aolion Pro Controller + battery | **PASS** | `SDL3 3.4.20` opened `057E:2009` (`NintendoSwitchPro`, `Wireless`), reported `sdl_power=(OnBattery, Some(100)%)` (`specs/001-feasibility-spike/evidence.md`). |
| `feasibility-spike` — Item 2: ViGEm virtual X360 visible & game-readable | **PASS** | Virtual X360 `serial=1` (`VID=045E PID=028E`) appeared in `[WinMM] slot=1`, `[DirectInput8] 'Controller (XBOX 360 For Windows)'`, and `[XInput] slot=0 wButtons=0x1200 sThumbLY=24000`. |
| `feasibility-spike` — Item 3: HidHide cloaks physical pad from games while app sees it | **PASS** | Un-whitelisted `probe_game_view.exe` dropped `WinMM` `2 -> 1`, `DirectInput8` `2 -> 1` (`'Wireless Gamepad'` hidden), and `057E:2009` HID returned `AccessDenied`; whitelisted `spike_runner.exe` kept `100%` battery + `raw_hid_open_ok=true`. |
| Workspace unit suite (`core-input-battery`, `profile-mapping`, `virtual-pad-output`, `hidhide-cloak`, `driver-bootstrap`) | **PASS** | `cargo test --workspace`: `13 passed; 0 failed` (`9` in `z-core`, `4` in `z-platform-windows`), `0` compiler warnings. |
| Release single-binary isolation check (`tray-ui`, `driver-bootstrap`) | **PASS** | Copied `target/release/z-controller.exe` (`21,086,208` bytes) alone into `%TEMP%\z-ctrl-single-bin-test` (`1` file, `0` adjacent DLLs); `--smoke-verify` exited `0`, reporting `driver_vigem_installed=true`, `driver_hidhide_installed=true`, `driver_hidhide_accessible=true`. |
| Tray idle RAM & CPU (`tray-ui`) | **PASS** | Production tray launch settled at `7.3s`: `WorkingSet64 = 15.34 MB` (`<= 25 MB`), `PrivateWorkingSet = 2.18 MB`, `CommitPrivate = 3.18 MB`, `IdleCPU = 31.25 ms` over `5.5s` (`~0%` CPU idle via `SDL_WaitEvent`). |
| UI Open / Hide to Tray / Quit lifecycle (`tray-ui`) | **PASS** | `CMD_OPEN` rendered window (`Visible=True`, `WorkingSet64 = 35.95 MB`, `PrivateWorkingSet = 8.41 MB`); `WM_CLOSE` hid back to tray and released `ui_session` (`Visible=False`, `WorkingSet64 = 2.61 MB`, `PrivateWorkingSet = 0.70 MB`); `CMD_QUIT` exited `0` (`Exited=True`). Zero `NIF_INFO` or `MessageBoxW` calls in codebase. |
| Multi-controller + Anti-loop + Non-inverted sticks (`profile-mapping`, `virtual-pad-output`) | **PASS** | `--smoke-dual-pad`: `controllers.len() == 2` (`virtual_serial = 1, 2`), `0` XInput self-capture loops. Live `probe_game_view.exe` read `[WinMM] active_slots=2`, `[DirectInput8] count=2`, `[XInput] connected_slots=2`: Slot `0` `wButtons=0x1000 sThumbLY=32767 sThumbRY=-32767` (SDL Up `-32768` -> XInput `+32767`); Slot `1` `wButtons=0x2000 sThumbLX=-14564`. |
| Post-exit cleanup & uncloak (`hidhide-cloak`) | **PASS** | Post-exit `probe_game_view.exe` verified `[WinMM] active_slots=0`, `[DirectInput8] count=0`, `[XInput] connected_slots=0`. |

---

## Transparent Hardware & Scope Disclosures

- **FIFA 23 Binary Absence (`A-001`)**: `FIFA23.exe` is not installed on `C:`, `D:`, `Y:`, or `Z:` on the dev machine. Double-input elimination (`AccessDenied` on physical HID + removal of DirectInput8 `'Wireless Gamepad'`) and XInput stick orientation (`-32768 -> +32767`) were proven using un-whitelisted Win32 `XInput1_4`, `DirectInput8`, `WinMM`, and raw `HID` queries (`probe_game_view.exe`) plus `joy.cpl` UI automation.
- **Steam Battery Parity (`A-004`)**: `Z:\games\steam\logs\controller.txt` confirms Steam opens `057E:2009` via Valve's HIDAPI Nintendo Switch driver, which computes Switch Pro battery via `round((level / 8.0) * 100.0)` matching `SDL_hidapi_switch` (`100%` observed) and `crates/z-core/src/battery.rs`.
- **Pro Controller Sleep Between Spike and Final Release Run (`A-005`)**: The Bluetooth Aolion Pro Controller (`057E:2009`) proved live SDL3 + `100%` battery + `HidHide` cloaking at `15:36` (`feasibility-spike`), then entered Bluetooth idle sleep (`Present = False`) prior to `16:09`. The final V1 release run (`16:09`) proved dual-pad `ViGEmBus` output, XInput anti-loop, stick non-inversion, standalone packaging, and tray RAM directly on the release executable via two simultaneous `SdlVirtualGamepad` feeds.
