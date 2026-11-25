use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::model::{DisconnectPolicy, HardwareKey, LogicalButton, VirtualTargetKind};

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

fn default_true() -> bool {
    true
}

fn default_deadzone() -> f32 {
    0.10
}

impl Profile {
    pub fn default_x360() -> Self {
        Self {
            id: "x360-fifa".to_string(),
            name: "Xbox 360 Default (FIFA)".to_string(),
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

    pub fn default_ds4() -> Self {
        Self {
            id: "ds4-default".to_string(),
            name: "DualShock 4 Default".to_string(),
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerBinding {
    pub profile_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_profile_id")]
    pub default_profile_id: String,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub bindings: BTreeMap<String, ControllerBinding>,
}

fn default_profile_id() -> String {
    "x360-fifa".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_profile_id: default_profile_id(),
            profiles: vec![Profile::default_x360(), Profile::default_ds4()],
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
        if self.profiles.is_empty() {
            self.profiles = vec![Profile::default_x360(), Profile::default_ds4()];
        }
        if !self.profiles.iter().any(|p| p.id == self.default_profile_id) {
            self.default_profile_id = self.profiles[0].id.clone();
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

    pub fn set_device_binding(&mut self, key: &HardwareKey, profile_id: String, enabled: bool) {
        self.bindings.insert(
            key.canonical_id(),
            ControllerBinding {
                profile_id,
                enabled,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip_and_binding_resolution() {
        let mut cfg = AppConfig::default();
        let key = HardwareKey::new(0x057E, 0x2009, "NSP98b6b0d12a2c");
        cfg.set_device_binding(&key, "ds4-default".to_string(), true);
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
}
