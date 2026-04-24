use std::{
 net::UdpSocket,
 time::{Duration, SystemTime, UNIX_EPOCH},
};

use glam::{EulerRot, Quat};
use rosc::{OscMessage, OscPacket, OscType};
use unvet_core::{
 AppError, AppResult,
 model::{RawTrackingFrame, TrackingFrame},
 ports::InputReceiver,
};

pub const VMC_OSC_DEFAULT_PORT: u16 = 39539;
const RECEIVE_BUFFER_BYTES: usize = 64 * 1024;
const DEFAULT_EYE_BLEND_MAX_ANGLE_DEG: f32 = 35.0;

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
}

impl Default for ReceiverOptions {
 fn default() -> Self {
  Self {
   udp_port: VMC_OSC_DEFAULT_PORT,
  }
 }
}

#[derive(Debug, Clone, Default)]
pub struct ReceiverStats {
 pub udp_packets_received: u64,
 pub osc_packets_decoded: u64,
 pub frames_emitted: u64,
 pub frames_ignored: u64,
 pub last_packet_timestamp_ms: Option<u64>,
 pub last_error: Option<String>,
}

pub struct VmcOscReceiver {
 options: ReceiverOptions,
 state: ConnectionState,
 active: bool,
 buffered_frame: Option<TrackingFrame>,
 socket: Option<UdpSocket>,
 receive_buffer: [u8; RECEIVE_BUFFER_BYTES],
 stats: ReceiverStats,
}

impl VmcOscReceiver {
 pub fn new(options: ReceiverOptions) -> Self {
  Self {
   options,
   state: ConnectionState::Disconnected,
   active: false,
   buffered_frame: None,
   socket: None,
   receive_buffer: [0; RECEIVE_BUFFER_BYTES],
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
  // Ensure stale sockets are released before rebinding during reconnect.
  self.socket = None;
  self.state = ConnectionState::Connecting;

  let bind_address = format!("0.0.0.0:{}", self.options.udp_port);
  let socket = UdpSocket::bind(&bind_address).map_err(|error| {
   self.record_error(format!("failed to bind VMC/OSC UDP socket on {bind_address}: {error}"));
   AppError::from(error)
  })?;

  socket.set_nonblocking(true)?;
  socket.set_read_timeout(Some(Duration::from_millis(1)))?;

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
  self.socket = None;
 }

 pub fn clear_error(&mut self) {
  self.stats.last_error = None;
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
  let mut latest = None;
  let Some(socket) = self.socket.as_ref().and_then(|socket| socket.try_clone().ok()) else {
   return;
  };

  loop {
   match socket.recv_from(&mut self.receive_buffer) {
    Ok((size, _source)) => {
     self.stats.udp_packets_received += 1;
     self.stats.last_packet_timestamp_ms = Some(now_millis());

     let packet = match decode_osc_packet(&self.receive_buffer[..size]) {
      Ok(packet) => packet,
      Err(error) => {
       self.stats.frames_ignored += 1;
       // Keep the link active on malformed OSC payloads and wait for the next packet.
       self.stats.last_error = Some(error);
       continue;
      },
     };

     self.stats.osc_packets_decoded += 1;
     if let Some(frame) = parse_tracking_frame_from_packet(&packet, now_millis()) {
      self.stats.frames_emitted += 1;
      latest = Some(frame);
     } else {
      self.stats.frames_ignored += 1;
     }
    },
    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
     break;
    },
    Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
     break;
    },
    Err(error) => {
     self.stats.frames_ignored += 1;
     self.record_error(format!("VMC/OSC UDP receive failed: {error}"));
     break;
    },
   }
  }

  if latest.is_some() {
   self.state = ConnectionState::Receiving;
   self.active = true;
   self.stats.last_error = None;
   self.buffered_frame = latest;
  }
 }

 fn record_error(&mut self, message: String) {
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

#[cfg(test)]
mod tests {
 use std::{net::UdpSocket, thread, time::Duration};

 use super::{ConnectionState, ReceiverOptions, VmcOscReceiver, parse_tracking_frame_from_packet, quaternion_to_rotation_deg};
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
  let mut receiver = VmcOscReceiver::new(ReceiverOptions { udp_port: port });
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
  let mut receiver = VmcOscReceiver::new(ReceiverOptions { udp_port: port });
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
}
