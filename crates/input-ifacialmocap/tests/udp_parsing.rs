use std::{net::UdpSocket, thread, time::Duration};

use unvet_core::ports::InputReceiver;
use unvet_input_ifacialmocap::{ConnectionState, IfacialMocapReceiver, ReceiverOptions, parse_tracking_frame};

fn acquire_free_port() -> u16 {
 let socket = UdpSocket::bind("127.0.0.1:0").expect("bind free local UDP port");
 socket.local_addr().expect("read local address").port()
}

#[test]
fn parse_tracking_frame_extracts_head_and_eye_values() {
 let packet = "head#2.5,-1.5,0.4|leftEye#1.2,-0.4,0.0|rightEye#1.6,-0.6,0.0|confidence#0.92";
 let frame = parse_tracking_frame(packet, 1234).expect("parse iFacialMocap packet");

 assert_eq!(frame.timestamp_ms, 1234);
 assert!((frame.head_yaw_deg - 2.5).abs() < 0.001);
 assert!((frame.head_pitch_deg + 1.5).abs() < 0.001);
 assert!((frame.head_roll_deg - 0.4).abs() < 0.001);
 assert!((frame.eye_yaw_deg - 1.4).abs() < 0.001);
 assert!((frame.eye_pitch_deg + 0.5).abs() < 0.001);
 assert!((frame.confidence - 0.91632).abs() < 0.001);
 assert!(frame.active);
}

#[test]
fn parse_tracking_frame_accepts_prefixed_head_segment() {
 let packet = "=head#2.5,-1.5,0.4|leftEye#1.2,-0.4,0.0|rightEye#1.6,-0.6,0.0|confidence#0.92";
 let frame = parse_tracking_frame(packet, 4321).expect("parse iFacialMocap packet with prefixed head");

 assert_eq!(frame.timestamp_ms, 4321);
 assert!((frame.head_yaw_deg - 2.5).abs() < 0.001);
 assert!((frame.eye_yaw_deg - 1.4).abs() < 0.001);
 assert!(frame.active);
}

#[test]
fn parse_tracking_frame_requires_head_and_both_eyes() {
 let packet = "head#2.5,-1.5,0.4|leftEye#1.2,-0.4,0.0";
 let error = parse_tracking_frame(packet, 1).expect_err("missing rightEye should fail");

 assert!(error.to_string().contains("rightEye field is missing"));
}

#[test]
fn udp_receiver_continues_after_broken_frame() {
 let udp_port = acquire_free_port();

 let options = ReceiverOptions {
  host: "127.0.0.1".to_owned(),
  udp_port,
  ..ReceiverOptions::default()
 };

 let mut receiver = IfacialMocapReceiver::new(options);
 receiver.connect().expect("receiver should connect in UDP mode");

 let sender = UdpSocket::bind("127.0.0.1:0").expect("create sender socket");
 let target = format!("127.0.0.1:{udp_port}");

 sender.send_to(b"broken_payload", &target).expect("send broken packet");
 sender
  .send_to(
   b"head#6.0,-2.0,0.3|leftEye#3.0,-1.1,0.0|rightEye#3.4,-1.3,0.0|confidence#0.85",
   &target,
  )
  .expect("send valid packet");

 let mut parsed_frame = None;
 for _ in 0..30 {
  if let Some(frame) = receiver.poll_frame() {
   parsed_frame = Some(frame);
   break;
  }
  thread::sleep(Duration::from_millis(5));
 }

 let frame = parsed_frame.expect("receiver should parse at least one valid UDP packet");
 assert!(frame.active);
 assert!((frame.eye_yaw_deg - 3.2).abs() < 0.001);
 assert_eq!(receiver.state(), ConnectionState::Receiving);
 assert!(receiver.stats().frames_parsed >= 1);
 assert!(receiver.stats().frames_dropped >= 1);

 receiver.disconnect();
 assert_eq!(receiver.state(), ConnectionState::Disconnected);
}

#[test]
fn udp_receiver_connects_when_preferred_bind_port_is_in_use() {
 let udp_port = acquire_free_port();
 let _port_holder = UdpSocket::bind(format!("0.0.0.0:{udp_port}")).expect("reserve UDP port");

 let options = ReceiverOptions {
  host: "127.0.0.1".to_owned(),
  udp_port,
  ..ReceiverOptions::default()
 };

 let mut receiver = IfacialMocapReceiver::new(options);
 receiver
  .connect()
  .expect("receiver should fall back to an ephemeral local UDP port");

 assert_eq!(receiver.state(), ConnectionState::Receiving);
 receiver.disconnect();
}
