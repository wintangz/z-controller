# Implementation Plan: `ui-neon-restyle`

**Branch**: `003-ui-neon-restyle` | **Date**: 2026-11-25 | **Spec**: [spec.md](spec.md)

## Summary

Restyle the existing pure-CPU software-rendered `egui` UI in `apps/z-controller/src/ui.rs` and `apps/z-controller/src/tray_win32.rs` to a static neon-on-black palette (`#0A0A10` / `#14141F` / `#26263A`, neon accents `#3DFF7A`, `#A970FF`, `#FF5CA8`, text `#EDEDF2` / `#9A9AB0`) with `Segoe UI` typography, rounded cards, horizontal battery bar (`<= 20%` pink / `> 20%` green), and a static 4-ring AA `Mesh` violet-to-pink gradient stroke + layered outer glow on active virtual-output cards.

## Architecture Preservation Check

- **UI Framework & Painter**: Unchanged (`egui 0.29.1` + `sw_painter.rs` + Win32 `tray_win32.rs`).
- **Layout & Features**: Unchanged — left sidebar controller list, bottom status bar, central `Profile Editor` + `Live Input Test` tabs keep every control and label in place.
- **Idle RAM & CPU**: Zero animation/repaint timers (`SDL_WaitEvent` + `GetMessageW`); lazy `Segoe UI` atlas freed on `WM_CLOSE`; `EmptyWorkingSet` trim retained.

## Key Implementation Units

1. **Theme, Palette & `Segoe UI` Font (`ui.rs`)**: `configure_neon_theme_and_fonts` loads `C:\Windows\Fonts\segoeui.ttf` into `FontDefinitions` and applies `Visuals::dark()` overrides (`#0A0A10`, `#14141F`, `#26263A`, `#EDEDF2`, `#9A9AB0`, subtle violet hover/focus strokes).
2. **Static Gradient Stroke + Layered Glow (`ui.rs`)**: `paint_gradient_rounded_rect_stroke` emits an anti-aliased closed triangle strip around rounded card edges with diagonal `#A970FF -> #FF5CA8` vertex colors; `paint_active_card_neon_edge_and_glow` renders 3 translucent outer glow passes + `1.5px` core gradient edge.
3. **Status Dots & Horizontal Battery Bar (`ui.rs`)**: `draw_battery_bar` renders a `#0A0A10` track with `#26263A` border, `#3DFF7A` (or `#FF5CA8` at `<= 20%`) fill bar, and `#EDEDF2` percentage badge beside it.
4. **Window Chrome & Review Capture (`tray_win32.rs`, `main.rs`)**: Update `CLEAR_BG_RGB` to `0x000A_0A10`, set dark DWM caption color `#0A0A10`, update tray icon pixel colors, and support `--preview-cards` for static screenshot capture.
