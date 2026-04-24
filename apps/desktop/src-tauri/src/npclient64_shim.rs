#![allow(non_snake_case)]

// NPClient-compatible behavior for interop. See THIRD_PARTY_NOTICES.md for
// attribution and license context for referenced compatibility materials.

use std::{
 ffi::c_void,
 fs::OpenOptions,
 io::Write,
 mem::size_of,
 ptr, slice,
 sync::atomic::{AtomicU32, Ordering},
};

const FT_MUTEX: &[u8] = b"FT_Mutext\0";
const FT_SHARED_MEM: &[u8] = b"FT_SharedMem\0";
const PAGE_READWRITE: u32 = 0x0000_0004;
const FILE_MAP_WRITE: u32 = 0x0000_0002;
const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

const TRACKIR_AXIS_MAX: f64 = 16383.0;
const TRACKIR_STATUS_OK: i16 = 0;
const TRACKIR_STATUS_DISABLED: i16 = 1;

const PART1_1_PREFIX: &[u8] = &[
 0x1d, 0x79, 0xce, 0x35, 0x1d, 0x95, 0x79, 0xdf, 0x4c, 0x8d, 0x55, 0xeb, 0x20, 0x17, 0x9f, 0x26, 0x3e, 0xf0, 0x88, 0x8e, 0x7a, 0x08, 0x11,
 0x52, 0xfc, 0xd8, 0x3f, 0xb9, 0xd2, 0x5c, 0x61, 0x03, 0x56, 0xfd, 0xbc, 0xb4, 0x0a, 0xf1, 0x13, 0x5d, 0x90, 0x0a, 0x0e, 0xee, 0x09, 0x19,
 0x45, 0x5a, 0xeb, 0xe3, 0xf0, 0x58, 0x5f, 0xac, 0x23, 0x84, 0x1f, 0xc5, 0xe3, 0xa6, 0x18, 0x5d, 0xb8, 0x47, 0xdc, 0xe6, 0xf2, 0x0b, 0x03,
 0x55, 0x61, 0xab, 0xe3, 0x57, 0xe3, 0x67, 0xcc, 0x16, 0x38, 0x3c, 0x11, 0x25, 0x88, 0x8a, 0x24, 0x7f, 0xf7, 0xeb, 0xf2, 0x5d, 0x82, 0x89,
 0x05, 0x53, 0x32, 0x6b, 0x28, 0x54, 0x13, 0xf6, 0xe7, 0x21, 0x1a, 0xc6, 0xe3, 0xe1, 0xff,
];

const PART1_2_PREFIX: &[u8] = &[
 0x6d, 0x0b, 0xab, 0x56, 0x74, 0xe6, 0x1c, 0xff, 0x24, 0xe8, 0x34, 0x8f, 0x00, 0x63, 0xed, 0x47, 0x5d, 0x9b, 0xe1, 0xe0, 0x1d, 0x02, 0x31,
 0x22, 0x89, 0xac, 0x1f, 0xc0, 0xbd, 0x29, 0x13, 0x23, 0x3e, 0x98, 0xdd, 0xd0, 0x2a, 0x98, 0x7d, 0x29, 0xff, 0x2a, 0x7a, 0x86, 0x6c, 0x39,
 0x22, 0x3b, 0x86, 0x86, 0xfa, 0x78, 0x31, 0xc3, 0x54, 0xa4, 0x78, 0xaa, 0xc3, 0xca, 0x77, 0x32, 0xd3, 0x67, 0xbd, 0x94, 0x9d, 0x7e, 0x6d,
 0x31, 0x6b, 0xa1, 0xc3, 0x14, 0x8c, 0x17, 0xb5, 0x64, 0x51, 0x5b, 0x79, 0x51, 0xa8, 0xcf, 0x5d, 0x1a, 0xb4, 0x84, 0x9c, 0x29, 0xf0, 0xe6,
 0x69, 0x73, 0x66, 0x0e, 0x4b, 0x3c, 0x7d, 0x99, 0x8b, 0x4e, 0x7d, 0xaf, 0x86, 0x92, 0xff,
];

const PART2_1_PREFIX: &[u8] = &[
 0x8b, 0x84, 0xfc, 0x8c, 0x71, 0xb5, 0xd9, 0xaa, 0xda, 0x32, 0xc7, 0xe9, 0x0c, 0x20, 0x40, 0xd4, 0x4b, 0x02, 0x89, 0xca, 0xde, 0x61, 0x9d,
 0xfb, 0xb3, 0x8c, 0x97, 0x8a, 0x13, 0x6a, 0x0f, 0xf8, 0xf8, 0x0d, 0x65, 0x1b, 0xe3, 0x05, 0x1e, 0xb6, 0xf6, 0xd9, 0x13, 0xad, 0xeb, 0x38,
 0xdd, 0x86, 0xfc, 0x59, 0x2e, 0xf6, 0x2e, 0xf4, 0xb0, 0xb0, 0xfd, 0xb0, 0x70, 0x23, 0xfb, 0xc9, 0x1a, 0x50, 0x89, 0x92, 0xf0, 0x01, 0x09,
 0xa1, 0xfd, 0x5b, 0x19, 0x29, 0x73, 0x59, 0x2b, 0x81, 0x83, 0x9e, 0x11, 0xf3, 0xa2, 0x1f, 0xc8, 0x24, 0x53, 0x60, 0x0a, 0x42, 0x78, 0x7a,
 0x39, 0xea, 0xc1, 0x59, 0xad, 0xc5, 0x00,
];

const PART2_2_PREFIX: &[u8] = &[
 0xe3, 0xe5, 0x8e, 0xe8, 0x06, 0xd4, 0xab, 0xcf, 0xfa, 0x51, 0xa6, 0x84, 0x69, 0x52, 0x21, 0xde, 0x6b, 0x71, 0xe6, 0xac, 0xaa, 0x16, 0xfc,
 0x89, 0xd6, 0xac, 0xe7, 0xf8, 0x7c, 0x09, 0x6a, 0x8b, 0x8b, 0x64, 0x0b, 0x7c, 0xc3, 0x61, 0x7f, 0xc2, 0x97, 0xd3, 0x33, 0xd9, 0x99, 0x59,
 0xbe, 0xed, 0xdc, 0x2c, 0x5d, 0x93, 0x5c, 0xd4, 0xdd, 0xdf, 0x8b, 0xd5, 0x1d, 0x46, 0x95, 0xbd, 0x10, 0x5a, 0xa9, 0xd1, 0x9f, 0x71, 0x70,
 0xd3, 0x94, 0x3c, 0x71, 0x5d, 0x53, 0x1c, 0x52, 0xe4, 0xc0, 0xf1, 0x7f, 0x87, 0xd0, 0x70, 0xa4, 0x04, 0x07, 0x05, 0x69, 0x2a, 0x16, 0x15,
 0x55, 0x85, 0xa6, 0x30, 0xc8, 0xb6, 0x00,
];

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

#[repr(C)]
pub struct TirData {
 status: i16,
 frame: i16,
 cksum: u32,
 roll: f32,
 pitch: f32,
 yaw: f32,
 tx: f32,
 ty: f32,
 tz: f32,
 padding: [f32; 9],
}

#[repr(C)]
pub struct TirSignature {
 dll_signature: [u8; 200],
 app_signature: [u8; 200],
}

static mut SHARED_MAP_HANDLE: *mut c_void = ptr::null_mut();
static mut SHARED_HEAP_PTR: *mut FreeTrackHeap = ptr::null_mut();
static mut FRAME_COUNTER: i16 = 0;
static mut ENCRYPTION_TABLE: [u8; 8] = [0; 8];
static mut ENCRYPTION_ENABLED: bool = false;
static mut ENCRYPTION_CHECKED: bool = false;
static mut LAST_ROLL: f64 = 0.0;
static mut LAST_PITCH: f64 = 0.0;
static mut LAST_YAW: f64 = 0.0;
static mut LAST_TX: f64 = 0.0;
static mut LAST_TY: f64 = 0.0;
static mut LAST_TZ: f64 = 0.0;

static NP_GETDATA_CALLS: AtomicU32 = AtomicU32::new(0);

fn trace_npclient_event(event: &str) {
 let log_path = std::env::temp_dir().join("unvet_npclient_trace.log");
 if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
  let _ = writeln!(file, "{event}");
 }
}

fn radians_to_trackir_axis(value: f32) -> f64 {
 (value as f64 * TRACKIR_AXIS_MAX) / std::f64::consts::PI
}

fn millimeters_to_trackir_axis(value: f32) -> f64 {
 value as f64 * TRACKIR_AXIS_MAX / 500.0
}

fn clamp_trackir_axis(value: f64) -> f32 {
 value.clamp(-TRACKIR_AXIS_MAX, TRACKIR_AXIS_MAX) as f32
}

fn read_i16_le(bytes: &[u8], offset: usize) -> i32 {
 i16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as i32
}

fn read_i8_signed(bytes: &[u8], offset: usize) -> i32 {
 (bytes[offset] as i8) as i32
}

fn trackir_cksum(bytes: &[u8]) -> u32 {
 if bytes.is_empty() {
  return 0;
 }

 let mut rounds = bytes.len() >> 2;
 let rem = bytes.len() % 4;
 let mut offset = 0usize;
 let mut c = bytes.len() as i32;
 let mut a2 = 0i32;

 while rounds != 0 {
  let a0 = read_i16_le(bytes, offset);
  a2 = read_i16_le(bytes, offset + 2);
  offset += 4;

  c = c.wrapping_add(a0);
  a2 ^= c.wrapping_shl(5);
  a2 = a2.wrapping_shl(11);
  c ^= a2;
  c = c.wrapping_add(c >> 11);
  rounds -= 1;
 }

 match rem {
  3 => {
   let a0 = read_i16_le(bytes, offset);
   a2 = read_i8_signed(bytes, offset + 2);
   c = c.wrapping_add(a0);
   a2 = a2.wrapping_shl(2) ^ c;
   c ^= a2.wrapping_shl(16);
   a2 = c >> 11;
  },
  2 => {
   a2 = read_i16_le(bytes, offset);
   c = c.wrapping_add(a2);
   c ^= c.wrapping_shl(11);
   a2 = c >> 17;
  },
  1 => {
   a2 = read_i8_signed(bytes, offset);
   c = c.wrapping_add(a2);
   c ^= c.wrapping_shl(10);
   a2 = c >> 1;
  },
  _ => {},
 }

 if rem != 0 {
  c = c.wrapping_add(a2);
 }

 c ^= c.wrapping_shl(3);
 c = c.wrapping_add(c >> 5);
 c ^= c.wrapping_shl(4);
 c = c.wrapping_add(c >> 17);
 c ^= c.wrapping_shl(25);
 c = c.wrapping_add(c >> 6);

 c as u32
}

fn enhance_in_place(data: &mut [u8], table: &[u8; 8]) {
 if data.is_empty() {
  return;
 }

 let mut table_ptr = 0usize;
 let mut var = 0x88u8;
 let mut size = data.len();

 while size != 0 {
  size -= 1;
  let tmp = data[size];
  data[size] = tmp ^ table[table_ptr] ^ var;
  var = var.wrapping_add((size as u8).wrapping_add(tmp));
  table_ptr += 1;

  if table_ptr >= table.len() {
   table_ptr -= table.len();
  }
 }
}

fn apply_signature_xor(out: &mut [u8; 200], lhs: &[u8], rhs: &[u8]) {
 for (index, slot) in out.iter_mut().enumerate() {
  let a = lhs.get(index).copied().unwrap_or(0);
  let b = rhs.get(index).copied().unwrap_or(0);
  *slot = a ^ b;
 }
}

unsafe fn ensure_shared_heap() -> bool {
 if !SHARED_HEAP_PTR.is_null() {
  return true;
 }

 let mutex_handle = unsafe { CreateMutexA(ptr::null(), 0, FT_MUTEX.as_ptr()) };
 if !mutex_handle.is_null() {
  unsafe {
   CloseHandle(mutex_handle);
  }
 }

 let map_handle = unsafe {
  CreateFileMappingA(
   INVALID_HANDLE_VALUE,
   ptr::null(),
   PAGE_READWRITE,
   0,
   size_of::<FreeTrackHeap>() as u32,
   FT_SHARED_MEM.as_ptr(),
  )
 };
 if map_handle.is_null() {
  return false;
 }

 let view = unsafe { MapViewOfFile(map_handle, FILE_MAP_WRITE, 0, 0, size_of::<FreeTrackHeap>()) };
 if view.is_null() {
  unsafe {
   CloseHandle(map_handle);
  }
  return false;
 }

 SHARED_MAP_HANDLE = map_handle;
 SHARED_HEAP_PTR = view as *mut FreeTrackHeap;
 true
}

unsafe fn close_shared_heap() {
 if !SHARED_HEAP_PTR.is_null() {
  unsafe {
   UnmapViewOfFile(SHARED_HEAP_PTR as *const c_void);
  }
  SHARED_HEAP_PTR = ptr::null_mut();
 }

 if !SHARED_MAP_HANDLE.is_null() {
  unsafe {
   CloseHandle(SHARED_MAP_HANDLE);
  }
  SHARED_MAP_HANDLE = ptr::null_mut();
 }

 ENCRYPTION_ENABLED = false;
 ENCRYPTION_CHECKED = false;
 ENCRYPTION_TABLE = [0; 8];
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(_dll: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
 const DLL_PROCESS_ATTACH: u32 = 1;
 const DLL_PROCESS_DETACH: u32 = 0;

 if reason == DLL_PROCESS_ATTACH {
  trace_npclient_event("DllMain: process attach");
 }

 if reason == DLL_PROCESS_DETACH {
  trace_npclient_event("DllMain: process detach");
  unsafe {
   close_shared_heap();
  }
 }

 1
}

#[no_mangle]
pub unsafe extern "system" fn NP_QueryVersion(version: *mut u16) -> i32 {
 if !version.is_null() {
  unsafe {
   *version = 0x0500;
  }
 }
 trace_npclient_event("NP_QueryVersion");
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_RegisterProgramProfileID(game_id: u16) -> i32 {
 unsafe {
  if ensure_shared_heap() {
   let heap = &mut *SHARED_HEAP_PTR;
   heap.game_id = game_id as i32;
   ENCRYPTION_ENABLED = false;
   ENCRYPTION_CHECKED = false;
   ENCRYPTION_TABLE = [0; 8];
  }
 }

 trace_npclient_event(&format!("NP_RegisterProgramProfileID: game_id={game_id}"));
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_GetData(data: *mut TirData) -> i32 {
 if data.is_null() {
  return TRACKIR_STATUS_DISABLED as i32;
 }

 unsafe {
  if !ensure_shared_heap() {
   return TRACKIR_STATUS_OK as i32;
  }

  if !SHARED_HEAP_PTR.is_null() {
   let heap = &*SHARED_HEAP_PTR;
   let source = &heap.data;

   LAST_YAW = radians_to_trackir_axis(source.yaw);
   LAST_PITCH = radians_to_trackir_axis(source.pitch);
   LAST_ROLL = radians_to_trackir_axis(source.roll);
   LAST_TX = millimeters_to_trackir_axis(source.x);
   LAST_TY = millimeters_to_trackir_axis(source.y);
   LAST_TZ = millimeters_to_trackir_axis(source.z);

   if heap.game_id == heap.game_id2 && !ENCRYPTION_CHECKED {
    ENCRYPTION_CHECKED = true;
    let table = heap.table;
    ENCRYPTION_ENABLED = table.iter().any(|value| *value != 0);
    ENCRYPTION_TABLE = table;
   }
  }

  FRAME_COUNTER = FRAME_COUNTER.wrapping_add(1);

  let frame = &mut *data;
  frame.frame = FRAME_COUNTER;

  // Keep hardware reported as enabled even when current pose is centered.
  frame.status = TRACKIR_STATUS_OK;
  frame.cksum = 0;
  frame.roll = clamp_trackir_axis(LAST_ROLL);
  frame.pitch = clamp_trackir_axis(LAST_PITCH);
  frame.yaw = clamp_trackir_axis(LAST_YAW);
  frame.tx = clamp_trackir_axis(LAST_TX);
  frame.ty = clamp_trackir_axis(LAST_TY);
  frame.tz = clamp_trackir_axis(LAST_TZ);
  frame.padding = [0.0; 9];

  let checksum_input = slice::from_raw_parts((frame as *const TirData).cast::<u8>(), size_of::<TirData>());
  frame.cksum = trackir_cksum(checksum_input);

  let log_frame = frame.frame;
  let log_status = frame.status;
  let log_yaw = frame.yaw;
  let log_pitch = frame.pitch;
  let log_tx = frame.tx;
  let log_cksum = frame.cksum;

  let encryption_table = ENCRYPTION_TABLE;
  if ENCRYPTION_ENABLED {
   let payload = slice::from_raw_parts_mut((frame as *mut TirData).cast::<u8>(), size_of::<TirData>());
   enhance_in_place(payload, &encryption_table);
  }

  let call_index = NP_GETDATA_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
  if call_index <= 5 || call_index % 300 == 0 {
   let encryption_enabled = ENCRYPTION_ENABLED;
   let (game_id, game_id2) = if SHARED_HEAP_PTR.is_null() {
    (0, 0)
   } else {
    let heap = &*SHARED_HEAP_PTR;
    (heap.game_id, heap.game_id2)
   };

   trace_npclient_event(&format!(
    "NP_GetData: call={call_index} frame={} status={} game_id={} game_id2={} enc={} yaw={:.2} pitch={:.2} tx={:.2} cksum={}",
    log_frame, log_status, game_id, game_id2, encryption_enabled, log_yaw, log_pitch, log_tx, log_cksum
   ));
  }
 }

 TRACKIR_STATUS_OK as i32
}

#[no_mangle]
pub unsafe extern "system" fn NP_ReCenter() -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_RegisterWindowHandle(_hwnd: *mut c_void) -> i32 {
 trace_npclient_event("NP_RegisterWindowHandle");
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_UnregisterWindowHandle() -> i32 {
 trace_npclient_event("NP_UnregisterWindowHandle");
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_RequestData(data_type: u16) -> i32 {
 trace_npclient_event(&format!("NP_RequestData: type={data_type}"));
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_StartDataTransmission() -> i32 {
 trace_npclient_event("NP_StartDataTransmission");
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_StopDataTransmission() -> i32 {
 trace_npclient_event("NP_StopDataTransmission");
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_StartCursor() -> i32 {
 trace_npclient_event("NP_StartCursor");
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_StopCursor() -> i32 {
 trace_npclient_event("NP_StopCursor");
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_SetParameter(_parameter: i32, _value: i32) -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_GetParameter(_parameter: i32, _value: *mut i32) -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NP_GetSignature(signature: *mut TirSignature) -> i32 {
 if !signature.is_null() {
  unsafe {
   let signature = &mut *signature;
   apply_signature_xor(&mut signature.dll_signature, PART1_2_PREFIX, PART1_1_PREFIX);
   apply_signature_xor(&mut signature.app_signature, PART2_1_PREFIX, PART2_2_PREFIX);
  }
 }
 trace_npclient_event("NP_GetSignature");
 0
}

#[no_mangle]
pub unsafe extern "system" fn NPPriv_ClientNotify() -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NPPriv_GetLastError() -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NPPriv_SetData(_data: *mut c_void) -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NPPriv_SetLastError(_error: i32) -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NPPriv_SetParameter(_parameter: i32, _value: i32) -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NPPriv_SetSignature(_signature: *const c_void) -> i32 {
 0
}

#[no_mangle]
pub unsafe extern "system" fn NPPriv_SetVersion(_version: u16) -> i32 {
 0
}

#[link(name = "Kernel32")]
unsafe extern "system" {
 fn CreateFileMappingA(
  file: *mut c_void,
  attributes: *const c_void,
  protect: u32,
  maximum_size_high: u32,
  maximum_size_low: u32,
  name: *const u8,
 ) -> *mut c_void;

 fn CreateMutexA(attributes: *const c_void, initial_owner: i32, name: *const u8) -> *mut c_void;

 fn MapViewOfFile(
  file_mapping: *mut c_void,
  desired_access: u32,
  file_offset_high: u32,
  file_offset_low: u32,
  number_of_bytes_to_map: usize,
 ) -> *mut c_void;

 fn UnmapViewOfFile(base_address: *const c_void) -> i32;
 fn CloseHandle(handle: *mut c_void) -> i32;
}
