use crate::model::{
    Ds4OutputState, LogicalButton, PhysicalGamepadState, VirtualOutputReport, VirtualTargetKind,
    X360OutputState,
};
use crate::profile::Profile;

/// Scales radial stick coordinates smoothly from `[deadzone .. 1.0]` -> `[0.0 .. 1.0]`.
pub fn apply_deadzone_radial(x: i16, y: i16, deadzone: f32) -> (i16, i16) {
    let dz = deadzone.clamp(0.0, 0.90);
    let nx = (x as f32 / 32767.0).clamp(-1.0, 1.0);
    let ny = (y as f32 / 32767.0).clamp(-1.0, 1.0);
    let mag = (nx * nx + ny * ny).sqrt();
    if mag <= dz || mag <= 1e-6 {
        return (0, 0);
    }
    let scaled = if dz <= 1e-6 {
        mag.min(1.0)
    } else {
        ((mag - dz) / (1.0 - dz)).clamp(0.0, 1.0)
    };
    let factor = scaled / mag;
    let ox = (nx * factor * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
    let oy = (ny * factor * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
    (ox, oy)
}

#[inline]
pub fn invert_i16_saturating(v: i16) -> i16 {
    if v == i16::MIN { i16::MAX } else { -v }
}

/// Converts SDL3 signed axis (`-32768..=32767`, `-32768 = UP`, `0 = center`, `+32767 = DOWN`)
/// directly to DS4 unsigned axis (`0..=255`, `0 = UP`, `128 = center`, `255 = DOWN`).
#[inline]
pub fn i16_axis_to_u8(v: i16) -> u8 {
    (((v as i32) + 32768).clamp(0, 65535) >> 8) as u8
}

/// Converts SDL3 trigger (`0..=32767`) to `0..=255`.
#[inline]
pub fn trigger_i16_to_u8(v: i16) -> u8 {
    if v <= 0 {
        0
    } else {
        ((v as i32 * 255 + 16383) / 32767).clamp(0, 255) as u8
    }
}

fn swap_face_if_enabled(btn: LogicalButton, swap_ab_xy: bool) -> LogicalButton {
    if !swap_ab_xy {
        return btn;
    }
    match btn {
        LogicalButton::South => LogicalButton::East,
        LogicalButton::East => LogicalButton::South,
        LogicalButton::West => LogicalButton::North,
        LogicalButton::North => LogicalButton::West,
        other => other,
    }
}

pub fn resolve_output_buttons(state: &PhysicalGamepadState, profile: &Profile) -> u32 {
    let mut out_mask = 0u32;
    for src in LogicalButton::ALL {
        if !state.is_pressed(src) {
            continue;
        }
        let target = match profile.button_remap.get(&src) {
            Some(Some(mapped)) => Some(*mapped),
            Some(None) => None, // explicitly unbound
            None => Some(swap_face_if_enabled(src, profile.swap_ab_xy)),
        };
        if let Some(dst) = target {
            out_mask |= dst.bit();
        }
    }
    out_mask
}

pub fn map_physical_to_virtual(
    state: &PhysicalGamepadState,
    profile: &Profile,
) -> VirtualOutputReport {
    let btn_mask = resolve_output_buttons(state, profile);
    let pressed = |b: LogicalButton| (btn_mask & b.bit()) != 0;

    let (lx, ly) = apply_deadzone_radial(state.left_x, state.left_y, profile.left_deadzone);
    let (rx, ry) = apply_deadzone_radial(state.right_x, state.right_y, profile.right_deadzone);
    let lt = trigger_i16_to_u8(state.left_trigger);
    let rt = trigger_i16_to_u8(state.right_trigger);

    match profile.target {
        VirtualTargetKind::Xbox360 => {
            let mut buttons = 0u16;
            if pressed(LogicalButton::DpadUp) {
                buttons |= 0x0001;
            }
            if pressed(LogicalButton::DpadDown) {
                buttons |= 0x0002;
            }
            if pressed(LogicalButton::DpadLeft) {
                buttons |= 0x0004;
            }
            if pressed(LogicalButton::DpadRight) {
                buttons |= 0x0008;
            }
            if pressed(LogicalButton::Start) {
                buttons |= 0x0010;
            }
            if pressed(LogicalButton::Back) {
                buttons |= 0x0020;
            }
            if pressed(LogicalButton::LeftStick) {
                buttons |= 0x0040;
            }
            if pressed(LogicalButton::RightStick) {
                buttons |= 0x0080;
            }
            if pressed(LogicalButton::LeftShoulder) {
                buttons |= 0x0100;
            }
            if pressed(LogicalButton::RightShoulder) {
                buttons |= 0x0200;
            }
            if pressed(LogicalButton::Guide) {
                buttons |= 0x0400;
            }
            if pressed(LogicalButton::South) {
                buttons |= 0x1000;
            }
            if pressed(LogicalButton::East) {
                buttons |= 0x2000;
            }
            if pressed(LogicalButton::West) {
                buttons |= 0x4000;
            }
            if pressed(LogicalButton::North) {
                buttons |= 0x8000;
            }

            VirtualOutputReport::Xbox360(X360OutputState {
                buttons,
                left_trigger: lt,
                right_trigger: rt,
                thumb_lx: lx,
                // SDL3 LeftY/RightY use negative = UP; XInput uses positive = UP.
                thumb_ly: invert_i16_saturating(ly),
                thumb_rx: rx,
                thumb_ry: invert_i16_saturating(ry),
            })
        }
        VirtualTargetKind::DualShock4 => {
            let dpad = match (
                pressed(LogicalButton::DpadUp),
                pressed(LogicalButton::DpadRight),
                pressed(LogicalButton::DpadDown),
                pressed(LogicalButton::DpadLeft),
            ) {
                (true, false, false, false) => 0u16,
                (true, true, false, false) => 1u16,
                (false, true, false, false) => 2u16,
                (false, true, true, false) => 3u16,
                (false, false, true, false) => 4u16,
                (false, false, true, true) => 5u16,
                (false, false, false, true) => 6u16,
                (true, false, false, true) => 7u16,
                _ => 8u16, // Neutral (0x08) — 0 means D-pad North on DS4!
            };

            let mut buttons = dpad;
            if pressed(LogicalButton::West) {
                buttons |= 1 << 4; // SQUARE
            }
            if pressed(LogicalButton::South) {
                buttons |= 1 << 5; // CROSS
            }
            if pressed(LogicalButton::East) {
                buttons |= 1 << 6; // CIRCLE
            }
            if pressed(LogicalButton::North) {
                buttons |= 1 << 7; // TRIANGLE
            }
            if pressed(LogicalButton::LeftShoulder) {
                buttons |= 1 << 8;
            }
            if pressed(LogicalButton::RightShoulder) {
                buttons |= 1 << 9;
            }
            if lt >= 30 {
                buttons |= 1 << 10;
            }
            if rt >= 30 {
                buttons |= 1 << 11;
            }
            if pressed(LogicalButton::Back) {
                buttons |= 1 << 12; // SHARE
            }
            if pressed(LogicalButton::Start) {
                buttons |= 1 << 13; // OPTIONS
            }
            if pressed(LogicalButton::LeftStick) {
                buttons |= 1 << 14;
            }
            if pressed(LogicalButton::RightStick) {
                buttons |= 1 << 15;
            }

            let mut special = 0u8;
            if pressed(LogicalButton::Guide) {
                special |= 1 << 0; // PS
            }

            VirtualOutputReport::DualShock4(Ds4OutputState {
                thumb_lx: i16_axis_to_u8(lx),
                // DS4 uses 0 = UP, 128 = center, 255 = DOWN (same sign as SDL3).
                thumb_ly: i16_axis_to_u8(ly),
                thumb_rx: i16_axis_to_u8(rx),
                thumb_ry: i16_axis_to_u8(ry),
                buttons,
                special,
                trigger_left: lt,
                trigger_right: rt,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_deadzone_zeroes_inside_and_scales_outside() {
        assert_eq!(apply_deadzone_radial(2000, -2000, 0.15), (0, 0));
        let (x, y) = apply_deadzone_radial(0, -32768, 0.10);
        assert_eq!(x, 0);
        assert!(y <= -32766);

        // Continuity just outside 10% deadzone should be small, not jump to 10%
        let (sx, sy) = apply_deadzone_radial(4000, 0, 0.10);
        assert_eq!(sy, 0);
        assert!(sx > 0 && sx < 2000, "unexpected jump sx={sx}");
    }

    #[test]
    fn x360_inverts_y_safely_and_ds4_preserves_y_and_neutral_dpad() {
        let mut profile_x = Profile::default_x360();
        profile_x.left_deadzone = 0.0;
        profile_x.right_deadzone = 0.0;

        let mut phys = PhysicalGamepadState {
            left_x: 0,
            left_y: i16::MIN, // Physical stick pushed UP
            right_x: 0,
            right_y: 32767, // Physical right stick pushed DOWN
            ..Default::default()
        };
        phys.set_button(LogicalButton::South, true);

        let VirtualOutputReport::Xbox360(xrep) = map_physical_to_virtual(&phys, &profile_x) else {
            panic!("expected Xbox360");
        };
        assert_eq!(xrep.thumb_ly, i16::MAX, "SDL UP (-32768) must map to X360 +32767");
        assert_eq!(xrep.thumb_ry, -32767, "SDL DOWN (+32767) must map to X360 -32767");
        assert_eq!(xrep.buttons, 0x1000); // A

        let mut profile_ds4 = Profile::default_ds4();
        profile_ds4.left_deadzone = 0.0;
        profile_ds4.right_deadzone = 0.0;
        let VirtualOutputReport::DualShock4(drep) = map_physical_to_virtual(&phys, &profile_ds4)
        else {
            panic!("expected DualShock4");
        };
        assert_eq!(drep.thumb_ly, 0, "SDL UP must map to DS4 0 (UP)");
        assert_eq!(drep.thumb_ry, 255, "SDL DOWN must map to DS4 255 (DOWN)");
        assert_eq!(drep.thumb_lx, 128);
        assert_eq!(drep.buttons & 0x000F, 8, "DS4 released D-Pad must be 8, never 0");
        assert_ne!(drep.buttons & (1 << 5), 0); // CROSS
    }

    #[test]
    fn swap_ab_xy_and_button_remap() {
        let mut p = Profile::default_x360();
        p.swap_ab_xy = true;
        p.button_remap.insert(LogicalButton::Back, Some(LogicalButton::Guide));
        p.button_remap.insert(LogicalButton::Start, None); // unbound

        let mut phys = PhysicalGamepadState::default();
        phys.set_button(LogicalButton::South, true);
        phys.set_button(LogicalButton::Back, true);
        phys.set_button(LogicalButton::Start, true);

        let VirtualOutputReport::Xbox360(rep) = map_physical_to_virtual(&phys, &p) else {
            panic!("expected x360");
        };
        assert_ne!(rep.buttons & 0x2000, 0, "South swapped to East (B)");
        assert_eq!(rep.buttons & 0x1000, 0, "A must not be pressed");
        assert_ne!(rep.buttons & 0x0400, 0, "Back remapped to Guide");
        assert_eq!(rep.buttons & 0x0010, 0, "Start unbound");
    }
}
