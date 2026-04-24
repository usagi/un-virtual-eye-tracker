use crate::model::TrackingFrame;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationOffsets {
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

impl Default for CalibrationOffsets {
 fn default() -> Self {
  Self {
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

impl CalibrationOffsets {
 pub fn from_frame(frame: TrackingFrame) -> Self {
  Self {
   head_yaw_offset_deg: frame.head_yaw_deg,
   head_pitch_offset_deg: frame.head_pitch_deg,
   head_roll_offset_deg: frame.head_roll_deg,
   eye_yaw_offset_deg: frame.eye_yaw_deg,
   eye_pitch_offset_deg: frame.eye_pitch_deg,
   left_eye_yaw_offset_deg: frame.left_eye_yaw_deg,
   left_eye_pitch_offset_deg: frame.left_eye_pitch_deg,
   right_eye_yaw_offset_deg: frame.right_eye_yaw_deg,
   right_eye_pitch_offset_deg: frame.right_eye_pitch_deg,
  }
 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeutralPoseCalibration {
 enabled: bool,
 calibrated: bool,
 offsets: CalibrationOffsets,
}

impl Default for NeutralPoseCalibration {
 fn default() -> Self {
  Self {
   enabled: true,
   calibrated: false,
   offsets: CalibrationOffsets::default(),
  }
 }
}

impl NeutralPoseCalibration {
 pub fn new(enabled: bool) -> Self {
  Self {
   enabled,
   ..Self::default()
  }
 }

 pub fn from_offsets(enabled: bool, offsets: CalibrationOffsets) -> Self {
  Self {
   enabled,
   calibrated: true,
   offsets,
  }
 }

 pub fn set_enabled(&mut self, enabled: bool) {
  self.enabled = enabled;
 }

 pub fn is_enabled(&self) -> bool {
  self.enabled
 }

 pub fn is_calibrated(&self) -> bool {
  self.calibrated
 }

 pub fn offsets(&self) -> CalibrationOffsets {
  self.offsets
 }

 pub fn reset(&mut self) {
  self.calibrated = false;
  self.offsets = CalibrationOffsets::default();
 }

 pub fn calibrate_from_frame(&mut self, frame: TrackingFrame) {
  self.offsets = CalibrationOffsets::from_frame(frame);
  self.calibrated = true;
 }

 pub fn apply(&self, frame: TrackingFrame) -> TrackingFrame {
  if !self.enabled || !self.calibrated {
   return frame;
  }

  TrackingFrame {
   timestamp_ms: frame.timestamp_ms,
   head_yaw_deg: frame.head_yaw_deg - self.offsets.head_yaw_offset_deg,
   head_pitch_deg: frame.head_pitch_deg - self.offsets.head_pitch_offset_deg,
   head_roll_deg: frame.head_roll_deg - self.offsets.head_roll_offset_deg,
   eye_yaw_deg: frame.eye_yaw_deg - self.offsets.eye_yaw_offset_deg,
   eye_pitch_deg: frame.eye_pitch_deg - self.offsets.eye_pitch_offset_deg,
   left_eye_yaw_deg: frame.left_eye_yaw_deg - self.offsets.left_eye_yaw_offset_deg,
   left_eye_pitch_deg: frame.left_eye_pitch_deg - self.offsets.left_eye_pitch_offset_deg,
   right_eye_yaw_deg: frame.right_eye_yaw_deg - self.offsets.right_eye_yaw_offset_deg,
   right_eye_pitch_deg: frame.right_eye_pitch_deg - self.offsets.right_eye_pitch_offset_deg,
   confidence: frame.confidence,
   active: frame.active,
  }
 }
}

#[cfg(test)]
mod tests {
 use crate::model::TrackingFrame;

 use super::{CalibrationOffsets, NeutralPoseCalibration};

 fn sample_frame() -> TrackingFrame {
  TrackingFrame {
   timestamp_ms: 100,
   head_yaw_deg: 4.0,
   head_pitch_deg: -2.0,
   head_roll_deg: 1.5,
   eye_yaw_deg: 3.0,
   eye_pitch_deg: -1.0,
   left_eye_yaw_deg: 2.8,
   left_eye_pitch_deg: -1.1,
   right_eye_yaw_deg: 3.2,
   right_eye_pitch_deg: -0.9,
   confidence: 0.85,
   active: true,
  }
 }

 #[test]
 fn apply_is_noop_when_not_calibrated() {
  let calibration = NeutralPoseCalibration::new(true);
  let frame = sample_frame();

  assert_eq!(calibration.apply(frame), frame);
 }

 #[test]
 fn calibrate_and_apply_subtracts_offsets() {
  let mut calibration = NeutralPoseCalibration::new(true);
  calibration.calibrate_from_frame(sample_frame());

  let adjusted = calibration.apply(TrackingFrame {
   head_yaw_deg: 5.5,
   head_pitch_deg: -1.0,
   head_roll_deg: 1.2,
   eye_yaw_deg: 3.4,
   eye_pitch_deg: -0.4,
   left_eye_yaw_deg: 3.1,
   left_eye_pitch_deg: -0.8,
   right_eye_yaw_deg: 3.7,
   right_eye_pitch_deg: -0.1,
   ..sample_frame()
  });

  assert!((adjusted.head_yaw_deg - 1.5).abs() < 0.001);
  assert!((adjusted.eye_yaw_deg - 0.4).abs() < 0.001);
  assert!((adjusted.right_eye_pitch_deg - 0.8).abs() < 0.001);
 }

 #[test]
 fn from_offsets_marks_calibrated() {
  let offsets = CalibrationOffsets {
   head_yaw_offset_deg: 1.0,
   ..CalibrationOffsets::default()
  };
  let calibration = NeutralPoseCalibration::from_offsets(true, offsets);

  assert!(calibration.is_calibrated());
  assert_eq!(calibration.offsets().head_yaw_offset_deg, 1.0);
 }
}
