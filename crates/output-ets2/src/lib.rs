use unvet_core::{
 AppResult,
 model::OutputFrame,
 ports::OutputBackend,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TruckSimCommand {
 pub camera_yaw: f32,
 pub camera_pitch: f32,
 pub look_back_left: bool,
 pub look_back_right: bool,
}

impl TruckSimCommand {
 fn neutral() -> Self {
  Self {
   camera_yaw: 0.0,
   camera_pitch: 0.0,
   look_back_left: false,
   look_back_right: false,
  }
 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TruckSimResponse {
 pub yaw_gain: f32,
 pub pitch_gain: f32,
 pub deadzone: f32,
 pub yaw_exponent: f32,
 pub pitch_exponent: f32,
 pub look_back_threshold: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruckSimPreset {
 Ets2,
 Ats,
}

impl Default for TruckSimResponse {
 fn default() -> Self {
    Self::for_preset(TruckSimPreset::Ets2)
 }
}

impl TruckSimResponse {
 pub fn for_preset(preset: TruckSimPreset) -> Self {
    match preset {
     TruckSimPreset::Ets2 => Self {
        yaw_gain: 1.15,
        pitch_gain: 0.9,
        deadzone: 0.05,
        yaw_exponent: 0.95,
        pitch_exponent: 1.05,
        look_back_threshold: 0.93,
     },
     TruckSimPreset::Ats => Self {
        yaw_gain: 1.22,
        pitch_gain: 0.94,
        deadzone: 0.045,
        yaw_exponent: 0.9,
        pitch_exponent: 1.0,
        look_back_threshold: 0.92,
     },
    }
 }

 pub fn sanitized(self) -> Self {
  Self {
     yaw_gain: self.yaw_gain.clamp(0.1, 3.0),
     pitch_gain: self.pitch_gain.clamp(0.1, 3.0),
     deadzone: self.deadzone.clamp(0.0, 0.95),
     yaw_exponent: self.yaw_exponent.clamp(0.1, 3.0),
     pitch_exponent: self.pitch_exponent.clamp(0.1, 3.0),
     look_back_threshold: self.look_back_threshold.clamp(0.6, 0.99),
  }
 }
}

pub struct Ets2Backend {
 enabled: bool,
 response: TruckSimResponse,
 last_command: TruckSimCommand,
}

impl Default for Ets2Backend {
 fn default() -> Self {
  Self {
   enabled: true,
   response: TruckSimResponse::default(),
   last_command: TruckSimCommand::neutral(),
  }
 }
}

impl Ets2Backend {
 pub fn set_response_preset(&mut self, preset: TruckSimPreset) {
  self.response = TruckSimResponse::for_preset(preset);
 }

 pub fn set_response(&mut self, response: TruckSimResponse) {
  self.response = response.sanitized();
 }

 fn map_axis(value_norm: f32, gain: f32, deadzone: f32, exponent: f32) -> f32 {
  let clamped = value_norm.clamp(-1.0, 1.0);
  let magnitude = clamped.abs();
  let deadzone = deadzone.clamp(0.0, 0.95);
  if magnitude <= deadzone {
   return 0.0;
  }

  let remapped = ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
  let curved = remapped.powf(exponent.clamp(0.1, 3.0));
  (clamped.signum() * curved * gain).clamp(-1.0, 1.0)
 }

 fn frame_to_command(frame: OutputFrame, response: TruckSimResponse) -> TruckSimCommand {
  if !frame.active {
   return TruckSimCommand::neutral();
  }

  let camera_yaw = Self::map_axis(frame.look_yaw_norm, response.yaw_gain, response.deadzone, response.yaw_exponent);
  let camera_pitch = Self::map_axis(
   frame.look_pitch_norm,
   response.pitch_gain,
   response.deadzone,
   response.pitch_exponent,
  );

    let response = response.sanitized();
    let threshold = response.look_back_threshold;
  TruckSimCommand {
   camera_yaw,
   camera_pitch,
   look_back_left: camera_yaw <= -threshold,
   look_back_right: camera_yaw >= threshold,
  }
 }

 fn dispatch_command(&self, command: TruckSimCommand) -> AppResult<()> {
  dispatch_command_platform(command)
 }
}

impl OutputBackend for Ets2Backend {
 fn backend_name(&self) -> &'static str {
  "ets2"
 }

 fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  if !self.enabled {
   return Ok(());
  }

  let command = Self::frame_to_command(frame, self.response);
  self.dispatch_command(command)?;
  self.last_command = command;
  Ok(())
 }

 fn set_enabled(&mut self, enabled: bool) {
  self.enabled = enabled;
 }

 fn is_enabled(&self) -> bool {
  self.enabled
 }
}

#[cfg(windows)]
fn dispatch_command_platform(command: TruckSimCommand) -> AppResult<()> {
 let _ = command;
 Ok(())
}

#[cfg(not(windows))]
fn dispatch_command_platform(command: TruckSimCommand) -> AppResult<()> {
 let _ = command;
 Ok(())
}

#[cfg(test)]
mod tests {
 use super::{Ets2Backend, TruckSimCommand, TruckSimPreset, TruckSimResponse};
 use unvet_core::model::OutputFrame;

 #[test]
 fn truck_sim_presets_are_distinct() {
  let ets2 = TruckSimResponse::for_preset(TruckSimPreset::Ets2);
  let ats = TruckSimResponse::for_preset(TruckSimPreset::Ats);

  assert_ne!(ets2, ats);
  assert!(ats.yaw_gain > ets2.yaw_gain);
 }

 #[test]
 fn frame_to_command_returns_neutral_when_inactive() {
  let command = Ets2Backend::frame_to_command(
   OutputFrame {
    look_yaw_norm: 1.0,
    look_pitch_norm: -1.0,
    confidence: 1.0,
    active: false,
   },
   TruckSimResponse::default(),
  );

  assert_eq!(command, TruckSimCommand::neutral());
 }

 #[test]
 fn frame_to_command_sets_look_back_flags() {
  let response = TruckSimResponse {
   yaw_gain: 1.0,
   pitch_gain: 1.0,
   deadzone: 0.0,
   yaw_exponent: 1.0,
   pitch_exponent: 1.0,
   look_back_threshold: 0.8,
  };
  let right = Ets2Backend::frame_to_command(
   OutputFrame {
    look_yaw_norm: 1.0,
    look_pitch_norm: 0.0,
    confidence: 1.0,
    active: true,
   },
   response,
  );
  let left = Ets2Backend::frame_to_command(
   OutputFrame {
    look_yaw_norm: -1.0,
    look_pitch_norm: 0.0,
    confidence: 1.0,
    active: true,
   },
   response,
  );

  assert!(right.look_back_right);
  assert!(!right.look_back_left);
  assert!(left.look_back_left);
  assert!(!left.look_back_right);
 }

 #[test]
 fn map_axis_respects_deadzone_and_clamp() {
  let in_deadzone = Ets2Backend::map_axis(0.03, 1.0, 0.05, 1.0);
  let clamped = Ets2Backend::map_axis(2.0, 3.0, 0.0, 1.0);

  assert_eq!(in_deadzone, 0.0);
  assert_eq!(clamped, 1.0);
 }

 #[test]
 fn set_response_sanitizes_values() {
  let mut backend = Ets2Backend::default();
  backend.set_response(TruckSimResponse {
   yaw_gain: 9.0,
   pitch_gain: -1.0,
   deadzone: 2.0,
   yaw_exponent: 0.01,
   pitch_exponent: 8.0,
   look_back_threshold: 0.1,
  });

  assert_eq!(backend.response.yaw_gain, 3.0);
  assert_eq!(backend.response.pitch_gain, 0.1);
  assert_eq!(backend.response.deadzone, 0.95);
  assert_eq!(backend.response.yaw_exponent, 0.1);
  assert_eq!(backend.response.pitch_exponent, 3.0);
  assert_eq!(backend.response.look_back_threshold, 0.6);
 }
}
