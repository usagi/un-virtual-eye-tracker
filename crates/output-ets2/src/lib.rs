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

impl Default for TruckSimResponse {
 fn default() -> Self {
  Self {
   yaw_gain: 1.15,
   pitch_gain: 0.9,
   deadzone: 0.05,
   yaw_exponent: 0.95,
   pitch_exponent: 1.05,
   look_back_threshold: 0.93,
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

  let threshold = response.look_back_threshold.clamp(0.6, 0.99);
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
