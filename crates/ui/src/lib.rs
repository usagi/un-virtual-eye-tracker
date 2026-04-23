use unvet_core::model::OutputFrame;

#[derive(Debug, Clone, Default)]
pub struct UiSnapshot {
 pub input_connected: bool,
 pub output_enabled: bool,
 pub look_yaw_norm: f32,
 pub look_pitch_norm: f32,
 pub confidence: f32,
}

impl UiSnapshot {
 pub fn update_from_output(&mut self, frame: OutputFrame) {
  self.look_yaw_norm = frame.look_yaw_norm;
  self.look_pitch_norm = frame.look_pitch_norm;
  self.confidence = frame.confidence;
 }
}
