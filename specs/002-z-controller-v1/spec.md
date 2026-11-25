# Feature Specification: `z-controller` V1

**Feature Branch**: `002-z-controller-v1`  
**Included Feature Slugs**: `feasibility-spike`, `core-input-battery`, `profile-mapping`, `virtual-pad-output`, `hidhide-cloak`, `driver-bootstrap`, `tray-ui`  
**Created**: 2026-11-25  
**Status**: Approved (Autonomous Spec Kit Run)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Seamless Physical-to-Virtual Controller Bridge & Cloaking (`core-input-battery`, `virtual-pad-output`, `hidhide-cloak`) (Priority: P1)

As a PC gamer using an Aolion Pro Controller over Bluetooth (spoofing Nintendo Switch Pro `057E:2009`) or USB (`Wireless Gamepad`), I want `z-controller` to read my controller via SDL3, translate it into a virtual Xbox 360 or DualShock 4 pad via ViGEmBus, forward game rumble back to my controller, and cloak the physical pad via HidHide so games like FIFA 23 see only the virtual controller with zero double input and correct stick Y-axis orientation.

**Independent Test**: Connect the Bluetooth Aolion Pro Controller (`057E:2009`), assign the default Xbox 360 profile with cloaking enabled, and verify un-whitelisted Win32 game APIs (`joy.cpl` / DirectInput8 / XInput) see only `Controller (XBOX 360 For Windows)` with `0` uncloaked `057E:2009` devices, while physical Up maps to positive XInput `sThumbLY` (`+32767`).

**Acceptance Scenarios**:
1. **Given** the Bluetooth Aolion Pro Controller (`057E:2009`) is connected and `z-controller` runs with an active `Xbox360` + cloak profile, **When** an un-whitelisted game queries WinMM/DirectInput8/XInput/HID, **Then** the physical `057E:2009` HID interface is denied (`ERROR_ACCESS_DENIED`), DirectInput8 omits `'Wireless Gamepad'` / `'Pro Controller'`, and XInput receives normalized inputs with Up = `+sThumbLY`.
2. **Given** a game sends rumble to the virtual Xbox 360 or DualShock 4 target, **When** `ViGEmBus` delivers a feedback notification, **Then** `z-controller` forwards `(large_motor, small_motor)` to the paired physical controller via `SDL_RumbleGamepad`.
3. **Given** `z-controller` exits normally, receives Ctrl+C, closes on session shutdown, or encounters a panic, **When** teardown runs, **Then** every device instance cloaked by `z-controller` is removed from HidHide's blacklist and HidHide active state is reconciled cleanly.

---

### User Story 2 - Multi-Controller Support, Profiles & Level-2 Mapping (`profile-mapping`, `core-input-battery`) (Priority: P1)

As a local multiplayer user, I want each connected physical controller to get its own virtual pad and manually assigned profile supporting Xbox 360 or DualShock 4 output, `swap-ab-xy`, per-button remapping, radial stick deadzone, and configurable disconnect behavior (`KeepNeutral` default vs `DropVirtual`), persisted across launches.

**Independent Test**: Connect two physical controllers simultaneously (`057E:2009` and a second pad), verify `z-controller` spawns two independent virtual targets (`serial=1`, `serial=2`) without capturing its own virtual pads, and verify unit/integration tests cover `swap-ab-xy`, per-button remap, radial deadzone scaling, and disconnect state transitions.

**Acceptance Scenarios**:
1. **Given** two physical pads are connected at once, **When** `z-controller` enumerates gamepads, **Then** both pads appear as distinct physical controllers with independent virtual targets and profiles, and PnP ancestor inspection prevents `z-controller` from opening its own ViGEmBus virtual pads.
2. **Given** a profile sets `disconnect_policy = KeepNeutral`, **When** the physical pad disconnects, **Then** the virtual pad remains plugged in at neutral (`0` sticks/triggers/buttons for X360; `128` sticks and `0x08` D-pad for DS4) and resumes transparently on reconnect.
3. **Given** a profile sets `disconnect_policy = DropVirtual`, **When** the physical pad disconnects, **Then** the virtual pad is unplugged from `ViGEmBus` immediately.

---

### User Story 3 - View-Only Battery Telemetry & Lightweight Tray UI (`core-input-battery`, `tray-ui`, `driver-bootstrap`) (Priority: P2)

As a desktop user, I want `z-controller` to live quietly in the Windows system tray with zero popups/notifications and idle RAM `<= 25 MB`, detect missing ViGEmBus/HidHide drivers on first run with bundled installers, and open a full window showing controller list + view-only battery matching Steam's reading, profile editor, and live input test view.

**Independent Test**: Run standalone release `z-controller.exe` in an isolated directory with zero companion DLLs, verify tray icon launches without popups, verify open UI displays controller list with battery %, profile editor, and live input test view, close window back to tray, and measure working set `<= 25 MB`.

**Acceptance Scenarios**:
1. **Given** `z-controller.exe` is copied alone into an empty directory, **When** launched on Windows, **Then** it extracts its runtime `SDL3.dll` into `%LOCALAPPDATA%\z-controller\runtime\` if absent and operates as a single portable executable.
2. **Given** ViGEmBus or HidHide is missing at launch, **When** the user opens the window, **Then** an inline banner offers driver installation via UAC elevation using bundled installers, and if HidHide is newly installed, an inline notice advises reboot without forcing it.
3. **Given** `z-controller.exe` sits idle in the tray with connected pads active, **When** measured after 5 seconds, **Then** process RAM (`WorkingSet64` and Private Working Set) is `<= 25 MB` and CPU usage is near 0% via event-driven `SDL_WaitEvent`.

### Edge Cases

- **HidHide Exclusive Control Handle (`ERROR_ACCESS_DENIED`)**: If `HidHideClient.exe` or `HidHideCLI.exe` holds `\\.\HidHide` open, `z-controller` surfaces an inline non-modal warning in the UI instead of crashing or showing a popup.
- **DS4 Neutral D-Pad & Stick Center**: `Ds4Report` uses `128` for stick centers and `0x08` (`Neutral`) for D-pad release (`0` means D-pad Up); `profile-mapping` MUST initialize neutral DS4 reports with `128` sticks and `0x08` D-pad.
- **`i16::MIN` Stick Inversion**: Negating `-32768` in `i16` overflows; `profile-mapping` MUST saturate `-(-32768)` to `i16::MAX` (`32767`).
- **Virtual Loop Prevention**: Virtual X360/DS4 pads created by `z-controller` appear as SDL3 gamepads; PnP ancestor inspection MUST ignore every devnode owned by `ViGEmBus` / `Nefarius`.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 (`core-input-battery`)**: System MUST read controllers via SDL3 3.4.20 with `gamecontrollerdb.txt` loaded via `SDL_AddGamepadMappingsFromIO`, `SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS=1`, and HIDAPI enabled for Switch, PS4, PS5, and Xbox.
- **FR-002 (`core-input-battery`)**: System MUST parse battery levels per family (`Nintendo` Switch `0x30`/`0x21` byte 2 matching Steam's `round((level/8.0)*100)`, `Sony` DS4/DualSense byte formula `min(level*10+5, 100)`, `Xbox` XInput levels) reconciled with `SDL_GetGamepadPowerInfo`, displayed view-only in the UI.
- **FR-003 (`core-input-battery`)**: System MUST use event-driven waiting (`SDL_WaitEvent` + `SdlEventWaker`) with zero busy polling and skip virtual ViGEmBus devices via PnP ancestor checking.
- **FR-004 (`profile-mapping`)**: System MUST implement Level-2 mapping only: layout normalization, `swap-ab-xy`, per-button remap (`15` logical buttons), scaled radial stick deadzone (`0.0..=0.5`), target choice (`Xbox360` or `DualShock4`), and per-profile disconnect policy (`KeepNeutral` default or `DropVirtual`). Macros, turbo, gyro output, and per-game auto-switching are explicitly excluded from v1.
- **FR-005 (`profile-mapping`)**: System MUST bind profiles per physical controller hardware identity (`VID:PID` + serial/path discriminator), support manual switching, and persist profiles/bindings atomically to `%APPDATA%\z-controller\config.toml`.
- **FR-006 (`virtual-pad-output`)**: System MUST define a cross-platform `OutputBackend` + `VirtualPadHandle` trait boundary (`z-core`) with a compile-verified `ReadOnlyNullBackend` stub for future macOS read-only support, and a full Windows `ViGEmBus` backend supporting virtual Xbox 360 and DualShock 4 pads plus event-driven rumble passthrough.
- **FR-007 (`hidhide-cloak`)**: System MUST control HidHide directly via `\\.\HidHide` IOCTLs (`0x80016000..=0x8001601C`), whitelist its own NT device executable path, cloak active physical pads when `cloak_physical = true`, hold the driver handle only during atomic IOCTL bursts, and uncloak owned instances on exit, Ctrl+C, session shutdown, or panic.
- **FR-008 (`driver-bootstrap`)**: System MUST embed official `ViGEmBus` (`v1.22.0`) and `HidHide` (`v1.5.230.0`) installers in `.rdata`, detect missing drivers on startup, launch elevated (`runas`) installation on user request, and display a non-forced inline reboot notice after HidHide installation.
- **FR-009 (`tray-ui`)**: System MUST reside in the Windows system tray (`Shell_NotifyIconW`), open a single full window containing controller list + view-only battery, profile editor, and live input test view, emit ZERO notifications/balloon tips/popups (`NIF_INFO` and `MessageBoxW` forbidden), and maintain idle RAM `<= 25 MB`.

### Key Entities

- **PhysicalController**: Connected hardware instance identified by `HardwareKey` (`vid`, `pid`, `serial_or_path`), PnP `instance_id`, `ControllerFamily`, `Transport`, `BatteryReading`, and live `PhysicalGamepadState`.
- **Profile**: Named mapping configuration (`id`, `name`, `target`, `cloak_physical`, `disconnect_policy`, `swap_ab_xy`, `left_deadzone`, `right_deadzone`, `rumble_enabled`, `button_remap`).
- **ControllerSlot**: Runtime binding associating a `HardwareKey` with its active `Profile`, optional live `VirtualPadHandle`, last sent `VirtualOutputReport` (for deduplication), and cloaked PnP instance ID.

## Success Criteria

- **SC-001**: `cargo test --workspace` passes 100% of unit and state-machine tests.
- **SC-002**: Single release binary `target/release/z-controller.exe` runs standalone from an isolated temp directory with zero adjacent DLLs.
- **SC-003**: Bluetooth Aolion Pro Controller (`057E:2009`) + `Xbox360` + cloak hides physical pad from `joy.cpl`/DirectInput8/HID (`0` uncloaked `057E:2009` visible -> zero double input), maps Up cleanly to `+sThumbLY` (zero inverted sticks), and reports battery % matching Steam's Switch HID formula.
- **SC-004**: Two simultaneously connected physical pads receive two independent virtual pads on `ViGEmBus` with zero self-capture loops.
- **SC-005**: Settled idle RAM of `z-controller.exe` in tray mode is `<= 25.0 MB` (`WorkingSet64` and `Working Set - Private`).

## Assumptions

- **A-001 (FIFA 23 Verification)**: `FIFA23.exe` is not installed on the dev machine (`C:`, `D:`, `Y:`, `Z:` inspected). FIFA 23 input behavior is verified directly on live hardware using an un-whitelisted Win32 game probe (`probe_game_view.exe`) exercising the exact input APIs FIFA 23 uses (XInput, DirectInput8, WinMM `joy.cpl`, and raw HID) alongside `joy.cpl` UI checks, and this absence of `FIFA23.exe` is reported transparently in final verification.
- **A-002 (GNU Toolchain & Native ViGEm IOCTL Client)**: `z-controller` builds on `stable-x86_64-pc-windows-gnu` with `windows-sys = "=0.59.0"` implementing `ViGEmBus` IOCTLs directly (avoiding `windows >= 0.60` `raw-dylib`/`dlltool` incompatibility on GNU without Visual Studio Build Tools).
- **A-003 (Single-Binary Embedded Runtime & Software UI)**: `SDL3.dll` (3.4.20) and driver installers are embedded via `include_bytes!` inside `.rdata` (demand-paged by Windows only when accessed). UI rendering uses a pure-CPU software rasterizer over `egui` (`default-features = false`) blitted via `gdi32::SetDIBitsToDevice`, freeing the framebuffer when minimized/closed to tray to avoid 40–80 MB D3D/OpenGL driver heaps and easily satisfy `<= 25 MB` idle RAM.
- **A-004 (Steam Battery Comparison)**: Live Steam UI is not currently scripted; Steam's `Z:\games\steam\logs\controller.txt` confirms Steam drives `057E:2009` via Valve's HIDAPI Nintendo Switch driver, which shares the exact `round((level/8.0)*100)` formula implemented in `SDL_hidapi_switch.c` and `z-core::battery::NintendoBatteryParser`.
