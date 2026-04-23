use std::mem::size_of;

use unvet_core::{
 AppError,
 AppResult,
 model::OutputFrame,
 ports::OutputBackend,
};

pub struct MouseBackend {
 enabled: bool,
 speed_scale: f32,
}

impl Default for MouseBackend {
 fn default() -> Self {
  Self {
   enabled: true,
     speed_scale: 20.0,
  }
 }
}

impl MouseBackend {
 fn frame_to_relative(frame: OutputFrame, speed_scale: f32) -> Option<(i32, i32)> {
    if !frame.active {
     return None;
    }

    let dx = (frame.look_yaw_norm.clamp(-1.0, 1.0) * speed_scale).round() as i32;
    let dy = (frame.look_pitch_norm.clamp(-1.0, 1.0) * speed_scale).round() as i32;
    if dx == 0 && dy == 0 {
     None
    } else {
     Some((dx, dy))
    }
 }

 fn send_relative(dx: i32, dy: i32) -> AppResult<()> {
    send_relative_platform(dx, dy)
 }
}

impl OutputBackend for MouseBackend {
 fn backend_name(&self) -> &'static str {
  "mouse"
 }

 fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
    if !self.enabled {
   return Ok(());
  }

    if let Some((dx, dy)) = Self::frame_to_relative(frame, self.speed_scale) {
     Self::send_relative(dx, dy)?;
    }
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
fn send_relative_platform(dx: i32, dy: i32) -> AppResult<()> {
 const INPUT_MOUSE: u32 = 0;
 const MOUSEEVENTF_MOVE: u32 = 0x0001;

 #[repr(C)]
 struct MouseInput {
  dx: i32,
  dy: i32,
  mouse_data: u32,
  flags: u32,
  time: u32,
  extra_info: usize,
 }

 #[repr(C)]
 struct Input {
  input_type: u32,
   mouse: MouseInput,
 }

 #[link(name = "User32")]
 unsafe extern "system" {
  fn SendInput(input_count: u32, inputs: *const Input, input_size: i32) -> u32;
 }

 let input = Input {
  input_type: INPUT_MOUSE,
   mouse: MouseInput {
    dx,
    dy,
    mouse_data: 0,
    flags: MOUSEEVENTF_MOVE,
    time: 0,
    extra_info: 0,
  },
 };

 let wrote = unsafe {
  SendInput(1, &input as *const Input, size_of::<Input>() as i32)
 };
 if wrote != 1 {
  return Err(AppError::InvalidState("failed to send relative mouse input".to_owned()));
 }

 Ok(())
}

#[cfg(not(windows))]
fn send_relative_platform(_dx: i32, _dy: i32) -> AppResult<()> {
 Ok(())
}

#[cfg(test)]
mod tests {
 use super::MouseBackend;
 use unvet_core::model::OutputFrame;

 #[test]
 fn frame_to_relative_returns_none_when_inactive() {
  let motion = MouseBackend::frame_to_relative(
   OutputFrame {
    look_yaw_norm: 1.0,
    look_pitch_norm: 1.0,
    confidence: 1.0,
    active: false,
   },
   20.0,
  );

  assert!(motion.is_none());
 }

 #[test]
 fn frame_to_relative_scales_axes() {
  let motion = MouseBackend::frame_to_relative(
   OutputFrame {
    look_yaw_norm: 0.5,
    look_pitch_norm: -0.25,
    confidence: 1.0,
    active: true,
   },
   20.0,
  );

  assert_eq!(motion, Some((10, -5)));
 }

 #[test]
 fn frame_to_relative_ignores_tiny_input_after_rounding() {
  let motion = MouseBackend::frame_to_relative(
   OutputFrame {
    look_yaw_norm: 0.01,
    look_pitch_norm: -0.01,
    confidence: 1.0,
    active: true,
   },
   20.0,
  );

  assert!(motion.is_none());
 }
}
