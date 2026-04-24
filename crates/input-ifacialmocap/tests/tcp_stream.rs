use std::{io::Write, net::TcpListener, thread, time::Duration};

use unvet_core::ports::InputReceiver;
use unvet_input_ifacialmocap::{ConnectionState, IfacialMocapReceiver, ReceiverOptions};

fn poll_until_frame(receiver: &mut IfacialMocapReceiver, attempts: usize) -> Option<unvet_core::model::TrackingFrame> {
 for _ in 0..attempts {
  if let Some(frame) = receiver.poll_frame() {
   return Some(frame);
  }
  thread::sleep(Duration::from_millis(5));
 }

 None
}

#[test]
fn tcp_receiver_reassembles_split_frames() {
 let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock TCP listener");
 let port = listener.local_addr().expect("read listener address").port();

 let sender = thread::spawn(move || {
  let (mut stream, _) = listener.accept().expect("accept receiver connection");
  stream
   .write_all(b"head#3.0,-1.5,0.2|leftEye#1.0,-0.6,0.0|")
   .expect("write first half frame");
  thread::sleep(Duration::from_millis(10));
  stream
   .write_all(b"rightEye#1.4,-0.8,0.0|confidence#0.9\n")
   .expect("write second half frame");
 });

 let mut receiver = IfacialMocapReceiver::new(ReceiverOptions {
  host: "127.0.0.1".to_owned(),
  tcp_port: port,
  use_tcp: true,
  ..ReceiverOptions::default()
 });
 receiver.connect().expect("connect TCP receiver");

 let frame = poll_until_frame(&mut receiver, 60).expect("expected frame from split TCP payload");
 assert!(frame.active);
 assert!((frame.head_yaw_deg - 3.0).abs() < 0.001);
 assert!((frame.eye_yaw_deg - 1.2).abs() < 0.001);
 assert_eq!(receiver.state(), ConnectionState::Receiving);
 assert!(receiver.stats().tcp_reads >= 1);
 assert!(receiver.stats().tcp_frames_reassembled >= 1);

 sender.join().expect("TCP sender thread should complete");
}

#[test]
fn tcp_receiver_handles_invalid_then_valid_frame() {
 let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock TCP listener");
 let port = listener.local_addr().expect("read listener address").port();

 let sender = thread::spawn(move || {
  let (mut stream, _) = listener.accept().expect("accept receiver connection");
  stream.write_all(b"invalid_payload\r").expect("write invalid payload");
  thread::sleep(Duration::from_millis(10));
  stream
   .write_all(b"head#4.0,-1.0,0.1|leftEye#2.0,-0.5,0.0|rightEye#2.6,-0.7,0.0|confidence#0.8\r")
   .expect("write valid payload");
 });

 let mut receiver = IfacialMocapReceiver::new(ReceiverOptions {
  host: "127.0.0.1".to_owned(),
  tcp_port: port,
  use_tcp: true,
  ..ReceiverOptions::default()
 });
 receiver.connect().expect("connect TCP receiver");

 let frame = poll_until_frame(&mut receiver, 60).expect("expected frame after invalid TCP payload");
 assert!(frame.active);
 assert!((frame.eye_pitch_deg + 0.6).abs() < 0.001);
 assert_eq!(receiver.state(), ConnectionState::Receiving);
 assert!(receiver.stats().frames_parsed >= 1);
 assert!(receiver.stats().frames_dropped >= 1);

 sender.join().expect("TCP sender thread should complete");
}
