use unvet_core::{AppResult, model::OutputFrame, ports::OutputBackend};

const FREETRACK_MAX_YAW_DEG: f32 = 75.0;
const FREETRACK_MAX_PITCH_DEG: f32 = 45.0;
const ETS2_GAME_ID: i32 = 13602;
const ATS_GAME_ID: i32 = 13603;
#[cfg(test)]
const ETS2_FTN_ID: &str = "00721B4BFF8B3EA3296600";
#[cfg(test)]
const ATS_FTN_ID: &str = "0243F6F5140400655ED300";

#[cfg(test)]
fn parse_hex_nibble(byte: u8) -> Option<u8> {
 match byte {
  b'0'..=b'9' => Some(byte - b'0'),
  b'a'..=b'f' => Some(byte - b'a' + 10),
  b'A'..=b'F' => Some(byte - b'A' + 10),
  _ => None,
 }
}

#[cfg(test)]
fn parse_freetrack_table_from_ftn_id(ftn_id: &str) -> Option<[u8; 8]> {
 let id_bytes = ftn_id.as_bytes();
 if id_bytes.len() != 22 {
  return None;
 }

 let mut raw = [0u8; 11];
 for (index, slot) in raw.iter_mut().enumerate() {
  let high = parse_hex_nibble(id_bytes[index * 2])?;
  let low = parse_hex_nibble(id_bytes[index * 2 + 1])?;
  *slot = (high << 4) | low;
 }

 // Match opentrack csv/do_scanf FTN_ID byte order.
 Some([raw[5], raw[4], raw[3], raw[2], raw[9], raw[8], raw[7], raw[6]])
}

fn freetrack_table_for_game_id(game_id: i32) -> [u8; 8] {
 match game_id {
  // ETS2/ATS entries are V160 in the FaceTrackNoIR game list, which keeps
  // encryption keys disabled (zero table) and expects plain NP payload.
  ETS2_GAME_ID | ATS_GAME_ID => [0; 8],
  _ => [0; 8],
 }
}

fn infer_game_id_from_process_names<'a>(process_names: impl IntoIterator<Item = &'a str>) -> Option<i32> {
 let mut has_ets2 = false;
 let mut has_ats = false;

 for name in process_names {
  let lowered = name.trim().to_ascii_lowercase();
  if lowered.ends_with("amtrucks.exe") {
   has_ats = true;
  }
  if lowered.ends_with("eurotrucks2.exe") {
   has_ets2 = true;
  }
 }

 if has_ats {
  Some(ATS_GAME_ID)
 } else if has_ets2 {
  Some(ETS2_GAME_ID)
 } else {
  None
 }
}

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

#[derive(Debug, Clone, Copy, PartialEq)]
struct FreeTrackPose {
 yaw: f32,
 pitch: f32,
 roll: f32,
 x: f32,
 y: f32,
 z: f32,
 raw_yaw: f32,
 raw_pitch: f32,
 raw_roll: f32,
 raw_x: f32,
 raw_y: f32,
 raw_z: f32,
}

pub struct Ets2Backend {
 enabled: bool,
 response: TruckSimResponse,
 apply_response: bool,
 max_yaw_deg: f32,
 max_pitch_deg: f32,
 last_command: TruckSimCommand,
 writer: Option<FreeTrackWriter>,
}

impl Default for Ets2Backend {
 fn default() -> Self {
  Self {
   enabled: true,
   response: TruckSimResponse::default(),
   apply_response: true,
   max_yaw_deg: FREETRACK_MAX_YAW_DEG,
   max_pitch_deg: FREETRACK_MAX_PITCH_DEG,
   last_command: TruckSimCommand::neutral(),
   writer: None,
  }
 }
}

impl Ets2Backend {
 pub fn relative_headtracking() -> Self {
  Self {
   apply_response: false,
   max_yaw_deg: 180.0,
   max_pitch_deg: 180.0,
   ..Self::default()
  }
 }

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

 fn frame_to_command(frame: OutputFrame, response: TruckSimResponse, apply_response: bool) -> TruckSimCommand {
  if !frame.active {
   return TruckSimCommand::neutral();
  }

  let camera_yaw = if apply_response {
   Self::map_axis(frame.look_yaw_norm, response.yaw_gain, response.deadzone, response.yaw_exponent)
  } else {
   frame.look_yaw_norm.clamp(-1.0, 1.0)
  };
  let camera_pitch = if apply_response {
   Self::map_axis(
    frame.look_pitch_norm,
    response.pitch_gain,
    response.deadzone,
    response.pitch_exponent,
   )
  } else {
   frame.look_pitch_norm.clamp(-1.0, 1.0)
  };

  let response = response.sanitized();
  let threshold = response.look_back_threshold;
  TruckSimCommand {
   camera_yaw,
   camera_pitch,
   look_back_left: apply_response && camera_yaw <= -threshold,
   look_back_right: apply_response && camera_yaw >= threshold,
  }
 }

 fn command_to_freetrack_pose(&self, command: TruckSimCommand) -> FreeTrackPose {
  const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

  let yaw_norm = if command.look_back_left {
   -1.0
  } else if command.look_back_right {
   1.0
  } else {
   command.camera_yaw
  }
  .clamp(-1.0, 1.0);
  let pitch_norm = command.camera_pitch.clamp(-1.0, 1.0);

  // FreeTrack-compatible values follow the same sign convention used by opentrack.
  let yaw = -yaw_norm * self.max_yaw_deg * DEG_TO_RAD;
  let pitch = -pitch_norm * self.max_pitch_deg * DEG_TO_RAD;
  let roll = 0.0;

  FreeTrackPose {
   yaw,
   pitch,
   roll,
   x: 0.0,
   y: 0.0,
   z: 0.0,
   raw_yaw: yaw,
   raw_pitch: pitch,
   raw_roll: roll,
   raw_x: 0.0,
   raw_y: 0.0,
   raw_z: 0.0,
  }
 }

 fn dispatch_command(&mut self, command: TruckSimCommand) -> AppResult<()> {
  if self.writer.is_none() {
   self.writer = Some(FreeTrackWriter::connect()?);
  }

  let pose = self.command_to_freetrack_pose(command);
  if let Some(writer) = self.writer.as_mut() {
   if let Err(error) = writer.write_pose(pose) {
    self.writer = None;
    return Err(error);
   }
  }

  Ok(())
 }
}

impl OutputBackend for Ets2Backend {
 fn backend_name(&self) -> &'static str {
  "ets2"
 }

 fn apply(&mut self, frame: OutputFrame) -> AppResult<()> {
  if !self.enabled {
   let neutral = TruckSimCommand::neutral();
   self.dispatch_command(neutral)?;
   self.last_command = neutral;
   return Ok(());
  }

  let command = Self::frame_to_command(frame, self.response, self.apply_response);
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
mod platform {
 use std::{ffi::c_void, mem::size_of, ptr};

 use super::FreeTrackPose;
 use unvet_core::{AppError, AppResult};

 const FREETRACK_HEAP: &str = "FT_SharedMem";
 const FREETRACK_MUTEX: &str = "FT_Mutext";

 const PAGE_READWRITE: u32 = 0x0000_0004;
 const FILE_MAP_WRITE: u32 = 0x0000_0002;
 const WAIT_OBJECT_0: u32 = 0x0000_0000;
 const WAIT_ABANDONED: u32 = 0x0000_0080;
 const WAIT_TIMEOUT: u32 = 0x0000_0102;
 const WAIT_LOCK_TIMEOUT_MS: u32 = 16;

 const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

 #[repr(C)]
 struct FreeTrackData {
  data_id: u32,
  cam_width: i32,
  cam_height: i32,
  yaw: f32,
  pitch: f32,
  roll: f32,
  x: f32,
  y: f32,
  z: f32,
  raw_yaw: f32,
  raw_pitch: f32,
  raw_roll: f32,
  raw_x: f32,
  raw_y: f32,
  raw_z: f32,
  x1: f32,
  y1: f32,
  x2: f32,
  y2: f32,
  x3: f32,
  y3: f32,
  x4: f32,
  y4: f32,
 }

 #[repr(C)]
 struct FreeTrackHeap {
  data: FreeTrackData,
  game_id: i32,
  table: [u8; 8],
  game_id2: i32,
 }

 pub struct FreeTrackWriter {
  map_handle: *mut c_void,
  mutex_handle: *mut c_void,
  heap_ptr: *mut FreeTrackHeap,
  last_game_id: i32,
 }

 unsafe impl Send for FreeTrackWriter {}

 fn infer_trucksim_game_id_platform() -> Option<i32> {
  const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;

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
  if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
   return None;
  }

  let mut entry = unsafe { std::mem::zeroed::<ProcessEntry32W>() };
  entry.size = size_of::<ProcessEntry32W>() as u32;

  let mut process_names = Vec::new();
  let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry as *mut ProcessEntry32W) } != 0;
  while has_entry {
   let name_len = entry.exe_file.iter().position(|ch| *ch == 0).unwrap_or(entry.exe_file.len());
   let raw_name = String::from_utf16_lossy(&entry.exe_file[..name_len]);
   process_names.push(raw_name);
   has_entry = unsafe { Process32NextW(snapshot, &mut entry as *mut ProcessEntry32W) } != 0;
  }

  unsafe {
   CloseHandle(snapshot);
  }

  super::infer_game_id_from_process_names(process_names.iter().map(String::as_str))
 }

 impl FreeTrackWriter {
  pub fn connect() -> AppResult<Self> {
   let map_name = wide_null(FREETRACK_HEAP);
   let map_handle = unsafe {
    CreateFileMappingW(
     INVALID_HANDLE_VALUE,
     ptr::null(),
     PAGE_READWRITE,
     0,
     size_of::<FreeTrackHeap>() as u32,
     map_name.as_ptr(),
    )
   };
   if map_handle.is_null() {
    return Err(AppError::InvalidState(format!(
     "failed to create/open FreeTrack shared memory (GetLastError={})",
     unsafe { GetLastError() }
    )));
   }

   let heap_view = unsafe { MapViewOfFile(map_handle, FILE_MAP_WRITE, 0, 0, size_of::<FreeTrackHeap>()) };
   if heap_view.is_null() {
    let error_code = unsafe { GetLastError() };
    unsafe {
     CloseHandle(map_handle);
    }
    return Err(AppError::InvalidState(format!(
     "failed to map FreeTrack shared memory (GetLastError={error_code})"
    )));
   }

   let mutex_name = wide_null(FREETRACK_MUTEX);
   let mutex_handle = unsafe { CreateMutexW(ptr::null(), 0, mutex_name.as_ptr()) };
   if mutex_handle.is_null() {
    let error_code = unsafe { GetLastError() };
    unsafe {
     UnmapViewOfFile(heap_view);
     CloseHandle(map_handle);
    }
    return Err(AppError::InvalidState(format!(
     "failed to create/open FreeTrack mutex (GetLastError={error_code})"
    )));
   }

   Ok(Self {
    map_handle,
    mutex_handle,
    heap_ptr: heap_view as *mut FreeTrackHeap,
    last_game_id: i32::MIN,
   })
  }

  pub fn write_pose(&mut self, pose: FreeTrackPose) -> AppResult<()> {
   let wait = unsafe { WaitForSingleObject(self.mutex_handle, WAIT_LOCK_TIMEOUT_MS) };
   if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
    if wait == WAIT_TIMEOUT {
     return Err(AppError::InvalidState(
      "timed out while waiting for FreeTrack shared memory mutex".to_owned(),
     ));
    }

    return Err(AppError::InvalidState(format!(
     "failed waiting for FreeTrack mutex (result={wait}, GetLastError={})",
     unsafe { GetLastError() }
    )));
   }

   unsafe {
    let heap = &mut *self.heap_ptr;
    let data = &mut heap.data;

    data.cam_width = 100;
    data.cam_height = 250;

    data.yaw = pose.yaw;
    data.pitch = pose.pitch;
    data.roll = pose.roll;
    data.x = pose.x;
    data.y = pose.y;
    data.z = pose.z;

    data.raw_yaw = pose.raw_yaw;
    data.raw_pitch = pose.raw_pitch;
    data.raw_roll = pose.raw_roll;
    data.raw_x = pose.raw_x;
    data.raw_y = pose.raw_y;
    data.raw_z = pose.raw_z;

    data.x1 = 0.0;
    data.y1 = 0.0;
    data.x2 = 0.0;
    data.y2 = 0.0;
    data.x3 = 0.0;
    data.y3 = 0.0;
    data.x4 = 0.0;
    data.y4 = 0.0;

    let mut game_id = heap.game_id;
    if game_id == 0 {
     if let Some(inferred_game_id) = infer_trucksim_game_id_platform() {
      heap.game_id = inferred_game_id;
      game_id = inferred_game_id;
     }
    }

    if game_id != self.last_game_id {
     // Match FreeTrack handshake semantics when a game connects or switches IDs.
     heap.table = super::freetrack_table_for_game_id(game_id);
     heap.game_id2 = game_id;
     data.data_id = 0;
     self.last_game_id = game_id;
    } else {
     data.data_id = data.data_id.wrapping_add(1);
    }

    ReleaseMutex(self.mutex_handle);
   }

   Ok(())
  }
 }

 impl Drop for FreeTrackWriter {
  fn drop(&mut self) {
   unsafe {
    if !self.heap_ptr.is_null() {
     UnmapViewOfFile(self.heap_ptr as *const c_void);
    }
    if !self.mutex_handle.is_null() {
     CloseHandle(self.mutex_handle);
    }
    if !self.map_handle.is_null() {
     CloseHandle(self.map_handle);
    }
   }
  }
 }

 fn wide_null(value: &str) -> Vec<u16> {
  value.encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>()
 }

 #[link(name = "Kernel32")]
 unsafe extern "system" {
  fn CreateFileMappingW(
   file: *mut c_void,
   attributes: *const c_void,
   protect: u32,
   maximum_size_high: u32,
   maximum_size_low: u32,
   name: *const u16,
  ) -> *mut c_void;

  fn MapViewOfFile(
   file_mapping: *mut c_void,
   desired_access: u32,
   file_offset_high: u32,
   file_offset_low: u32,
   number_of_bytes_to_map: usize,
  ) -> *mut c_void;

  fn UnmapViewOfFile(base_address: *const c_void) -> i32;
  fn CloseHandle(handle: *mut c_void) -> i32;

  fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16) -> *mut c_void;
  fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
  fn ReleaseMutex(handle: *mut c_void) -> i32;

  fn GetLastError() -> u32;
 }
}

#[cfg(windows)]
use platform::FreeTrackWriter;

#[cfg(not(windows))]
struct FreeTrackWriter;

#[cfg(not(windows))]
impl FreeTrackWriter {
 fn connect() -> AppResult<Self> {
  Ok(Self)
 }

 fn write_pose(&mut self, _pose: FreeTrackPose) -> AppResult<()> {
  Ok(())
 }
}

#[cfg(test)]
mod tests {
 use super::{
  ATS_FTN_ID, ATS_GAME_ID, ETS2_FTN_ID, ETS2_GAME_ID, Ets2Backend, FREETRACK_MAX_PITCH_DEG, FREETRACK_MAX_YAW_DEG, TruckSimCommand,
  TruckSimPreset, TruckSimResponse, freetrack_table_for_game_id, infer_game_id_from_process_names, parse_freetrack_table_from_ftn_id,
 };
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

 #[test]
 fn command_to_freetrack_pose_respects_sign_and_range() {
  let pose = Ets2Backend::command_to_freetrack_pose(TruckSimCommand {
   camera_yaw: 1.0,
   camera_pitch: -1.0,
   look_back_left: false,
   look_back_right: false,
  });

  let expected_yaw = -FREETRACK_MAX_YAW_DEG.to_radians();
  let expected_pitch = FREETRACK_MAX_PITCH_DEG.to_radians();

  assert!((pose.yaw - expected_yaw).abs() < 0.0001);
  assert!((pose.pitch - expected_pitch).abs() < 0.0001);
 }

 #[test]
 fn command_to_freetrack_pose_look_back_overrides_yaw() {
  let left = Ets2Backend::command_to_freetrack_pose(TruckSimCommand {
   camera_yaw: 0.0,
   camera_pitch: 0.0,
   look_back_left: true,
   look_back_right: false,
  });
  let right = Ets2Backend::command_to_freetrack_pose(TruckSimCommand {
   camera_yaw: 0.0,
   camera_pitch: 0.0,
   look_back_left: false,
   look_back_right: true,
  });

  assert!((left.yaw - FREETRACK_MAX_YAW_DEG.to_radians()).abs() < 0.0001);
  assert!((right.yaw + FREETRACK_MAX_YAW_DEG.to_radians()).abs() < 0.0001);
 }

 #[test]
 fn parse_freetrack_table_follows_opentrack_order() {
  let ets2 = parse_freetrack_table_from_ftn_id(ETS2_FTN_ID).expect("parse ETS2 FTN_ID");
  let ats = parse_freetrack_table_from_ftn_id(ATS_FTN_ID).expect("parse ATS FTN_ID");

  assert_eq!(ets2, [0x8B, 0xFF, 0x4B, 0x1B, 0x66, 0x29, 0xA3, 0x3E]);
  assert_eq!(ats, [0x04, 0x14, 0xF5, 0xF6, 0xD3, 0x5E, 0x65, 0x00]);
 }

 #[test]
 fn freetrack_table_for_supported_trucksim_game_ids() {
  assert_eq!(freetrack_table_for_game_id(ETS2_GAME_ID), [0; 8]);
  assert_eq!(freetrack_table_for_game_id(ATS_GAME_ID), [0; 8]);
  assert_eq!(freetrack_table_for_game_id(0), [0; 8]);
 }

 #[test]
 fn infer_game_id_prefers_ats_then_ets2() {
  assert_eq!(
   infer_game_id_from_process_names(["notepad.exe", "eurotrucks2.exe"]),
   Some(ETS2_GAME_ID)
  );
  assert_eq!(
   infer_game_id_from_process_names(["eurotrucks2.exe", "amtrucks.exe"]),
   Some(ATS_GAME_ID)
  );
  assert_eq!(infer_game_id_from_process_names(["cmd.exe"]), None);
 }
}
