# Feature Specification: `ui-fix-round`

**Feature Branch**: `004-ui-fix-round`  
**Slug**: `ui-fix-round`  
**Created**: 2026-11-25  
**Status**: Approved (Autonomous Spec Kit Run)

## Requirements

1. **Layout Tightening (`ui-fix-round`)**: Apply a consistent `4 / 8 / 12 / 16` px spacing scale across all margins, padding, gaps, and panel/card edges with aligned full-width containers and zero stray gaps or protruding dividers.
2. **Profile Editor with `x360ce`-Style Controller Diagram (`ui-fix-round`)**: Rebuild the Profile Editor around a full software-painted controller diagram in the neon theme (body, left/right sticks, D-Pad, face buttons, shoulders, triggers with analog fill, center cluster, and paddles) tailored per family (`Xbox`, `PlayStation`, `Nintendo` face-button labels/positions and stick/D-Pad layouts). Clicking a part selects it for remapping; physical input highlights its diagram part live. Synchronized text mapping list sits beside the diagram.
3. **Profile Selector Bug Fix (`ui-fix-round`)**: Fix both root causes breaking profile `ComboBox` selection (Win32 mouse-down/up synchronous paint separation + `UpdateWindow` + bounded follow-up popup paint in `tray_win32.rs`, and explicit click-driven selection in `ui.rs`). Prove end-to-end with two profiles having a distinct mapping that flipping the selector switches active output.
4. **Clean Default Profiles (`ui-fix-round`)**: Delete all game-named shipped defaults (`"Xbox 360 Default (FIFA)"` / `x360-fifa`, `ds4-default`). Ship **exactly 3 built-in 1:1 defaults**:
   - `"Xbox 360"` (`id: "xbox-360"`, `Xbox` family, `Xbox360` target)
   - `"DualSense"` (`id: "dualsense"`, `Sony` family, `DualShock4` virtual target — ViGEmBus has no true virtual DualSense)
   - `"Nintendo Switch Pro Controller"` (`id: "switch-pro"`, `Nintendo` family, `Xbox360` target, Nintendo layout preserved, `swap_ab_xy = false`)
5. **Remove Live Input Test Tab (`ui-fix-round`)**: Remove the redundant `Live Input Test` tab completely; each controller card's live status view is the single place for live input testing.
6. **Compact Main Window + Separate Profile Editor Window (`ui-fix-round`)**: Default window shrinks to compact controller cards + status bar (`~392x580`); clicking `Edit Profile` on any card opens the full diagram Profile Editor in a separate larger window (`~980x660`), whose close returns focus to the compact window without exiting the app.
7. **Misc Buttons for All Families (`ui-fix-round`)**: Track all 26 SDL3 gamepad buttons (`Misc1` Capture/Share/Mute, `Touchpad`, paddles `RightPaddle1`..`LeftPaddle2`, `Misc2..Misc6`), query `SDL_GamepadHasButton` for device capability detection, display exposed/family extra buttons in the live card view, and allow every extra button as a remap source (and `Touchpad` on DS4 output).

## Assumptions

- **A-001 (`DualSense` Virtual Output Target)**: Because ViGEmBus provides only virtual Xbox 360 (`045E:028E`) and DualShock 4 (`054C:05C4`) targets, the built-in `"DualSense"` profile uses the `Sony` layout family for diagram/labels and maps cleanly 1:1 to `VirtualTargetKind::DualShock4` without faking an unsupported DualSense kernel driver.
- **A-002 (Legacy Config Migration)**: Existing user configs referencing `"x360-fifa"` or `"ds4-default"` transparently migrate to `"xbox-360"` / `"dualsense"` during `AppConfig::ensure_valid()`.
- **A-003 (Automated Probe & Screenshots via Own Software)**: `--probe-profile-switch`, `--show-editor`, and `--live-demo-pad` exercise the real SDL3 -> `EngineHandle` -> `ViGEmBus` -> Win32 UI stack using `SdlVirtualGamepad` and `probe_game_view.exe` with zero games and zero external installs.
