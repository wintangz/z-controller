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
    Misc1,
    RightPaddle1,
    LeftPaddle1,
    RightPaddle2,
    LeftPaddle2,
    Touchpad,
    Misc2,
    Misc3,
    Misc4,
    Misc5,
    Misc6,
}

impl LogicalButton {
    pub const ALL: [LogicalButton; 26] = [
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
        Self::Misc1,
        Self::RightPaddle1,
        Self::LeftPaddle1,
        Self::RightPaddle2,
        Self::LeftPaddle2,
        Self::Touchpad,
        Self::Misc2,
        Self::Misc3,
        Self::Misc4,
        Self::Misc5,
        Self::Misc6,
    ];

    pub const STANDARD: [LogicalButton; 15] = [
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

    pub const EXTRA: [LogicalButton; 11] = [
        Self::Misc1,
        Self::Touchpad,
        Self::RightPaddle1,
        Self::LeftPaddle1,
        Self::RightPaddle2,
        Self::LeftPaddle2,
        Self::Misc2,
        Self::Misc3,
        Self::Misc4,
        Self::Misc5,
        Self::Misc6,
    ];

    /// Buttons that virtual ViGEm outputs (Xbox 360 / DualShock 4) can emit directly.
    pub const VIRTUAL_TARGETS: [LogicalButton; 16] = [
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
        Self::Touchpad,
    ];

    pub fn bit(self) -> u32 {
        1u32 << (self as u32)
    }

    pub fn is_extra(self) -> bool {
        !matches!(
            self,
            Self::South
                | Self::East
                | Self::West
                | Self::North
                | Self::LeftShoulder
                | Self::RightShoulder
                | Self::Back
                | Self::Start
                | Self::Guide
                | Self::LeftStick
                | Self::RightStick
                | Self::DpadUp
                | Self::DpadDown
                | Self::DpadLeft
                | Self::DpadRight
        )
    }

    pub fn short_label(self) -> &'static str {
        self.family_label(ControllerFamily::Generic)
    }

    pub fn family_label(self, family: ControllerFamily) -> &'static str {
        match (family, self) {
            (ControllerFamily::Xbox, Self::South) => "A (South)",
            (ControllerFamily::Xbox, Self::East) => "B (East)",
            (ControllerFamily::Xbox, Self::West) => "X (West)",
            (ControllerFamily::Xbox, Self::North) => "Y (North)",
            (ControllerFamily::Xbox, Self::LeftShoulder) => "LB",
            (ControllerFamily::Xbox, Self::RightShoulder) => "RB",
            (ControllerFamily::Xbox, Self::Back) => "View",
            (ControllerFamily::Xbox, Self::Start) => "Menu",
            (ControllerFamily::Xbox, Self::Guide) => "Xbox",
            (ControllerFamily::Xbox, Self::LeftStick) => "LS Click",
            (ControllerFamily::Xbox, Self::RightStick) => "RS Click",
            (ControllerFamily::Xbox, Self::Misc1) => "Share",

            (ControllerFamily::Sony, Self::South) => "Cross",
            (ControllerFamily::Sony, Self::East) => "Circle",
            (ControllerFamily::Sony, Self::West) => "Square",
            (ControllerFamily::Sony, Self::North) => "Triangle",
            (ControllerFamily::Sony, Self::LeftShoulder) => "L1",
            (ControllerFamily::Sony, Self::RightShoulder) => "R1",
            (ControllerFamily::Sony, Self::Back) => "Create / Share",
            (ControllerFamily::Sony, Self::Start) => "Options",
            (ControllerFamily::Sony, Self::Guide) => "PS",
            (ControllerFamily::Sony, Self::LeftStick) => "L3",
            (ControllerFamily::Sony, Self::RightStick) => "R3",
            (ControllerFamily::Sony, Self::Misc1) => "Mute / Mic",
            (ControllerFamily::Sony, Self::Touchpad) => "Touchpad Click",

            (ControllerFamily::Nintendo, Self::South) => "B (South)",
            (ControllerFamily::Nintendo, Self::East) => "A (East)",
            (ControllerFamily::Nintendo, Self::West) => "Y (West)",
            (ControllerFamily::Nintendo, Self::North) => "X (North)",
            (ControllerFamily::Nintendo, Self::LeftShoulder) => "L",
            (ControllerFamily::Nintendo, Self::RightShoulder) => "R",
            (ControllerFamily::Nintendo, Self::Back) => "- (Minus)",
            (ControllerFamily::Nintendo, Self::Start) => "+ (Plus)",
            (ControllerFamily::Nintendo, Self::Guide) => "Home",
            (ControllerFamily::Nintendo, Self::LeftStick) => "L3",
            (ControllerFamily::Nintendo, Self::RightStick) => "R3",
            (ControllerFamily::Nintendo, Self::Misc1) => "Capture",

            (_, Self::South) => "South (A/B)",
            (_, Self::East) => "East (B/A)",
            (_, Self::West) => "West (X/Y)",
            (_, Self::North) => "North (Y/X)",
            (_, Self::LeftShoulder) => "LB / L1",
            (_, Self::RightShoulder) => "RB / R1",
            (_, Self::Back) => "Back / -",
            (_, Self::Start) => "Start / +",
            (_, Self::Guide) => "Guide / Home",
            (_, Self::LeftStick) => "LS / L3",
            (_, Self::RightStick) => "RS / R3",
            (_, Self::DpadUp) => "D-Up",
            (_, Self::DpadDown) => "D-Down",
            (_, Self::DpadLeft) => "D-Left",
            (_, Self::DpadRight) => "D-Right",
            (_, Self::Misc1) => "Misc1 (Cap/Shr)",
            (_, Self::RightPaddle1) => "Paddle R4 (P1)",
            (_, Self::LeftPaddle1) => "Paddle L4 (P3)",
            (_, Self::RightPaddle2) => "Paddle R5 (P2)",
            (_, Self::LeftPaddle2) => "Paddle L5 (P4)",
            (_, Self::Touchpad) => "Touchpad Click",
            (_, Self::Misc2) => "Misc 2",
            (_, Self::Misc3) => "Misc 3",
            (_, Self::Misc4) => "Misc 4",
            (_, Self::Misc5) => "Misc 5",
            (_, Self::Misc6) => "Misc 6",
        }
    }

    pub fn family_chip(self, family: ControllerFamily) -> &'static str {
        match (family, self) {
            (ControllerFamily::Xbox, Self::South) => "A",
            (ControllerFamily::Xbox, Self::East) => "B",
            (ControllerFamily::Xbox, Self::West) => "X",
            (ControllerFamily::Xbox, Self::North) => "Y",
            (ControllerFamily::Xbox, Self::LeftShoulder) => "LB",
            (ControllerFamily::Xbox, Self::RightShoulder) => "RB",
            (ControllerFamily::Xbox, Self::Back) => "Vw",
            (ControllerFamily::Xbox, Self::Start) => "Mn",
            (ControllerFamily::Xbox, Self::Guide) => "Xb",
            (ControllerFamily::Xbox, Self::LeftStick) => "LS",
            (ControllerFamily::Xbox, Self::RightStick) => "RS",
            (ControllerFamily::Xbox, Self::Misc1) => "Shr",

            (ControllerFamily::Sony, Self::South) => "Cr",
            (ControllerFamily::Sony, Self::East) => "Ci",
            (ControllerFamily::Sony, Self::West) => "Sq",
            (ControllerFamily::Sony, Self::North) => "Tr",
            (ControllerFamily::Sony, Self::LeftShoulder) => "L1",
            (ControllerFamily::Sony, Self::RightShoulder) => "R1",
            (ControllerFamily::Sony, Self::Back) => "Sh",
            (ControllerFamily::Sony, Self::Start) => "Op",
            (ControllerFamily::Sony, Self::Guide) => "PS",
            (ControllerFamily::Sony, Self::LeftStick) => "L3",
            (ControllerFamily::Sony, Self::RightStick) => "R3",
            (ControllerFamily::Sony, Self::Misc1) => "Mic",
            (ControllerFamily::Sony, Self::Touchpad) => "TP",

            (ControllerFamily::Nintendo, Self::South) => "B",
            (ControllerFamily::Nintendo, Self::East) => "A",
            (ControllerFamily::Nintendo, Self::West) => "Y",
            (ControllerFamily::Nintendo, Self::North) => "X",
            (ControllerFamily::Nintendo, Self::LeftShoulder) => "L",
            (ControllerFamily::Nintendo, Self::RightShoulder) => "R",
            (ControllerFamily::Nintendo, Self::Back) => "-",
            (ControllerFamily::Nintendo, Self::Start) => "+",
            (ControllerFamily::Nintendo, Self::Guide) => "Hm",
            (ControllerFamily::Nintendo, Self::LeftStick) => "L3",
            (ControllerFamily::Nintendo, Self::RightStick) => "R3",
            (ControllerFamily::Nintendo, Self::Misc1) => "Cap",

            (_, Self::South) => "A",
            (_, Self::East) => "B",
            (_, Self::West) => "X",
            (_, Self::North) => "Y",
            (_, Self::LeftShoulder) => "LB",
            (_, Self::RightShoulder) => "RB",
            (_, Self::Back) => "Bk",
            (_, Self::Start) => "St",
            (_, Self::Guide) => "Gd",
            (_, Self::LeftStick) => "L3",
            (_, Self::RightStick) => "R3",
            (_, Self::DpadUp) => "Up",
            (_, Self::DpadDown) => "Dn",
            (_, Self::DpadLeft) => "Lt",
            (_, Self::DpadRight) => "Rt",
            (_, Self::Misc1) => "M1",
            (_, Self::RightPaddle1) => "R4",
            (_, Self::LeftPaddle1) => "L4",
            (_, Self::RightPaddle2) => "R5",
            (_, Self::LeftPaddle2) => "L5",
            (_, Self::Touchpad) => "TP",
            (_, Self::Misc2) => "M2",
            (_, Self::Misc3) => "M3",
            (_, Self::Misc4) => "M4",
            (_, Self::Misc5) => "M5",
            (_, Self::Misc6) => "M6",
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
