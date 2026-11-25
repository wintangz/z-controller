# Tasks: `polish-addon`

- [x] T001 Capture baseline Editor full-window paint time (`55.74 ms`) and 10s circular drag CPU (`5.594 s`, `55.94%` of 1 core / `3.5%` system) on `6e1857b`
- [x] T002 Create `specs/005-polish-addon/{spec.md,plan.md,tasks.md}` and generate multi-size neon `apps/z-controller/assets/z-controller.ico` (`16/20/24/32/48/64/128/256`) + `specs/005-polish-addon/icon-preview.png`
- [x] T003 Embed `app.rc` icon resource via `build.rs` and load resource `#1` for Explorer, taskbar, title bar, and tray in `apps/z-controller/src/tray_win32.rs`
- [x] T004 Redraw smooth Bezier vector controller diagrams for Xbox, Sony DualSense, and Nintendo Switch Pro with full symbols and unified cross D-Pad in `apps/z-controller/src/diagram.rs`
- [x] T005 Implement Win32 DIB double-buffered back buffer, quad fast path, static offscreen cache, dirty-rect clipping, and `WM_ENTERSIZEMOVE` drag guard across `sw_painter.rs`, `ui.rs`, and `tray_win32.rs`
- [x] T006 Run `cargo test --workspace`, verify all 8 icon sizes + manifest in `z-controller.exe`, benchmark AFTER full-window paint time & 10s circular drag CPU %, inspect screenshots, and re-measure tray RAM/CPU
- [x] T007 Update `specs/005-polish-addon/verification.md` and commit cleanly
