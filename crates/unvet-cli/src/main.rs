use std::{
 env,
 path::PathBuf,
 thread,
 time::{Duration, Instant},
};

use tracing::{info, warn};
use unvet_config::{AppConfig, InputSource, MappingBlendPreset, MappingConfig, MappingCurvePreset, VmcOscPassthroughMode};
use unvet_core::{
 calibration::NeutralPoseCalibration,
 filter::OutputFrameSmoother,
 logging,
 mapping::{map_angle_to_normalized, mix_eye_and_head, resolve_head_eye_mix, AxisMappingSettings, HeadEyeBlendPreset, ResponseCurvePreset},
 model::{OutputFrame, TrackingFrame},
 ports::InputReceiver,
 AppResult,
};
use unvet_input_ifacialmocap::{IfacialMocapReceiver, ReceiverOptions};
use unvet_input_vmc_osc::{
 PassthroughMode as VmcReceiverPassthroughMode, PassthroughOptions as VmcReceiverPassthroughOptions,
 ReceiverOptions as VmcOscReceiverOptions, VmcOscReceiver,
};
use unvet_output::OutputBackendLayer;

const OUTPUT_EASING_ALPHA_MIN: f32 = 0.01;
const OUTPUT_EASING_ALPHA_MAX: f32 = 1.0;

enum ActiveInputReceiver {
 IfacialMocap(IfacialMocapReceiver),
 VmcOsc(VmcOscReceiver),
}

impl ActiveInputReceiver {
 fn from_config(config: &AppConfig) -> Self {
  match config.input.source {
   InputSource::IfacialmocapUdp | InputSource::IfacialmocapTcp => {
    let mut options = ReceiverOptions::default();
    options.host = config.input.host.clone();
    options.udp_port = config.input.udp_port;
    options.tcp_port = config.input.tcp_port;
    options.use_tcp = matches!(config.input.source, InputSource::IfacialmocapTcp);
    Self::IfacialMocap(IfacialMocapReceiver::new(options))
   },
   InputSource::VmcOsc => {
    let mut options = VmcOscReceiverOptions::default();
    options.udp_port = config.input.vmc_osc_port;
    options.passthrough = VmcReceiverPassthroughOptions {
     enabled: config.vmc_osc_passthrough.enabled,
     targets: config.vmc_osc_passthrough.targets.clone(),
     mode: match config.vmc_osc_passthrough.mode {
      VmcOscPassthroughMode::RawUdpForward => VmcReceiverPassthroughMode::RawUdpForward,
     },
    };
    Self::VmcOsc(VmcOscReceiver::new(options))
   },
  }
 }

 fn connect(&mut self) -> AppResult<()> {
  match self {
   Self::IfacialMocap(receiver) => receiver.connect(),
   Self::VmcOsc(receiver) => receiver.connect(),
  }
 }

 fn poll_frame(&mut self) -> Option<TrackingFrame> {
  match self {
   Self::IfacialMocap(receiver) => receiver.poll_frame(),
   Self::VmcOsc(receiver) => receiver.poll_frame(),
  }
 }

 fn is_active(&self) -> bool {
  match self {
   Self::IfacialMocap(receiver) => receiver.is_active(),
   Self::VmcOsc(receiver) => receiver.is_active(),
  }
 }

 fn source_name(&self) -> &'static str {
  match self {
   Self::IfacialMocap(receiver) => receiver.source_name(),
   Self::VmcOsc(receiver) => receiver.source_name(),
  }
 }
}

fn default_config_path() -> PathBuf {
 PathBuf::from("config/unvet.toml")
}

fn build_output_frame(frame: TrackingFrame, mapping: &MappingConfig, input_source: InputSource) -> OutputFrame {
 let blend_preset = match mapping.head_eye_blend_preset {
  MappingBlendPreset::Custom => HeadEyeBlendPreset::Custom,
  MappingBlendPreset::Balanced => HeadEyeBlendPreset::Balanced,
  MappingBlendPreset::EyeDominant => HeadEyeBlendPreset::EyeDominant,
  MappingBlendPreset::HeadDominant => HeadEyeBlendPreset::HeadDominant,
 };
 let (yaw_mix, pitch_mix) = resolve_head_eye_mix(blend_preset, mapping.eye_head_mix_yaw, mapping.eye_head_mix_pitch);

 let (eye_yaw_deg, eye_pitch_deg, head_yaw_deg, head_pitch_deg) = match input_source {
  InputSource::IfacialmocapUdp | InputSource::IfacialmocapTcp => {
   // iFacialMocap reports horizontal/vertical in an axis order opposite to our internal yaw/pitch labels.
   (frame.eye_pitch_deg, frame.eye_yaw_deg, frame.head_pitch_deg, frame.head_yaw_deg)
  },
  InputSource::VmcOsc => (frame.eye_yaw_deg, frame.eye_pitch_deg, frame.head_yaw_deg, frame.head_pitch_deg),
 };

 let mixed_yaw = mix_eye_and_head(eye_yaw_deg, head_yaw_deg, yaw_mix);
 let mixed_pitch = mix_eye_and_head(eye_pitch_deg, head_pitch_deg, pitch_mix);
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

 let yaw_raw = map_angle_to_normalized(mixed_yaw, yaw_settings);
 let pitch_raw = map_angle_to_normalized(mixed_pitch, pitch_settings);

 OutputFrame {
  look_yaw_norm: {
   let yaw_multiplier = if yaw_raw >= 0.0 { mapping.yaw_pos_output_multiplier } else { mapping.yaw_neg_output_multiplier };
   (yaw_raw * yaw_multiplier * if mapping.invert_output_yaw { -1.0 } else { 1.0 }).clamp(-1.0, 1.0)
  },
  look_pitch_norm: {
   let pitch_multiplier = if pitch_raw >= 0.0 { mapping.pitch_pos_output_multiplier } else { mapping.pitch_neg_output_multiplier };
   (pitch_raw * pitch_multiplier * if mapping.invert_output_pitch { -1.0 } else { 1.0 }).clamp(-1.0, 1.0)
  },
  look_yaw_norm_raw: yaw_raw,
  look_pitch_norm_raw: pitch_raw,
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

 let mut receiver = ActiveInputReceiver::from_config(&config);
 if let Err(error) = receiver.connect() {
  warn!(error = %error, "input receiver startup failed; running with fallback frame for bootstrap");
 }

 let mut calibration = build_calibration(&config);
 let mut active_mapping = config.effective_mapping();
 active_mapping.smoothing_alpha = active_mapping
  .smoothing_alpha
  .clamp(OUTPUT_EASING_ALPHA_MIN, OUTPUT_EASING_ALPHA_MAX);
 let mut frame_smoother = OutputFrameSmoother::new(active_mapping.smoothing_alpha);

 let mut output_layer = OutputBackendLayer::new(&config.output);
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
  let output_live = config.output.enabled;

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
   let output_frame = build_output_frame(calibrated_frame, &active_mapping, config.input.source);
   let smoothed_output = if active_mapping.output_easing_enabled {
    frame_smoother.update(output_frame)
   } else {
    frame_smoother.reset();
    output_frame
   };
   if output_live {
    output_layer.apply(smoothed_output)?;
   }

   last_frame_at = Instant::now();
   forced_idle_output = false;
  } else if !forced_idle_output && last_frame_at.elapsed() >= no_frame_idle_timeout {
   // On temporary tracking loss, send an inactive frame once to avoid stuck output state.
   if output_live {
    output_layer.apply(OutputFrame::default())?;
   }
   forced_idle_output = true;
  }

  thread::sleep(poll_interval);
 }
}
