use unvet_core::{AppResult, model::OutputFrame, ports::OutputBackend};

pub struct Ets2Backend {
 enabled: bool,
 yaw_gain: f32,
 pitch_gain: f32,
}

impl Default for Ets2Backend {
 fn default() -> Self {
  Self {
   enabled: true,
   yaw_gain: 1.0,
   pitch_gain: 0.85,
  }
 }
}

impl OutputBackend for Ets2Backend {
 fn backend_name(&self) -> &'static str {
  "ets2"
 }

 fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  if !self.enabled || !frame.active {
   return Ok(());
  }

  let _game_yaw = frame.look_yaw_norm.clamp(-1.0, 1.0) * self.yaw_gain;
  let _game_pitch = frame.look_pitch_norm.clamp(-1.0, 1.0) * self.pitch_gain;
  Ok(())
 }

 fn set_enabled(&mut self, enabled: bool) {
  self.enabled = enabled;
 }

 fn is_enabled(&self) -> bool {
  self.enabled
 }
}
