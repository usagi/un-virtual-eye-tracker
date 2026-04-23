#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisMappingSettings {
 pub sensitivity: f32,
 pub deadzone: f32,
 pub max_input_angle_deg: f32,
}

impl Default for AxisMappingSettings {
 fn default() -> Self {
  Self {
   sensitivity: 1.0,
   deadzone: 0.06,
   max_input_angle_deg: 30.0,
  }
 }
}

pub fn mix_eye_and_head(eye_deg: f32, head_deg: f32, eye_weight: f32) -> f32 {
 let eye_weight = eye_weight.clamp(0.0, 1.0);
 eye_deg * eye_weight + head_deg * (1.0 - eye_weight)
}

pub fn map_angle_to_normalized(angle_deg: f32, settings: AxisMappingSettings) -> f32 {
 if !angle_deg.is_finite() {
  return 0.0;
 }

 let max_input_angle_deg = settings.max_input_angle_deg.abs();
 if max_input_angle_deg <= f32::EPSILON {
  return 0.0;
 }

 let normalized = (angle_deg / max_input_angle_deg).clamp(-1.0, 1.0);
 let deadzone = settings.deadzone.clamp(0.0, 0.95);
 let magnitude = normalized.abs();

 if magnitude <= deadzone {
  return 0.0;
 }

 let remapped_magnitude = ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
 let sensitivity = settings.sensitivity.clamp(0.0, 3.0);
 let mapped = remapped_magnitude * sensitivity;

 normalized.signum() * mapped.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
 use super::{
  map_angle_to_normalized,
  mix_eye_and_head,
  AxisMappingSettings,
 };

 #[test]
 fn mix_eye_and_head_uses_weight() {
  let value = mix_eye_and_head(10.0, 2.0, 0.75);
  assert!((value - 8.0).abs() < 0.001);
 }

 #[test]
 fn deadzone_suppresses_small_values() {
  let output = map_angle_to_normalized(
   1.0,
   AxisMappingSettings {
    deadzone: 0.2,
    max_input_angle_deg: 10.0,
    ..AxisMappingSettings::default()
   },
  );

  assert_eq!(output, 0.0);
 }

 #[test]
 fn sensitivity_scales_after_deadzone() {
  let baseline = map_angle_to_normalized(
   8.0,
   AxisMappingSettings {
    sensitivity: 1.0,
    deadzone: 0.1,
    max_input_angle_deg: 10.0,
   },
  );
  let boosted = map_angle_to_normalized(
   8.0,
   AxisMappingSettings {
    sensitivity: 2.0,
    deadzone: 0.1,
    max_input_angle_deg: 10.0,
   },
  );

  assert!(boosted > baseline);
  assert!(boosted <= 1.0);
 }
}