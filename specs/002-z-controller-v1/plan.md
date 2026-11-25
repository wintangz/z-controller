# Implementation Plan: `z-controller` V1

**Branch**: `002-z-controller-v1` | **Date**: 2026-11-25 | **Spec**: [spec.md](spec.md)

## Summary

Build `z-controller` as a lightweight single-binary Windows system-tray application in Rust (`stable-x86_64-pc-windows-gnu`) structured into four focused workspace crates: `z-sdl3` (embedded SDL3 3.4.20 loader + `gamecontrollerdb.txt`), `z-core` (OS-independent battery parsers, Level-2 mapping, profile management, and `OutputBackend` trait), `z-platform-windows` (native `windows-sys = "=0.59.0"` `ViGEmBus`, `HidHide` IOCTLs, PnP virtual anti-loop filter, and demand-paged driver bootstrap), and `z-controller` (event-driven engine + pure-CPU software-rendered `egui` UI + Win32 tray host).

## Technical Context

**Language/Version**: Rust `1.100.0` (Edition 2024, `stable-x86_64-pc-windows-gnu`)  
**Primary Dependencies**: `windows-sys = "=0.59.0"`, `serde`, `toml`, `thiserror`, `bitflags`, `egui` (`default-features = false`, `default_fonts` only)  
**Storage**: `%APPDATA%\z-controller\config.toml` (profiles + per-controller bindings)  
**Testing**: `cargo test --workspace` + live dev-machine hardware acceptance (`probe_game_view.exe` + `z-controller.exe --smoke-verify` + live tray RAM telemetry)  
**Target Platform**: Windows 10/11 x64 (`OutputBackend` trait structured for future macOS read-only backend)  
**Performance & Memory Goals**: Idle RAM `<= 25 MB` (`WorkingSet64` and Private Working Set); event-driven `SDL_WaitEvent` idle (near 0% CPU, zero idle IOCTLs via report deduplication).

## Constitution Check

- **I. Slug-Only Feature Identity**: Verified — all docs and modules map cleanly to `core-input-battery`, `profile-mapping`, `virtual-pad-output`, `hidhide-cloak`, `driver-bootstrap`, `tray-ui`.
- **II. Lightweight Single-Binary Architecture**: Verified — pure-CPU `egui` software rasterizer avoids GPU driver heaps; pixel buffer freed on hide to tray; embedded `SDL3.dll` & driver installers demand-paged from `.rdata`.
- **III. Shared Core + Per-OS Output Trait**: Verified — `z-core` has zero Windows dependencies and compiles with `ReadOnlyNullBackend`.
- **IV. Safety & Zero Popups**: Verified — panic hook, `SetConsoleCtrlHandler`, `WM_ENDSESSION`, and `Drop` call `emergency_uncloak()`; zero `MessageBoxW` or `NIF_INFO`.

## Project Structure

```text
y:\Repos\z-controller\
├── Cargo.toml
├── assets/
│   ├── gamecontrollerdb.txt
│   └── drivers/
│       ├── ViGEmBus_Setup.exe
│       └── HidHide_Setup.exe
├── crates/
│   ├── z-core/                     # core-input-battery + profile-mapping + OutputBackend trait
│   ├── z-sdl3/                     # SDL3 3.4.20 dynamic loader & event/gamepad/rumble FFI
│   └── z-platform-windows/         # vigem, hidhide, pnp, drivers, raw_battery, WindowsOutputBackend
└── apps/
    └── z-controller/               # tray-ui + event-driven engine + sw_painter + main
```

## Memory Budget (`<= 25 MB` Ceiling)

| Component | Tray-Idle Target | Window-Open Target |
|---|---|---|
| Rust binary `.text` + private heap | ~3.0 MB | ~4.0 MB |
| `SDL3.dll` runtime (loaded once) | ~3.5 MB | ~3.5 MB |
| `gamecontrollerdb.txt` parsed by SDL3 | ~1.5 MB | ~1.5 MB |
| Demand-paged installers in `.rdata` | 0.0 MB (unmapped until install) | 0.0 MB |
| `egui` context + font atlas (`default_fonts`) | ~1.5 MB | ~5.0 MB |
| Software framebuffer (`960x660x4`) | **0.0 MB** (freed on hide) | ~2.5 MB |
| **Estimated Total Working Set** | **~9.5–13 MB** | **~16.5–21 MB** |
