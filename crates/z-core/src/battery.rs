use serde::{Deserialize, Serialize};

use crate::model::ControllerFamily;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BatteryStatus {
    OnBattery,
    Charging,
    Charged,
    Wired,
    #[default]
    Unknown,
}

impl BatteryStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::OnBattery => "On Battery",
            Self::Charging => "Charging",
            Self::Charged => "Charged",
            Self::Wired => "Wired",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BatteryReading {
    pub percent: Option<u8>,
    pub status: BatteryStatus,
    pub source: &'static str,
}

impl BatteryReading {
    pub fn format_badge(&self) -> String {
        match (self.percent, self.status) {
            (_, BatteryStatus::Wired) => "Wired".to_string(),
            (Some(pct), BatteryStatus::Charging) => format!("{pct}% (Charging)"),
            (Some(pct), BatteryStatus::Charged) => format!("{pct}% (Charged)"),
            (Some(pct), BatteryStatus::OnBattery) => format!("{pct}%"),
            (Some(pct), _) => format!("{pct}%"),
            (None, status) => status.label().to_string(),
        }
    }
}

pub struct NintendoBatteryParser;

impl NintendoBatteryParser {
    /// Parses Nintendo Switch Pro / Joy-Con HID input reports `0x30` or `0x21`.
    /// Matches Valve / SDL3 `SDL_hidapi_switch.c` battery nibble logic:
    /// - `charging = (byte2 & 0x10) != 0`
    /// - `level = (byte2 & 0xE0) >> 4` (even steps 0..=8)
    /// - `percent = round((level / 8.0) * 100.0)`
    pub fn parse_switch_report(report: &[u8]) -> Option<BatteryReading> {
        if report.len() < 3 || (report[0] != 0x30 && report[0] != 0x21) {
            return None;
        }
        let b2 = report[2];
        let charging = (b2 & 0x10) != 0;
        let level = ((b2 & 0xE0) >> 4).min(8);
        let percent = ((level as f32 / 8.0) * 100.0).round() as u8;
        let status = if charging {
            if level >= 8 {
                BatteryStatus::Charged
            } else {
                BatteryStatus::Charging
            }
        } else {
            BatteryStatus::OnBattery
        };
        Some(BatteryReading {
            percent: Some(percent),
            status,
            source: "nintendo-hid",
        })
    }
}

pub struct SonyBatteryParser;

impl SonyBatteryParser {
    /// Parses DualShock 4 battery byte matching `SDL_hidapi_ps4.c`.
    pub fn parse_ds4_byte(uc_battery_level: u8) -> BatteryReading {
        let cable = (uc_battery_level & 0x10) != 0;
        let level = uc_battery_level & 0x0F;
        if cable {
            if level >= 11 {
                BatteryReading {
                    percent: Some(100),
                    status: BatteryStatus::Charged,
                    source: "sony-ds4-hid",
                }
            } else {
                BatteryReading {
                    percent: Some((level.saturating_mul(10).saturating_add(5)).min(100)),
                    status: BatteryStatus::Charging,
                    source: "sony-ds4-hid",
                }
            }
        } else {
            BatteryReading {
                percent: Some((level.saturating_mul(10).saturating_add(5)).min(100)),
                status: BatteryStatus::OnBattery,
                source: "sony-ds4-hid",
            }
        }
    }

    /// Parses DualSense battery byte matching `SDL_hidapi_ps5.c`.
    pub fn parse_dualsense_byte(uc_battery_level: u8) -> BatteryReading {
        let status_nibble = (uc_battery_level >> 4) & 0x0F;
        let level = uc_battery_level & 0x0F;
        if level == 0x0C {
            return BatteryReading {
                percent: None,
                status: BatteryStatus::Unknown,
                source: "sony-dualsense-hid",
            };
        }
        let pct = (level.saturating_mul(10).saturating_add(5)).min(100);
        match status_nibble {
            0 => BatteryReading {
                percent: Some(pct),
                status: BatteryStatus::OnBattery,
                source: "sony-dualsense-hid",
            },
            1 => BatteryReading {
                percent: Some(pct),
                status: BatteryStatus::Charging,
                source: "sony-dualsense-hid",
            },
            2 => BatteryReading {
                percent: Some(100),
                status: BatteryStatus::Charged,
                source: "sony-dualsense-hid",
            },
            _ => BatteryReading {
                percent: None,
                status: BatteryStatus::Unknown,
                source: "sony-dualsense-hid",
            },
        }
    }
}

pub struct XboxBatteryParser;

impl XboxBatteryParser {
    /// Maps XInput `XINPUT_BATTERY_INFORMATION` (`BATTERY_TYPE_*`, `BATTERY_LEVEL_*`).
    pub fn parse_xinput(battery_type: u8, battery_level: u8) -> BatteryReading {
        match battery_type {
            0x01 => BatteryReading {
                percent: Some(100),
                status: BatteryStatus::Wired,
                source: "xbox-xinput",
            },
            0x02 | 0x03 => {
                let pct = match battery_level {
                    0 => 0,
                    1 => 25,
                    2 => 50,
                    _ => 100,
                };
                BatteryReading {
                    percent: Some(pct),
                    status: BatteryStatus::OnBattery,
                    source: "xbox-xinput",
                }
            }
            _ => BatteryReading {
                percent: None,
                status: BatteryStatus::Unknown,
                source: "xbox-xinput",
            },
        }
    }
}

pub fn reconcile_battery(
    family: ControllerFamily,
    sdl_reading: BatteryReading,
    family_override: Option<BatteryReading>,
) -> BatteryReading {
    if sdl_reading.percent.is_some() || sdl_reading.status == BatteryStatus::Wired {
        let mut out = sdl_reading;
        if out.source.is_empty() {
            out.source = match family {
                ControllerFamily::Nintendo => "sdl3-nintendo",
                ControllerFamily::Sony => "sdl3-sony",
                ControllerFamily::Xbox => "sdl3-xbox",
                ControllerFamily::Generic => "sdl3-generic",
            };
        }
        return out;
    }
    family_override.unwrap_or(sdl_reading)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nintendo_switch_0x30_nibble_matches_steam_formula() {
        // 0x80 -> level 8, not charging -> 100% OnBattery
        let full = NintendoBatteryParser::parse_switch_report(&[0x30, 0x00, 0x80]).unwrap();
        assert_eq!(full.percent, Some(100));
        assert_eq!(full.status, BatteryStatus::OnBattery);

        // 0x60 -> level 6, not charging -> 75% OnBattery
        let med = NintendoBatteryParser::parse_switch_report(&[0x30, 0x00, 0x60]).unwrap();
        assert_eq!(med.percent, Some(75));
        assert_eq!(med.status, BatteryStatus::OnBattery);

        // 0x50 -> level 4 + charging bit 0x10 -> 50% Charging
        let chg = NintendoBatteryParser::parse_switch_report(&[0x30, 0x00, 0x50]).unwrap();
        assert_eq!(chg.percent, Some(50));
        assert_eq!(chg.status, BatteryStatus::Charging);

        // 0x90 -> level 8 + charging bit 0x10 -> 100% Charged
        let done = NintendoBatteryParser::parse_switch_report(&[0x21, 0x01, 0x90]).unwrap();
        assert_eq!(done.percent, Some(100));
        assert_eq!(done.status, BatteryStatus::Charged);

        // 0x20 -> level 2 -> 25%
        let low = NintendoBatteryParser::parse_switch_report(&[0x30, 0x00, 0x20]).unwrap();
        assert_eq!(low.percent, Some(25));
    }

    #[test]
    fn sony_ds4_and_dualsense_battery_parsers() {
        let ds4_bat = SonyBatteryParser::parse_ds4_byte(0x07);
        assert_eq!(ds4_bat.percent, Some(75));
        assert_eq!(ds4_bat.status, BatteryStatus::OnBattery);

        let ds4_full = SonyBatteryParser::parse_ds4_byte(0x1B);
        assert_eq!(ds4_full.percent, Some(100));
        assert_eq!(ds4_full.status, BatteryStatus::Charged);

        let ds5_chg = SonyBatteryParser::parse_dualsense_byte(0x14);
        assert_eq!(ds5_chg.percent, Some(45));
        assert_eq!(ds5_chg.status, BatteryStatus::Charging);

        let ds5_none = SonyBatteryParser::parse_dualsense_byte(0x0C);
        assert_eq!(ds5_none.status, BatteryStatus::Unknown);
    }

    #[test]
    fn xbox_xinput_battery_levels() {
        let wired = XboxBatteryParser::parse_xinput(0x01, 0);
        assert_eq!(wired.status, BatteryStatus::Wired);

        let med = XboxBatteryParser::parse_xinput(0x02, 2);
        assert_eq!(med.percent, Some(50));
        assert_eq!(med.status, BatteryStatus::OnBattery);
    }
}
