use unvet_core::{AppResult, model::OutputFrame, ports::OutputBackend};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisState {
 Negative,
 Neutral,
 Positive,
}

pub struct KeyboardBackend {
 enabled: bool,
 press_threshold: f32,
 release_threshold: f32,
 x_state: AxisState,
 y_state: AxisState,
}

impl Default for KeyboardBackend {
 fn default() -> Self {
  Self {
   enabled: true,
   press_threshold: 0.35,
   release_threshold: 0.2,
   x_state: AxisState::Neutral,
   y_state: AxisState::Neutral,
  }
 }
}

impl KeyboardBackend {
 fn next_state(value: f32, current: AxisState, press: f32, release: f32) -> AxisState {
  match current {
   AxisState::Negative => {
    if value >= -release {
     AxisState::Neutral
    } else {
     AxisState::Negative
    }
   },
   AxisState::Positive => {
    if value <= release {
     AxisState::Neutral
    } else {
     AxisState::Positive
    }
   },
   AxisState::Neutral => {
    if value <= -press {
     AxisState::Negative
    } else if value >= press {
     AxisState::Positive
    } else {
     AxisState::Neutral
    }
   },
  }
 }
}

impl OutputBackend for KeyboardBackend {
 fn backend_name(&self) -> &'static str {
  "keyboard"
 }

 fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  if !self.enabled || !frame.active {
   self.x_state = AxisState::Neutral;
   self.y_state = AxisState::Neutral;
   return Ok(());
  }

  self.x_state = Self::next_state(frame.look_yaw_norm, self.x_state, self.press_threshold, self.release_threshold);
  self.y_state = Self::next_state(frame.look_pitch_norm, self.y_state, self.press_threshold, self.release_threshold);
  Ok(())
 }

 fn set_enabled(&mut self, enabled: bool) {
  self.enabled = enabled;
 }

 fn is_enabled(&self) -> bool {
  self.enabled
 }
}
