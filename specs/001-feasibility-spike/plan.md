# Implementation Plan: `feasibility-spike`

**Branch**: `001-feasibility-spike` | **Date**: 2026-11-25 | **Spec**: [spec.md](spec.md)

## Summary

Validate toolchain linking (`x86_64-pc-windows-gnu` vs `x86_64-pc-windows-msvc` fallback), SDL3 3.4.20 controller + battery reading, `vigem-rust` Xbox 360 virtual controller injection into `joy.cpl` + game input APIs, and direct Win32 `DeviceIoControl` HidHide cloaking on the live dev machine before building the `tray-ui` application shell.

## Technical Context

**Language/Version**: Rust `1.100.0` (Edition 2024)  
**Primary Dependencies**: `vigem-rust = "0.2"`, `windows-sys = "0.61"` (or `0.59`), embedded `SDL3.dll` `3.4.20`, `assets/gamecontrollerdb.txt`  
**Testing**: Live dev-machine dual-process verification (`spike_runner.exe` whitelisted + `probe_game_view.exe` un-whitelisted + `control.exe joy.cpl` UIAutomation inspection)  
**Target Platform**: Windows 10/11 x64 (`x86_64-pc-windows-gnu` or `x86_64-pc-windows-msvc`)  
**Constraints**: Zero leftover HidHide cloak state after run; `HidHideClient.exe` closed before driver IOCTL test.

## Architecture for Spike

```text
crates/
├── z-sdl3/                 # Safe Rust dynamic loader + FFI over embedded SDL3 3.4.20
└── z-platform-windows/     # ViGEmBus wrapper + HidHide IOCTL client + PnP utilities + spike binaries
    └── src/bin/
        ├── spike_runner.rs     # Whitelisted driver & SDL3 spike runner
        └── probe_game_view.rs  # Un-whitelisted WinMM + DirectInput8 + XInput + HID probe
```

## Verification Gates

1. `cargo build --bin spike_runner --bin probe_game_view` compiles and links cleanly (logging MSVC switch if GNU fails).
2. `probe_game_view` + `joy.cpl` baseline before cloak shows physical `057E:2009`.
3. `spike_runner` verifies Item 1 (SDL3 reads `057E:2009` + battery), Item 2 (`ViGEm` X360 visible in `joy.cpl` and `probe_game_view` reads injected axes/buttons), Item 3 (`HidHide` cloaks `057E:2009` from `joy.cpl` & `probe_game_view` while `spike_runner` still reads SDL3), then cleanly uncloaks.
