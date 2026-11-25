# Tasks: `ui-neon-restyle`

**Input**: `specs/003-ui-neon-restyle/{spec.md,plan.md}`

- [x] T001 Author `specs/003-ui-neon-restyle/{spec.md,plan.md,tasks.md}` for `ui-neon-restyle`
- [x] T002 Implement neon-on-black palette constants, `Segoe UI` font loading, `Visuals::dark()` styling, and static gradient rounded-rect stroke + layered glow painter in `apps/z-controller/src/ui.rs`
- [x] T003 Restyle controller cards (1px `#26263A` subtle border vs active gradient neon edge + outer glow), connected `#3DFF7A` dot, horizontal battery bar (`#3DFF7A` / `#FF5CA8 <= 20%`), profile editor accents, and live test radars in `apps/z-controller/src/ui.rs` keeping all existing controls and labels in place
- [x] T004 Wire `configure_neon_theme_and_fonts`, `#0A0A10` background clear + dark DWM title bar, neon tray icon colors, and `--preview-cards` screenshot hook in `apps/z-controller/src/{tray_win32.rs,main.rs}`
- [x] T005 Add unit tests in `apps/z-controller/src/ui.rs` covering battery threshold colors, gradient interpolation, and snapshot rendering across both tabs
- [x] T006 Run `cargo test --workspace` and `cargo build --release --workspace` (zero warnings/failures)
- [x] T007 Re-measure production tray idle RAM (`<= 25 MB`) & CPU (`~0`), capture `specs/003-ui-neon-restyle/screenshot.png`, and visually verify the rendered window
- [x] T008 Record results in `specs/003-ui-neon-restyle/verification.md` and commit separately naming `ui-neon-restyle`
- [x] T009 Deduplicate physical axis/button state changes in `apps/z-controller/src/engine.rs` so repaints occur strictly on input state transitions when visible (`live-status-view`)
- [x] T010 Implement `draw_card_live_status` on each controller card in `apps/z-controller/src/ui.rs`: L/R stick circular wells with moving dots, LT/RT analog trigger bars, and 15 button pills lighting in neon accent on physical press (`live-status-view`)
- [x] T011 Add `live-status-view` unit tests in `apps/z-controller/src/ui.rs` (stick/trigger normalization, 15-button completeness, lit button + deflected stick snapshot render) and wire `--live-demo-pad` in `apps/z-controller/src/main.rs`
- [x] T012 Run `cargo test --workspace`, re-measure tray idle RAM/CPU, and capture live `SdlVirtualGamepad` screenshot in `specs/003-ui-neon-restyle/screenshot.png`
- [x] T013 Visually inspect `screenshot.png`, update `specs/003-ui-neon-restyle/verification.md`, and commit naming `live-status-view`
