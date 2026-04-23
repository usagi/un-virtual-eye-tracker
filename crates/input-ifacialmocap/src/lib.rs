use unvet_core::{model::TrackingFrame, ports::InputReceiver};

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
}

impl Default for ReceiverOptions {
 fn default() -> Self {
  Self {
   host: "0.0.0.0".to_owned(),
   udp_port: IFACIALMOCAP_UDP_PORT,
   tcp_port: IFACIALMOCAP_TCP_PORT,
   use_tcp: false,
  }
 }
}

pub struct IfacialMocapReceiver {
 options: ReceiverOptions,
 state: ConnectionState,
 active: bool,
 buffered_frame: Option<TrackingFrame>,
}

impl IfacialMocapReceiver {
 pub fn new(options: ReceiverOptions) -> Self {
  Self {
   options,
   state: ConnectionState::Disconnected,
   active: false,
   buffered_frame: None,
  }
 }

 pub fn options(&self) -> &ReceiverOptions {
  &self.options
 }

 pub fn state(&self) -> ConnectionState {
  self.state
 }

 pub fn connect(&mut self) {
  self.state = ConnectionState::Connecting;
  self.state = ConnectionState::Receiving;
  self.active = true;
 }

 pub fn disconnect(&mut self) {
  self.state = ConnectionState::Disconnected;
  self.active = false;
  self.buffered_frame = None;
 }

 pub fn ingest_mock_frame(&mut self, frame: TrackingFrame) {
  self.buffered_frame = Some(frame);
 }
}

impl InputReceiver for IfacialMocapReceiver {
 fn source_name(&self) -> &'static str {
  "ifacialmocap"
 }

 fn poll_frame(&mut self) -> Option<TrackingFrame> {
  self.buffered_frame.take()
 }

 fn is_active(&self) -> bool {
  self.active
 }
}
