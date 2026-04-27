use crate::model::{OutputFrame, TrackingFrame};

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

 pub fn set_alpha(&mut self, alpha: f32) {
  self.alpha = alpha.clamp(0.0, 1.0);
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

 pub fn set_alpha(&mut self, alpha: f32) {
  self.yaw.set_alpha(alpha);
  self.pitch.set_alpha(alpha);
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

/// The maximum angle (degrees) for all primary axes for a frame to be considered "near origin".
const NEAR_ORIGIN_DEG: f32 = 2.0;

/// Minimum single-frame delta (degrees, any primary axis) required to classify a
/// near-origin frame as a tracking glitch rather than a genuine centre look.
const DEFAULT_SPIKE_THRESHOLD_DEG: f32 = 10.0;

/// Maximum number of consecutive glitch frames to hold before accepting the new input.
const DEFAULT_MAX_HOLD_FRAMES: u32 = 5;

/// Detects and suppresses sudden "origin-spike" glitches in the tracking input.
///
/// When upstream trackers (e.g. MediaPipe via Warudo/VSeeFace) lose face tracking,
/// they often fall back to a neutral/origin pose — sending all bone rotations at
/// zero degrees.  This creates brief but very visible jitter in the eye-tracker
/// output.
///
/// The stabilizer holds the last known-good [`TrackingFrame`] whenever a new frame
/// looks like an origin spike: all primary angles are near zero **and** the jump
/// from the previous frame was larger than [`DEFAULT_SPIKE_THRESHOLD_DEG`].  After
/// at most [`DEFAULT_MAX_HOLD_FRAMES`] consecutive held frames the new input is
/// accepted regardless, so the filter never freezes the output permanently.
#[derive(Debug, Clone)]
pub struct TrackingFrameStabilizer {
 enabled: bool,
 last_good: Option<TrackingFrame>,
 hold_count: u32,
 max_hold_frames: u32,
}

impl TrackingFrameStabilizer {
 pub fn new(enabled: bool) -> Self {
  Self {
   enabled,
   last_good: None,
   hold_count: 0,
   max_hold_frames: DEFAULT_MAX_HOLD_FRAMES,
  }
 }

 pub fn set_enabled(&mut self, enabled: bool) {
  self.enabled = enabled;
  if !enabled {
   self.reset();
  }
 }

 pub fn is_enabled(&self) -> bool {
  self.enabled
 }

 pub fn reset(&mut self) {
  self.last_good = None;
  self.hold_count = 0;
 }

 /// Filter `frame`, returning either the frame itself (if it looks valid) or
 /// the last known-good frame (if it looks like a tracking-loss origin spike).
 pub fn update(&mut self, frame: TrackingFrame) -> TrackingFrame {
  if !self.enabled {
   return frame;
  }

  let Some(last) = self.last_good else {
   self.last_good = Some(frame);
   return frame;
  };

  // Safety valve: never hold longer than max_hold_frames in a row.
  if self.hold_count >= self.max_hold_frames {
   self.last_good = Some(frame);
   self.hold_count = 0;
   return frame;
  }

  if is_origin_spike(last, frame) {
   self.hold_count += 1;
   return last;
  }

  self.last_good = Some(frame);
  self.hold_count = 0;
  frame
 }
}

/// Returns `true` when `current` looks like a tracking-loss fallback frame.
///
/// A frame is considered a spike when:
/// - All four primary angles (eye yaw/pitch, head yaw/pitch) are within
///   [`NEAR_ORIGIN_DEG`] of zero, **and**
/// - The largest single-axis delta from the previous frame is at least
///   [`DEFAULT_SPIKE_THRESHOLD_DEG`], indicating a sudden jump rather than a
///   genuine slow drift towards centre.
fn is_origin_spike(last: TrackingFrame, current: TrackingFrame) -> bool {
 let near_origin = current.eye_yaw_deg.abs() < NEAR_ORIGIN_DEG
  && current.eye_pitch_deg.abs() < NEAR_ORIGIN_DEG
  && current.head_yaw_deg.abs() < NEAR_ORIGIN_DEG
  && current.head_pitch_deg.abs() < NEAR_ORIGIN_DEG;

 if !near_origin {
  return false;
 }

 let max_delta = (current.eye_yaw_deg - last.eye_yaw_deg)
  .abs()
  .max((current.eye_pitch_deg - last.eye_pitch_deg).abs())
  .max((current.head_yaw_deg - last.head_yaw_deg).abs())
  .max((current.head_pitch_deg - last.head_pitch_deg).abs());

 max_delta >= DEFAULT_SPIKE_THRESHOLD_DEG
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

 #[test]
 fn output_frame_smoother_allows_runtime_alpha_changes() {
  let mut smoother = OutputFrameSmoother::new(1.0);
  let _ = smoother.update(OutputFrame {
   look_yaw_norm: 0.0,
   look_pitch_norm: 0.0,
   confidence: 1.0,
   active: true,
  });

  smoother.set_alpha(0.25);
  let frame = smoother.update(OutputFrame {
   look_yaw_norm: 1.0,
   look_pitch_norm: -1.0,
   confidence: 1.0,
   active: true,
  });

  assert!((frame.look_yaw_norm - 0.25).abs() < 0.0001);
  assert!((frame.look_pitch_norm + 0.25).abs() < 0.0001);
 }
}

#[cfg(test)]
mod stabilizer_tests {
 use super::TrackingFrameStabilizer;
 use crate::model::TrackingFrame;

 fn make_frame(eye_yaw: f32, eye_pitch: f32, head_yaw: f32, head_pitch: f32) -> TrackingFrame {
  TrackingFrame {
   timestamp_ms: 0,
   head_yaw_deg: head_yaw,
   head_pitch_deg: head_pitch,
   head_roll_deg: 0.0,
   eye_yaw_deg: eye_yaw,
   eye_pitch_deg: eye_pitch,
   left_eye_yaw_deg: eye_yaw,
   left_eye_pitch_deg: eye_pitch,
   right_eye_yaw_deg: eye_yaw,
   right_eye_pitch_deg: eye_pitch,
   confidence: 1.0,
   active: true,
  }
 }

 #[test]
 fn disabled_passes_all_frames_through() {
  let mut stabilizer = TrackingFrameStabilizer::new(false);
  let origin = make_frame(0.0, 0.0, 0.0, 0.0);
  let frame = stabilizer.update(origin);
  assert_eq!(frame, origin);
 }

 #[test]
 fn first_frame_always_passes_through() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  let frame = make_frame(0.0, 0.0, 0.0, 0.0);
  let out = stabilizer.update(frame);
  assert_eq!(out, frame);
 }

 #[test]
 fn non_spike_frame_passes_through() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  let _ = stabilizer.update(make_frame(15.0, 5.0, 10.0, 2.0));
  let next = make_frame(13.0, 4.0, 9.0, 1.5);
  let out = stabilizer.update(next);
  assert_eq!(out, next);
 }

 #[test]
 fn origin_spike_is_replaced_with_last_good_frame() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  let good = make_frame(20.0, 8.0, 12.0, 3.0);
  let _ = stabilizer.update(good);

  // Sudden jump to all-zeros — should be suppressed.
  let spike = make_frame(0.0, 0.0, 0.0, 0.0);
  let out = stabilizer.update(spike);
  assert_eq!(out, good, "spike frame should be replaced by last good frame");
 }

 #[test]
 fn near_origin_large_jump_is_suppressed() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  let _ = stabilizer.update(make_frame(15.0, 0.0, 0.0, 0.0));

  // Eye yaw jumps 15 degrees to 0 — qualifies as a spike.
  let spike = make_frame(0.0, 0.0, 0.0, 0.0);
  let out = stabilizer.update(spike);
  assert!((out.eye_yaw_deg - 15.0).abs() < 0.001, "spike should be held");
 }

 #[test]
 fn small_drift_to_origin_is_not_suppressed() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  // Start near-origin already; small delta should pass through.
  let _ = stabilizer.update(make_frame(1.5, 0.5, 0.8, 0.2));
  let near_zero = make_frame(0.5, 0.3, 0.4, 0.1);
  let out = stabilizer.update(near_zero);
  assert_eq!(out, near_zero, "small drift should not be suppressed");
 }

 #[test]
 fn hold_count_resets_after_max_hold_frames() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  let good = make_frame(20.0, 10.0, 5.0, 2.0);
  let _ = stabilizer.update(good);

  let spike = make_frame(0.0, 0.0, 0.0, 0.0);
  // Feed more spikes than max_hold_frames (5); the stabilizer must eventually accept.
  for _ in 0..5 {
   stabilizer.update(spike);
  }
  // After max_hold_frames held, the next frame must be accepted regardless.
  let accepted = stabilizer.update(spike);
  assert_eq!(accepted, spike, "frame must be accepted after max hold reached");
 }

 #[test]
 fn non_origin_frame_after_spike_restores_tracking() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  let good = make_frame(20.0, 8.0, 12.0, 3.0);
  let _ = stabilizer.update(good);

  // Spike followed immediately by a valid non-origin frame.
  let _ = stabilizer.update(make_frame(0.0, 0.0, 0.0, 0.0));
  let recovery = make_frame(18.0, 7.0, 11.0, 2.5);
  let out = stabilizer.update(recovery);
  assert_eq!(out, recovery, "valid frame after spike should be accepted");
 }

 #[test]
 fn reset_clears_state() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  let good = make_frame(20.0, 8.0, 12.0, 3.0);
  let _ = stabilizer.update(good);
  stabilizer.reset();

  // After reset the first frame is treated as fresh — no hold.
  let origin = make_frame(0.0, 0.0, 0.0, 0.0);
  let out = stabilizer.update(origin);
  assert_eq!(out, origin, "first frame after reset should pass through");
 }

 #[test]
 fn set_enabled_false_resets_and_disables() {
  let mut stabilizer = TrackingFrameStabilizer::new(true);
  let _ = stabilizer.update(make_frame(20.0, 8.0, 0.0, 0.0));

  stabilizer.set_enabled(false);
  // Should pass origin spike right through when disabled.
  let spike = make_frame(0.0, 0.0, 0.0, 0.0);
  let out = stabilizer.update(spike);
  assert_eq!(out, spike);
 }
}
