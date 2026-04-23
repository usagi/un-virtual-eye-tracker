use std::{
 io::Read,
 net::{TcpStream, ToSocketAddrs, UdpSocket},
 time::{Duration, SystemTime, UNIX_EPOCH},
};

use unvet_core::{
 model::{RawTrackingFrame, TrackingFrame},
 ports::InputReceiver,
 AppError, AppResult,
};

pub const IFACIALMOCAP_UDP_PORT: u16 = 49983;
pub const IFACIALMOCAP_TCP_PORT: u16 = 49986;
const TCP_REASSEMBLY_MAX_BYTES: usize = 64 * 1024;

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
 tcp_stream: Option<TcpStream>,
 tcp_read_buffer: [u8; 4096],
 tcp_reassembly_buffer: Vec<u8>,
 stats: ReceiverStats,
}

#[derive(Debug, Clone, Default)]
pub struct ReceiverStats {
 pub udp_packets_received: u64,
 pub tcp_reads: u64,
 pub tcp_bytes_received: u64,
 pub tcp_frames_reassembled: u64,
 pub frames_parsed: u64,
 pub frames_dropped: u64,
 pub last_packet_timestamp_ms: Option<u64>,
 pub last_error: Option<String>,
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
   tcp_stream: None,
   tcp_read_buffer: [0; 4096],
   tcp_reassembly_buffer: Vec::with_capacity(4096),
   stats: ReceiverStats::default(),
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
   return self.connect_tcp();
  }

  self.connect_udp()
 }

 fn connect_udp(&mut self) -> AppResult<()> {
  if self.options.use_tcp {
   let error = AppError::InvalidState("TCP mode is not implemented yet".to_owned());
   self.record_error(error.to_string());
   return Err(error);
  }

  self.state = ConnectionState::Connecting;

  let bind_address = format!("0.0.0.0:{}", self.options.udp_port);
  let socket = match UdpSocket::bind(&bind_address) {
   Ok(socket) => socket,
   Err(error) => {
    self.record_error(format!("failed to bind UDP socket on {bind_address}: {error}"));
    return Err(AppError::from(error));
   },
  };
  socket.set_nonblocking(true)?;
  socket.set_read_timeout(Some(Duration::from_millis(1)))?;

  let mut destination_candidates = match format!("{}:{}", self.options.host, self.options.udp_port).to_socket_addrs() {
   Ok(candidates) => candidates,
   Err(error) => {
    self.record_error(format!("invalid UDP host '{}': {error}", self.options.host));
    return Err(AppError::InvalidState(format!("invalid UDP host '{}': {error}", self.options.host)));
   },
  };
  let destination = match destination_candidates.next() {
   Some(destination) => destination,
   None => {
    let message = format!(
     "no UDP destination could be resolved for '{}:{}'",
     self.options.host, self.options.udp_port
    );
    self.record_error(message.clone());
    return Err(AppError::InvalidState(message));
   },
  };

  if let Err(error) = socket.send_to(self.options.start_command.as_bytes(), destination) {
   self.record_error(format!("failed to send start command to {destination}: {error}"));
   return Err(AppError::from(error));
  }

  self.socket = Some(socket);
  self.state = ConnectionState::Receiving;
  self.active = true;
  self.stats.last_error = None;
  Ok(())
 }

 fn connect_tcp(&mut self) -> AppResult<()> {
  self.state = ConnectionState::Connecting;

  let mut destination_candidates = match format!("{}:{}", self.options.host, self.options.tcp_port).to_socket_addrs() {
   Ok(candidates) => candidates,
   Err(error) => {
    self.record_error(format!("invalid TCP host '{}': {error}", self.options.host));
    return Err(AppError::InvalidState(format!("invalid TCP host '{}': {error}", self.options.host)));
   },
  };
  let destination = match destination_candidates.next() {
   Some(destination) => destination,
   None => {
    let message = format!(
     "no TCP destination could be resolved for '{}:{}'",
     self.options.host, self.options.tcp_port
    );
    self.record_error(message.clone());
    return Err(AppError::InvalidState(message));
   },
  };

  let stream = match TcpStream::connect(destination) {
   Ok(stream) => stream,
   Err(error) => {
    self.record_error(format!("failed to connect TCP stream to {destination}: {error}"));
    return Err(AppError::from(error));
   },
  };
  stream.set_nonblocking(true)?;
  stream.set_read_timeout(Some(Duration::from_millis(1)))?;

  self.tcp_stream = Some(stream);
  self.tcp_reassembly_buffer.clear();
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
  self.tcp_stream = None;
  self.tcp_reassembly_buffer.clear();
 }

 pub fn stats(&self) -> &ReceiverStats {
  &self.stats
 }

 pub fn clear_error(&mut self) {
  self.stats.last_error = None;
  if self.socket.is_some() || self.tcp_stream.is_some() {
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
  if self.options.use_tcp {
   self.try_read_tcp_frames();
  } else {
   self.try_read_udp_frames();
  }
 }

 fn try_read_udp_frames(&mut self) {
  let mut latest = None;

  let Some(socket) = self.socket.as_ref().and_then(|socket| socket.try_clone().ok()) else {
   return;
  };

  loop {
   match socket.recv_from(&mut self.receive_buffer) {
    Ok((size, _source)) => {
     self.stats.udp_packets_received += 1;
     self.stats.last_packet_timestamp_ms = Some(now_millis());

     let packet = String::from_utf8_lossy(&self.receive_buffer[..size]);
     match parse_tracking_frame(&packet, now_millis()) {
      Ok(frame) => {
       self.stats.frames_parsed += 1;
       latest = Some(frame);
      },
      Err(error) => {
       self.stats.frames_dropped += 1;
       self.stats.last_error = Some(error.to_string());
      },
     }
    },
    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
     break;
    },
    Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
     break;
    },
    Err(error) => {
     self.stats.frames_dropped += 1;
     self.record_error(format!("UDP receive failed: {error}"));
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

 fn try_read_tcp_frames(&mut self) {
  let mut latest = None;

  let Some(mut stream) = self.tcp_stream.as_ref().and_then(|stream| stream.try_clone().ok()) else {
   return;
  };

  loop {
   match stream.read(&mut self.tcp_read_buffer) {
    Ok(0) => {
     self.record_error("TCP stream closed by peer".to_owned());
     break;
    },
    Ok(size) => {
     self.stats.tcp_reads += 1;
     self.stats.tcp_bytes_received += size as u64;
     self.stats.last_packet_timestamp_ms = Some(now_millis());
     self.tcp_reassembly_buffer.extend_from_slice(&self.tcp_read_buffer[..size]);
     self.trim_tcp_reassembly_buffer();
     if let Some(frame) = self.parse_reassembled_tcp_frames() {
      latest = Some(frame);
     }
    },
    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
     break;
    },
    Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
     break;
    },
    Err(error) => {
     self.stats.frames_dropped += 1;
     self.record_error(format!("TCP receive failed: {error}"));
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

 fn trim_tcp_reassembly_buffer(&mut self) {
  if self.tcp_reassembly_buffer.len() <= TCP_REASSEMBLY_MAX_BYTES {
   return;
  }

  let overflow = self.tcp_reassembly_buffer.len() - TCP_REASSEMBLY_MAX_BYTES;
  self.tcp_reassembly_buffer.drain(..overflow);
  self.stats.frames_dropped += 1;
  self.stats.last_error = Some(format!("TCP reassembly buffer overflow: dropped {overflow} byte(s)"));
 }

 fn parse_reassembled_tcp_frames(&mut self) -> Option<TrackingFrame> {
  let mut latest = None;

  while let Some(frame_end_index) = self.tcp_reassembly_buffer.iter().position(|byte| is_tcp_delimiter(*byte)) {
   let frame_with_delimiter: Vec<u8> = self.tcp_reassembly_buffer.drain(..=frame_end_index).collect();
   let raw_frame = &frame_with_delimiter[..frame_with_delimiter.len().saturating_sub(1)];
   let payload = String::from_utf8_lossy(raw_frame);
   let payload = payload
    .trim_matches(|character| character == '\r' || character == '\n' || character == '\0')
    .trim();
   if payload.is_empty() {
    continue;
   }

   self.stats.tcp_frames_reassembled += 1;
   match parse_tracking_frame(payload, now_millis()) {
    Ok(frame) => {
     self.stats.frames_parsed += 1;
     latest = Some(frame);
    },
    Err(error) => {
     self.stats.frames_dropped += 1;
     self.stats.last_error = Some(error.to_string());
    },
   }
  }

  latest
 }

 fn record_error(&mut self, message: String) {
  self.state = ConnectionState::Error;
  self.active = false;
  self.stats.last_error = Some(message);
 }
}

fn is_tcp_delimiter(byte: u8) -> bool {
 matches!(byte, b'\n' | b'\0' | b'\r')
}

impl InputReceiver for IfacialMocapReceiver {
 fn source_name(&self) -> &'static str {
  "ifacialmocap"
 }

 fn poll_frame(&mut self) -> Option<TrackingFrame> {
  self.try_read_frames();
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
 let (right_eye_yaw_deg, right_eye_pitch_deg, _) =
  right_eye.ok_or_else(|| AppError::InvalidData("rightEye field is missing".to_owned()))?;

 Ok(TrackingFrame::from_raw(RawTrackingFrame {
  timestamp_ms,
  head_yaw_deg,
  head_pitch_deg,
  head_roll_deg,
  eye_yaw_deg: None,
  eye_pitch_deg: None,
  left_eye_yaw_deg: Some(left_eye_yaw_deg),
  left_eye_pitch_deg: Some(left_eye_pitch_deg),
  right_eye_yaw_deg: Some(right_eye_yaw_deg),
  right_eye_pitch_deg: Some(right_eye_pitch_deg),
  reported_confidence: Some(confidence),
  reported_active: Some(confidence > 0.2),
 }))
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
