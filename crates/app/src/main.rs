use std::{
 env,
 path::PathBuf,
 thread,
 time::{Duration, Instant},
};

use tracing::{info, warn};
use unvet_config::{AppConfig, InputSource, MappingBlendPreset, MappingConfig, MappingCurvePreset};
use unvet_core::{
 calibration::NeutralPoseCalibration,
 filter::OutputFrameSmoother,
 logging,
 mapping::{AxisMappingSettings, HeadEyeBlendPreset, ResponseCurvePreset, map_angle_to_normalized, mix_eye_and_head, resolve_head_eye_mix},
 model::{OutputFrame, TrackingFrame},
 ports::InputReceiver,
};
use unvet_input_ifacialmocap::{IfacialMocapReceiver, ReceiverOptions};
use unvet_output::OutputBackendLayer;

fn default_config_path() -> PathBuf {
 PathBuf::from("config/unvet.toml")
}

fn build_output_frame(frame: TrackingFrame, mapping: &MappingConfig) -> OutputFrame {
 let blend_preset = match mapping.head_eye_blend_preset {
  MappingBlendPreset::Custom => HeadEyeBlendPreset::Custom,
  MappingBlendPreset::Balanced => HeadEyeBlendPreset::Balanced,
  MappingBlendPreset::EyeDominant => HeadEyeBlendPreset::EyeDominant,
  MappingBlendPreset::HeadDominant => HeadEyeBlendPreset::HeadDominant,
 };
 let (yaw_mix, pitch_mix) = resolve_head_eye_mix(blend_preset, mapping.eye_head_mix_yaw, mapping.eye_head_mix_pitch);

 let mixed_yaw = mix_eye_and_head(frame.eye_yaw_deg, frame.head_yaw_deg, yaw_mix);
 let mixed_pitch = mix_eye_and_head(frame.eye_pitch_deg, frame.head_pitch_deg, pitch_mix);
 let response_curve = match mapping.response_curve_preset {
  MappingCurvePreset::Linear => ResponseCurvePreset::Linear,
  MappingCurvePreset::Smooth => ResponseCurvePreset::Smooth,
  MappingCurvePreset::Aggressive => ResponseCurvePreset::Aggressive,
 };

 let yaw_settings = AxisMappingSettings {
  sensitivity: mapping.yaw_sensitivity,
  deadzone: mapping.deadzone_percent,
  max_input_angle_deg: 35.0,
  response_curve,
 };
 let pitch_settings = AxisMappingSettings {
  sensitivity: mapping.pitch_sensitivity,
  deadzone: mapping.deadzone_percent,
  max_input_angle_deg: 25.0,
  response_curve,
 };

 OutputFrame {
  look_yaw_norm: map_angle_to_normalized(mixed_yaw, yaw_settings),
  look_pitch_norm: map_angle_to_normalized(mixed_pitch, pitch_settings),
  confidence: frame.confidence,
  active: frame.active,
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
 let active_mapping = config.effective_mapping();
 let mut frame_smoother = OutputFrameSmoother::new(active_mapping.smoothing_alpha);

 let mut output_layer = OutputBackendLayer::new(config.output.backend);
 output_layer.set_enabled(config.output.enabled)?;

 let poll_interval = Duration::from_millis(8);
 let no_frame_idle_timeout = Duration::from_millis(250);
 let reconnect_interval = Duration::from_secs(1);
 let mut last_frame_at = Instant::now();
 let mut last_reconnect_attempt_at = Instant::now();
 let mut forced_idle_output = false;

 info!(
  receiver = receiver.source_name(),
  backend = output_layer.active_backend_name()?,
  "UNVET runtime loop started; press Ctrl+C to stop"
 );

 loop {
  if !receiver.is_active() && last_reconnect_attempt_at.elapsed() >= reconnect_interval {
   match receiver.connect() {
    Ok(()) => info!(receiver = receiver.source_name(), "input receiver reconnected"),
    Err(error) => warn!(error = %error, "input receiver reconnect failed"),
   }
   last_reconnect_attempt_at = Instant::now();
  }

  if let Some(frame) = receiver.poll_frame() {
   if config.calibration.enabled && config.calibration.capture_on_start && !config.calibration.calibrated && receiver.is_active() {
    calibration.calibrate_from_frame(frame);
    config.calibration.set_offsets(calibration.offsets());
    config.calibration.capture_on_start = false;
    config.save_to_path(&config_path)?;
    info!(path = %config_path.display(), "neutral calibration captured and persisted");
   }

   let calibrated_frame = calibration.apply(frame);
   let output_frame = build_output_frame(calibrated_frame, &active_mapping);
   let smoothed_output = frame_smoother.update(output_frame);
   output_layer.apply(smoothed_output)?;

   last_frame_at = Instant::now();
   forced_idle_output = false;
  } else if !forced_idle_output && last_frame_at.elapsed() >= no_frame_idle_timeout {
   // On temporary tracking loss, send an inactive frame once to avoid stuck output state.
   output_layer.apply(OutputFrame::default())?;
   forced_idle_output = true;
  }

  thread::sleep(poll_interval);
 }
}
