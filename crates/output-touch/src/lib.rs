use unvet_core::{AppError, AppResult, model::OutputFrame, ports::OutputBackend};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenPoint {
 x: i32,
 y: i32,
}

pub struct TouchBackend {
 enabled: bool,
 last_point: Option<ScreenPoint>,
}

impl Default for TouchBackend {
 fn default() -> Self {
  Self {
   enabled: true,
   last_point: None,
  }
 }
}

impl TouchBackend {
 fn map_point_from_norm(look_yaw_norm: f32, look_pitch_norm: f32, screen_width: i32, screen_height: i32) -> ScreenPoint {
  let width = screen_width.max(1);
  let height = screen_height.max(1);

  let normalized_x = ((look_yaw_norm.clamp(-1.0, 1.0) + 1.0) * 0.5).clamp(0.0, 1.0);
  let normalized_y = ((look_pitch_norm.clamp(-1.0, 1.0) + 1.0) * 0.5).clamp(0.0, 1.0);

  ScreenPoint {
   x: (normalized_x * (width - 1) as f32).round() as i32,
   y: (normalized_y * (height - 1) as f32).round() as i32,
  }
 }

 fn map_frame_to_point(frame: OutputFrame) -> AppResult<Option<ScreenPoint>> {
  if !frame.active {
   return Ok(None);
  }

  let (screen_width, screen_height) = screen_size_platform()?;
  Ok(Some(Self::map_point_from_norm(
   frame.look_yaw_norm,
   frame.look_pitch_norm,
   screen_width,
   screen_height,
  )))
 }

 fn send_absolute(point: ScreenPoint) -> AppResult<()> {
  set_cursor_pos_platform(point.x, point.y)
 }
}

impl OutputBackend for TouchBackend {
 fn backend_name(&self) -> &'static str {
  "touch"
 }

 fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  if !self.enabled {
   self.last_point = None;
   return Ok(());
  }

  if let Some(point) = Self::map_frame_to_point(frame)? {
   if self.last_point != Some(point) {
    Self::send_absolute(point)?;
    self.last_point = Some(point);
   }
  } else {
   self.last_point = None;
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
fn screen_size_platform() -> AppResult<(i32, i32)> {
 const SM_CXSCREEN: i32 = 0;
 const SM_CYSCREEN: i32 = 1;

 #[link(name = "User32")]
 unsafe extern "system" {
  fn GetSystemMetrics(index: i32) -> i32;
 }

 let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
 let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
 if width <= 0 || height <= 0 {
  return Err(AppError::InvalidState(
   "failed to query screen size for absolute pointer output".to_owned(),
  ));
 }

 Ok((width, height))
}

#[cfg(not(windows))]
fn screen_size_platform() -> AppResult<(i32, i32)> {
 Ok((1920, 1080))
}

#[cfg(windows)]
fn set_cursor_pos_platform(x: i32, y: i32) -> AppResult<()> {
 #[link(name = "User32")]
 unsafe extern "system" {
  fn SetCursorPos(x: i32, y: i32) -> i32;
 }

 let ok = unsafe { SetCursorPos(x, y) };
 if ok == 0 {
  return Err(AppError::InvalidState("failed to set absolute cursor position".to_owned()));
 }

 Ok(())
}

#[cfg(not(windows))]
fn set_cursor_pos_platform(_x: i32, _y: i32) -> AppResult<()> {
 Ok(())
}

#[cfg(test)]
mod tests {
 use super::{ScreenPoint, TouchBackend};
 use unvet_core::model::OutputFrame;
 use unvet_core::ports::OutputBackend;

 #[test]
 fn map_point_from_norm_centers_when_zero() {
  let point = TouchBackend::map_point_from_norm(0.0, 0.0, 1920, 1080);
  assert_eq!(point, ScreenPoint { x: 960, y: 540 });
 }

 #[test]
 fn map_point_from_norm_maps_to_screen_bounds() {
  let point = TouchBackend::map_point_from_norm(1.0, -1.0, 100, 80);
  assert_eq!(point, ScreenPoint { x: 99, y: 0 });
 }

 #[test]
 fn map_frame_to_point_returns_none_when_inactive() {
  let point = TouchBackend::map_frame_to_point(OutputFrame {
   look_yaw_norm: 1.0,
   look_pitch_norm: 1.0,
   confidence: 1.0,
   active: false,
   ..OutputFrame::default()
  })
  .expect("map inactive frame");

  assert!(point.is_none());
 }

 #[test]
 fn disabling_backend_clears_cached_point() {
  let mut backend = TouchBackend::default();
  backend.last_point = Some(ScreenPoint { x: 10, y: 20 });
  backend.set_enabled(false);

  backend
   .apply(OutputFrame {
    look_yaw_norm: 0.0,
    look_pitch_norm: 0.0,
    confidence: 0.0,
    active: true,
    ..OutputFrame::default()
   })
   .expect("apply while disabled");

  assert!(backend.last_point.is_none());
 }
}
