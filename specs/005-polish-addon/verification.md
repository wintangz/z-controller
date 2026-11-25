# Verification Report: `polish-addon`

## Build, Automated Tests & Budgets
- `cargo test --workspace`: **PASS** (`24 passed, 0 failed`, 0 warnings: `z-controller` 10, `z-core` 10, `z-platform-windows` 4).
- `cargo build --release --workspace`: **PASS** (0 warnings).
- Isolated release tray (`T+3s` & `T+8s`): `WorkingSet64 = 16.85 MB` (`<= 25.0 MB`), `PrivateMemorySize64 = 3.92 MB`, 5-second idle CPU delta = `0.0156 s` (`0.02%` total system CPU).
- Post-test `probe_game_view.exe`: `[XInput] connected_slots=0`.

## Item-by-Item Results (1–3)

1. **Redraw Controller Diagram**: **PASS**
   - `apps/z-controller/src/diagram.rs` now draws symmetric cubic-Bezier controller shells triangulated cleanly with `ear_clip_polygon` for Xbox, Sony DualSense, and Nintendo Switch Pro (`bezier_body_silhouettes_triangulate_cleanly_for_all_families`, `diagram_renders_all_three_families_without_panic`).
   - Replaced all cramped text abbreviations (`"Xb"`, `"Vw"`, `"Mn"`, `"Shr"`, `"Cr"`, `"Op"`, `"Hm"`, `"Cap"`) with full vector symbols/glyphs (`Xbox X` / `View` overlapping squares / `Menu` lines / `Share` tray-arrow; `Touchpad` lightbar / `Create` 3-ray sunburst / `Options` lines / `PS` / `Mic Mute`; `Minus` / `Plus` / `Capture` circle-in-square / `Home` house), unified 12-vertex cross (`+`) D-Pad with chevrons, ringed sticks, and rounded shoulder/trigger bars.
   - Visually verified at 150% DPI in [screenshot-editor-xbox-redrawn.png](file:///y:/Repos/z-controller/specs/005-polish-addon/screenshot-editor-xbox-redrawn.png), [screenshot-editor-sony-redrawn.png](file:///y:/Repos/z-controller/specs/005-polish-addon/screenshot-editor-sony-redrawn.png), and [screenshot-editor-nintendo-redrawn.png](file:///y:/Repos/z-controller/specs/005-polish-addon/screenshot-editor-nintendo-redrawn.png).

2. **Fix Window-Drag Lag**: **PASS**
   - Persistent Win32 `CreateDIBSection` + `CreateCompatibleDC` double-buffered back buffer + single `BitBlt` presentation (`present_rect`), static offscreen chrome cache keyed by `static_ui_hash`, dirty-rect clipping, `WM_ERASEBKGND => 1`, and `WM_ENTERSIZEMOVE`..`WM_EXITSIZEMOVE` deferral of live-input updates applied to both Profile Editor and Compact Main windows.
   - **Proving numbers (Release, Native 125% DPI)**:
     - Editor OS/expose `RedrawWindow` repaint time: **`55.74 ms` -> `0.83 ms`** (67x faster; pure `BitBlt` from DIB back buffer).
     - Editor full uncached repaint time (forced via `WM_MOUSEMOVE`): **`55.74 ms` -> `47.24 ms`** (plus partial dirty-rect clipping on live pad updates).
     - Editor 10-second circular drag CPU (`WM_ENTERSIZEMOVE`..`WM_EXITSIZEMOVE`): **`5.594 s` (`55.94%` of 1 core / `3.50%` system) -> `0.328 s` (`3.28%` of 1 core / `0.20%` system)** — 17x reduction.
     - Compact Main 5-second circular drag CPU: **`0.203 s` (`4.06%` of 1 core)**.

3. **App Icon Resource**: **PASS**
   - Original neon squircle + gamepad + neon-green `"Z"` multi-size icon ([z-controller.ico](file:///y:/Repos/z-controller/apps/z-controller/assets/z-controller.ico)) compiled via [app.rc](file:///y:/Repos/z-controller/apps/z-controller/app.rc) & [build.rs](file:///y:/Repos/z-controller/apps/z-controller/build.rs) and loaded via `LoadImageW` in [tray_win32.rs](file:///y:/Repos/z-controller/apps/z-controller/src/tray_win32.rs).
   - Verified directly on `target\release\z-controller.exe`: `PerMonitorV2=True; IconGroupCount=8; Sizes=[16x16,20x20,24x24,32x32,48x48,64x64,128x128,256x256]`.
   - Showcase & window titlebar screenshots: [icon-preview.png](file:///y:/Repos/z-controller/specs/005-polish-addon/icon-preview.png), [screenshot-compact-icon.png](file:///y:/Repos/z-controller/specs/005-polish-addon/screenshot-compact-icon.png).
