use crate::model::OutputFrame;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExponentialSmoother {
 pub alpha: f32,
 state: Option<f32>,
}

impl ExponentialSmoother {
 pub fn new(alpha: f32) -> Self {
  Self {
   alpha: alpha.clamp(0.0, 1.0),
   state: None,
  }
 }

 pub fn reset(&mut self) {
  self.state = None;
 }

 pub fn update(&mut self, sample: f32) -> f32 {
  if !sample.is_finite() {
   return self.state.unwrap_or_default();
  }

  let next = match self.state {
   Some(prev) => prev + (sample - prev) * self.alpha,
   None => sample,
  };
  self.state = Some(next);
  next
 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputFrameSmoother {
 yaw: ExponentialSmoother,
 pitch: ExponentialSmoother,
}

impl OutputFrameSmoother {
 pub fn new(alpha: f32) -> Self {
  Self {
   yaw: ExponentialSmoother::new(alpha),
   pitch: ExponentialSmoother::new(alpha),
  }
 }

 pub fn reset(&mut self) {
  self.yaw.reset();
  self.pitch.reset();
 }

 pub fn update(&mut self, frame: OutputFrame) -> OutputFrame {
  OutputFrame {
   look_yaw_norm: self.yaw.update(frame.look_yaw_norm).clamp(-1.0, 1.0),
   look_pitch_norm: self.pitch.update(frame.look_pitch_norm).clamp(-1.0, 1.0),
   confidence: frame.confidence,
   active: frame.active,
  }
 }
}

#[cfg(test)]
mod tests {
 use super::{ExponentialSmoother, OutputFrameSmoother};
 use crate::model::OutputFrame;

 #[test]
 fn first_sample_passes_through() {
  let mut smoother = ExponentialSmoother::new(0.18);
  let value = smoother.update(0.8);

  assert!((value - 0.8).abs() < 0.0001);
 }

 #[test]
 fn alpha_one_tracks_immediately() {
  let mut smoother = ExponentialSmoother::new(1.0);
  smoother.update(0.0);
  let value = smoother.update(1.0);

  assert!((value - 1.0).abs() < 0.0001);
 }

 #[test]
 fn alpha_zero_keeps_initial_value() {
  let mut smoother = ExponentialSmoother::new(0.0);
  smoother.update(0.4);
  let value = smoother.update(0.9);

  assert!((value - 0.4).abs() < 0.0001);
 }

 #[test]
 fn output_frame_smoother_updates_axes_independently() {
  let mut smoother = OutputFrameSmoother::new(0.5);
  let _ = smoother.update(OutputFrame {
   look_yaw_norm: 1.0,
   look_pitch_norm: -1.0,
   confidence: 1.0,
   active: true,
  });

  let frame = smoother.update(OutputFrame {
   look_yaw_norm: 0.0,
   look_pitch_norm: 1.0,
   confidence: 0.7,
   active: false,
  });

  assert!((frame.look_yaw_norm - 0.5).abs() < 0.0001);
  assert!((frame.look_pitch_norm - 0.0).abs() < 0.0001);
  assert!((frame.confidence - 0.7).abs() < 0.0001);
  assert!(!frame.active);
 }

 #[test]
 fn non_finite_input_keeps_previous_state() {
  let mut smoother = ExponentialSmoother::new(0.3);
  smoother.update(0.25);
  let value = smoother.update(f32::NAN);

  assert!((value - 0.25).abs() < 0.0001);
 }
}