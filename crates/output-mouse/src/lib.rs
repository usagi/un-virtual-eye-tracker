use unvet_core::{AppResult, model::OutputFrame, ports::OutputBackend};

#[derive(Debug, Clone)]
pub struct MouseBackend {
 enabled: bool,
 speed_scale: f32,
}

impl Default for MouseBackend {
 fn default() -> Self {
  Self {
   enabled: true,
   speed_scale: 18.0,
  }
 }
}

impl OutputBackend for MouseBackend {
 fn backend_name(&self) -> &'static str {
  "mouse"
 }

 fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  if !self.enabled || !frame.active {
   return Ok(());
  }

  let _dx = frame.look_yaw_norm.clamp(-1.0, 1.0) * self.speed_scale;
  let _dy = frame.look_pitch_norm.clamp(-1.0, 1.0) * self.speed_scale;
  Ok(())
 }

 fn set_enabled(&mut self, enabled: bool) {
  self.enabled = enabled;
 }

 fn is_enabled(&self) -> bool {
  self.enabled
 }
}
