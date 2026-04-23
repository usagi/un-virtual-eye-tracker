#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawTrackingFrame {
 pub timestamp_ms: u64,
 pub head_yaw_deg: f32,
 pub head_pitch_deg: f32,
 pub head_roll_deg: f32,
 pub eye_yaw_deg: Option<f32>,
 pub eye_pitch_deg: Option<f32>,
 pub left_eye_yaw_deg: Option<f32>,
 pub left_eye_pitch_deg: Option<f32>,
 pub right_eye_yaw_deg: Option<f32>,
 pub right_eye_pitch_deg: Option<f32>,
 pub reported_confidence: Option<f32>,
 pub reported_active: Option<bool>,
}

impl Default for RawTrackingFrame {
 fn default() -> Self {
  Self {
   timestamp_ms: 0,
   head_yaw_deg: 0.0,
   head_pitch_deg: 0.0,
   head_roll_deg: 0.0,
   eye_yaw_deg: None,
   eye_pitch_deg: None,
   left_eye_yaw_deg: None,
   left_eye_pitch_deg: None,
   right_eye_yaw_deg: None,
   right_eye_pitch_deg: None,
   reported_confidence: None,
   reported_active: None,
  }
 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackingNormalizationConfig {
 pub default_confidence: f32,
 pub default_active: bool,
 pub active_confidence_threshold: f32,
 pub invalid_confidence: f32,
 pub max_abs_head_angle_deg: f32,
 pub max_abs_eye_angle_deg: f32,
 pub max_eye_delta_deg: f32,
 pub eye_disagreement_penalty: f32,
}

impl Default for TrackingNormalizationConfig {
 fn default() -> Self {
  Self {
   default_confidence: 1.0,
   default_active: true,
   active_confidence_threshold: 0.2,
   invalid_confidence: 0.0,
   max_abs_head_angle_deg: 120.0,
   max_abs_eye_angle_deg: 85.0,
   max_eye_delta_deg: 35.0,
   eye_disagreement_penalty: 0.35,
  }
 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackingFrame {
 pub timestamp_ms: u64,
 pub head_yaw_deg: f32,
 pub head_pitch_deg: f32,
 pub head_roll_deg: f32,
 pub eye_yaw_deg: f32,
 pub eye_pitch_deg: f32,
 pub left_eye_yaw_deg: f32,
 pub left_eye_pitch_deg: f32,
 pub right_eye_yaw_deg: f32,
 pub right_eye_pitch_deg: f32,
 pub confidence: f32,
 pub active: bool,
}

impl Default for TrackingFrame {
 fn default() -> Self {
  Self {
   timestamp_ms: 0,
   head_yaw_deg: 0.0,
   head_pitch_deg: 0.0,
   head_roll_deg: 0.0,
   eye_yaw_deg: 0.0,
   eye_pitch_deg: 0.0,
   left_eye_yaw_deg: 0.0,
   left_eye_pitch_deg: 0.0,
   right_eye_yaw_deg: 0.0,
   right_eye_pitch_deg: 0.0,
   confidence: 0.0,
   active: false,
  }
 }
}

impl TrackingFrame {
 pub fn from_raw(raw: RawTrackingFrame) -> Self {
  Self::from_raw_with_config(raw, &TrackingNormalizationConfig::default())
 }

 pub fn from_raw_with_config(raw: RawTrackingFrame, config: &TrackingNormalizationConfig) -> Self {
  let left_eye_yaw_deg = raw.left_eye_yaw_deg.unwrap_or_else(|| raw.eye_yaw_deg.unwrap_or_default());
  let right_eye_yaw_deg = raw.right_eye_yaw_deg.unwrap_or_else(|| raw.eye_yaw_deg.unwrap_or(left_eye_yaw_deg));
  let left_eye_pitch_deg = raw.left_eye_pitch_deg.unwrap_or_else(|| raw.eye_pitch_deg.unwrap_or_default());
  let right_eye_pitch_deg = raw
   .right_eye_pitch_deg
   .unwrap_or_else(|| raw.eye_pitch_deg.unwrap_or(left_eye_pitch_deg));
  let eye_yaw_deg = raw.eye_yaw_deg.unwrap_or((left_eye_yaw_deg + right_eye_yaw_deg) * 0.5);
  let eye_pitch_deg = raw.eye_pitch_deg.unwrap_or((left_eye_pitch_deg + right_eye_pitch_deg) * 0.5);

  let has_finite_values = [
   raw.head_yaw_deg,
   raw.head_pitch_deg,
   raw.head_roll_deg,
   eye_yaw_deg,
   eye_pitch_deg,
   left_eye_yaw_deg,
   left_eye_pitch_deg,
   right_eye_yaw_deg,
   right_eye_pitch_deg,
  ]
  .iter()
  .all(|value| value.is_finite());

  let within_angle_limits = raw.head_yaw_deg.abs() <= config.max_abs_head_angle_deg
   && raw.head_pitch_deg.abs() <= config.max_abs_head_angle_deg
   && raw.head_roll_deg.abs() <= config.max_abs_head_angle_deg
   && eye_yaw_deg.abs() <= config.max_abs_eye_angle_deg
   && eye_pitch_deg.abs() <= config.max_abs_eye_angle_deg
   && left_eye_yaw_deg.abs() <= config.max_abs_eye_angle_deg
   && left_eye_pitch_deg.abs() <= config.max_abs_eye_angle_deg
   && right_eye_yaw_deg.abs() <= config.max_abs_eye_angle_deg
   && right_eye_pitch_deg.abs() <= config.max_abs_eye_angle_deg;

  let eye_delta = (left_eye_yaw_deg - right_eye_yaw_deg)
   .abs()
   .max((left_eye_pitch_deg - right_eye_pitch_deg).abs());
  let valid_frame = has_finite_values && within_angle_limits;

  let mut confidence = raw.reported_confidence.unwrap_or(config.default_confidence).clamp(0.0, 1.0);
  let disagreement_ratio = if config.max_eye_delta_deg <= 0.0 {
   1.0
  } else {
   (eye_delta / config.max_eye_delta_deg).clamp(0.0, 1.0)
  };
  let reliability_penalty = disagreement_ratio * config.eye_disagreement_penalty.clamp(0.0, 1.0);
  confidence = (confidence * (1.0 - reliability_penalty)).clamp(0.0, 1.0);

  let active = if !valid_frame {
   confidence = config.invalid_confidence.clamp(0.0, 1.0);
   false
  } else {
   raw.reported_active.unwrap_or(config.default_active) && confidence >= config.active_confidence_threshold
  };

  Self {
   timestamp_ms: raw.timestamp_ms,
   head_yaw_deg: raw.head_yaw_deg,
   head_pitch_deg: raw.head_pitch_deg,
   head_roll_deg: raw.head_roll_deg,
   eye_yaw_deg,
   eye_pitch_deg,
   left_eye_yaw_deg,
   left_eye_pitch_deg,
   right_eye_yaw_deg,
   right_eye_pitch_deg,
   confidence,
   active,
  }
 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputFrame {
 pub look_yaw_norm: f32,
 pub look_pitch_norm: f32,
 pub confidence: f32,
 pub active: bool,
}

impl Default for OutputFrame {
 fn default() -> Self {
  Self {
   look_yaw_norm: 0.0,
   look_pitch_norm: 0.0,
   confidence: 0.0,
   active: false,
  }
 }
}

#[cfg(test)]
mod tests {
 use super::{RawTrackingFrame, TrackingFrame};

 #[test]
 fn tracking_frame_normalization_uses_reported_values() {
  let frame = TrackingFrame::from_raw(RawTrackingFrame {
   timestamp_ms: 100,
   head_yaw_deg: 1.0,
   head_pitch_deg: -2.0,
   head_roll_deg: 0.5,
   eye_yaw_deg: Some(3.0),
   eye_pitch_deg: Some(-1.0),
   left_eye_yaw_deg: Some(2.8),
   left_eye_pitch_deg: Some(-1.1),
   right_eye_yaw_deg: Some(3.2),
   right_eye_pitch_deg: Some(-0.9),
   reported_confidence: Some(0.8),
   reported_active: Some(true),
  });

  assert_eq!(frame.timestamp_ms, 100);
  assert_eq!(frame.eye_yaw_deg, 3.0);
  assert_eq!(frame.eye_pitch_deg, -1.0);
  assert!((frame.confidence - 0.7968).abs() < 0.0001);
  assert!(frame.active);
 }

 #[test]
 fn normalization_clamps_confidence() {
  let frame = TrackingFrame::from_raw(RawTrackingFrame {
   reported_confidence: Some(3.5),
   ..RawTrackingFrame::default()
  });

  assert_eq!(frame.confidence, 1.0);
 }

 #[test]
 fn normalization_derives_eye_average_when_combined_eye_is_missing() {
  let frame = TrackingFrame::from_raw(RawTrackingFrame {
   left_eye_yaw_deg: Some(2.0),
   right_eye_yaw_deg: Some(4.0),
   left_eye_pitch_deg: Some(-3.0),
   right_eye_pitch_deg: Some(-1.0),
   ..RawTrackingFrame::default()
  });

  assert_eq!(frame.eye_yaw_deg, 3.0);
  assert_eq!(frame.eye_pitch_deg, -2.0);
 }

 #[test]
 fn invalid_angle_marks_frame_inactive() {
  let frame = TrackingFrame::from_raw(RawTrackingFrame {
   head_yaw_deg: 160.0,
   reported_confidence: Some(0.9),
   reported_active: Some(true),
   ..RawTrackingFrame::default()
  });

  assert!(!frame.active);
  assert_eq!(frame.confidence, 0.0);
 }

 #[test]
 fn high_eye_disagreement_reduces_confidence() {
  let frame = TrackingFrame::from_raw(RawTrackingFrame {
   left_eye_yaw_deg: Some(5.0),
   right_eye_yaw_deg: Some(-20.0),
   left_eye_pitch_deg: Some(2.0),
   right_eye_pitch_deg: Some(-10.0),
   reported_confidence: Some(1.0),
   reported_active: Some(true),
   ..RawTrackingFrame::default()
  });

  assert!(frame.confidence < 1.0);
  assert!(frame.active);
 }

 #[test]
 fn confidence_threshold_can_disable_active_state() {
  let frame = TrackingFrame::from_raw(RawTrackingFrame {
   reported_confidence: Some(0.1),
   reported_active: Some(true),
   ..RawTrackingFrame::default()
  });

  assert!(!frame.active);
 }
}
