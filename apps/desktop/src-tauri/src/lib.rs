use std::{
 path::PathBuf,
 sync::{Arc, Mutex},
 thread,
 time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::State;
use unvet_config::{
 AppConfig,
 InputSource,
 MappingBlendPreset,
 MappingConfig,
 MappingCurvePreset,
 OutputBackendKind,
};
use unvet_core::{
 calibration::NeutralPoseCalibration,
 filter::OutputFrameSmoother,
 mapping::{
  AxisMappingSettings,
  HeadEyeBlendPreset,
  ResponseCurvePreset,
  map_angle_to_normalized,
  mix_eye_and_head,
  resolve_head_eye_mix,
 },
 model::{OutputFrame, TrackingFrame},
 ports::InputReceiver,
};
use unvet_input_ifacialmocap::{IfacialMocapReceiver, ReceiverOptions};
use unvet_output::OutputBackendLayer;

const POLL_INTERVAL: Duration = Duration::from_millis(8);
const IDLE_TIMEOUT: Duration = Duration::from_millis(250);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeSnapshot {
 input_connected: bool,
 output_enabled: bool,
 paused: bool,
 input_source: InputSource,
 output_backend: OutputBackendKind,
 look_yaw_norm: f32,
 look_pitch_norm: f32,
 confidence: f32,
 active: bool,
 last_error: Option<String>,
 updated_at_ms: u64,
}

impl RuntimeSnapshot {
 fn from_config(config: &AppConfig) -> Self {
  Self {
   input_connected: false,
   output_enabled: config.output.enabled,
   paused: false,
   input_source: config.input.source,
   output_backend: config.output.backend,
   look_yaw_norm: 0.0,
   look_pitch_norm: 0.0,
   confidence: 0.0,
   active: false,
   last_error: None,
   updated_at_ms: now_millis(),
  }
 }
}

#[derive(Debug)]
struct RuntimeShared {
 desired_input_source: InputSource,
 desired_output_backend: OutputBackendKind,
 desired_output_enabled: bool,
 desired_paused: bool,
 request_recalibration: bool,
 snapshot: RuntimeSnapshot,
}

#[derive(Clone)]
struct RuntimeState {
 shared: Arc<Mutex<RuntimeShared>>,
}

impl RuntimeState {
 fn new(config: &AppConfig) -> Self {
  Self {
   shared: Arc::new(Mutex::new(RuntimeShared {
    desired_input_source: config.input.source,
    desired_output_backend: config.output.backend,
    desired_output_enabled: config.output.enabled,
    desired_paused: false,
    request_recalibration: false,
    snapshot: RuntimeSnapshot::from_config(config),
   })),
  }
 }
}

#[derive(Debug, Clone, Copy)]
struct RuntimeDesired {
 input_source: InputSource,
 output_backend: OutputBackendKind,
 output_enabled: bool,
 paused: bool,
 recalibration_requested: bool,
}

#[tauri::command]
fn get_runtime_snapshot(state: State<RuntimeState>) -> RuntimeSnapshot {
 let guard = state.shared.lock().expect("runtime state lock");
 guard.snapshot.clone()
}

#[tauri::command]
fn set_output_enabled(enabled: bool, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_output_enabled = enabled;
 guard.snapshot.output_enabled = enabled;
 guard.snapshot.updated_at_ms = now_millis();
}

#[tauri::command]
fn set_paused(paused: bool, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_paused = paused;
 guard.snapshot.paused = paused;
 guard.snapshot.updated_at_ms = now_millis();
}

#[tauri::command]
fn set_input_source(source: InputSource, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_input_source = source;
 guard.snapshot.input_source = source;
 guard.snapshot.updated_at_ms = now_millis();
}

#[tauri::command]
fn set_output_backend(backend: OutputBackendKind, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_output_backend = backend;
 guard.snapshot.output_backend = backend;
 guard.snapshot.updated_at_ms = now_millis();
}

#[tauri::command]
fn request_recalibration(state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.request_recalibration = true;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
 let config = load_config_or_default();
 let runtime_state = RuntimeState::new(&config);
 spawn_runtime_loop(runtime_state.shared.clone(), config);

 tauri::Builder::default()
  .manage(runtime_state)
  .invoke_handler(tauri::generate_handler![
   get_runtime_snapshot,
   set_output_enabled,
   set_paused,
   set_input_source,
   set_output_backend,
   request_recalibration,
  ])
  .setup(|app| {
   if cfg!(debug_assertions) {
    app.handle().plugin(
     tauri_plugin_log::Builder::default()
      .level(log::LevelFilter::Info)
      .build(),
    )?;
   }
   Ok(())
  })
  .run(tauri::generate_context!())
  .expect("error while running tauri application");
}

fn load_config_or_default() -> AppConfig {
 let local_path = PathBuf::from("config/unvet.toml");
 let workspace_path = PathBuf::from("../../config/unvet.toml");

 if let Ok(config) = AppConfig::load_or_default(&local_path) {
  return config;
 }

 if let Ok(config) = AppConfig::load_or_default(&workspace_path) {
  return config;
 }

 AppConfig::default()
}

fn spawn_runtime_loop(shared: Arc<Mutex<RuntimeShared>>, config: AppConfig) {
 thread::spawn(move || {
  let active_mapping = config.effective_mapping();
  let mut frame_smoother = OutputFrameSmoother::new(active_mapping.smoothing_alpha);
  let mut calibration = build_calibration(&config);

  let mut receiver_options = ReceiverOptions::default();
  receiver_options.host = config.input.host.clone();
  receiver_options.udp_port = config.input.udp_port;
  receiver_options.tcp_port = config.input.tcp_port;
  receiver_options.use_tcp = matches!(config.input.source, InputSource::IfacialmocapTcp);

  let mut receiver = IfacialMocapReceiver::new(receiver_options);
  let mut output_layer = OutputBackendLayer::new(config.output.backend);
  let mut active_input_source = config.input.source;
  let mut active_backend = config.output.backend;
  let mut last_reconnect_attempt_at = Instant::now();
  let mut last_frame_at = Instant::now();
  let mut forced_idle_output = false;

  if let Err(error) = output_layer.set_enabled(config.output.enabled) {
   set_snapshot_error(&shared, format!("output init failed: {error}"));
  }

  if let Err(error) = receiver.connect() {
   set_snapshot_error(&shared, format!("input connect failed: {error}"));
  } else {
   clear_snapshot_error(&shared);
  }

  loop {
   let desired = consume_desired(&shared);

   if desired.input_source != active_input_source {
    receiver.disconnect();
    let mut options = receiver.options().clone();
    options.use_tcp = matches!(desired.input_source, InputSource::IfacialmocapTcp);
    receiver = IfacialMocapReceiver::new(options);
    active_input_source = desired.input_source;
    if let Err(error) = receiver.connect() {
     set_snapshot_error(&shared, format!("input reconnect failed: {error}"));
    } else {
     clear_snapshot_error(&shared);
    }
    last_reconnect_attempt_at = Instant::now();
   }

   if desired.output_backend != active_backend {
    match output_layer.set_active_backend(desired.output_backend) {
      Ok(()) => {
       active_backend = desired.output_backend;
       clear_snapshot_error(&shared);
      },
     Err(error) => set_snapshot_error(&shared, format!("backend switch failed: {error}")),
    }
   }

   if let Err(error) = output_layer.set_enabled(desired.output_enabled && !desired.paused) {
    set_snapshot_error(&shared, format!("output enable failed: {error}"));
   }

   if !receiver.is_active() && last_reconnect_attempt_at.elapsed() >= RECONNECT_INTERVAL {
    if let Err(error) = receiver.connect() {
     set_snapshot_error(&shared, format!("input reconnect failed: {error}"));
    } else {
     clear_snapshot_error(&shared);
    }
    last_reconnect_attempt_at = Instant::now();
   }

   if desired.paused {
    if !forced_idle_output {
     let _ = output_layer.apply(OutputFrame::default());
     forced_idle_output = true;
    }
    refresh_snapshot_metadata(
     &shared,
     receiver.is_active(),
     desired.output_enabled,
     desired.paused,
     active_input_source,
     active_backend,
    );
    thread::sleep(POLL_INTERVAL);
    continue;
   }

   if let Some(frame) = receiver.poll_frame() {
    if desired.recalibration_requested {
     calibration.calibrate_from_frame(frame);
    }

    let calibrated_frame = calibration.apply(frame);
    let output_frame = build_output_frame(calibrated_frame, &active_mapping);
    let smoothed_output = frame_smoother.update(output_frame);

    if let Err(error) = output_layer.apply(smoothed_output) {
      set_snapshot_error(&shared, format!("output apply failed: {error}"));
    } else {
     clear_snapshot_error(&shared);
    }

    last_frame_at = Instant::now();
    forced_idle_output = false;
    refresh_snapshot_from_frame(
     &shared,
     smoothed_output,
     receiver.is_active(),
     desired.output_enabled,
     desired.paused,
     active_input_source,
     active_backend,
    );
   } else {
    if !forced_idle_output && last_frame_at.elapsed() >= IDLE_TIMEOUT {
     let _ = output_layer.apply(OutputFrame::default());
     forced_idle_output = true;
     refresh_snapshot_from_frame(
      &shared,
      OutputFrame::default(),
      receiver.is_active(),
      desired.output_enabled,
      desired.paused,
      active_input_source,
      active_backend,
     );
    } else {
     refresh_snapshot_metadata(
      &shared,
      receiver.is_active(),
      desired.output_enabled,
      desired.paused,
      active_input_source,
      active_backend,
     );
    }
   }

   thread::sleep(POLL_INTERVAL);
  }
 });
}

fn consume_desired(shared: &Arc<Mutex<RuntimeShared>>) -> RuntimeDesired {
 let mut guard = shared.lock().expect("runtime state lock");
 let recalibration_requested = guard.request_recalibration;
 guard.request_recalibration = false;

 RuntimeDesired {
  input_source: guard.desired_input_source,
  output_backend: guard.desired_output_backend,
  output_enabled: guard.desired_output_enabled,
  paused: guard.desired_paused,
  recalibration_requested,
 }
}

fn refresh_snapshot_from_frame(
 shared: &Arc<Mutex<RuntimeShared>>,
 frame: OutputFrame,
 input_connected: bool,
 output_enabled: bool,
 paused: bool,
 input_source: InputSource,
 output_backend: OutputBackendKind,
) {
 let mut guard = shared.lock().expect("runtime state lock");
 guard.snapshot.look_yaw_norm = frame.look_yaw_norm;
 guard.snapshot.look_pitch_norm = frame.look_pitch_norm;
 guard.snapshot.confidence = frame.confidence;
 guard.snapshot.active = frame.active;
 guard.snapshot.input_connected = input_connected;
 guard.snapshot.output_enabled = output_enabled;
 guard.snapshot.paused = paused;
 guard.snapshot.input_source = input_source;
 guard.snapshot.output_backend = output_backend;
 guard.snapshot.updated_at_ms = now_millis();
}

fn refresh_snapshot_metadata(
 shared: &Arc<Mutex<RuntimeShared>>,
 input_connected: bool,
 output_enabled: bool,
 paused: bool,
 input_source: InputSource,
 output_backend: OutputBackendKind,
) {
 let mut guard = shared.lock().expect("runtime state lock");
 guard.snapshot.input_connected = input_connected;
 guard.snapshot.output_enabled = output_enabled;
 guard.snapshot.paused = paused;
 guard.snapshot.input_source = input_source;
 guard.snapshot.output_backend = output_backend;
 guard.snapshot.updated_at_ms = now_millis();
}

fn set_snapshot_error(shared: &Arc<Mutex<RuntimeShared>>, message: String) {
 let mut guard = shared.lock().expect("runtime state lock");
 guard.snapshot.last_error = Some(message);
 guard.snapshot.updated_at_ms = now_millis();
}

fn clear_snapshot_error(shared: &Arc<Mutex<RuntimeShared>>) {
 let mut guard = shared.lock().expect("runtime state lock");
 guard.snapshot.last_error = None;
 guard.snapshot.updated_at_ms = now_millis();
}

fn now_millis() -> u64 {
 SystemTime::now()
  .duration_since(UNIX_EPOCH)
  .map(|duration| duration.as_millis() as u64)
  .unwrap_or(0)
}

fn build_calibration(config: &AppConfig) -> NeutralPoseCalibration {
 match config.calibration.offsets() {
  Some(offsets) => NeutralPoseCalibration::from_offsets(config.calibration.enabled, offsets),
  None => NeutralPoseCalibration::new(config.calibration.enabled),
 }
}

fn build_output_frame(frame: TrackingFrame, mapping: &MappingConfig) -> OutputFrame {
 let blend_preset = match mapping.head_eye_blend_preset {
  MappingBlendPreset::Custom => HeadEyeBlendPreset::Custom,
  MappingBlendPreset::Balanced => HeadEyeBlendPreset::Balanced,
  MappingBlendPreset::EyeDominant => HeadEyeBlendPreset::EyeDominant,
  MappingBlendPreset::HeadDominant => HeadEyeBlendPreset::HeadDominant,
 };
 let (yaw_mix, pitch_mix) = resolve_head_eye_mix(
  blend_preset,
  mapping.eye_head_mix_yaw,
  mapping.eye_head_mix_pitch,
 );

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
