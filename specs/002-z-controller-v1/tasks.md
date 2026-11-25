# Tasks: `z-controller` V1

**Input**: `specs/002-z-controller-v1/{spec.md,plan.md,data-model.md,contracts/hidhide-ioctl.md}`

## Phase 1: Core Architecture (`core-input-battery`, `profile-mapping`, `virtual-pad-output`)

- [ ] T001 Create `crates/z-core` crate with `model.rs`, `battery.rs`, `mapping.rs`, `profile.rs`, `backend.rs` (zero Windows API dependencies)
- [ ] T002 [P] Implement per-family battery parsers (`NintendoBatteryParser`, `SonyBatteryParser`, `XboxBatteryParser`) + SDL3 power reconciler in `crates/z-core/src/battery.rs` (`core-input-battery`)
- [ ] T003 [P] Implement Level-2 mapping (`swap-ab-xy`, 15-button remap, radial scaled deadzone, X360 Y-axis inversion vs DS4 non-inversion, DS4 `0x08` neutral D-pad, `i16::MIN` safe negation) in `crates/z-core/src/mapping.rs` (`profile-mapping`)
- [ ] T004 [P] Implement `Profile`, `ControllerBinding`, `AppConfig`, default X360/DS4 profiles, and atomic `%APPDATA%\z-controller\config.toml` persistence in `crates/z-core/src/profile.rs` (`profile-mapping`)
- [ ] T005 [P] Implement `OutputBackend` + `VirtualPadHandle` traits and `ReadOnlyNullBackend` stub in `crates/z-core/src/backend.rs` (`virtual-pad-output`)
- [ ] T006 Write comprehensive unit tests in `crates/z-core` covering battery parsers, mapping invariants, config round-trip, and `KeepNeutral` vs `DropVirtual` state transitions

## Phase 2: Windows Platform Backend (`virtual-pad-output`, `hidhide-cloak`, `driver-bootstrap`)

- [ ] T007 Implement `crates/z-platform-windows/src/drivers.rs` (`driver-bootstrap`): embedded `ViGEmBus_Setup.exe` & `HidHide_Setup.exe` via `include_bytes!`, missing-driver detection, elevated UAC install (`runas`), non-forced reboot flag
- [ ] T008 Implement `crates/z-platform-windows/src/backend_impl.rs` (`WindowsOutputBackend` implementing `z_core::backend::OutputBackend` over `vigem.rs`, `hidhide.rs`, `pnp.rs`, and `drivers.rs`)
- [ ] T009 Add unit/contract tests in `crates/z-platform-windows` (`hidhide` multi-sz encoding/decoding, IOCTL constants, PnP ID parsing)

## Phase 3: Event-Driven Engine & Tray UI (`tray-ui`, `core-input-battery`, `hidhide-cloak`)

- [ ] T010 Create `apps/z-controller` crate and verify `egui` (`default-features = false`) builds cleanly on `stable-x86_64-pc-windows-gnu` without `dlltool`/`raw-dylib`
- [ ] T011 Implement `apps/z-controller/src/engine.rs`: event-driven `SDL_WaitEvent` loop, PnP virtual device filter, report deduplication, multi-controller virtual slots, rumble forwarding via `SdlEventWaker`, disconnect handling (`KeepNeutral` / `DropVirtual`), panic/Ctrl-C/shutdown uncloak
- [ ] T012 Implement `apps/z-controller/src/sw_painter.rs`: pure-CPU software rasterizer for `egui::ClippedPrimitive` + `gdi32::SetDIBitsToDevice` (frees pixel framebuffer when window hidden to tray)
- [ ] T013 Implement `apps/z-controller/src/tray_win32.rs`: Win32 window + `Shell_NotifyIconW` system tray icon + context menu + `WM_ENDSESSION` uncloak, strictly zero `NIF_INFO` or `MessageBoxW`
- [ ] T014 Implement `apps/z-controller/src/ui.rs`: controller list with view-only battery, profile editor (target, cloak, disconnect policy, rumble, `swap-ab-xy`, stick deadzone, 15-button remap), live input test view (dual stick deadzone radar, triggers, buttons, telemetry), inline driver/HidHide/RAM status bar
- [ ] T015 Wire CLI verification support (`--smoke-verify <path>` + normal tray startup) in `apps/z-controller/src/main.rs`

## Phase 4: Verification & V1 Acceptance (`test`)

- [ ] T016 Run `cargo test --workspace` and verify zero warnings/failures
- [ ] T017 Build release binary (`cargo build --release --workspace`) and verify single-binary execution from an isolated temp directory with zero adjacent DLLs
- [ ] T018 Execute end-to-end V1 Acceptance on dev machine (Bluetooth Pro Controller `057E:2009` -> X360 + HidHide cloak -> zero double input & zero inverted sticks in `probe_game_view`, battery vs Steam formula, two pads simultaneous test, tray idle RAM `<= 25 MB`, clean uncloak on quit)
- [ ] T019 Record all pass/fail results + proving numbers in `specs/002-z-controller-v1/verification.md` and commit to `main`
