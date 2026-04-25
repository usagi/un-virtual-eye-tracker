use unvet_config::{OutputBackendKind, OutputConfig, OutputSendFilterConfig, OutputSendFilterMode};
use unvet_core::{AppError, AppResult, model::OutputFrame, ports::OutputBackend};

struct BackendEntry {
 kind: OutputBackendKind,
 backend: Box<dyn OutputBackend>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFilterStatus {
 pub allowed: bool,
 pub active_process_name: Option<String>,
}

pub struct OutputBackendLayer {
 enabled: bool,
 active_backend: OutputBackendKind,
 backends: Vec<BackendEntry>,
 send_filter: OutputSendFilterConfig,
 send_gate_open: bool,
}

impl OutputBackendLayer {
 pub fn new(output: &OutputConfig) -> Self {
  Self::with_backends(
   output.backend,
   output.send_filter.clone(),
   vec![
    (OutputBackendKind::Ets2, Box::new(unvet_output_ets2::Ets2Backend::default())),
    (OutputBackendKind::Mouse, Box::new(unvet_output_mouse::MouseBackend::default())),
    (
     OutputBackendKind::Keyboard,
     Box::new(unvet_output_keyboard::KeyboardBackend::default()),
    ),
    (OutputBackendKind::Touch, Box::new(unvet_output_touch::TouchBackend::default())),
   ],
  )
  .expect("default output backend layer should be valid")
 }

 pub fn with_backends(
  requested_backend: OutputBackendKind,
  send_filter: OutputSendFilterConfig,
  backends: Vec<(OutputBackendKind, Box<dyn OutputBackend>)>,
 ) -> AppResult<Self> {
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
   send_filter,
   send_gate_open: false,
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

 pub fn send_filter(&self) -> &OutputSendFilterConfig {
  &self.send_filter
 }

 pub fn send_filter_status(&self) -> AppResult<SendFilterStatus> {
  send_filter_status_inner(&self.send_filter)
 }

 pub fn set_send_filter(&mut self, send_filter: OutputSendFilterConfig) -> AppResult<()> {
  self.send_filter = send_filter;
  if self.enabled && self.send_gate_open {
   self.deactivate_backend(self.active_backend)?;
   self.send_gate_open = false;
  }
  Ok(())
 }

 pub fn set_active_backend(&mut self, next_backend: OutputBackendKind) -> AppResult<()> {
  if self.active_backend == next_backend {
   return Ok(());
  }

  let _ = self.find_backend_index(next_backend)?;
  if self.enabled {
   self.deactivate_backend(self.active_backend)?;
   self.send_gate_open = false;
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
   self.send_gate_open = false;
  }

  self.enabled = enabled;
  self.sync_enabled_state();
  Ok(())
 }

 pub fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  if !self.enabled {
   return Ok(());
  }

  let index = self.find_backend_index(self.active_backend)?;
  let send_filter_status = self.send_filter_status()?;
  if !send_filter_status.allowed {
   if self.send_gate_open {
    self.backends[index].backend.apply(OutputFrame::default())?;
    self.send_gate_open = false;
   }
   return Ok(());
  }

  self.send_gate_open = true;
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

pub fn list_running_process_names() -> AppResult<Vec<String>> {
 running_process_names_platform()
}

fn send_filter_status_inner(filter: &OutputSendFilterConfig) -> AppResult<SendFilterStatus> {
 match filter.mode {
  OutputSendFilterMode::Unrestricted => Ok(SendFilterStatus {
   allowed: true,
   active_process_name: None,
  }),
  OutputSendFilterMode::ForegroundProcess => {
   let active_process_name = foreground_process_name_platform()?;
   Ok(SendFilterStatus {
    allowed: process_name_matches_filter(filter, active_process_name.as_deref()),
    active_process_name,
   })
  },
 }
}

fn process_name_matches_filter(filter: &OutputSendFilterConfig, active_process_name: Option<&str>) -> bool {
 match filter.mode {
  OutputSendFilterMode::Unrestricted => true,
  OutputSendFilterMode::ForegroundProcess => {
   let active = match active_process_name.and_then(normalize_process_name) {
    Some(name) => name,
    None => return false,
   };

   filter
    .process_names
    .iter()
    .filter_map(|name| normalize_process_name(name))
    .any(|name| name == active)
  },
 }
}

fn normalize_process_name(name: &str) -> Option<String> {
 let trimmed = name.trim();
 if trimmed.is_empty() {
  return None;
 }

 let normalized_path = trimmed.replace('/', "\\");
 let file_name = normalized_path.rsplit('\\').next().unwrap_or(trimmed).trim();
 if file_name.is_empty() {
  return None;
 }

 Some(file_name.to_ascii_lowercase())
}

#[cfg(windows)]
fn foreground_process_name_platform() -> AppResult<Option<String>> {
 use std::ffi::c_void;

 const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

 #[link(name = "User32")]
 unsafe extern "system" {
  fn GetForegroundWindow() -> *mut c_void;
  fn GetWindowThreadProcessId(hwnd: *mut c_void, process_id: *mut u32) -> u32;
 }

 #[link(name = "Kernel32")]
 unsafe extern "system" {
  fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
  fn QueryFullProcessImageNameW(process: *mut c_void, flags: u32, exe_name: *mut u16, size: *mut u32) -> i32;
  fn CloseHandle(handle: *mut c_void) -> i32;
 }

 let hwnd = unsafe { GetForegroundWindow() };
 if hwnd.is_null() {
  return Ok(None);
 }

 let mut process_id = 0u32;
 unsafe {
  GetWindowThreadProcessId(hwnd, &mut process_id);
 }
 if process_id == 0 {
  return Ok(None);
 }

 let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
 if process.is_null() {
  return Ok(None);
 }

 let mut buffer = [0u16; 1024];
 let mut size = buffer.len() as u32;
 let queried = unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size) };
 unsafe {
  CloseHandle(process);
 }

 if queried == 0 || size == 0 {
  return Ok(None);
 }

 let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
 Ok(normalize_process_name(&full_path))
}

#[cfg(windows)]
fn running_process_names_platform() -> AppResult<Vec<String>> {
 use std::ffi::c_void;

 const TH32CS_SNAPPROCESS: u32 = 0x00000002;
 const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

 #[repr(C)]
 struct ProcessEntry32W {
  size: u32,
  usage_count: u32,
  process_id: u32,
  default_heap_id: usize,
  module_id: u32,
  thread_count: u32,
  parent_process_id: u32,
  priority_class_base: i32,
  flags: u32,
  exe_file: [u16; 260],
 }

 #[link(name = "Kernel32")]
 unsafe extern "system" {
  fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
  fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
  fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
  fn CloseHandle(handle: *mut c_void) -> i32;
 }

 let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
 if snapshot == INVALID_HANDLE_VALUE {
  return Ok(Vec::new());
 }

 let mut entry = unsafe { std::mem::zeroed::<ProcessEntry32W>() };
 entry.size = size_of::<ProcessEntry32W>() as u32;

 let mut names = Vec::new();
 let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry as *mut ProcessEntry32W) } != 0;
 while has_entry {
  let name_len = entry.exe_file.iter().position(|ch| *ch == 0).unwrap_or(entry.exe_file.len());
  let raw_name = String::from_utf16_lossy(&entry.exe_file[..name_len]);
  if let Some(name) = normalize_process_name(&raw_name) {
   names.push(name);
  }
  has_entry = unsafe { Process32NextW(snapshot, &mut entry as *mut ProcessEntry32W) } != 0;
 }

 unsafe {
  CloseHandle(snapshot);
 }

 names.sort();
 names.dedup();
 Ok(names)
}

#[cfg(not(windows))]
fn foreground_process_name_platform() -> AppResult<Option<String>> {
 Ok(None)
}

#[cfg(not(windows))]
fn running_process_names_platform() -> AppResult<Vec<String>> {
 Ok(Vec::new())
}

#[cfg(test)]
mod tests {
 use std::sync::{Arc, Mutex};

 use super::{OutputBackendLayer, process_name_matches_filter};
 use unvet_config::{OutputBackendKind, OutputSendFilterConfig, OutputSendFilterMode};
 use unvet_core::{AppResult, model::OutputFrame, ports::OutputBackend};

 fn unrestricted_filter() -> OutputSendFilterConfig {
  OutputSendFilterConfig::default()
 }

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
  let result = OutputBackendLayer::with_backends(OutputBackendKind::Ets2, unrestricted_filter(), Vec::new());
  assert!(result.is_err());
 }

 #[test]
 fn apply_routes_to_active_backend_only() {
  let ets2_state = Arc::new(Mutex::new(MockBackendState::default()));
  let mouse_state = Arc::new(Mutex::new(MockBackendState::default()));
  let keyboard_state = Arc::new(Mutex::new(MockBackendState::default()));

  let mut layer = OutputBackendLayer::with_backends(
   OutputBackendKind::Mouse,
   unrestricted_filter(),
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
   unrestricted_filter(),
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
   unrestricted_filter(),
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

 #[test]
 fn process_filter_accepts_case_insensitive_process_name() {
  let filter = OutputSendFilterConfig {
   mode: OutputSendFilterMode::ForegroundProcess,
   process_names: vec!["EuroTrucks2.exe".to_owned(), "amtrucks.exe".to_owned()],
  };

  assert!(process_name_matches_filter(&filter, Some(r#"C:\\Games\\SCS\\eurotrucks2.exe"#),));
  assert!(process_name_matches_filter(&filter, Some("AMTRUCKS.EXE")));
  assert!(!process_name_matches_filter(&filter, Some("notepad.exe")));
 }

 #[test]
 fn process_filter_blocks_when_no_target_process_is_configured() {
  let filter = OutputSendFilterConfig {
   mode: OutputSendFilterMode::ForegroundProcess,
   process_names: Vec::new(),
  };

  assert!(!process_name_matches_filter(&filter, Some("eurotrucks2.exe")));
 }
}
