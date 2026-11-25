# Tasks: `ui-fix-round`

- [x] T001 Author `specs/004-ui-fix-round/{spec.md,plan.md,tasks.md}` for `ui-fix-round`
- [x] T002 Expand `LogicalButton` to all 26 SDL3 buttons with family labels, DS4 touchpad click mapping, and exact 3 built-in 1:1 profiles (`Xbox 360`, `DualSense`, `Nintendo Switch Pro Controller`) in `crates/z-core/src/{model.rs,mapping.rs,profile.rs}`
- [x] T003 Wire `Misc2..=Misc6`, `SDL_GamepadHasButton`, and `present_buttons_mask` + dual-window `HWND` wakeups in `crates/z-sdl3/src/lib.rs` and `apps/z-controller/src/engine.rs`
- [x] T004 Implement `x360ce`-style software-painted neon controller diagram (`Xbox`, `Sony`, `Nintendo` layouts, click-to-select, live highlights) in `apps/z-controller/src/diagram.rs`
- [x] T005 Implement compact main window, separate Profile Editor window, misc-button live card chips, `4/8/12/16` spacing tightening, `Live Input Test` tab removal, and `ComboBox` selector fix in `apps/z-controller/src/{ui.rs,tray_win32.rs,main.rs}`
- [x] T006 Run `cargo test --workspace`, verify zero game-named defaults remain, run `--probe-profile-switch` live proof, and re-measure tray idle RAM (`<= 25 MB`) & CPU (`~0`)
- [x] T007 Capture and visually inspect all review screenshots (`screenshot-compact.png`, `screenshot-editor-xbox.png`, `screenshot-editor-sony.png`, `screenshot-editor-nintendo.png`)
- [x] T008 Record proving numbers in `specs/004-ui-fix-round/verification.md` and commit cleanly naming `ui-fix-round`
