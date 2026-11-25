# Feature Specification: `feasibility-spike`

**Feature Branch**: `001-feasibility-spike`  
**Feature Slug**: `feasibility-spike`  
**Created**: 2026-11-25  
**Status**: In Progress

## Purpose & Scenarios

Before constructing the `tray-ui` application shell, `feasibility-spike` proves on the live dev machine that the chosen Rust toolchain and core Windows driver interfaces satisfy all three foundational hardware prerequisites.

### User Story 1 - Physical Controller & Battery Read (`feasibility-spike`) (Priority: P1)

As the hardware integrator, I want SDL3 3.4.20 + `gamecontrollerdb.txt` to open the Bluetooth Aolion Pro Controller (`057E:2009`) and read live inputs and battery level so `core-input-battery` has a verified input foundation.

**Independent Test**: Run `spike_runner` with the Aolion Pro Controller connected over Bluetooth and verify SDL3 opens `057E:2009`, records battery state/percent matching Switch `0x30` byte-2 telemetry, and detects all currently connected physical controllers.

**Acceptance Scenarios**:
1. **Given** the Bluetooth Aolion Pro Controller (`057E:2009`) is connected, **When** `spike_runner` initializes SDL3 (`SDL_JOYSTICK_HIDAPI_SWITCH=1`) and loads `assets/gamecontrollerdb.txt`, **Then** SDL3 opens the gamepad and reports non-error battery percentage/state plus live axis/button capability.

---

### User Story 2 - Virtual Xbox 360 Injection & Game Visibility (`feasibility-spike`) (Priority: P1)

As the hardware integrator, I want `vigem-rust` over `ViGEmBus` to spawn a virtual Xbox 360 pad visible in `joy.cpl` and readable by an un-whitelisted Win32 game probe (`probe_game_view`), proving `virtual-pad-output`.

**Independent Test**: Spawn a virtual Xbox 360 pad in `spike_runner`, inject a known synthetic state (`thumb_ly = +24000`, `A = pressed`), and confirm `joy.cpl` lists `Controller (XBOX 360 For Windows)` while `probe_game_view` reads the exact injected XInput + WinMM state.

---

### User Story 3 - HidHide Self-Whitelist & Physical Pad Cloaking (`feasibility-spike`) (Priority: P1)

As the hardware integrator, I want native Rust `DeviceIoControl` IOCTLs to `\\.\HidHide` to whitelist `spike_runner` and cloak the physical Aolion Pro Controller so `joy.cpl` and `probe_game_view` see zero physical `057E:2009` device while `spike_runner` continues reading live SDL3 input and battery, proving `hidhide-cloak`.

**Independent Test**: With HidHide cloak active on `HID\{00001124-0000-1000-8000-00805F9B34FB}_VID&0002057E_PID&2009\9&1A2DBC92&1&0000`, verify `joy.cpl` and `probe_game_view` no longer enumerate the physical pad and raw HID open returns `ERROR_ACCESS_DENIED`, while `spike_runner` reads SDL3 successfully, then restore uncloaked state cleanly.

## Requirements

- **FR-001**: Verify toolchain linking of `vigem-rust`, `windows-sys`, and `SDL3` on `stable-x86_64-pc-windows-gnu`, or switch to `stable-x86_64-pc-windows-msvc` with minimal Visual Studio Build Tools and log the switch if GNU fails to link ViGEm.
- **FR-002**: Prove SDL3 3.4.20 reads the Bluetooth Aolion Pro Controller (`057E:2009`) including battery level.
- **FR-003**: Prove `vigem-rust` spawns a virtual Xbox 360 pad appearing in `joy.cpl` and readable via XInput / WinMM / DirectInput8 from an un-whitelisted process (`probe_game_view`).
- **FR-004**: Prove native Win32 IOCTLs against `\\.\HidHide` whitelist `spike_runner.exe` (via NT `\Device\HarddiskVolume...` path), cloak the physical pad from `joy.cpl` and `probe_game_view`, allow `spike_runner` to continue reading SDL3, and restore uncloaked state cleanly on exit.

## Success Criteria

- **SC-001**: All 3 spike checks report **PASS** with concrete captured outputs (controller IDs, battery %, `joy.cpl` UI list items, `XInputGetState` values, DirectInput8 counts before/during/after cloak, and `CreateFileW` error codes).

## Assumptions & Environment Log

- **A-001 (HidHide Exclusive Handle Lock)**: HidHide's `ControlDevice.c` sets `WdfDeviceInitSetExclusive(TRUE)`, so only one process may hold `\\.\HidHide` open at a time. At initial inspection `HidHideClient.exe` (PID 2876) was running and held the exclusive handle (`HidHideCLI.exe` failed with `0x0005 Access is denied`). Spike and integration runs close `HidHideClient.exe` first and never hold `\\.\HidHide` open across idle waits.
- **A-002 (Toolchain & ViGEmBus Linking)**: Initial testing of `vigem-rust 0.2.1` on `stable-x86_64-pc-windows-gnu` failed because `vigem-rust` pulls `windows 0.62` (`windows-link` `raw-dylib`), which spawns `dlltool.exe` -> `as.exe` (absent from rustup's `self-contained` bin directory). Meanwhile `windows-sys = "=0.59.0"` uses precompiled `windows_x86_64_gnu` `.a` import libraries and compiles cleanly in `< 1s` on `stable-x86_64-pc-windows-gnu`. After the user cancelled a slow MSVC Build Tools download (`"stop installing, I will do it manually"`), `z-platform-windows` implements the MIT-documented `ViGEmBus` kernel IOCTL contract directly over `windows-sys = "=0.59.0"` so the entire project builds natively on `stable-x86_64-pc-windows-gnu` with zero external SDK or Build Tools dependencies.
- **A-003 (SDL3 Single-Binary Strategy)**: Official `SDL3-devel-3.4.20-mingw.zip` provides `libSDL3.dll.a` import library plus `SDL3.dll` (2.85 MB) rather than a standalone static `libSDL3.a`. Embedding `SDL3.dll` in `.rdata` via `include_bytes!` and extracting once to `%LOCALAPPDATA%\z-controller\runtime\SDL3-3.4.20.dll` + loading dynamically via `LoadLibraryExW` preserves the strict single-binary distribution requirement without needing CMake/C compilers just to compile SDL3 from source.
- **A-004 (Connected Hardware at Spike Time)**: Two physical controllers are present on the dev machine: `HID\{00001124-0000-1000-8000-00805F9B34FB}_VID&0002057E_PID&2009\9&1A2DBC92&1&0000` (Bluetooth Nintendo Switch Pro / Aolion Pro Controller) and `HID\VID_045E&PID_028E&IG_00\3&8968588&0&0000` (Xbox 360 / XInput HID pad).
