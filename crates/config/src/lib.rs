use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use unvet_core::{AppError, AppResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputSource {
 IfacialmocapUdp,
 IfacialmocapTcp,
}

impl Default for InputSource {
 fn default() -> Self {
  Self::IfacialmocapUdp
 }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputBackendKind {
 Ets2,
 Mouse,
 Keyboard,
}

impl Default for OutputBackendKind {
 fn default() -> Self {
  Self::Ets2
 }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct InputConfig {
 pub source: InputSource,
 pub host: String,
 pub udp_port: u16,
 pub tcp_port: u16,
}

impl Default for InputConfig {
 fn default() -> Self {
  Self {
   source: InputSource::default(),
   host: "127.0.0.1".to_owned(),
   udp_port: 49983,
   tcp_port: 49986,
  }
 }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OutputConfig {
 pub backend: OutputBackendKind,
 pub enabled: bool,
}

impl Default for OutputConfig {
 fn default() -> Self {
  Self {
   backend: OutputBackendKind::default(),
   enabled: true,
  }
 }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MappingConfig {
 pub smoothing_alpha: f32,
 pub deadzone_percent: f32,
 pub eye_head_mix_yaw: f32,
 pub eye_head_mix_pitch: f32,
}

impl Default for MappingConfig {
 fn default() -> Self {
  Self {
   smoothing_alpha: 0.18,
   deadzone_percent: 0.06,
   eye_head_mix_yaw: 0.7,
   eye_head_mix_pitch: 0.4,
  }
 }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RuntimeConfig {
 pub pause_on_unfocused: bool,
 pub hotkey_toggle: String,
 pub hotkey_recalibrate: String,
}

impl Default for RuntimeConfig {
 fn default() -> Self {
  Self {
   pause_on_unfocused: true,
   hotkey_toggle: "Ctrl+Shift+E".to_owned(),
   hotkey_recalibrate: "Ctrl+Shift+R".to_owned(),
  }
 }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AppConfig {
 pub input: InputConfig,
 pub output: OutputConfig,
 pub mapping: MappingConfig,
 pub runtime: RuntimeConfig,
}

impl AppConfig {
 pub fn load_or_default(path: &Path) -> AppResult<Self> {
  if !path.exists() {
   return Ok(Self::default());
  }

  let raw = fs::read_to_string(path)?;
  Self::from_toml(&raw).map_err(|err| AppError::Config(format!("{} ({})", err, path.display())))
 }

 pub fn from_toml(raw: &str) -> AppResult<Self> {
  toml::from_str(raw).map_err(|err| AppError::Config(format!("failed to parse config: {err}")))
 }

 pub fn save_to_path(&self, path: &Path) -> AppResult<()> {
  if let Some(parent) = path.parent() {
   if !parent.as_os_str().is_empty() {
    fs::create_dir_all(parent)?;
   }
  }

  let content = toml::to_string_pretty(self).map_err(|err| AppError::Config(format!("failed to serialize config: {err}")))?;
  fs::write(path, content)?;
  Ok(())
 }
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn default_config_roundtrip() {
  let source = toml::to_string_pretty(&AppConfig::default()).expect("serialize default config");
  let parsed = AppConfig::from_toml(&source).expect("parse serialized config");

  assert_eq!(parsed, AppConfig::default());
 }
}
