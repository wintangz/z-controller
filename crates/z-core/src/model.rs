use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HardwareKey {
    pub vid: u16,
    pub pid: u16,
    pub discriminator: String,
}

impl HardwareKey {
    pub fn new(vid: u16, pid: u16, discriminator: impl Into<String>) -> Self {
        Self {
            vid,
            pid,
            discriminator: discriminator.into(),
        }
    }

    pub fn canonical_id(&self) -> String {
        format!("{:04x}:{:04x}:{}", self.vid, self.pid, self.discriminator)
    }
}

impl fmt::Display for HardwareKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04X}:{:04X} ({})", self.vid, self.pid, self.discriminator)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ControllerFamily {
    Nintendo,
    Sony,
    Xbox,
    Generic,
}

impl ControllerFamily {
    pub fn detect(vid: u16, pid: u16, name: &str) -> Self {
        match vid {
            0x057E => Self::Nintendo,
            0x054C => Self::Sony,
            0x045E => Self::Xbox,
            _ => {
                let lower = name.to_ascii_lowercase();
                if lower.contains("nintendo") || lower.contains("switch") || lower.contains("pro controller") {
                    Self::Nintendo
                } else if lower.contains("dualshock") || lower.contains("dualsense") || lower.contains("ps4") || lower.contains("ps5") {
                    Self::Sony
                } else if lower.contains("xbox") || (vid == 0x045E && pid != 0) {
                    Self::Xbox
                } else {
                    Self::Generic
                }
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Nintendo => "Nintendo",
            Self::Sony => "Sony",
            Self::Xbox => "Xbox",
            Self::Generic => "Generic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Transport {
    Bluetooth,
    Usb,
    Unknown,
}

impl Transport {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bluetooth => "BT",
            Self::Usb => "USB",
            Self::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LogicalButton {
    South,
    East,
    West,
    North,
    LeftShoulder,
    RightShoulder,
    Back,
    Start,
    Guide,
    LeftStick,
    RightStick,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

impl LogicalButton {
    pub const ALL: [LogicalButton; 15] = [
        Self::South,
        Self::East,
        Self::West,
        Self::North,
        Self::LeftShoulder,
        Self::RightShoulder,
        Self::Back,
        Self::Start,
        Self::Guide,
        Self::LeftStick,
        Self::RightStick,
        Self::DpadUp,
        Self::DpadDown,
        Self::DpadLeft,
        Self::DpadRight,
    ];

    pub fn bit(self) -> u32 {
        1u32 << (self as u32)
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::South => "South (A/B)",
            Self::East => "East (B/A)",
            Self::West => "West (X/Y)",
            Self::North => "North (Y/X)",
            Self::LeftShoulder => "LB / L1",
            Self::RightShoulder => "RB / R1",
            Self::Back => "Back / -",
            Self::Start => "Start / +",
            Self::Guide => "Guide / Home",
            Self::LeftStick => "LS / L3",
            Self::RightStick => "RS / R3",
            Self::DpadUp => "D-Up",
            Self::DpadDown => "D-Down",
            Self::DpadLeft => "D-Left",
            Self::DpadRight => "D-Right",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalGamepadState {
    pub buttons_mask: u32,
    pub left_x: i16,
    pub left_y: i16,
    pub right_x: i16,
    pub right_y: i16,
    /// `0..=32767` in SDL3 Gamepad axis space.
    pub left_trigger: i16,
    /// `0..=32767` in SDL3 Gamepad axis space.
    pub right_trigger: i16,
}

impl PhysicalGamepadState {
    pub fn is_pressed(&self, btn: LogicalButton) -> bool {
        (self.buttons_mask & btn.bit()) != 0
    }

    pub fn set_button(&mut self, btn: LogicalButton, down: bool) {
        if down {
            self.buttons_mask |= btn.bit();
        } else {
            self.buttons_mask &= !btn.bit();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VirtualTargetKind {
    Xbox360,
    DualShock4,
}

impl VirtualTargetKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Xbox360 => "Xbox 360",
            Self::DualShock4 => "DualShock 4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DisconnectPolicy {
    #[default]
    KeepNeutral,
    DropVirtual,
}

impl DisconnectPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::KeepNeutral => "Keep virtual pad at neutral",
            Self::DropVirtual => "Drop virtual pad",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct X360OutputState {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ds4OutputState {
    pub thumb_lx: u8,
    pub thumb_ly: u8,
    pub thumb_rx: u8,
    pub thumb_ry: u8,
    pub buttons: u16,
    pub special: u8,
    pub trigger_left: u8,
    pub trigger_right: u8,
}

impl Default for Ds4OutputState {
    fn default() -> Self {
        Self {
            thumb_lx: 128,
            thumb_ly: 128,
            thumb_rx: 128,
            thumb_ry: 128,
            buttons: 0x0008, // Ds4Dpad::Neutral
            special: 0,
            trigger_left: 0,
            trigger_right: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualOutputReport {
    Xbox360(X360OutputState),
    DualShock4(Ds4OutputState),
}

impl VirtualOutputReport {
    pub fn neutral(kind: VirtualTargetKind) -> Self {
        match kind {
            VirtualTargetKind::Xbox360 => Self::Xbox360(X360OutputState::default()),
            VirtualTargetKind::DualShock4 => Self::DualShock4(Ds4OutputState::default()),
        }
    }

    pub fn kind(&self) -> VirtualTargetKind {
        match self {
            Self::Xbox360(_) => VirtualTargetKind::Xbox360,
            Self::DualShock4(_) => VirtualTargetKind::DualShock4,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RumbleCommand {
    pub large_motor: u8,
    pub small_motor: u8,
}
