use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use unvet_core::{AppError, AppResult, calibration::CalibrationOffsets};

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MappingCurvePreset {
 Linear,
 Smooth,
 Aggressive,
}

impl Default for MappingCurvePreset {
 fn default() -> Self {
  Self::Linear
 }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MappingBlendPreset {
 Custom,
 Balanced,
 EyeDominant,
 HeadDominant,
}

impl Default for MappingBlendPreset {
 fn default() -> Self {
  Self::Custom
 }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MappingProfile {
 Global,
 Ets2,
 Ats,
}

impl Default for MappingProfile {
 fn default() -> Self {
  Self::Global
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
 pub yaw_sensitivity: f32,
 pub pitch_sensitivity: f32,
 pub response_curve_preset: MappingCurvePreset,
 pub head_eye_blend_preset: MappingBlendPreset,
 pub eye_head_mix_yaw: f32,
 pub eye_head_mix_pitch: f32,
}

impl Default for MappingConfig {
 fn default() -> Self {
  Self {
   smoothing_alpha: 0.18,
   deadzone_percent: 0.06,
   yaw_sensitivity: 1.0,
   pitch_sensitivity: 1.0,
   response_curve_preset: MappingCurvePreset::default(),
   head_eye_blend_preset: MappingBlendPreset::default(),
   eye_head_mix_yaw: 0.7,
   eye_head_mix_pitch: 0.4,
  }
 }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MappingProfilesConfig {
 pub active: MappingProfile,
 pub ets2: MappingConfig,
 pub ats: MappingConfig,
}

impl Default for MappingProfilesConfig {
 fn default() -> Self {
  let mut ets2 = MappingConfig::default();
  ets2.head_eye_blend_preset = MappingBlendPreset::Balanced;
  ets2.eye_head_mix_yaw = 0.72;
  ets2.eye_head_mix_pitch = 0.45;

  let mut ats = MappingConfig::default();
  ats.head_eye_blend_preset = MappingBlendPreset::Balanced;
  ats.eye_head_mix_yaw = 0.68;
  ats.eye_head_mix_pitch = 0.42;
  ats.smoothing_alpha = 0.2;
  ats.deadzone_percent = 0.07;

  Self {
   active: MappingProfile::default(),
   ets2,
   ats,
  }
 }
}

impl MappingProfilesConfig {
 pub fn mapping_for(&self, profile: MappingProfile) -> Option<&MappingConfig> {
  match profile {
   MappingProfile::Global => None,
   MappingProfile::Ets2 => Some(&self.ets2),
   MappingProfile::Ats => Some(&self.ats),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CalibrationConfig {
 pub enabled: bool,
 pub capture_on_start: bool,
 pub calibrated: bool,
 pub head_yaw_offset_deg: f32,
 pub head_pitch_offset_deg: f32,
 pub head_roll_offset_deg: f32,
 pub eye_yaw_offset_deg: f32,
 pub eye_pitch_offset_deg: f32,
 pub left_eye_yaw_offset_deg: f32,
 pub left_eye_pitch_offset_deg: f32,
 pub right_eye_yaw_offset_deg: f32,
 pub right_eye_pitch_offset_deg: f32,
}

impl Default for CalibrationConfig {
 fn default() -> Self {
  Self {
   enabled: true,
   capture_on_start: false,
   calibrated: false,
   head_yaw_offset_deg: 0.0,
   head_pitch_offset_deg: 0.0,
   head_roll_offset_deg: 0.0,
   eye_yaw_offset_deg: 0.0,
   eye_pitch_offset_deg: 0.0,
   left_eye_yaw_offset_deg: 0.0,
   left_eye_pitch_offset_deg: 0.0,
   right_eye_yaw_offset_deg: 0.0,
   right_eye_pitch_offset_deg: 0.0,
  }
 }
}

impl CalibrationConfig {
 pub fn offsets(&self) -> Option<CalibrationOffsets> {
  if !self.calibrated {
   return None;
  }

  Some(CalibrationOffsets {
   head_yaw_offset_deg: self.head_yaw_offset_deg,
   head_pitch_offset_deg: self.head_pitch_offset_deg,
   head_roll_offset_deg: self.head_roll_offset_deg,
   eye_yaw_offset_deg: self.eye_yaw_offset_deg,
   eye_pitch_offset_deg: self.eye_pitch_offset_deg,
   left_eye_yaw_offset_deg: self.left_eye_yaw_offset_deg,
   left_eye_pitch_offset_deg: self.left_eye_pitch_offset_deg,
   right_eye_yaw_offset_deg: self.right_eye_yaw_offset_deg,
   right_eye_pitch_offset_deg: self.right_eye_pitch_offset_deg,
  })
 }

 pub fn set_offsets(&mut self, offsets: CalibrationOffsets) {
  self.calibrated = true;
  self.head_yaw_offset_deg = offsets.head_yaw_offset_deg;
  self.head_pitch_offset_deg = offsets.head_pitch_offset_deg;
  self.head_roll_offset_deg = offsets.head_roll_offset_deg;
  self.eye_yaw_offset_deg = offsets.eye_yaw_offset_deg;
  self.eye_pitch_offset_deg = offsets.eye_pitch_offset_deg;
  self.left_eye_yaw_offset_deg = offsets.left_eye_yaw_offset_deg;
  self.left_eye_pitch_offset_deg = offsets.left_eye_pitch_offset_deg;
  self.right_eye_yaw_offset_deg = offsets.right_eye_yaw_offset_deg;
  self.right_eye_pitch_offset_deg = offsets.right_eye_pitch_offset_deg;
 }

 pub fn clear_offsets(&mut self) {
  self.calibrated = false;
  self.head_yaw_offset_deg = 0.0;
  self.head_pitch_offset_deg = 0.0;
  self.head_roll_offset_deg = 0.0;
  self.eye_yaw_offset_deg = 0.0;
  self.eye_pitch_offset_deg = 0.0;
  self.left_eye_yaw_offset_deg = 0.0;
  self.left_eye_pitch_offset_deg = 0.0;
  self.right_eye_yaw_offset_deg = 0.0;
  self.right_eye_pitch_offset_deg = 0.0;
 }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AppConfig {
 pub input: InputConfig,
 pub output: OutputConfig,
 pub mapping: MappingConfig,
 pub mapping_profiles: MappingProfilesConfig,
 pub runtime: RuntimeConfig,
 pub calibration: CalibrationConfig,
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

 pub fn effective_mapping(&self) -> MappingConfig {
  match self.mapping_profiles.mapping_for(self.mapping_profiles.active) {
   Some(profile_mapping) => profile_mapping.clone(),
   None => self.mapping.clone(),
  }
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use unvet_core::calibration::CalibrationOffsets;

 #[test]
 fn default_config_roundtrip() {
  let source = toml::to_string_pretty(&AppConfig::default()).expect("serialize default config");
  let parsed = AppConfig::from_toml(&source).expect("parse serialized config");

  assert_eq!(parsed, AppConfig::default());
 }

 #[test]
 fn calibration_offsets_can_roundtrip() {
  let mut calibration = CalibrationConfig::default();
  assert!(calibration.offsets().is_none());

  calibration.set_offsets(CalibrationOffsets {
   head_yaw_offset_deg: 1.2,
   head_pitch_offset_deg: -0.8,
   ..CalibrationOffsets::default()
  });

  let offsets = calibration.offsets().expect("offsets should be available after set");
  assert!((offsets.head_yaw_offset_deg - 1.2).abs() < 0.001);
  assert!((offsets.head_pitch_offset_deg + 0.8).abs() < 0.001);

  calibration.clear_offsets();
  assert!(calibration.offsets().is_none());
 }

 #[test]
 fn effective_mapping_uses_global_by_default() {
  let mut config = AppConfig::default();
  config.mapping.yaw_sensitivity = 1.4;

  let effective = config.effective_mapping();
  assert!((effective.yaw_sensitivity - 1.4).abs() < 0.001);
 }

 #[test]
 fn effective_mapping_can_select_per_game_profile() {
  let mut config = AppConfig::default();
  config.mapping.yaw_sensitivity = 0.5;
  config.mapping_profiles.active = MappingProfile::Ets2;
  config.mapping_profiles.ets2.yaw_sensitivity = 1.7;

  let effective = config.effective_mapping();
  assert!((effective.yaw_sensitivity - 1.7).abs() < 0.001);
 }
}
