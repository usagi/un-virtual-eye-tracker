use std::{
 net::{
  ToSocketAddrs,
  UdpSocket
 },
 time::{
  Duration,
  SystemTime,
  UNIX_EPOCH
 }
};

use unvet_core::{
 model::TrackingFrame,
 ports::InputReceiver,
 AppError,
 AppResult
};

pub const IFACIALMOCAP_UDP_PORT: u16 = 49983;
pub const IFACIALMOCAP_TCP_PORT: u16 = 49986;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
 Disconnected,
 Connecting,
 Receiving,
 Error,
}

#[derive(Debug, Clone)]
pub struct ReceiverOptions {
 pub host: String,
 pub udp_port: u16,
 pub tcp_port: u16,
 pub use_tcp: bool,
 pub start_command: String,
}

impl Default for ReceiverOptions {
 fn default() -> Self {
  Self {
   host: "0.0.0.0".to_owned(),
   udp_port: IFACIALMOCAP_UDP_PORT,
   tcp_port: IFACIALMOCAP_TCP_PORT,
   use_tcp: false,
     start_command: "iFacialMocap_sahne".to_owned(),
  }
 }
}

pub struct IfacialMocapReceiver {
 options: ReceiverOptions,
 state: ConnectionState,
 active: bool,
 buffered_frame: Option<TrackingFrame>,
 socket: Option<UdpSocket>,
 receive_buffer: [u8; 8192],
}

impl IfacialMocapReceiver {
 pub fn new(options: ReceiverOptions) -> Self {
  Self {
   options,
   state: ConnectionState::Disconnected,
   active: false,
   buffered_frame: None,
    socket: None,
    receive_buffer: [0; 8192],
  }
 }

 pub fn options(&self) -> &ReceiverOptions {
  &self.options
 }

 pub fn state(&self) -> ConnectionState {
  self.state
 }

 pub fn connect(&mut self) -> AppResult<()> {
  if self.options.use_tcp {
   self.state = ConnectionState::Error;
   self.active = false;
   return Err(AppError::InvalidState("TCP mode is not implemented yet".to_owned()));
  }

  self.state = ConnectionState::Connecting;

  let bind_address = format!("0.0.0.0:{}", self.options.udp_port);
  let socket = UdpSocket::bind(&bind_address)?;
  socket.set_nonblocking(true)?;
  socket.set_read_timeout(Some(Duration::from_millis(1)))?;

  let mut destination_candidates = format!("{}:{}", self.options.host, self.options.udp_port)
   .to_socket_addrs()
   .map_err(|error| AppError::InvalidState(format!("invalid UDP host '{}': {error}", self.options.host)))?;
  let destination = destination_candidates
   .next()
   .ok_or_else(|| AppError::InvalidState(format!("no UDP destination could be resolved for '{}:{}'", self.options.host, self.options.udp_port)))?;

  socket.send_to(self.options.start_command.as_bytes(), destination)?;

  self.socket = Some(socket);
  self.state = ConnectionState::Receiving;
  self.active = true;
  Ok(())
 }

 pub fn disconnect(&mut self) {
  self.state = ConnectionState::Disconnected;
  self.active = false;
  self.buffered_frame = None;
  self.socket = None;
 }

 pub fn ingest_mock_frame(&mut self, frame: TrackingFrame) {
  self.buffered_frame = Some(frame);
 }

 fn try_read_udp_frames(&mut self) {
  let mut latest = None;

  let Some(socket) = self.socket.as_ref() else {
   return;
  };

  loop {
   match socket.recv_from(&mut self.receive_buffer) {
    Ok((size, _source)) => {
     let packet = String::from_utf8_lossy(&self.receive_buffer[..size]);
     if let Ok(frame) = parse_tracking_frame(&packet, now_millis()) {
      latest = Some(frame);
     }
    }
    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
     break;
    }
    Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
     break;
    }
    Err(_error) => {
     self.state = ConnectionState::Error;
     self.active = false;
     break;
    }
   }
  }

  if latest.is_some() {
   self.state = ConnectionState::Receiving;
   self.active = true;
   self.buffered_frame = latest;
  }
 }
}

impl InputReceiver for IfacialMocapReceiver {
 fn source_name(&self) -> &'static str {
  "ifacialmocap"
 }

 fn poll_frame(&mut self) -> Option<TrackingFrame> {
    self.try_read_udp_frames();
  self.buffered_frame.take()
 }

 fn is_active(&self) -> bool {
  self.active
 }
}

pub fn parse_tracking_frame(packet: &str, timestamp_ms: u64) -> AppResult<TrackingFrame> {
 let mut head = None;
 let mut left_eye = None;
 let mut right_eye = None;
 let mut confidence = 1.0_f32;

 for segment in packet.split('|') {
  let trimmed = segment.trim();
  if trimmed.is_empty() {
   continue;
  }

  if let Some(raw) = trimmed.strip_prefix("head#") {
   head = Some(parse_triplet(raw)?);
   continue;
  }

  if let Some(raw) = trimmed.strip_prefix("leftEye#") {
   left_eye = Some(parse_triplet(raw)?);
   continue;
  }

  if let Some(raw) = trimmed.strip_prefix("rightEye#") {
   right_eye = Some(parse_triplet(raw)?);
   continue;
  }

  if let Some(raw) = trimmed.strip_prefix("confidence#") {
   if let Ok(value) = raw.trim().parse::<f32>() {
    confidence = value.clamp(0.0, 1.0);
   }
  }
 }

 let (head_yaw_deg, head_pitch_deg, head_roll_deg) = head.ok_or_else(|| AppError::InvalidData("head field is missing".to_owned()))?;
 let (left_eye_yaw_deg, left_eye_pitch_deg, _) = left_eye.ok_or_else(|| AppError::InvalidData("leftEye field is missing".to_owned()))?;
 let (right_eye_yaw_deg, right_eye_pitch_deg, _) = right_eye.ok_or_else(|| AppError::InvalidData("rightEye field is missing".to_owned()))?;

 let eye_yaw_deg = (left_eye_yaw_deg + right_eye_yaw_deg) * 0.5;
 let eye_pitch_deg = (left_eye_pitch_deg + right_eye_pitch_deg) * 0.5;

 Ok(TrackingFrame {
  timestamp_ms,
  head_yaw_deg,
  head_pitch_deg,
  head_roll_deg,
  eye_yaw_deg,
  eye_pitch_deg,
  left_eye_yaw_deg,
  left_eye_pitch_deg,
  right_eye_yaw_deg,
  right_eye_pitch_deg,
  confidence,
  active: confidence > 0.2
 })
}

fn parse_triplet(raw: &str) -> AppResult<(f32, f32, f32)> {
 let mut values = raw.split(',').map(str::trim);
 let x = values
  .next()
  .ok_or_else(|| AppError::InvalidData(format!("invalid triplet '{}': missing x", raw)))?
  .parse::<f32>()
  .map_err(|error| AppError::InvalidData(format!("invalid triplet '{}': {error}", raw)))?;
 let y = values
  .next()
  .ok_or_else(|| AppError::InvalidData(format!("invalid triplet '{}': missing y", raw)))?
  .parse::<f32>()
  .map_err(|error| AppError::InvalidData(format!("invalid triplet '{}': {error}", raw)))?;
 let z = values
  .next()
  .ok_or_else(|| AppError::InvalidData(format!("invalid triplet '{}': missing z", raw)))?
  .parse::<f32>()
  .map_err(|error| AppError::InvalidData(format!("invalid triplet '{}': {error}", raw)))?;

 Ok((x, y, z))
}

fn now_millis() -> u64 {
 SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
