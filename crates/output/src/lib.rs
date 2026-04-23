use unvet_config::OutputBackendKind;
use unvet_core::{AppError, AppResult, model::OutputFrame, ports::OutputBackend};

struct BackendEntry {
 kind: OutputBackendKind,
 backend: Box<dyn OutputBackend>,
}

pub struct OutputBackendLayer {
 enabled: bool,
 active_backend: OutputBackendKind,
 backends: Vec<BackendEntry>,
}

impl OutputBackendLayer {
 pub fn new(active_backend: OutputBackendKind) -> Self {
  Self::with_backends(
   active_backend,
   vec![
    (OutputBackendKind::Ets2, Box::new(unvet_output_ets2::Ets2Backend::default())),
    (OutputBackendKind::Mouse, Box::new(unvet_output_mouse::MouseBackend::default())),
    (
     OutputBackendKind::Keyboard,
     Box::new(unvet_output_keyboard::KeyboardBackend::default()),
    ),
   ],
  )
  .expect("default output backend layer should be valid")
 }

 pub fn with_backends(requested_backend: OutputBackendKind, backends: Vec<(OutputBackendKind, Box<dyn OutputBackend>)>) -> AppResult<Self> {
  if backends.is_empty() {
   return Err(AppError::InvalidState(
    "output backend layer requires at least one backend".to_owned(),
   ));
  }

  let mut entries = Vec::with_capacity(backends.len());
  for (kind, backend) in backends {
   entries.push(BackendEntry { kind, backend });
  }

  let active_backend = if entries.iter().any(|entry| entry.kind == requested_backend) {
   requested_backend
  } else {
   entries[0].kind
  };

  let mut layer = Self {
   enabled: true,
   active_backend,
   backends: entries,
  };
  layer.sync_enabled_state();
  Ok(layer)
 }

 pub fn active_backend_kind(&self) -> OutputBackendKind {
  self.active_backend
 }

 pub fn active_backend_name(&self) -> AppResult<&'static str> {
  let index = self.find_backend_index(self.active_backend)?;
  Ok(self.backends[index].backend.backend_name())
 }

 pub fn is_enabled(&self) -> bool {
  self.enabled
 }

 pub fn set_active_backend(&mut self, next_backend: OutputBackendKind) -> AppResult<()> {
  if self.active_backend == next_backend {
   return Ok(());
  }

  let _ = self.find_backend_index(next_backend)?;
  if self.enabled {
   self.deactivate_backend(self.active_backend)?;
  }

  self.active_backend = next_backend;
  self.sync_enabled_state();
  Ok(())
 }

 pub fn set_enabled(&mut self, enabled: bool) -> AppResult<()> {
  if self.enabled == enabled {
   return Ok(());
  }

  if self.enabled && !enabled {
   self.deactivate_backend(self.active_backend)?;
  }

  self.enabled = enabled;
  self.sync_enabled_state();
  Ok(())
 }

 pub fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  let index = self.find_backend_index(self.active_backend)?;
  self.backends[index].backend.apply(frame)
 }

 fn deactivate_backend(&mut self, kind: OutputBackendKind) -> AppResult<()> {
  let index = self.find_backend_index(kind)?;
  let backend = self.backends[index].backend.as_mut();
  backend.set_enabled(false);
  backend.apply(OutputFrame::default())
 }

 fn sync_enabled_state(&mut self) {
  for entry in &mut self.backends {
   let enabled = self.enabled && entry.kind == self.active_backend;
   entry.backend.set_enabled(enabled);
  }
 }

 fn find_backend_index(&self, kind: OutputBackendKind) -> AppResult<usize> {
  self
   .backends
   .iter()
   .position(|entry| entry.kind == kind)
   .ok_or_else(|| AppError::InvalidState(format!("output backend is not registered: {:?}", kind)))
 }
}

#[cfg(test)]
mod tests {
 use std::sync::{Arc, Mutex};

 use super::OutputBackendLayer;
 use unvet_config::OutputBackendKind;
 use unvet_core::{AppResult, model::OutputFrame, ports::OutputBackend};

 #[derive(Debug, Clone, Default)]
 struct MockBackendState {
  enabled_history: Vec<bool>,
  applied_frames: Vec<OutputFrame>,
 }

 struct MockBackend {
  name: &'static str,
  state: Arc<Mutex<MockBackendState>>,
  enabled: bool,
 }

 impl MockBackend {
  fn new(name: &'static str, state: Arc<Mutex<MockBackendState>>) -> Self {
   Self {
    name,
    state,
    enabled: true,
   }
  }
 }

 impl OutputBackend for MockBackend {
  fn backend_name(&self) -> &'static str {
   self.name
  }

  fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
   let mut state = self.state.lock().expect("mock backend state lock");
   state.applied_frames.push(frame);
   Ok(())
  }

  fn set_enabled(&mut self, enabled: bool) {
   self.enabled = enabled;
   let mut state = self.state.lock().expect("mock backend state lock");
   state.enabled_history.push(enabled);
  }

  fn is_enabled(&self) -> bool {
   self.enabled
  }
 }

 fn sample_frame() -> OutputFrame {
  OutputFrame {
   look_yaw_norm: 0.7,
   look_pitch_norm: -0.4,
   confidence: 1.0,
   active: true,
  }
 }

 fn snapshot(state: &Arc<Mutex<MockBackendState>>) -> MockBackendState {
  state.lock().expect("mock backend state lock").clone()
 }

 #[test]
 fn with_backends_rejects_empty_registration() {
  let result = OutputBackendLayer::with_backends(OutputBackendKind::Ets2, Vec::new());
  assert!(result.is_err());
 }

 #[test]
 fn apply_routes_to_active_backend_only() {
  let ets2_state = Arc::new(Mutex::new(MockBackendState::default()));
  let mouse_state = Arc::new(Mutex::new(MockBackendState::default()));
  let keyboard_state = Arc::new(Mutex::new(MockBackendState::default()));

  let mut layer = OutputBackendLayer::with_backends(
   OutputBackendKind::Mouse,
   vec![
    (OutputBackendKind::Ets2, Box::new(MockBackend::new("ets2", ets2_state.clone()))),
    (OutputBackendKind::Mouse, Box::new(MockBackend::new("mouse", mouse_state.clone()))),
    (
     OutputBackendKind::Keyboard,
     Box::new(MockBackend::new("keyboard", keyboard_state.clone())),
    ),
   ],
  )
  .expect("build output backend layer");

  layer.apply(sample_frame()).expect("apply frame to active backend");

  assert_eq!(snapshot(&ets2_state).applied_frames.len(), 0);
  assert_eq!(snapshot(&mouse_state).applied_frames.len(), 1);
  assert_eq!(snapshot(&keyboard_state).applied_frames.len(), 0);
 }

 #[test]
 fn switching_backend_deactivates_previous_backend() {
  let ets2_state = Arc::new(Mutex::new(MockBackendState::default()));
  let mouse_state = Arc::new(Mutex::new(MockBackendState::default()));
  let keyboard_state = Arc::new(Mutex::new(MockBackendState::default()));

  let mut layer = OutputBackendLayer::with_backends(
   OutputBackendKind::Keyboard,
   vec![
    (OutputBackendKind::Ets2, Box::new(MockBackend::new("ets2", ets2_state.clone()))),
    (OutputBackendKind::Mouse, Box::new(MockBackend::new("mouse", mouse_state.clone()))),
    (
     OutputBackendKind::Keyboard,
     Box::new(MockBackend::new("keyboard", keyboard_state.clone())),
    ),
   ],
  )
  .expect("build output backend layer");

  layer.apply(sample_frame()).expect("apply on keyboard backend");
  layer.set_active_backend(OutputBackendKind::Ets2).expect("switch active backend");

  let keyboard = snapshot(&keyboard_state);
  assert_eq!(keyboard.applied_frames.len(), 2);
  assert_eq!(keyboard.applied_frames[1], OutputFrame::default());

  let ets2 = snapshot(&ets2_state);
  assert_eq!(ets2.enabled_history.last().copied(), Some(true));
 }

 #[test]
 fn disabling_layer_flushes_active_backend_once() {
  let ets2_state = Arc::new(Mutex::new(MockBackendState::default()));
  let mouse_state = Arc::new(Mutex::new(MockBackendState::default()));
  let keyboard_state = Arc::new(Mutex::new(MockBackendState::default()));

  let mut layer = OutputBackendLayer::with_backends(
   OutputBackendKind::Keyboard,
   vec![
    (OutputBackendKind::Ets2, Box::new(MockBackend::new("ets2", ets2_state))),
    (OutputBackendKind::Mouse, Box::new(MockBackend::new("mouse", mouse_state))),
    (
     OutputBackendKind::Keyboard,
     Box::new(MockBackend::new("keyboard", keyboard_state.clone())),
    ),
   ],
  )
  .expect("build output backend layer");

  layer.apply(sample_frame()).expect("apply on keyboard backend");
  layer.set_enabled(false).expect("disable output layer");

  let keyboard = snapshot(&keyboard_state);
  assert_eq!(keyboard.applied_frames.len(), 2);
  assert_eq!(keyboard.applied_frames[1], OutputFrame::default());
  assert_eq!(keyboard.enabled_history.last().copied(), Some(false));
 }
}
