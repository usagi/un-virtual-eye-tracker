use std::{env, path::PathBuf};

use tracing::{info, warn};
use unvet_config::{
 AppConfig,
 InputSource,
 OutputBackendKind
};
use unvet_core::{
 calibration::NeutralPoseCalibration,
 logging,
 model::{OutputFrame, TrackingFrame},
 ports::{InputReceiver, OutputBackend},
};
use unvet_input_ifacialmocap::{IfacialMocapReceiver, ReceiverOptions};

fn default_config_path() -> PathBuf {
 PathBuf::from("config/unvet.toml")
}

fn build_output_frame(frame: TrackingFrame, config: &AppConfig) -> OutputFrame {
 let yaw_mix = config.mapping.eye_head_mix_yaw;
 let pitch_mix = config.mapping.eye_head_mix_pitch;

 let mixed_yaw = yaw_mix * frame.eye_yaw_deg + (1.0 - yaw_mix) * frame.head_yaw_deg;
 let mixed_pitch = pitch_mix * frame.eye_pitch_deg + (1.0 - pitch_mix) * frame.head_pitch_deg;

 OutputFrame {
  look_yaw_norm: (mixed_yaw / 35.0).clamp(-1.0, 1.0),
  look_pitch_norm: (mixed_pitch / 25.0).clamp(-1.0, 1.0),
  confidence: frame.confidence,
  active: frame.active,
 }
}

fn select_backend(kind: OutputBackendKind) -> Box<dyn OutputBackend> {
 match kind {
  OutputBackendKind::Ets2 => Box::new(unvet_output_ets2::Ets2Backend::default()),
  OutputBackendKind::Mouse => Box::new(unvet_output_mouse::MouseBackend::default()),
  OutputBackendKind::Keyboard => Box::new(unvet_output_keyboard::KeyboardBackend::default()),
 }
}

fn build_calibration(config: &AppConfig) -> NeutralPoseCalibration {
 match config.calibration.offsets() {
  Some(offsets) => NeutralPoseCalibration::from_offsets(config.calibration.enabled, offsets),
  None => NeutralPoseCalibration::new(config.calibration.enabled),
 }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
 logging::init_logging("info");

 let config_path = env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(default_config_path);
 let mut config = AppConfig::load_or_default(&config_path)?;

 if !config_path.exists() {
  config.save_to_path(&config_path)?;
  info!(path = %config_path.display(), "default config generated");
 }

 let mut receiver_options = ReceiverOptions::default();
 receiver_options.host = config.input.host.clone();
 receiver_options.udp_port = config.input.udp_port;
 receiver_options.tcp_port = config.input.tcp_port;
 receiver_options.use_tcp = matches!(config.input.source, InputSource::IfacialmocapTcp);

 let mut receiver = IfacialMocapReceiver::new(receiver_options);
 if let Err(error) = receiver.connect() {
  warn!(error = %error, "input receiver startup failed; running with fallback frame for bootstrap");
 }

 let mut calibration = build_calibration(&config);

 let mut backend = select_backend(config.output.backend);
 backend.set_enabled(config.output.enabled);

 receiver.ingest_mock_frame(TrackingFrame {
  timestamp_ms: 0,
  head_yaw_deg: 2.0,
  head_pitch_deg: -1.5,
  head_roll_deg: 0.0,
  eye_yaw_deg: 4.0,
  eye_pitch_deg: -2.0,
  left_eye_yaw_deg: 3.8,
  left_eye_pitch_deg: -2.2,
  right_eye_yaw_deg: 4.2,
  right_eye_pitch_deg: -1.8,
  confidence: 1.0,
  active: true,
 });

 if let Some(frame) = receiver.poll_frame() {
    if config.calibration.enabled && config.calibration.capture_on_start && !config.calibration.calibrated && receiver.is_active() {
     calibration.calibrate_from_frame(frame);
     config.calibration.set_offsets(calibration.offsets());
     config.calibration.capture_on_start = false;
     config.save_to_path(&config_path)?;
     info!(path = %config_path.display(), "neutral calibration captured and persisted");
    }

    let calibrated_frame = calibration.apply(frame);
    let output_frame = build_output_frame(calibrated_frame, &config);
  backend.apply(output_frame)?;
  info!(backend = backend.backend_name(), "bootstrap output frame applied");
 }

 info!(receiver = receiver.source_name(), "UNVET bootstrap complete");
 Ok(())
}
