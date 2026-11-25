
# Feature Specification: `ui-neon-restyle`

**Feature Branch**: `003-ui-neon-restyle`  
**Slug**: `ui-neon-restyle`  
**Created**: 2026-11-25  
**Status**: Approved (Autonomous Spec Kit Run)

## User Scenarios & Testing

### User Story 1 - Modern Neon-on-Black Software UI Restyle (`ui-neon-restyle`) (Priority: P1)

As a desktop user opening `z-controller` from the system tray, I want a refined neon-on-black visual style inspired by dark utility dashboards—clean `#14141F` panels on a `#0A0A10` backdrop, crisp `1px` `#26263A` rounded borders, Segoe UI typography, neon status dots and horizontal battery bars, and a static violet-to-pink gradient neon border with a soft layered outer glow around whichever controller card has an active virtual output—without changing layout, controls, tray architecture, or exceeding the `<= 25 MB` tray idle memory ceiling.

**Acceptance Scenarios**:
1. **Given** the window is open with controller cards rendered, **When** a controller's virtual output is active, **Then** its card displays a smooth violet (`#A970FF`) to pink (`#FF5CA8`) gradient stroke and a soft multi-layer translucent outer glow painted statically (zero animation loops, zero continuous repaint).
2. **Given** a controller card is inactive (`virtual_active = false`), **When** painted, **Then** it uses `#14141F` fill, rounded corners, and a crisp `1px` `#26263A` border.
3. **Given** any controller has a battery reading, **When** displayed in the controller card, **Then** it shows a horizontal bar with neon-green (`#3DFF7A`) fill (or neon-pink `#FF5CA8` when `<= 20%`) and `#EDEDF2` percentage text beside it, with neon colors restricted to accents, edges, and indicators only.
4. **Given** the user closes the window back to the system tray and lets `z-controller.exe` sit idle, **When** memory and CPU are measured, **Then** tray idle `WorkingSet64` stays `<= 25.0 MB` and idle CPU stays `~0`.

### User Story 2 - Live Physical Status View Inside Controller Cards (`live-status-view`) (Priority: P1)

As a user checking my controller on the main screen, I want each controller card to include a live physical status view showing every button lighting up in neon accent when physically pressed, left and right stick position dots tracking real stick deflections, and compact analog fill bars for `LT`/`RT` triggers—reading directly from the engine's existing SDL3 physical input stream and repainting on state change only, with zero CPU/RAM overhead when hidden in the tray.

**Acceptance Scenarios**:
1. **Given** a connected controller reports physical input via SDL3, **When** buttons are pressed, sticks deflected, or triggers pulled, **Then** only changed state triggers a repaint, pressed buttons light up with `#A970FF`/`#FF5CA8` neon accents, stick dots track real `(x, y)` coordinates, and trigger bars fill proportionally.
2. **Given** the window is hidden in the system tray, **When** input or idle time elapses, **Then** no UI paint occurs and idle CPU remains `~0` with tray `WorkingSet64 <= 25.0 MB`.

## Requirements

- **FR-001 (`ui-neon-restyle`)**: Apply exact palette: background `#0A0A10`, panel surface `#14141F`, subtle border `#26263A`, neon green `#3DFF7A`, neon violet `#A970FF`, neon pink `#FF5CA8`, primary text `#EDEDF2`, muted text `#9A9AB0`. Neon colors MUST NOT be used as large filled areas or body text.
- **FR-002 (`ui-neon-restyle`)**: Use Windows `Segoe UI` font (`segoeui.ttf`) and keep all existing controls and labels in their exact existing positions with zero layout/feature/screen additions and zero framework changes (`egui` + pure-CPU `SoftwarePainter` + Win32 tray host).
- **FR-003 (`ui-neon-restyle`)**: Controller cards use rounded corners and `1px` `#26263A` borders; active virtual output cards render a static violet-to-pink gradient stroke plus layered translucent static outer glow. Connected status uses `#3DFF7A` dot; battery uses a horizontal bar (`#3DFF7A`, or `#FF5CA8` at `<= 20%`) with percentage text beside it.
- **FR-004 (`ui-neon-restyle`)**: Static painting only — no animation loops, no continuous repaint, idle CPU `~0`, and tray idle `WorkingSet64 <= 25.0 MB`.
- **FR-005 (`live-status-view`)**: Render a live physical input status view inside each controller card showing all `15` logical buttons (`#A970FF`/`#FF5CA8` neon highlight on physical press), L/R mini stick circular wells with moving position dots, and compact L/R analog trigger fill bars directly from `ctrl.physical_state`.
- **FR-006 (`live-status-view`)**: Skip redundant `SDL_EVENT_GAMEPAD_AXIS_MOTION` / button events whose values did not change, paint only on state change when the window is visible, and incur zero UI cost when closed to tray.

## Assumptions

- **A-001 (Lazy Segoe UI Font Loading)**: `C:\Windows\Fonts\segoeui.ttf` is loaded lazily inside `configure_neon_theme_and_fonts` when the UI window opens (falling back gracefully to `egui` built-in fonts if missing) and unloaded on `WM_CLOSE` along with `ui_session`, preserving minimal tray idle RAM.
- **A-002 (Dark Title Bar Chrome)**: Non-client Win32 title bar attributes (`DWMWA_USE_IMMERSIVE_DARK_MODE` and `DWMWA_CAPTION_COLOR = #0A0A10`) are applied via dynamic `LoadLibraryW("dwmapi.dll")` so the window frame harmonizes with `#0A0A10` without adding crate dependencies.
- **A-003 (Deterministic Review Screenshot Flag)**: A CLI-only `--preview-cards` flag populates two static sample controller cards (an active `057E:2009` card at `86%` battery and an inactive `054C:0CE6` card at `18%` battery) solely to capture a review screenshot when physical gamepads are asleep; memory/CPU verification runs production `z-controller.exe` without preview flags.
- **A-004 (Live Screenshot via Own `SdlVirtualGamepad`)**: A CLI-only `--live-demo-pad` flag attaches our own `SdlVirtualGamepad` (`z-sdl3`) to hold `South` (`A`) pressed, deflect Left Stick (`24000, -21000`), and pull `RT` (`~68%`) through the real SDL3 -> `EngineHandle` -> `ViGEmBus` pipeline for live screenshot inspection without using any game.
