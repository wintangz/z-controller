# Verification Report: `ui-fix-round`

## Build & Automated Test Suite
- `cargo test --workspace`: **PASS** (`20 passed, 0 failed`, 0 warnings: `z-controller` 6, `z-core` 10, `z-platform-windows` 4, `z-sdl3` 0, `probe_game_view` 0, `spike_runner` 0).
- `cargo build --release --workspace`: **PASS** (0 warnings).
- Game-named default search (`Select-String -Path crates\*\src\*.rs, apps\*\src\*.rs -Pattern "FIFA|x360-fifa|ds4-default"`): **0** game-named defaults shipped; only legacy config migration clauses (`x360-fifa` -> `xbox-360`, `ds4-default` -> `dualsense`) remain in `crates/z-core/src/profile.rs`.

## Budgets (Idle Tray, Release Binary, Zero Dev Flags)
- Isolated release tray working set (`T+3s` & `T+8s`): **`16.8 MB`** (`<= 25.0 MB` budget), Private Memory **`3.79 MB`**.
- 5-second idle CPU delta: **`0.0312 s`** (**`0.039%`** total CPU across logical cores; ~0).
- Post-test `probe_game_view.exe`: `[XInput] connected_slots=0`.

## Item-by-Item Results (1–7)

1. **Layout Tightening**: **PASS** — Uniform `4 / 8 / 12 / 16` spacing scale (`SPACE_XS..SPACE_LG`) applied across `apps/z-controller/src/ui.rs` and `apps/z-controller/src/diagram.rs`; aligned card dividers and panel columns verified visually across all 4 screenshots (`412x640` compact, `1040x680` editor) with 0 clipping and 0 protruding borders.
2. **Profile Editor with Controller Diagram**: **PASS** — `apps/z-controller/src/diagram.rs` renders Xbox (`LT/RT`, `LB/RB`, asymmetric sticks, `Y/X/B/A`, `Xb/Vw/Mn/Shr`), Sony (`L2/R2`, `L1/R1`, symmetric lower sticks, `△/□/○/✕`, `Cr/Touchpad/Op/PS/Mic`), and Nintendo (`ZL/ZR`, `L/R`, asymmetric sticks, `X/Y/A/B`, `-/+/Cap/Hm`), plus back paddles (`L4/L5/R5/R4`), live physical highlights, click-to-select remapping, and synchronized right-hand mapping table; verified in `screenshot-editor-xbox.png`, `screenshot-editor-sony.png`, and `screenshot-editor-nintendo.png`.
3. **Profile Selector Bug**: **PASS** — Root cause fixed in `apps/z-controller/src/tray_win32.rs` (`SetCapture`/`ReleaseCapture`, synchronous `InvalidateRect` + `UpdateWindow`, and bounded `WM_APP_IMMEDIATE_REPAINT` on `egui` zero-delay popup requests) and `apps/z-controller/src/ui.rs` (`selectable_label` + `.clicked()`). Proved both by unit test `clicking_profile_selector_emits_select_device_profile_command` and live ViGEmBus/XInput probe `z-controller.exe --probe-profile-switch`:
   - `controller_canonical_id`: `045e:028e:sdl-3`
   - `before_profile_id`: `xbox-360` -> `after_profile_id`: `probe-swap`
   - `before_xinput_buttons`: `0x1000` (`A`) -> `after_xinput_buttons`: `0x2000` (`B`) while holding physical `South` (`emitted_commands_count = 1`, `combo_clicked_xy = [95.0, 452.0]`, `item_clicked_xy = [122.5, 539.0]`).
4. **Default Profiles**: **PASS** — `AppConfig::default().profiles.len() == 3`, shipping exclusively `"Xbox 360"` (`xbox-360`), `"DualSense"` (`dualsense`, `ControllerFamily::Sony`, `VirtualTargetKind::DualShock4`), and `"Nintendo Switch Pro Controller"` (`switch-pro`, `ControllerFamily::Nintendo`, `VirtualTargetKind::Xbox360`, `swap_ab_xy = false`), all clean 1:1 mappings. Verified by `profile::tests::default_ships_exact_three_clean_builtin_profiles`.
5. **Remove `Live Input Test` Tab**: **PASS** — `MainTab` enum and `draw_live_test` deleted from `apps/z-controller/src/ui.rs`; controller cards' live view is the sole live input test UI (`screenshot-compact.png`).
6. **Compact Main Window**: **PASS** — Main window shrinks to `412x640` controller cards + status bar (`screenshot-compact.png`); clicking `Edit Profile` opens the separate `1040x680` `ZControllerEditorHost` Profile Editor window (`screenshot-editor-*.png`), whose `WM_CLOSE` hides only the editor window without exiting tray operation.
7. **Misc Buttons for All Families**: **PASS** — `LogicalButton` expanded to all 26 SDL3 buttons (`South..=Misc6`) paired with `SDL_GamepadHasButton` probing (`present_buttons_mask`), family labels (`Cap` Nintendo, `Touchpad` / `Cr` / `Mic` Sony, `Shr` Xbox, paddles `L4/L5/R4/R5`), live chips on controller cards, diagram rendering, DS4 touchpad click special bit mapping, and remap source support (`card_live_status_covers_all_standard_and_extra_buttons` & `swap_ab_xy_and_misc_paddle_button_remap` passing).

## Screenshots
- [screenshot-compact.png](file:///y:/Repos/z-controller/specs/004-ui-fix-round/screenshot-compact.png)
- [screenshot-editor-xbox.png](file:///y:/Repos/z-controller/specs/004-ui-fix-round/screenshot-editor-xbox.png)
- [screenshot-editor-sony.png](file:///y:/Repos/z-controller/specs/004-ui-fix-round/screenshot-editor-sony.png)
- [screenshot-editor-nintendo.png](file:///y:/Repos/z-controller/specs/004-ui-fix-round/screenshot-editor-nintendo.png)
- [screenshot-compact-150dpi.png](file:///y:/Repos/z-controller/specs/004-ui-fix-round/screenshot-compact-150dpi.png)
- [screenshot-editor-xbox-150dpi.png](file:///y:/Repos/z-controller/specs/004-ui-fix-round/screenshot-editor-xbox-150dpi.png)

## Add-on: DPI Awareness (`PerMonitorV2`)
- **Embedded manifest**: `apps/z-controller/z-controller.manifest` embedded via `apps/z-controller/build.rs` (`/MANIFEST:EMBED`, `/MANIFESTINPUT:...`); verified in `target\release\z-controller.exe` `RT_MANIFEST #1` (`dpiAware=true/PM`, `dpiAwareness=PerMonitorV2`).
- **Live monitor & `WM_DPICHANGED`**: On the host's 125% display (`winDpi=120`), `z-controller.exe --show` opened with `awareness=2` (`DPI_AWARENESS_PER_MONITOR_AWARE`) at native `513x798` physical pixels; triggering `WM_DPICHANGED` to 150% (`144 DPI`) cleanly resized & re-rendered `616x958` (`screenshot-compact-150dpi.png`) and `1558x1018` (`screenshot-editor-xbox-150dpi.png`) with crisp `Segoe UI` glyphs and diagram strokes.
- **Re-checked tests & release build**: `cargo test --workspace` **22 passed, 0 failed** (incl. `dpi_scaling_math_and_outer_window_size_scale_monotonically` and `high_dpi_scaling_renders_larger_font_atlas_and_crisp_150pct_buffer`), **0 warnings**.
- **Re-checked isolated tray budgets**: `WorkingSet64 = 16.82 MB` (`<= 25.0 MB`), `PrivateMemorySize64 = 3.82 MB`, 5-second idle CPU delta = `0.0312 s` (`0.039%` total CPU). Post-exit `probe_game_view.exe`: `[XInput] connected_slots=0`.
