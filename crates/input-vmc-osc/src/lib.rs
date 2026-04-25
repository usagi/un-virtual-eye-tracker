use std::{
 collections::HashSet,
 net::{SocketAddr, ToSocketAddrs, UdpSocket},
 sync::{
  atomic::{AtomicBool, AtomicU64, Ordering},
  mpsc::{sync_channel, SyncSender, TrySendError},
  Arc, Mutex,
 },
 thread::{self, JoinHandle},
 time::{Duration, SystemTime, UNIX_EPOCH},
};

use glam::{EulerRot, Quat};
use rosc::{OscMessage, OscPacket, OscType};
use unvet_core::{
 model::{RawTrackingFrame, TrackingFrame},
 ports::InputReceiver,
 AppError, AppResult,
};

pub const VMC_OSC_DEFAULT_PORT: u16 = 39539;
const RECEIVE_BUFFER_BYTES: usize = 64 * 1024;
const DEFAULT_EYE_BLEND_MAX_ANGLE_DEG: f32 = 35.0;
const PASSTHROUGH_QUEUE_CAPACITY: usize = 512;
const SOCKET_READ_TIMEOUT_MS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassthroughMode {
 RawUdpForward,
}

impl Default for PassthroughMode {
 fn default() -> Self {
  Self::RawUdpForward
 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
 Disconnected,
 Connecting,
 Receiving,
 Error,
}

#[derive(Debug, Clone)]
pub struct ReceiverOptions {
 pub udp_port: u16,
 pub passthrough: PassthroughOptions,
}

#[derive(Debug, Clone, Default)]
pub struct PassthroughOptions {
 pub enabled: bool,
 pub targets: Vec<String>,
 pub mode: PassthroughMode,
}

impl Default for ReceiverOptions {
 fn default() -> Self {
  Self {
   udp_port: VMC_OSC_DEFAULT_PORT,
   passthrough: PassthroughOptions::default(),
  }
 }
}

#[derive(Debug, Clone, Default)]
pub struct ReceiverStats {
 pub udp_packets_received: u64,
 pub osc_packets_decoded: u64,
 pub frames_emitted: u64,
 pub frames_ignored: u64,
 pub passthrough_targets_active: usize,
 pub passthrough_packets_forwarded: u64,
 pub passthrough_packets_failed: u64,
 pub passthrough_packets_dropped: u64,
 pub passthrough_last_warning: Option<String>,
 pub last_packet_timestamp_ms: Option<u64>,
 pub last_error: Option<String>,
}

pub struct VmcOscReceiver {
 options: ReceiverOptions,
 state: ConnectionState,
 active: bool,
 buffered_frame: Option<TrackingFrame>,
 socket: Option<UdpSocket>,
 passthrough_targets: Vec<SocketAddr>,
 passthrough_worker: Option<PassthroughWorker>,
 reader_stop: Arc<AtomicBool>,
 reader_handle: Option<JoinHandle<()>>,
 reader_shared: Arc<Mutex<ReaderRuntimeShared>>,
 latest_frame_shared: Arc<Mutex<Option<TrackingFrame>>>,
 stats: ReceiverStats,
}

#[derive(Default)]
struct ReaderRuntimeShared {
 udp_packets_received: u64,
 osc_packets_decoded: u64,
 frames_emitted: u64,
 frames_ignored: u64,
 last_packet_timestamp_ms: Option<u64>,
 last_error: Option<String>,
}

#[derive(Clone)]
struct PassthroughDispatch {
 sender: SyncSender<Vec<u8>>,
 dropped_packets: Arc<AtomicU64>,
 last_warning: Arc<Mutex<Option<String>>>,
}

impl PassthroughDispatch {
 fn forward_payload(&self, payload: &[u8]) -> Option<String> {
  match self.sender.try_send(payload.to_vec()) {
   Ok(()) => None,
   Err(TrySendError::Full(_payload)) => {
    self.dropped_packets.fetch_add(1, Ordering::Relaxed);
    let warning = "passthrough queue is full; dropping packet".to_owned();
    if let Ok(mut guard) = self.last_warning.lock() {
     *guard = Some(warning.clone());
    }
    Some(warning)
   },
   Err(TrySendError::Disconnected(_payload)) => {
    self.dropped_packets.fetch_add(1, Ordering::Relaxed);
    let warning = "passthrough worker is disconnected; dropping packet".to_owned();
    if let Ok(mut guard) = self.last_warning.lock() {
     *guard = Some(warning.clone());
    }
    Some(warning)
   },
  }
 }
}

#[derive(Default)]
struct PassthroughStatsSnapshot {
 forwarded_packets: u64,
 failed_packets: u64,
 dropped_packets: u64,
 last_warning: Option<String>,
}

struct PassthroughWorker {
 sender: SyncSender<Vec<u8>>,
 handle: JoinHandle<()>,
 forwarded_packets: Arc<AtomicU64>,
 failed_packets: Arc<AtomicU64>,
 dropped_packets: Arc<AtomicU64>,
 last_warning: Arc<Mutex<Option<String>>>,
}

impl PassthroughWorker {
 fn spawn(targets: Vec<SocketAddr>) -> Option<Self> {
  let (sender, receiver) = sync_channel::<Vec<u8>>(PASSTHROUGH_QUEUE_CAPACITY);
  let forwarded_packets = Arc::new(AtomicU64::new(0));
  let failed_packets = Arc::new(AtomicU64::new(0));
  let dropped_packets = Arc::new(AtomicU64::new(0));
  let last_warning = Arc::new(Mutex::new(None));

  let thread_forwarded = Arc::clone(&forwarded_packets);
  let thread_failed = Arc::clone(&failed_packets);
  let thread_warning = Arc::clone(&last_warning);

  let handle = thread::Builder::new()
   .name("unvet-vmc-osc-passthrough".to_owned())
   .spawn(move || {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
     Ok(socket) => socket,
     Err(error) => {
      if let Ok(mut guard) = thread_warning.lock() {
       *guard = Some(format!("passthrough worker bind failed: {error}"));
      }
      return;
     },
    };

    while let Ok(payload) = receiver.recv() {
     for target in &targets {
      match socket.send_to(&payload, target) {
       Ok(sent) if sent == payload.len() => {
        thread_forwarded.fetch_add(1, Ordering::Relaxed);
       },
       Ok(sent) => {
        thread_failed.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut guard) = thread_warning.lock() {
         *guard = Some(format!("short passthrough send to {target}: sent {sent} / {} bytes", payload.len()));
        }
       },
       Err(error) => {
        thread_failed.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut guard) = thread_warning.lock() {
         *guard = Some(format!("passthrough send to {target} failed: {error}"));
        }
       },
      }
     }
    }
   })
   .ok()?;

  Some(Self {
   sender,
   handle,
   forwarded_packets,
   failed_packets,
   dropped_packets,
   last_warning,
  })
 }

 fn dispatcher(&self) -> PassthroughDispatch {
  PassthroughDispatch {
   sender: self.sender.clone(),
   dropped_packets: Arc::clone(&self.dropped_packets),
   last_warning: Arc::clone(&self.last_warning),
  }
 }

 fn snapshot(&self) -> PassthroughStatsSnapshot {
  let mut result = PassthroughStatsSnapshot::default();
  result.forwarded_packets = self.forwarded_packets.load(Ordering::Relaxed);
  result.failed_packets = self.failed_packets.load(Ordering::Relaxed);
  result.dropped_packets = self.dropped_packets.load(Ordering::Relaxed);
  if let Ok(guard) = self.last_warning.lock() {
   result.last_warning = guard.clone();
  }
  result
 }

 fn stop(self) {
  drop(self.sender);
  let _ = self.handle.join();
 }
}

impl VmcOscReceiver {
 pub fn new(options: ReceiverOptions) -> Self {
  Self {
   options,
   state: ConnectionState::Disconnected,
   active: false,
   buffered_frame: None,
   socket: None,
   passthrough_targets: Vec::new(),
   passthrough_worker: None,
   reader_stop: Arc::new(AtomicBool::new(false)),
   reader_handle: None,
   reader_shared: Arc::new(Mutex::new(ReaderRuntimeShared::default())),
   latest_frame_shared: Arc::new(Mutex::new(None)),
   stats: ReceiverStats::default(),
  }
 }

 pub fn options(&self) -> &ReceiverOptions {
  &self.options
 }

 pub fn state(&self) -> ConnectionState {
  self.state
 }

 pub fn stats(&self) -> &ReceiverStats {
  &self.stats
 }

 pub fn connect(&mut self) -> AppResult<()> {
  // Ensure stale sockets and worker threads are released before rebinding during reconnect.
  self.stop_reader_thread();
  self.socket = None;
  if let Some(worker) = self.passthrough_worker.take() {
   worker.stop();
  }
  self.state = ConnectionState::Connecting;

  let bind_address = format!("0.0.0.0:{}", self.options.udp_port);
  let socket = UdpSocket::bind(&bind_address).map_err(|error| {
   self.record_error(format!("failed to bind VMC/OSC UDP socket on {bind_address}: {error}"));
   AppError::from(error)
  })?;

  socket.set_nonblocking(false)?;
  socket.set_read_timeout(Some(Duration::from_millis(SOCKET_READ_TIMEOUT_MS)))?;

  let (passthrough_targets, passthrough_warnings) = compile_passthrough_targets(&self.options);
  self.passthrough_targets = passthrough_targets;
  self.stats.passthrough_targets_active = self.passthrough_targets.len();
  self.stats.passthrough_packets_forwarded = 0;
  self.stats.passthrough_packets_failed = 0;
  self.stats.passthrough_packets_dropped = 0;
  self.stats.passthrough_last_warning = if passthrough_warnings.is_empty() {
   None
  } else {
   Some(passthrough_warnings.join("; "))
  };

  self.passthrough_worker = if self.options.passthrough.enabled && !self.passthrough_targets.is_empty() {
   let worker = PassthroughWorker::spawn(self.passthrough_targets.clone());
   if worker.is_none() {
    self.stats.passthrough_last_warning = Some("failed to spawn passthrough worker thread".to_owned());
   }
   worker
  } else {
   None
  };

  if let Ok(mut guard) = self.reader_shared.lock() {
   *guard = ReaderRuntimeShared::default();
  }
  if let Ok(mut guard) = self.latest_frame_shared.lock() {
   *guard = None;
  }

  let reader_socket = socket.try_clone()?;
  let passthrough_dispatch = self.passthrough_worker.as_ref().map(PassthroughWorker::dispatcher);
  self.spawn_reader_thread(reader_socket, passthrough_dispatch)?;

  self.socket = Some(socket);
  self.state = ConnectionState::Receiving;
  self.active = true;
  self.stats.last_error = None;
  Ok(())
 }

 pub fn disconnect(&mut self) {
  self.state = ConnectionState::Disconnected;
  self.active = false;
  self.buffered_frame = None;
  self.stop_reader_thread();
  self.socket = None;
  if let Some(worker) = self.passthrough_worker.take() {
   worker.stop();
  }
  if let Ok(mut guard) = self.reader_shared.lock() {
   *guard = ReaderRuntimeShared::default();
  }
  if let Ok(mut guard) = self.latest_frame_shared.lock() {
   *guard = None;
  }
  self.passthrough_targets.clear();
  self.stats.passthrough_targets_active = 0;
 }

 pub fn clear_error(&mut self) {
  self.stats.last_error = None;
  if let Ok(mut guard) = self.reader_shared.lock() {
   guard.last_error = None;
  }
  if self.socket.is_some() {
   self.state = ConnectionState::Receiving;
   self.active = true;
  } else {
   self.state = ConnectionState::Disconnected;
   self.active = false;
  }
 }

 pub fn ingest_mock_frame(&mut self, frame: TrackingFrame) {
  self.buffered_frame = Some(frame);
 }

 fn try_read_frames(&mut self) {
  let Some(_socket) = self.socket.as_ref() else {
   return;
  };

  if let Ok(guard) = self.reader_shared.lock() {
   self.stats.udp_packets_received = guard.udp_packets_received;
   self.stats.osc_packets_decoded = guard.osc_packets_decoded;
   self.stats.frames_emitted = guard.frames_emitted;
   self.stats.frames_ignored = guard.frames_ignored;
   self.stats.last_packet_timestamp_ms = guard.last_packet_timestamp_ms;
   self.stats.last_error = guard.last_error.clone();
  }

  if let Ok(mut guard) = self.latest_frame_shared.lock() {
   if let Some(frame) = guard.take() {
    self.buffered_frame = Some(frame);
    self.state = ConnectionState::Receiving;
    self.active = true;
    self.stats.last_error = None;
   }
  }

  self.refresh_passthrough_stats_from_worker();
 }

 fn spawn_reader_thread(&mut self, socket: UdpSocket, passthrough_dispatch: Option<PassthroughDispatch>) -> AppResult<()> {
  self.reader_stop.store(false, Ordering::Relaxed);

  let stop_flag = Arc::clone(&self.reader_stop);
  let shared = Arc::clone(&self.reader_shared);
  let latest_frame_shared = Arc::clone(&self.latest_frame_shared);

  let handle = thread::Builder::new()
   .name("unvet-vmc-osc-reader".to_owned())
   .spawn(move || {
    let mut receive_buffer = [0; RECEIVE_BUFFER_BYTES];

    while !stop_flag.load(Ordering::Relaxed) {
     match socket.recv_from(&mut receive_buffer) {
      Ok((size, _source)) => {
       let timestamp_ms = now_millis();
       let payload = &receive_buffer[..size];

       if let Some(dispatch) = &passthrough_dispatch {
        let _ = dispatch.forward_payload(payload);
       }

       let mut emitted_frame = None;
       let mut decode_failed = None;
       let mut parsed_packet = false;

       match decode_osc_packet(payload) {
        Ok(packet) => {
         parsed_packet = true;
         emitted_frame = parse_tracking_frame_from_packet(&packet, timestamp_ms);
        },
        Err(error) => {
         decode_failed = Some(error);
        },
       }

       if let Ok(mut guard) = shared.lock() {
        guard.udp_packets_received = guard.udp_packets_received.saturating_add(1);
        guard.last_packet_timestamp_ms = Some(timestamp_ms);

        if parsed_packet {
         guard.osc_packets_decoded = guard.osc_packets_decoded.saturating_add(1);
         if emitted_frame.is_some() {
          guard.frames_emitted = guard.frames_emitted.saturating_add(1);
          guard.last_error = None;
         } else {
          guard.frames_ignored = guard.frames_ignored.saturating_add(1);
         }
        } else {
         guard.frames_ignored = guard.frames_ignored.saturating_add(1);
         guard.last_error = decode_failed;
        }
       }

       if let Some(frame) = emitted_frame {
        if let Ok(mut guard) = latest_frame_shared.lock() {
         *guard = Some(frame);
        }
       }
      },
      Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
       continue;
      },
      Err(error) => {
       if let Ok(mut guard) = shared.lock() {
        guard.frames_ignored = guard.frames_ignored.saturating_add(1);
        guard.last_error = Some(format!("VMC/OSC UDP receive failed: {error}"));
       }
      },
     }
    }
   })
   .map_err(AppError::from)?;

  self.reader_handle = Some(handle);
  Ok(())
 }

 fn stop_reader_thread(&mut self) {
  self.reader_stop.store(true, Ordering::Relaxed);
  if let Some(handle) = self.reader_handle.take() {
   let _ = handle.join();
  }
  self.reader_stop.store(false, Ordering::Relaxed);
 }

 fn refresh_passthrough_stats_from_worker(&mut self) {
  let Some(worker) = &self.passthrough_worker else {
   return;
  };

  let snapshot = worker.snapshot();
  self.stats.passthrough_packets_forwarded = snapshot.forwarded_packets;
  self.stats.passthrough_packets_failed = snapshot.failed_packets;
  self.stats.passthrough_packets_dropped = snapshot.dropped_packets;
  if let Some(warning) = snapshot.last_warning {
   self.stats.passthrough_last_warning = Some(warning);
  }
 }

 fn record_error(&mut self, message: String) {
  self.stop_reader_thread();
  if let Some(worker) = self.passthrough_worker.take() {
   worker.stop();
  }
  self.state = ConnectionState::Error;
  self.active = false;
  self.socket = None;
  self.stats.last_error = Some(message);
 }
}

impl InputReceiver for VmcOscReceiver {
 fn source_name(&self) -> &'static str {
  "vmc_osc"
 }

 fn poll_frame(&mut self) -> Option<TrackingFrame> {
  self.try_read_frames();
  self.buffered_frame.take()
 }

 fn is_active(&self) -> bool {
  self.active
 }
}

pub fn parse_tracking_frame_from_packet(packet: &OscPacket, timestamp_ms: u64) -> Option<TrackingFrame> {
 let mut pose = VmcPose::default();
 collect_pose_from_packet(packet, &mut pose);
 pose.into_tracking_frame(timestamp_ms)
}

fn decode_osc_packet(payload: &[u8]) -> Result<OscPacket, String> {
 rosc::decoder::decode_udp(payload)
  .map(|(_remaining, packet)| packet)
  .map_err(|error| format!("failed to decode OSC UDP packet: {error}"))
}

#[derive(Debug, Clone, Copy, Default)]
struct RotationDeg {
 yaw_deg: f32,
 pitch_deg: f32,
 roll_deg: f32,
}

#[derive(Debug, Default)]
struct EyeBlendState {
 look_left: Option<f32>,
 look_right: Option<f32>,
 look_up: Option<f32>,
 look_down: Option<f32>,
}

impl EyeBlendState {
 fn has_any(&self) -> bool {
  self.look_left.is_some() || self.look_right.is_some() || self.look_up.is_some() || self.look_down.is_some()
 }

 fn as_eye_angles_deg(&self) -> Option<(f32, f32)> {
  if !self.has_any() {
   return None;
  }

  let left = self.look_left.unwrap_or(0.0).clamp(0.0, 1.0);
  let right = self.look_right.unwrap_or(0.0).clamp(0.0, 1.0);
  let up = self.look_up.unwrap_or(0.0).clamp(0.0, 1.0);
  let down = self.look_down.unwrap_or(0.0).clamp(0.0, 1.0);

  let yaw_deg = (left - right) * DEFAULT_EYE_BLEND_MAX_ANGLE_DEG;
  let pitch_deg = (up - down) * DEFAULT_EYE_BLEND_MAX_ANGLE_DEG;
  Some((yaw_deg, pitch_deg))
 }
}

#[derive(Debug, Default)]
struct VmcPose {
 saw_pose_message: bool,
 head: Option<RotationDeg>,
 left_eye: Option<RotationDeg>,
 right_eye: Option<RotationDeg>,
 eye_blend: EyeBlendState,
}

impl VmcPose {
 fn into_tracking_frame(self, timestamp_ms: u64) -> Option<TrackingFrame> {
  if !self.saw_pose_message {
   return None;
  }

  let head = self.head?;

  let blend_eyes = self.eye_blend.as_eye_angles_deg();
  let (left_eye, right_eye) = match (self.left_eye, self.right_eye, blend_eyes) {
   (Some(left), Some(right), _) => (left, right),
   (Some(left), None, _) => (left, left),
   (None, Some(right), _) => (right, right),
   (None, None, Some((yaw_deg, pitch_deg))) => (
    RotationDeg {
     yaw_deg,
     pitch_deg,
     roll_deg: 0.0,
    },
    RotationDeg {
     yaw_deg,
     pitch_deg,
     roll_deg: 0.0,
    },
   ),
   (None, None, None) => (
    RotationDeg {
     yaw_deg: 0.0,
     pitch_deg: 0.0,
     roll_deg: 0.0,
    },
    RotationDeg {
     yaw_deg: 0.0,
     pitch_deg: 0.0,
     roll_deg: 0.0,
    },
   ),
  };

  Some(TrackingFrame::from_raw(RawTrackingFrame {
   timestamp_ms,
   head_yaw_deg: head.yaw_deg,
   head_pitch_deg: head.pitch_deg,
   head_roll_deg: head.roll_deg,
   eye_yaw_deg: None,
   eye_pitch_deg: None,
   left_eye_yaw_deg: Some(left_eye.yaw_deg),
   left_eye_pitch_deg: Some(left_eye.pitch_deg),
   right_eye_yaw_deg: Some(right_eye.yaw_deg),
   right_eye_pitch_deg: Some(right_eye.pitch_deg),
   reported_confidence: Some(1.0),
   reported_active: Some(true),
  }))
 }
}

fn collect_pose_from_packet(packet: &OscPacket, pose: &mut VmcPose) {
 match packet {
  OscPacket::Message(message) => collect_pose_from_message(message, pose),
  OscPacket::Bundle(bundle) => {
   for packet in &bundle.content {
    collect_pose_from_packet(packet, pose);
   }
  },
 }
}

fn collect_pose_from_message(message: &OscMessage, pose: &mut VmcPose) {
 if message.addr.eq_ignore_ascii_case("/VMC/Ext/Bone/Pos") {
  pose.saw_pose_message = true;
  let Some((bone_name, rotation)) = parse_bone_rotation(message) else {
   return;
  };

  if is_head_bone(&bone_name) {
   pose.head = Some(rotation);
  } else if is_left_eye_bone(&bone_name) {
   pose.left_eye = Some(rotation);
  } else if is_right_eye_bone(&bone_name) {
   pose.right_eye = Some(rotation);
  }
  return;
 }

 if message.addr.eq_ignore_ascii_case("/VMC/Ext/Blend/Val") {
  pose.saw_pose_message = true;
  let Some((name, value)) = parse_blend_value(message) else {
   return;
  };

  if name.eq_ignore_ascii_case("LookLeft") {
   pose.eye_blend.look_left = Some(value);
  } else if name.eq_ignore_ascii_case("LookRight") {
   pose.eye_blend.look_right = Some(value);
  } else if name.eq_ignore_ascii_case("LookUp") {
   pose.eye_blend.look_up = Some(value);
  } else if name.eq_ignore_ascii_case("LookDown") {
   pose.eye_blend.look_down = Some(value);
  }
 }
}

fn parse_bone_rotation(message: &OscMessage) -> Option<(String, RotationDeg)> {
 if message.args.len() < 8 {
  return None;
 }

 let bone_name = osc_string(message.args.first()?)?.to_owned();
 let qx = osc_number(message.args.get(4)?)?;
 let qy = osc_number(message.args.get(5)?)?;
 let qz = osc_number(message.args.get(6)?)?;
 let qw = osc_number(message.args.get(7)?)?;

 quaternion_to_rotation_deg(qx, qy, qz, qw).map(|rotation| (bone_name, rotation))
}

fn parse_blend_value(message: &OscMessage) -> Option<(String, f32)> {
 if message.args.len() < 2 {
  return None;
 }

 let name = osc_string(message.args.first()?)?.to_owned();
 let value = osc_number(message.args.get(1)?)?;
 Some((name, value))
}

fn quaternion_to_rotation_deg(x: f32, y: f32, z: f32, w: f32) -> Option<RotationDeg> {
 let quat = Quat::from_xyzw(x, y, z, w);
 if !quat.is_finite() {
  return None;
 }

 let normalized = quat.normalize();
 if !normalized.is_finite() {
  return None;
 }

 let (yaw_rad, pitch_rad, roll_rad) = normalized.to_euler(EulerRot::YXZ);
 Some(RotationDeg {
  yaw_deg: yaw_rad.to_degrees(),
  pitch_deg: pitch_rad.to_degrees(),
  roll_deg: roll_rad.to_degrees(),
 })
}

fn is_head_bone(name: &str) -> bool {
 name.eq_ignore_ascii_case("Head") || name.eq_ignore_ascii_case("J_Bip_C_Head")
}

fn is_left_eye_bone(name: &str) -> bool {
 name.eq_ignore_ascii_case("LeftEye")
  || name.eq_ignore_ascii_case("Eye_L")
  || name.eq_ignore_ascii_case("J_Adj_L_FaceEye")
  || name.eq_ignore_ascii_case("J_Bip_L_Eye")
}

fn is_right_eye_bone(name: &str) -> bool {
 name.eq_ignore_ascii_case("RightEye")
  || name.eq_ignore_ascii_case("Eye_R")
  || name.eq_ignore_ascii_case("J_Adj_R_FaceEye")
  || name.eq_ignore_ascii_case("J_Bip_R_Eye")
}

fn osc_string(value: &OscType) -> Option<&str> {
 match value {
  OscType::String(text) => Some(text.as_str()),
  _ => None,
 }
}

fn osc_number(value: &OscType) -> Option<f32> {
 match value {
  OscType::Float(number) => Some(*number),
  OscType::Double(number) => Some(*number as f32),
  OscType::Int(number) => Some(*number as f32),
  OscType::Long(number) => Some(*number as f32),
  _ => None,
 }
}

fn now_millis() -> u64 {
 SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn compile_passthrough_targets(options: &ReceiverOptions) -> (Vec<SocketAddr>, Vec<String>) {
 if !options.passthrough.enabled {
  return (Vec::new(), Vec::new());
 }

 let mut unique = HashSet::new();
 let mut targets = Vec::new();
 let mut warnings = Vec::new();

 for raw in &options.passthrough.targets {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
   continue;
  }

  let Some((host, port)) = parse_target_host_port(trimmed) else {
   warnings.push(format!("ignored invalid passthrough target `{trimmed}`"));
   continue;
  };

  if is_self_loop_target(&host, port, options.udp_port) {
   warnings.push(format!("ignored self-loop passthrough target `{trimmed}`"));
   continue;
  }

  let target = format_target_for_send(&host, port);
  let Some(target_address) = resolve_target_address(&target) else {
   warnings.push(format!("ignored unresolved passthrough target `{trimmed}`"));
   continue;
  };

  if !unique.insert(target_address) {
   continue;
  }

  targets.push(target_address);
 }

 (targets, warnings)
}

fn resolve_target_address(target: &str) -> Option<SocketAddr> {
 target.to_socket_addrs().ok()?.next()
}

fn parse_target_host_port(raw: &str) -> Option<(String, u16)> {
 let text = raw.trim();
 if text.is_empty() {
  return None;
 }

 if let Some(rest) = text.strip_prefix('[') {
  let end = rest.find(']')?;
  let host = rest[..end].trim().to_ascii_lowercase();
  if host.is_empty() {
   return None;
  }

  let port_text = rest[end + 1..].strip_prefix(':')?.trim();
  if port_text.is_empty() || port_text.contains(':') {
   return None;
  }

  let port = port_text.parse::<u16>().ok()?;
  if port == 0 {
   return None;
  }

  return Some((host, port));
 }

 let mut parts = text.rsplitn(2, ':');
 let port_text = parts.next()?.trim();
 let host = parts.next()?.trim().to_ascii_lowercase();
 if host.is_empty() || host.contains(':') {
  return None;
 }

 let port = port_text.parse::<u16>().ok()?;
 if port == 0 {
  return None;
 }

 Some((host, port))
}

fn format_target_for_send(host: &str, port: u16) -> String {
 if host.contains(':') {
  format!("[{host}]:{port}")
 } else {
  format!("{host}:{port}")
 }
}

fn is_self_loop_target(host: &str, port: u16, listen_port: u16) -> bool {
 if port != listen_port {
  return false;
 }

 matches!(host, "127.0.0.1" | "localhost" | "::1" | "0.0.0.0" | "::")
}

#[cfg(test)]
mod tests {
 use std::{net::UdpSocket, thread, time::Duration};

 use super::{parse_tracking_frame_from_packet, quaternion_to_rotation_deg, ConnectionState, ReceiverOptions, VmcOscReceiver};
 use glam::{EulerRot, Quat};
 use rosc::{OscMessage, OscPacket, OscType};
 use unvet_core::ports::InputReceiver;

 fn bone_packet(name: &str, yaw_deg: f32, pitch_deg: f32, roll_deg: f32) -> OscPacket {
  let quat = Quat::from_euler(EulerRot::YXZ, yaw_deg.to_radians(), pitch_deg.to_radians(), roll_deg.to_radians());

  OscPacket::Message(OscMessage {
   addr: "/VMC/Ext/Bone/Pos".to_owned(),
   args: vec![
    OscType::String(name.to_owned()),
    OscType::Float(0.0),
    OscType::Float(0.0),
    OscType::Float(0.0),
    OscType::Float(quat.x),
    OscType::Float(quat.y),
    OscType::Float(quat.z),
    OscType::Float(quat.w),
   ],
  })
 }

 #[test]
 fn quaternion_conversion_preserves_angles() {
  let result = quaternion_to_rotation_deg(0.0, 0.0, 0.0, 1.0).expect("identity quaternion should convert");
  assert!(result.yaw_deg.abs() < 0.001);
  assert!(result.pitch_deg.abs() < 0.001);
  assert!(result.roll_deg.abs() < 0.001);
 }

 #[test]
 fn parser_maps_head_and_eye_bones() {
  let packet = OscPacket::Bundle(rosc::OscBundle {
   timetag: (0, 0).into(),
   content: vec![
    bone_packet("Head", 12.0, -5.0, 2.0),
    bone_packet("LeftEye", 8.0, -3.0, 0.0),
    bone_packet("RightEye", 9.0, -4.0, 0.0),
   ],
  });

  let frame = parse_tracking_frame_from_packet(&packet, 100).expect("expected frame from VMC bundle");
  assert!((frame.head_yaw_deg - 12.0).abs() < 0.1);
  assert!((frame.head_pitch_deg + 5.0).abs() < 0.1);
  assert!((frame.left_eye_yaw_deg - 8.0).abs() < 0.1);
  assert!((frame.right_eye_pitch_deg + 4.0).abs() < 0.1);
  assert!(frame.active);
 }

 #[test]
 fn parser_uses_blend_eyes_when_eye_bones_are_missing() {
  let packet = OscPacket::Bundle(rosc::OscBundle {
   timetag: (0, 0).into(),
   content: vec![
    bone_packet("Head", 0.0, 0.0, 0.0),
    OscPacket::Message(OscMessage {
     addr: "/VMC/Ext/Blend/Val".to_owned(),
     args: vec![OscType::String("LookLeft".to_owned()), OscType::Float(0.6)],
    }),
    OscPacket::Message(OscMessage {
     addr: "/VMC/Ext/Blend/Val".to_owned(),
     args: vec![OscType::String("LookDown".to_owned()), OscType::Float(0.4)],
    }),
   ],
  });

  let frame = parse_tracking_frame_from_packet(&packet, 101).expect("expected frame from blend values");
  assert!(frame.left_eye_yaw_deg > 20.0);
  assert!(frame.left_eye_pitch_deg < -10.0);
  assert!((frame.left_eye_yaw_deg - frame.right_eye_yaw_deg).abs() < 0.001);
 }

 #[test]
 fn parser_ignores_packets_without_head_pose() {
  let packet = OscPacket::Message(OscMessage {
   addr: "/VMC/Ext/Blend/Val".to_owned(),
   args: vec![OscType::String("LookUp".to_owned()), OscType::Float(1.0)],
  });

  assert!(parse_tracking_frame_from_packet(&packet, 9).is_none());
 }

 fn acquire_free_port() -> u16 {
  let socket = UdpSocket::bind("127.0.0.1:0").expect("bind free local UDP port");
  socket.local_addr().expect("read local UDP address").port()
 }

 #[test]
 fn receiver_stays_connected_after_malformed_packet_and_parses_next_valid_packet() {
  let port = acquire_free_port();
  let mut receiver = VmcOscReceiver::new(ReceiverOptions {
   udp_port: port,
   ..ReceiverOptions::default()
  });
  receiver.connect().expect("connect VMC receiver");

  let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender UDP socket");
  let target = format!("127.0.0.1:{port}");
  sender.send_to(b"not osc", &target).expect("send malformed payload");

  let valid_packet = bone_packet("Head", 4.0, -2.0, 0.0);
  let encoded = rosc::encoder::encode(&valid_packet).expect("encode OSC payload");
  sender.send_to(&encoded, &target).expect("send valid OSC payload");

  let mut parsed = None;
  for _ in 0..30 {
   parsed = receiver.poll_frame();
   if parsed.is_some() {
    break;
   }
   thread::sleep(Duration::from_millis(5));
  }

  let frame = parsed.expect("receiver should parse valid payload after malformed packet");
  assert!((frame.head_yaw_deg - 4.0).abs() < 0.1);
  assert!((frame.head_pitch_deg + 2.0).abs() < 0.1);
  assert!(receiver.is_active());
  assert_eq!(receiver.state(), ConnectionState::Receiving);
  assert!(receiver.stats().frames_ignored >= 1);
 }

 #[test]
 fn receiver_parses_large_bundle_payload_from_socket() {
  let port = acquire_free_port();
  let mut receiver = VmcOscReceiver::new(ReceiverOptions {
   udp_port: port,
   ..ReceiverOptions::default()
  });
  receiver.connect().expect("connect VMC receiver");

  let mut content = vec![bone_packet("Head", 7.0, -3.0, 1.0)];
  for index in 0..220 {
   content.push(OscPacket::Message(OscMessage {
    addr: "/VMC/Ext/Blend/Val".to_owned(),
    args: vec![
     OscType::String(if index % 2 == 0 {
      "LookLeft".to_owned()
     } else {
      "LookDown".to_owned()
     }),
     OscType::Float((index % 10) as f32 * 0.1),
    ],
   }));
  }

  let payload = rosc::encoder::encode(&OscPacket::Bundle(rosc::OscBundle {
   timetag: (0, 0).into(),
   content,
  }))
  .expect("encode large VMC bundle");
  assert!(payload.len() > rosc::decoder::MTU);

  let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender UDP socket");
  let target = format!("127.0.0.1:{port}");
  sender.send_to(&payload, &target).expect("send large OSC payload");

  let mut parsed = None;
  for _ in 0..40 {
   parsed = receiver.poll_frame();
   if parsed.is_some() {
    break;
   }
   thread::sleep(Duration::from_millis(5));
  }

  let frame = parsed.expect("receiver should parse large VMC bundle payload");
  assert!((frame.head_yaw_deg - 7.0).abs() < 0.1);
  assert!((frame.head_pitch_deg + 3.0).abs() < 0.1);
  assert!(receiver.stats().udp_packets_received >= 1);
 }

 #[test]
 fn receiver_forwards_raw_udp_to_multiple_targets() {
  let listen_port = acquire_free_port();
  let target1_port = acquire_free_port();
  let target2_port = acquire_free_port();

  let mut receiver = VmcOscReceiver::new(ReceiverOptions {
   udp_port: listen_port,
   passthrough: super::PassthroughOptions {
    enabled: true,
    targets: vec![format!("127.0.0.1:{target1_port}"), format!("127.0.0.1:{target2_port}")],
    mode: super::PassthroughMode::RawUdpForward,
   },
  });
  receiver.connect().expect("connect VMC receiver");

  let receiver_target = format!("127.0.0.1:{listen_port}");
  let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender UDP socket");

  let listener1 = UdpSocket::bind(format!("127.0.0.1:{target1_port}")).expect("bind passthrough listener1");
  let listener2 = UdpSocket::bind(format!("127.0.0.1:{target2_port}")).expect("bind passthrough listener2");
  listener1
   .set_read_timeout(Some(Duration::from_millis(800)))
   .expect("set listener1 timeout");
  listener2
   .set_read_timeout(Some(Duration::from_millis(800)))
   .expect("set listener2 timeout");

  let valid_packet = bone_packet("Head", 3.0, -1.0, 0.0);
  let payload = rosc::encoder::encode(&valid_packet).expect("encode OSC payload");
  sender.send_to(&payload, receiver_target).expect("send packet to receiver");

  for _ in 0..40 {
   let _ = receiver.poll_frame();
   if receiver.stats().passthrough_packets_forwarded >= 2 {
    break;
   }
   thread::sleep(Duration::from_millis(5));
  }

  let mut received1 = vec![0; 8192];
  let mut received2 = vec![0; 8192];
  let (size1, _) = listener1
   .recv_from(&mut received1)
   .expect("listener1 should receive forwarded datagram");
  let (size2, _) = listener2
   .recv_from(&mut received2)
   .expect("listener2 should receive forwarded datagram");

  assert_eq!(&received1[..size1], payload.as_slice());
  assert_eq!(&received2[..size2], payload.as_slice());
  assert_eq!(receiver.stats().passthrough_targets_active, 2);
  assert!(receiver.stats().passthrough_packets_forwarded >= 2);
 }

 #[test]
 fn receiver_filters_self_loop_passthrough_target() {
  let listen_port = acquire_free_port();
  let target_port = acquire_free_port();

  let mut receiver = VmcOscReceiver::new(ReceiverOptions {
   udp_port: listen_port,
   passthrough: super::PassthroughOptions {
    enabled: true,
    targets: vec![format!("127.0.0.1:{listen_port}"), format!("127.0.0.1:{target_port}")],
    mode: super::PassthroughMode::RawUdpForward,
   },
  });
  receiver.connect().expect("connect VMC receiver");

  assert_eq!(receiver.stats().passthrough_targets_active, 1);
  assert!(receiver
   .stats()
   .passthrough_last_warning
   .as_ref()
   .is_some_and(|message| message.contains("self-loop")));
 }
}
