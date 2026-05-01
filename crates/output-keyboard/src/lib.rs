use std::mem::size_of;

use unvet_core::{AppError, AppResult, model::OutputFrame, ports::OutputBackend};

const VK_A: u16 = 0x41;
const VK_D: u16 = 0x44;
const VK_W: u16 = 0x57;
const VK_S: u16 = 0x53;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisState {
 Negative,
 Neutral,
 Positive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyEventKind {
 Down,
 Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyEvent {
 virtual_key: u16,
 kind: KeyEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Keybinds {
 left: u16,
 right: u16,
 up: u16,
 down: u16,
}

impl Default for Keybinds {
 fn default() -> Self {
  Self {
   left: VK_A,
   right: VK_D,
   up: VK_W,
   down: VK_S,
  }
 }
}

pub struct KeyboardBackend {
 enabled: bool,
 press_threshold: f32,
 release_threshold: f32,
 x_state: AxisState,
 y_state: AxisState,
 keybinds: Keybinds,
}

impl Default for KeyboardBackend {
 fn default() -> Self {
  Self {
   enabled: true,
   press_threshold: 0.35,
   release_threshold: 0.2,
   x_state: AxisState::Neutral,
   y_state: AxisState::Neutral,
   keybinds: Keybinds::default(),
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

 fn transition_events(previous: AxisState, next: AxisState, negative_key: u16, positive_key: u16) -> [Option<KeyEvent>; 2] {
  let mut events = [None, None];
  let mut index = 0;

  if previous == AxisState::Negative && next != AxisState::Negative {
   events[index] = Some(KeyEvent {
    virtual_key: negative_key,
    kind: KeyEventKind::Up,
   });
   index += 1;
  }

  if previous == AxisState::Positive && next != AxisState::Positive {
   events[index] = Some(KeyEvent {
    virtual_key: positive_key,
    kind: KeyEventKind::Up,
   });
   index += 1;
  }

  if next == AxisState::Negative && previous != AxisState::Negative {
   events[index] = Some(KeyEvent {
    virtual_key: negative_key,
    kind: KeyEventKind::Down,
   });
   index += 1;
  }

  if next == AxisState::Positive && previous != AxisState::Positive {
   events[index] = Some(KeyEvent {
    virtual_key: positive_key,
    kind: KeyEventKind::Down,
   });
  }

  events
 }

 fn collect_events_for_axis(events: &mut Vec<KeyEvent>, previous: AxisState, next: AxisState, negative_key: u16, positive_key: u16) {
  for event in Self::transition_events(previous, next, negative_key, positive_key)
   .into_iter()
   .flatten()
  {
   events.push(event);
  }
 }

 fn collect_release_events(&self) -> Vec<KeyEvent> {
  let mut events = Vec::with_capacity(4);
  Self::collect_events_for_axis(
   &mut events,
   self.x_state,
   AxisState::Neutral,
   self.keybinds.left,
   self.keybinds.right,
  );
  Self::collect_events_for_axis(&mut events, self.y_state, AxisState::Neutral, self.keybinds.up, self.keybinds.down);
  events
 }

 fn collect_frame_events(&self, frame: OutputFrame) -> (AxisState, AxisState, Vec<KeyEvent>) {
  let mut events = Vec::with_capacity(4);

  let next_x = if frame.active {
   Self::next_state(frame.look_yaw_norm, self.x_state, self.press_threshold, self.release_threshold)
  } else {
   AxisState::Neutral
  };
  let next_y = if frame.active {
   Self::next_state(frame.look_pitch_norm, self.y_state, self.press_threshold, self.release_threshold)
  } else {
   AxisState::Neutral
  };

  Self::collect_events_for_axis(&mut events, self.x_state, next_x, self.keybinds.left, self.keybinds.right);
  Self::collect_events_for_axis(&mut events, self.y_state, next_y, self.keybinds.up, self.keybinds.down);

  (next_x, next_y, events)
 }

 fn send_key_events(events: &[KeyEvent]) -> AppResult<()> {
  for event in events {
   send_key_event_platform(event.virtual_key, event.kind)?;
  }
  Ok(())
 }
}

impl OutputBackend for KeyboardBackend {
 fn backend_name(&self) -> &'static str {
  "keyboard"
 }

 fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  if !self.enabled {
   let release_events = self.collect_release_events();
   Self::send_key_events(&release_events)?;
   self.x_state = AxisState::Neutral;
   self.y_state = AxisState::Neutral;
   return Ok(());
  }

  let (next_x, next_y, events) = self.collect_frame_events(frame);
  Self::send_key_events(&events)?;
  self.x_state = next_x;
  self.y_state = next_y;
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
fn send_key_event_platform(virtual_key: u16, kind: KeyEventKind) -> AppResult<()> {
 const INPUT_KEYBOARD: u32 = 1;
 const KEYEVENTF_KEYUP: u32 = 0x0002;

 #[repr(C)]
 struct KeyboardInput {
  virtual_key: u16,
  scan_code: u16,
  flags: u32,
  time: u32,
  extra_info: usize,
 }

 #[repr(C)]
 struct Input {
  input_type: u32,
  keyboard: KeyboardInput,
 }

 #[link(name = "User32")]
 unsafe extern "system" {
  fn SendInput(input_count: u32, inputs: *const Input, input_size: i32) -> u32;
 }

 let flags = match kind {
  KeyEventKind::Down => 0,
  KeyEventKind::Up => KEYEVENTF_KEYUP,
 };
 let input = Input {
  input_type: INPUT_KEYBOARD,
  keyboard: KeyboardInput {
   virtual_key,
   scan_code: 0,
   flags,
   time: 0,
   extra_info: 0,
  },
 };

 let wrote = unsafe { SendInput(1, &input as *const Input, size_of::<Input>() as i32) };
 if wrote != 1 {
  return Err(AppError::InvalidState("failed to send keyboard input".to_owned()));
 }

 Ok(())
}

#[cfg(not(windows))]
fn send_key_event_platform(_virtual_key: u16, _kind: KeyEventKind) -> AppResult<()> {
 Ok(())
}

#[cfg(test)]
mod tests {
 use super::{AxisState, KeyEvent, KeyEventKind, KeyboardBackend, VK_A, VK_D, VK_S, VK_W};
 use unvet_core::model::OutputFrame;

 #[test]
 fn next_state_enters_direction_from_neutral() {
  let state = KeyboardBackend::next_state(-0.4, AxisState::Neutral, 0.35, 0.2);
  assert_eq!(state, AxisState::Negative);

  let state = KeyboardBackend::next_state(0.5, AxisState::Neutral, 0.35, 0.2);
  assert_eq!(state, AxisState::Positive);
 }

 #[test]
 fn next_state_uses_hysteresis_for_release() {
  let held_negative = KeyboardBackend::next_state(-0.21, AxisState::Negative, 0.35, 0.2);
  assert_eq!(held_negative, AxisState::Negative);

  let released_negative = KeyboardBackend::next_state(-0.19, AxisState::Negative, 0.35, 0.2);
  assert_eq!(released_negative, AxisState::Neutral);
 }

 #[test]
 fn transition_negative_to_positive_releases_then_presses() {
  let events = KeyboardBackend::transition_events(AxisState::Negative, AxisState::Positive, VK_A, VK_D);
  assert_eq!(
   events,
   [
    Some(KeyEvent {
     virtual_key: VK_A,
     kind: KeyEventKind::Up,
    }),
    Some(KeyEvent {
     virtual_key: VK_D,
     kind: KeyEventKind::Down,
    }),
   ]
  );
 }

 #[test]
 fn collect_frame_events_maps_yaw_and_pitch_to_wasd() {
  let backend = KeyboardBackend::default();
  let (next_x, next_y, events) = backend.collect_frame_events(OutputFrame {
   look_yaw_norm: 0.6,
   look_pitch_norm: -0.7,
   confidence: 1.0,
   active: true,
   ..OutputFrame::default()
  });

  assert_eq!(next_x, AxisState::Positive);
  assert_eq!(next_y, AxisState::Negative);
  assert_eq!(
   events,
   vec![
    KeyEvent {
     virtual_key: VK_D,
     kind: KeyEventKind::Down,
    },
    KeyEvent {
     virtual_key: VK_W,
     kind: KeyEventKind::Down,
    },
   ]
  );
 }

 #[test]
 fn inactive_frame_releases_pressed_keys() {
  let backend = KeyboardBackend {
   x_state: AxisState::Negative,
   y_state: AxisState::Positive,
   ..KeyboardBackend::default()
  };
  let (next_x, next_y, events) = backend.collect_frame_events(OutputFrame {
   look_yaw_norm: -0.9,
   look_pitch_norm: 0.9,
   confidence: 0.0,
   active: false,
   ..OutputFrame::default()
  });

  assert_eq!(next_x, AxisState::Neutral);
  assert_eq!(next_y, AxisState::Neutral);
  assert_eq!(
   events,
   vec![
    KeyEvent {
     virtual_key: VK_A,
     kind: KeyEventKind::Up,
    },
    KeyEvent {
     virtual_key: VK_S,
     kind: KeyEventKind::Up,
    },
   ]
  );
 }
}
