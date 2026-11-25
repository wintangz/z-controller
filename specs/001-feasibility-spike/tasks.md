# Tasks: `feasibility-spike`

**Input**: `specs/001-feasibility-spike/spec.md`, `specs/001-feasibility-spike/plan.md`

## Phase 1: Toolchain & Workspace Foundation (`feasibility-spike`)

- [x] T001 Create workspace `Cargo.toml`, `crates/z-sdl3`, and `crates/z-platform-windows`
- [x] T002 Verify `vigem-rust` linking on `x86_64-pc-windows-gnu`; document `windows 0.62` (`raw-dylib`/`dlltool`) GNU blocker and implement native `windows-sys = "=0.59.0"` `ViGEmBus` IOCTL client in `crates/z-platform-windows/src/vigem.rs` (log decision in `spec.md` `A-002`)

## Phase 2: SDL3, ViGEm & HidHide Spike Implementation (`feasibility-spike`)

- [x] T003 [P] Implement `crates/z-sdl3` dynamic loader & FFI bindings for `SDL3.dll` 3.4.20 (`SDL_SetHint`, `SDL_Init`, `SDL_AddGamepadMappingsFromIO`, `SDL_GetGamepads`, `SDL_OpenGamepad`, `SDL_GetGamepadPowerInfo`, `SDL_WaitEventTimeout`, `SDL_PushEvent`, `SDL_RumbleGamepad`)
- [x] T004 [P] Implement `crates/z-platform-windows/src/hidhide.rs` exact IOCTL contract (`0x80016000..=0x8001601C`), NT DOS path conversion, and emergency uncloak tracking
- [x] T005 [P] Implement `crates/z-platform-windows/src/vigem.rs` X360/DS4 virtual pad creation, state update, and rumble feedback IOCTLs
- [x] T006 Implement `spike_runner.rs` (whitelisted subject) and `probe_game_view.rs` (un-whitelisted WinMM + DirectInput8 + XInput + raw HID probe)

## Phase 3: Live Hardware Verification (`feasibility-spike`)

- [x] T007 Close `HidHideClient.exe` cleanly so `\\.\HidHide` exclusive handle is available
- [x] T008 Execute live hardware spike covering Item 1 (SDL3 + battery on `057E:2009`), Item 2 (ViGEm X360 in `joy.cpl`/WinMM/DirectInput8/XInput via `probe_game_view`), and Item 3 (HidHide cloak hiding physical pad from `probe_game_view` while `spike_runner` still reads SDL3)
- [x] T009 Record exact pass/fail + proving numbers in `specs/001-feasibility-spike/evidence.md` and verify HidHide uncloaks cleanly
