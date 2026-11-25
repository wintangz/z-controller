# z-controller Constitution

## Core Principles

### I. Slug-Only Feature Identity
Every feature in specifications, plans, tasks, code documentation, commits, and status reports MUST be referenced strictly by its canonical slug:
- `feasibility-spike`
- `core-input-battery`
- `profile-mapping`
- `virtual-pad-output`
- `hidhide-cloak`
- `driver-bootstrap`
- `tray-ui`

### II. Lightweight Single-Binary Architecture
The deliverable MUST be a single standalone Windows executable (`z-controller.exe`) targeting idle RAM `<= 25 MB` (private working set and total working set measured at rest in tray mode). Heavy GPU UI frameworks (WebView2, Electron, ANGLE/OpenGL/D3D driver heaps) are prohibited when pure-CPU rasterization keeps memory under ceiling. Input processing MUST be event-driven (`SDL_WaitEvent` / OS wakeups) with zero busy polling and zero idle IOCTL churn.

### III. Shared Cross-Platform Core + Per-OS Output Trait
Input normalization, per-family battery parsing, profile storage, and Level-2 mapping (`swap-ab-xy`, per-button remap, stick radial deadzone, disconnect policy) MUST live in `z-core` with zero Windows API dependencies. OS-specific virtual pad creation, controller cloaking, and driver bootstrap MUST sit behind an `OutputBackend` trait boundary so a future macOS read-only backend compiles without refactoring core logic.

### IV. Safety, Cloaking Hygiene & Zero Popups
Every HidHide cloak applied by `z-controller` MUST be tracked per owned device instance and cleanly uncloaked on normal exit, Ctrl+C, session shutdown, or panic. `z-controller` MUST NEVER emit notifications, balloon tips (`NIF_INFO`), toast alerts, or modal `MessageBoxW` popups of any kind.

### V. Spec-Driven Autonomous Delivery & Hardware Proof
Every milestone runs `specify -> plan -> tasks -> implement -> test` with no speculative unverified claims. Hardware-facing claims MUST be verified against live dev-machine devices (`057E:2009` Bluetooth Pro Controller and second pad) and un-whitelisted Win32 input consumers (`joy.cpl` / DirectInput8 / XInput).

## Scope Boundaries (V1)
- Mapping Level 2 only: layout normalization, `swap-ab-xy`, per-button remap, stick radial deadzone.
- NO macro/turbo in v1.
- NO gyro/IMU output mapping in v1.
- NO per-game process auto-switching in v1 (manual switching only; auto-switch is phase 2).
- Battery is view-only in the UI.
- Driver install prompts via UAC elevation only when drivers are missing; HidHide reboot requirement is surfaced inline and never forced.

**Version**: 1.0.0 | **Ratified**: 2026-11-25 | **Last Amended**: 2026-11-25
