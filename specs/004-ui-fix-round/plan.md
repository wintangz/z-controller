# Implementation Plan: `ui-fix-round`

**Branch**: `004-ui-fix-round` | **Date**: 2026-11-25 | **Spec**: [spec.md](spec.md)

## Architecture & Module Breakdown

1. **`crates/z-core/src/{model.rs,mapping.rs,profile.rs}`**:
   - Expand `LogicalButton` from 15 to all 26 SDL3 gamepad buttons (`South..=Misc6`), add `family_label` / `family_chip`, DS4 `Touchpad` special bit (`1 << 1`), and arbitrary extra-button remap source support.
   - Replace legacy game-named defaults with the exact 3 built-in 1:1 profiles (`"xbox-360"`, `"dualsense"`, `"switch-pro"`) + `Profile::family` + legacy migration.
2. **`crates/z-sdl3/src/lib.rs` & `apps/z-controller/src/engine.rs`**:
   - Add `Misc2..=Misc6`, `SDL_GamepadHasButton` (`has_button`), 21-button virtual test pad support (`nbuttons = 21`, `button_mask = 0x001F_FFFF`), `present_buttons_mask`, and dual-HWND wakeup support (`ui_hwnd` + `editor_hwnd`).
3. **`apps/z-controller/src/diagram.rs`**:
   - Pure software-painted `egui::Painter` `x360ce`-style vector diagram supporting `Xbox`, `Sony`, and `Nintendo` layouts, clickable part selection, live physical press highlights + analog stick dots + trigger fill bars.
4. **`apps/z-controller/src/{ui.rs,tray_win32.rs,main.rs}`**:
   - Strict `4 / 8 / 12 / 16` spacing scale everywhere.
   - Compact controller-cards main window (`392x580`), separate lazy Profile Editor window (`980x660`), `Live Input Test` tab removed.
   - Win32 synchronous click/capture + bounded zero-delay popup repaint (`followup_paint_budget`) and explicit UI `ComboBox` profile-switching dispatch + `--probe-profile-switch` hardware proof.
