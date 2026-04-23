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
