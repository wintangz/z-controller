# Feature Specification: `ui-sync-1`

**Feature Branch**: `main` (building on `7fbe785`, superseding `driver-ux-fix`)
**Created**: 2026-11-26
**Status**: Active

## Requirements
- **FIX 1 — WT Header Lockup**:
  - Embed the canonical unmodified `assets/logo-mark.png` via `include_bytes!`.
  - First in-window row becomes the app masthead: 24px WT logo mark on the left of `"z-controller"` (`16.0pt` Regular), followed by short muted functional caption `"Virtual pad & cloak"`, with ThemeToggle (`28x28`) and `Refresh` moved to the right end of the masthead row.
  - `"Controllers"` and `"Drivers"` remain section headings below in section treatment.
- **FIX 2 — `DESIGN.md` Part I Shared Core Compliance**:
  - Adopt `DESIGN.md` at repo root and implement Part I (`§1`–`§9`) unchanged: Dark/Light surface & status tokens, `Segoe UI` / `Segoe UI Semibold` (`ProportionalStrong` for identities & modal titles only) + `Consolas Regular` (`FontFamily::Monospace`) for numeric readouts (`RAM`, versions, `VID:PID`, battery/trigger `%`), `>= 10.5pt` floor everywhere (`FC7`), `4/8/12/16` spacing scale, differentiated corner radii (`9px` full-pill non-interactive badges, `8px` panels/rims, `6px` outline buttons/inputs/toggles, `4px` checkboxes/chips), `§5` active gradient rim, `§6` `PrimaryOutline` / `SecondaryOutline` / `SubtleOutline` / `Active-Highlight` buttons with crisp `1px` perimeter rings and visible hover/pressed states in both modes, `§7` chip discipline, `§8` saturated status discipline.
- **FIX 3 — Driver Cards and Section Behaviour**:
  - `Working` driver cards show NO action buttons (`name`, `REQUIRED`/`OPTIONAL` chip, publisher, version, `Status: Working` — nothing else).
  - `"I already have this — Locate…"` and `"Reinstall (Later Option)…"` render ONLY in non-Working states.
  - When all drivers report `Working`, the `Drivers` section is hidden by default and the main window shrinks to hug remaining content; it auto-appears expanded if any driver becomes non-Working, with a manual icon chevron toggle in the status bar and `Drivers` header.
- **FIX 4 — App Icon = Original WT Mark**:
  - Replace the application icon (`1 ICON`) with the original unmodified WT mark generated from `assets/logo-mark.png` across sizes `16/20/24/32/40/48/64/128/256` (`apps/z-controller/assets/wt-app.ico`), using the exact trimmed `1.04x` framing matching `y-storage`.
  - Applies everywhere the app icon appears: exe file icon, window title bar, taskbar button, Alt-Tab, and system tray icon.
  - Keep `apps/z-controller/assets/z-controller.ico` (the fused WT + controller asset) untouched in the repo.

