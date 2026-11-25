# Feature Specification: `polish-addon`

**Branch / Context**: `main` (after `6e1857b`)

## Scope
Three targeted polish improvements with zero new crates, zero external image assets in runtime rendering, zero installs, and all existing RAM/CPU budgets preserved (`tray WorkingSet <= 25 MB`, `idle CPU ~ 0`, `cargo test --workspace` passing).

1. **Redraw Controller Diagram**: Replace the balloon-circle body with smooth symmetric cubic-Bezier controller silhouettes tailored per family (`Xbox`, `Sony DualSense`, `Nintendo Switch Pro`). Remove abbreviated center labels (`"Xb"`, `"Vw"`, `"Mn"`, `"Shr"`, `"Cr"`, `"Op"`, `"Hm"`, `"Cap"`) in favor of full symbols/vector glyphs (`Xbox` X / View / Menu / Share, `PlayStation` Touchpad + Create / Options / PS / Mic, `Nintendo` `-` / `+` / Capture / Home). Render ringed sticks with center dot, unified plus (`+`) D-pad, uniform circular face buttons with family symbols, and rounded shoulder/trigger bars. Keep software `egui::Painter` vector rendering only.
2. **Fix Window-Drag Lag**: Eliminate ~56 ms full-frame CPU rasterization during window drags in both the Profile Editor and Compact Main windows by backing `SoftwarePainter` with a persistent Win32 `CreateDIBSection` + `CreateCompatibleDC` double-buffered back buffer, adding an axis-aligned solid-quad fast path + static offscreen cache + dirty-rect partial repainting, suppressing live-highlight repaints during `WM_ENTERSIZEMOVE..WM_EXITSIZEMOVE`, and blitting via a single `BitBlt` on `WM_PAINT`.
3. **App Icon Resource**: Create an original multi-size `.ico` (`16, 20, 24, 32, 48, 64, 128, 256`) matching `z-controller`'s neon-on-black identity (`#0A0A10`/`#14141F` squircle, `#A970FF`->`#FF5CA8` border, `#3DFF7A` neon green gamepad/"Z" emblem) and embed it via Win32 `RT_GROUP_ICON #1` (`app.rc`) so Explorer, taskbar, window title bars, and tray icon all display it. Save `icon-preview.png` for review.
