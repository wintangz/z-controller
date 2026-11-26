use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::driver::DriverId;
use crate::model::{
    ControllerFamily, DisconnectPolicy, HardwareKey, LogicalButton, VirtualTargetKind,
};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize config TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default = "default_family")]
    pub family: ControllerFamily,
    pub target: VirtualTargetKind,
    #[serde(default = "default_true")]
    pub cloak_physical: bool,
    #[serde(default)]
    pub disconnect_policy: DisconnectPolicy,
    #[serde(default)]
    pub swap_ab_xy: bool,
    #[serde(default = "default_deadzone")]
    pub left_deadzone: f32,
    #[serde(default = "default_deadzone")]
    pub right_deadzone: f32,
    #[serde(default = "default_true")]
    pub rumble_enabled: bool,
    #[serde(default)]
    pub button_remap: BTreeMap<LogicalButton, Option<LogicalButton>>,
}

fn default_family() -> ControllerFamily {
    ControllerFamily::Xbox
}

fn default_true() -> bool {
    true
}

fn default_deadzone() -> f32 {
    0.10
}

impl Profile {
    pub fn builtin_xbox360() -> Self {
        Self {
            id: "xbox-360".to_string(),
            name: "Xbox 360".to_string(),
            family: ControllerFamily::Xbox,
            target: VirtualTargetKind::Xbox360,
            cloak_physical: true,
            disconnect_policy: DisconnectPolicy::KeepNeutral,
            swap_ab_xy: false,
            left_deadzone: 0.10,
            right_deadzone: 0.10,
            rumble_enabled: true,
            button_remap: BTreeMap::new(),
        }
    }

    pub fn builtin_dualsense() -> Self {
        Self {
            id: "dualsense".to_string(),
            name: "DualSense".to_string(),
            family: ControllerFamily::Sony,
            target: VirtualTargetKind::DualShock4,
            cloak_physical: true,
            disconnect_policy: DisconnectPolicy::KeepNeutral,
            swap_ab_xy: false,
            left_deadzone: 0.10,
            right_deadzone: 0.10,
            rumble_enabled: true,
            button_remap: BTreeMap::new(),
        }
    }

    pub fn builtin_switch_pro() -> Self {
        Self {
            id: "switch-pro".to_string(),
            name: "Nintendo Switch Pro Controller".to_string(),
            family: ControllerFamily::Nintendo,
            target: VirtualTargetKind::Xbox360,
            cloak_physical: true,
            disconnect_policy: DisconnectPolicy::KeepNeutral,
            swap_ab_xy: false,
            left_deadzone: 0.10,
            right_deadzone: 0.10,
            rumble_enabled: true,
            button_remap: BTreeMap::new(),
        }
    }

    pub fn is_builtin_id(id: &str) -> bool {
        matches!(id, "xbox-360" | "dualsense" | "switch-pro")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerBinding {
    pub profile_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub show_live_input: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_profile_id")]
    pub default_profile_id: String,
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub vigem_located_dir: Option<String>,
    #[serde(default)]
    pub hidhide_located_dir: Option<String>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub bindings: BTreeMap<String, ControllerBinding>,
}

fn default_profile_id() -> String {
    "xbox-360".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_profile_id: default_profile_id(),
            theme_mode: ThemeMode::Dark,
            vigem_located_dir: None,
            hidhide_located_dir: None,
            profiles: vec![
                Profile::builtin_xbox360(),
                Profile::builtin_dualsense(),
                Profile::builtin_switch_pro(),
            ],
            bindings: BTreeMap::new(),
        }
    }
}

impl AppConfig {
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join("z-controller").join("config.toml")
    }

    pub fn load_or_default(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(raw) => toml::from_str::<Self>(&raw)
                .map(|mut cfg| {
                    cfg.ensure_valid();
                    cfg
                })
                .unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let encoded = toml::to_string_pretty(self)?;
        let tmp = path.with_extension(format!("toml.tmp.{}", std::process::id()));
        std::fs::write(&tmp, encoded)?;
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn ensure_valid(&mut self) {
        // Migrate legacy v1 default IDs to clean built-in defaults.
        for b in self.bindings.values_mut() {
            if b.profile_id == "x360-fifa" {
                b.profile_id = "xbox-360".to_string();
            } else if b.profile_id == "ds4-default" {
                b.profile_id = "dualsense".to_string();
            }
        }
        if self.default_profile_id == "x360-fifa" {
            self.default_profile_id = "xbox-360".to_string();
        } else if self.default_profile_id == "ds4-default" {
            self.default_profile_id = "dualsense".to_string();
        }
        self.profiles
            .retain(|p| p.id != "x360-fifa" && p.id != "ds4-default");

        for builtin in [
            Profile::builtin_xbox360(),
            Profile::builtin_dualsense(),
            Profile::builtin_switch_pro(),
        ] {
            if !self.profiles.iter().any(|p| p.id == builtin.id) {
                self.profiles.push(builtin);
            }
        }
        if !self
            .profiles
            .iter()
            .any(|p| p.id == self.default_profile_id)
        {
            self.default_profile_id = "xbox-360".to_string();
        }
        for p in &mut self.profiles {
            p.left_deadzone = p.left_deadzone.clamp(0.0, 0.50);
            p.right_deadzone = p.right_deadzone.clamp(0.0, 0.50);
        }
    }

    pub fn profile_by_id(&self, id: &str) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == id)
            .or_else(|| self.profiles.iter().find(|p| p.id == self.default_profile_id))
            .unwrap_or(&self.profiles[0])
    }

    pub fn profile_for_device(&self, key: &HardwareKey) -> (bool, &Profile) {
        if let Some(binding) = self.bindings.get(&key.canonical_id()) {
            (binding.enabled, self.profile_by_id(&binding.profile_id))
        } else {
            (true, self.profile_by_id(&self.default_profile_id))
        }
    }

    pub fn show_live_input_for_device(&self, key: &HardwareKey) -> bool {
        self.bindings
            .get(&key.canonical_id())
            .is_none_or(|b| b.show_live_input)
    }

    pub fn set_device_binding(&mut self, key: &HardwareKey, profile_id: String, enabled: bool) {
        let show_live_input = self.show_live_input_for_device(key);
        self.bindings.insert(
            key.canonical_id(),
            ControllerBinding {
                profile_id,
                enabled,
                show_live_input,
            },
        );
    }

    pub fn set_device_show_live_input(&mut self, key: &HardwareKey, show_live_input: bool) {
        let (enabled, profile) = self.profile_for_device(key);
        let profile_id = profile.id.clone();
        self.bindings.insert(
            key.canonical_id(),
            ControllerBinding {
                profile_id,
                enabled,
                show_live_input,
            },
        );
    }

    pub fn located_dir(&self, driver: DriverId) -> Option<&str> {
        match driver {
            DriverId::ViGEmBus => self.vigem_located_dir.as_deref(),
            DriverId::HidHide => self.hidhide_located_dir.as_deref(),
        }
    }

    pub fn set_located_dir(&mut self, driver: DriverId, dir: Option<String>) {
        match driver {
            DriverId::ViGEmBus => self.vigem_located_dir = dir,
            DriverId::HidHide => self.hidhide_located_dir = dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ships_exact_three_clean_builtin_profiles() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.profiles.len(), 3);
        assert_eq!(cfg.profiles[0].id, "xbox-360");
        assert_eq!(cfg.profiles[0].name, "Xbox 360");
        assert_eq!(cfg.profiles[0].target, VirtualTargetKind::Xbox360);
        assert!(!cfg.profiles[0].swap_ab_xy);

        assert_eq!(cfg.profiles[1].id, "dualsense");
        assert_eq!(cfg.profiles[1].name, "DualSense");
        assert_eq!(cfg.profiles[1].family, ControllerFamily::Sony);
        assert_eq!(cfg.profiles[1].target, VirtualTargetKind::DualShock4);
        assert!(!cfg.profiles[1].swap_ab_xy);

        assert_eq!(cfg.profiles[2].id, "switch-pro");
        assert_eq!(cfg.profiles[2].name, "Nintendo Switch Pro Controller");
        assert_eq!(cfg.profiles[2].family, ControllerFamily::Nintendo);
        assert!(!cfg.profiles[2].swap_ab_xy);
    }

    #[test]
    fn config_round_trip_and_binding_resolution() {
        let mut cfg = AppConfig::default();
        let key = HardwareKey::new(0x057E, 0x2009, "NSP98b6b0d12a2c");
        cfg.set_device_binding(&key, "dualsense".to_string(), true);
        let dir = std::env::temp_dir().join(format!("z_ctrl_cfg_test_{}", std::process::id()));
        let file = dir.join("config.toml");
        cfg.save_atomic(&file).unwrap();

        let loaded = AppConfig::load_or_default(&file);
        assert_eq!(loaded, cfg);
        let (enabled, p) = loaded.profile_for_device(&key);
        assert!(enabled);
        assert_eq!(p.target, VirtualTargetKind::DualShock4);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn show_live_input_defaults_expanded_and_persists_across_restarts() {
        let mut cfg = AppConfig::default();
        let key = HardwareKey::new(0x057E, 0x2009, "NSP-COLLAPSE-TEST");
        assert!(cfg.show_live_input_for_device(&key));

        // Collapsing preserves enabled/profile defaults and survives save/reload.
        cfg.set_device_show_live_input(&key, false);
        assert!(!cfg.show_live_input_for_device(&key));
        assert!(cfg.profile_for_device(&key).0);

        // Changing profile/enabled preserves show_live_input=false.
        cfg.set_device_binding(&key, "switch-pro".to_string(), false);
        assert!(!cfg.show_live_input_for_device(&key));

        let dir = std::env::temp_dir().join(format!("z_ctrl_collapse_test_{}", std::process::id()));
        let file = dir.join("config.toml");
        cfg.save_atomic(&file).unwrap();

        let reloaded = AppConfig::load_or_default(&file);
        assert!(!reloaded.show_live_input_for_device(&key));
        let (enabled, prof) = reloaded.profile_for_device(&key);
        assert!(!enabled);
        assert_eq!(prof.id, "switch-pro");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn theme_mode_defaults_to_dark_and_persists_across_restarts() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.theme_mode, ThemeMode::Dark);
        assert_eq!(cfg.theme_mode.toggled(), ThemeMode::Light);

        cfg.theme_mode = ThemeMode::Light;
        let dir = std::env::temp_dir().join(format!("z_ctrl_theme_test_{}", std::process::id()));
        let file = dir.join("config.toml");
        cfg.save_atomic(&file).unwrap();

        let reloaded = AppConfig::load_or_default(&file);
        assert_eq!(reloaded.theme_mode, ThemeMode::Light);

        // Legacy config missing theme_mode loads as Dark.
        let legacy_toml = "default_profile_id = \"xbox-360\"\n";
        std::fs::write(&file, legacy_toml).unwrap();
        let legacy = AppConfig::load_or_default(&file);
        assert_eq!(legacy.theme_mode, ThemeMode::Dark);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
