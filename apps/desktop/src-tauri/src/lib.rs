use std::{
 collections::HashSet,
 path::PathBuf,
 sync::{Arc, Mutex},
 thread,
 time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "windows")]
use std::process::{Child, Command, Stdio};

use serde::Serialize;
use tauri::State;
use unvet_config::{
 AppConfig, ClutchHotkeyMode, InputConfig, InputSource, MappingBlendPreset, MappingConfig, MappingCurvePreset, OutputBackendKind, OutputSendFilterConfig,
 OutputSendFilterMode, VmcOscPassthroughConfig, VmcOscPassthroughMode,
};
use unvet_core::{
 calibration::NeutralPoseCalibration,
 filter::{OutputFrameSmoother, TrackingFrameStabilizer},
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
use unvet_output::{list_running_process_names, OutputBackendLayer, SendFilterStatus};

#[cfg(target_os = "windows")]
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const POLL_INTERVAL: Duration = Duration::from_millis(8);
const IDLE_TIMEOUT: Duration = Duration::from_millis(250);
const INPUT_LIVE_TIMEOUT: Duration = Duration::from_secs(1);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(1);
const AXIS_MULTIPLIER_MIN: f32 = 0.1;
const AXIS_MULTIPLIER_MAX: f32 = 9.0;
const OUTPUT_EASING_ALPHA_MIN: f32 = 0.01;
const OUTPUT_EASING_ALPHA_MAX: f32 = 1.0;

#[cfg(target_os = "windows")]
const TRACKIR_DUMMY_PROCESS_NAME: &str = "TrackIR.exe";
#[cfg(target_os = "windows")]
const TRACKIR_INSTALLER_REG_VALUE: &str = "Path";

enum RuntimeInputReceiver {
 IfacialMocap(IfacialMocapReceiver),
 VmcOsc(VmcOscReceiver),
}

impl RuntimeInputReceiver {
 fn from_input_config(config: &InputConfig, passthrough: &VmcOscPassthroughConfig, source: InputSource) -> Self {
  match source {
   InputSource::IfacialmocapUdp | InputSource::IfacialmocapTcp => {
    let mut options = ReceiverOptions::default();
    options.host = config.host.clone();
    options.udp_port = config.udp_port;
    options.tcp_port = config.tcp_port;
    options.use_tcp = matches!(source, InputSource::IfacialmocapTcp);
    Self::IfacialMocap(IfacialMocapReceiver::new(options))
   },
   InputSource::VmcOsc => {
    let mut options = VmcOscReceiverOptions::default();
    options.udp_port = config.vmc_osc_port;
    options.passthrough = to_vmc_receiver_passthrough_options(passthrough);
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

 fn disconnect(&mut self) {
  match self {
   Self::IfacialMocap(receiver) => receiver.disconnect(),
   Self::VmcOsc(receiver) => receiver.disconnect(),
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

 fn idle_diagnostic_message(&self) -> Option<String> {
  match self {
   Self::IfacialMocap(receiver) => {
    let stats = receiver.stats();
    if stats.frames_parsed > 0 {
     return None;
    }

    if stats.frames_dropped > 0 {
     let parse_error = stats.last_error.clone().unwrap_or_else(|| "unknown parsing error".to_owned());
     return Some(format!("tracking packets could not be parsed: {parse_error}"));
    }

    let options = receiver.options();
    let loopback_hint = if options.host.eq_ignore_ascii_case("127.0.0.1") || options.host.eq_ignore_ascii_case("localhost") {
     "; current input.host is loopback, so set it to your iFacialMocap device IP"
    } else {
     ""
    };

    Some(format!(
     "no tracking packets received from {}:{} yet; verify iFacialMocap target IP/port and config input.host{}",
     options.host, options.udp_port, loopback_hint
    ))
   },
   Self::VmcOsc(receiver) => {
    let stats = receiver.stats();
    if stats.frames_emitted > 0 {
     return None;
    }

    if stats.udp_packets_received == 0 {
     return Some(format!(
      "no VMC/OSC packets received yet on UDP port {}; verify sender target host/port",
      receiver.options().udp_port
     ));
    }

    if let Some(error) = stats.last_error.clone() {
     return Some(format!("VMC/OSC packets were received but ignored: {error}"));
    }

    Some(format!(
     "VMC/OSC packets were received on UDP port {} but no usable head pose was found; verify /VMC/Ext/Bone/Pos for Head",
     receiver.options().udp_port
    ))
   },
  }
 }
}

fn to_vmc_receiver_passthrough_options(config: &VmcOscPassthroughConfig) -> VmcReceiverPassthroughOptions {
 VmcReceiverPassthroughOptions {
  enabled: config.enabled,
  targets: config.targets.clone(),
  mode: match config.mode {
   VmcOscPassthroughMode::RawUdpForward => VmcReceiverPassthroughMode::RawUdpForward,
  },
 }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeSnapshot {
 input_connected: bool,
 output_enabled: bool,
 output_clutch_engaged: bool,
 output_clutch_hotkey: String,
 output_clutch_hotkey_mode: ClutchHotkeyMode,
 persist_session_settings: bool,
 paused: bool,
 input_source: InputSource,
 vmc_osc_port: u16,
 vmc_osc_passthrough_enabled: bool,
 vmc_osc_passthrough_mode: VmcOscPassthroughMode,
 vmc_osc_passthrough_targets: Vec<String>,
 output_backend: OutputBackendKind,
 output_send_filter_mode: OutputSendFilterMode,
 output_send_filter_process_names: Vec<String>,
 output_send_filter_allowed: bool,
 output_send_filter_active_process: Option<String>,
 yaw_output_multiplier: f32,
 pitch_output_multiplier: f32,
 invert_output_yaw: bool,
 invert_output_pitch: bool,
 spike_rejection_enabled: bool,
 output_easing_enabled: bool,
 output_easing_alpha: f32,
 look_yaw_norm: f32,
 look_pitch_norm: f32,
 confidence: f32,
 active: bool,
 last_error: Option<String>,
 updated_at_ms: u64,
}

impl RuntimeSnapshot {
 fn from_config(config: &AppConfig) -> Self {
  let restore = config.runtime.persist_session_settings;
  Self {
   input_connected: false,
   output_enabled: if restore { config.output.enabled } else { true },
   output_clutch_engaged: true,
   output_clutch_hotkey: config.runtime.hotkey_toggle.clone(),
   output_clutch_hotkey_mode: config.runtime.clutch_hotkey_mode,
   persist_session_settings: restore,
   paused: false,
   input_source: if restore { config.input.source } else { InputSource::default() },
   vmc_osc_port: config.input.vmc_osc_port,
   vmc_osc_passthrough_enabled: if restore {
    config.vmc_osc_passthrough.enabled
   } else {
    VmcOscPassthroughConfig::default().enabled
   },
   vmc_osc_passthrough_mode: config.vmc_osc_passthrough.mode,
   vmc_osc_passthrough_targets: if restore {
    config.vmc_osc_passthrough.targets.clone()
   } else {
    VmcOscPassthroughConfig::default().targets
   },
   output_backend: if restore {
    config.output.backend
   } else {
    OutputBackendKind::default()
   },
   output_send_filter_mode: if restore {
    config.output.send_filter.mode
   } else {
    OutputSendFilterMode::default()
   },
   output_send_filter_process_names: if restore {
    config.output.send_filter.process_names.clone()
   } else {
    Vec::new()
   },
   output_send_filter_allowed: if restore {
    matches!(config.output.send_filter.mode, OutputSendFilterMode::Unrestricted)
   } else {
    true
   },
   output_send_filter_active_process: None,
   yaw_output_multiplier: if restore {
    config.mapping.yaw_output_multiplier.clamp(AXIS_MULTIPLIER_MIN, AXIS_MULTIPLIER_MAX)
   } else {
    1.0
   },
   pitch_output_multiplier: if restore {
    config
     .mapping
     .pitch_output_multiplier
     .clamp(AXIS_MULTIPLIER_MIN, AXIS_MULTIPLIER_MAX)
   } else {
    1.0
   },
   spike_rejection_enabled: if restore { config.input_filter.spike_rejection_enabled } else { false },
   invert_output_yaw: if restore { config.mapping.invert_output_yaw } else { false },
   invert_output_pitch: if restore { config.mapping.invert_output_pitch } else { false },
   output_easing_enabled: if restore { config.mapping.output_easing_enabled } else { true },
   output_easing_alpha: if restore {
    config
     .mapping
     .smoothing_alpha
     .clamp(OUTPUT_EASING_ALPHA_MIN, OUTPUT_EASING_ALPHA_MAX)
   } else {
    0.18
   },
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
 config_path: PathBuf,
 desired_input_source: InputSource,
 desired_vmc_osc_port: u16,
 desired_vmc_osc_passthrough_enabled: bool,
 desired_vmc_osc_passthrough_mode: VmcOscPassthroughMode,
 desired_vmc_osc_passthrough_targets: Vec<String>,
 desired_output_backend: OutputBackendKind,
 desired_output_send_filter: OutputSendFilterConfig,
 desired_output_enabled: bool,
 desired_output_clutch_engaged: bool,
 desired_yaw_output_multiplier: f32,
 desired_pitch_output_multiplier: f32,
 desired_invert_output_yaw: bool,
 desired_invert_output_pitch: bool,
 desired_spike_rejection_enabled: bool,
 desired_output_easing_enabled: bool,
 desired_output_easing_alpha: f32,
 desired_persist_session_settings: bool,
 desired_paused: bool,
 request_recalibration: bool,
 snapshot: RuntimeSnapshot,
}

#[derive(Clone)]
struct RuntimeState {
 shared: Arc<Mutex<RuntimeShared>>,
}

impl RuntimeState {
 fn new(config: &AppConfig, config_path: PathBuf) -> Self {
  let snapshot = RuntimeSnapshot::from_config(config);
  Self {
   shared: Arc::new(Mutex::new(RuntimeShared {
    config_path,
    desired_input_source: snapshot.input_source,
    desired_vmc_osc_port: snapshot.vmc_osc_port,
    desired_vmc_osc_passthrough_enabled: snapshot.vmc_osc_passthrough_enabled,
    desired_vmc_osc_passthrough_mode: snapshot.vmc_osc_passthrough_mode,
    desired_vmc_osc_passthrough_targets: snapshot.vmc_osc_passthrough_targets.clone(),
    desired_output_backend: snapshot.output_backend,
    desired_output_send_filter: OutputSendFilterConfig {
     mode: snapshot.output_send_filter_mode,
     process_names: snapshot.output_send_filter_process_names.clone(),
    },
    desired_output_enabled: snapshot.output_enabled,
    desired_output_clutch_engaged: true,
    desired_yaw_output_multiplier: snapshot.yaw_output_multiplier,
    desired_pitch_output_multiplier: snapshot.pitch_output_multiplier,
    desired_invert_output_yaw: snapshot.invert_output_yaw,
    desired_invert_output_pitch: snapshot.invert_output_pitch,
    desired_spike_rejection_enabled: snapshot.spike_rejection_enabled,
    desired_output_easing_enabled: snapshot.output_easing_enabled,
    desired_output_easing_alpha: snapshot.output_easing_alpha,
    desired_persist_session_settings: snapshot.persist_session_settings,
    desired_paused: false,
    request_recalibration: false,
    snapshot,
   })),
  }
 }
}

#[derive(Debug, Clone)]
struct RuntimeDesired {
 input_source: InputSource,
 vmc_osc_port: u16,
 vmc_osc_passthrough_enabled: bool,
 vmc_osc_passthrough_mode: VmcOscPassthroughMode,
 vmc_osc_passthrough_targets: Vec<String>,
 output_backend: OutputBackendKind,
 output_send_filter: OutputSendFilterConfig,
 output_enabled: bool,
 output_clutch_engaged: bool,
 yaw_output_multiplier: f32,
 pitch_output_multiplier: f32,
 invert_output_yaw: bool,
 invert_output_pitch: bool,
 spike_rejection_enabled: bool,
 output_easing_enabled: bool,
 output_easing_alpha: f32,
 paused: bool,
 recalibration_requested: bool,
}

#[derive(Debug, Clone)]
struct OutputFilterSnapshot {
 mode: OutputSendFilterMode,
 process_names: Vec<String>,
 allowed: bool,
 active_process_name: Option<String>,
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
 drop(guard);
 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn set_output_clutch(engaged: bool, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_output_clutch_engaged = engaged;
 guard.snapshot.output_clutch_engaged = engaged;
 guard.snapshot.updated_at_ms = now_millis();
}

#[tauri::command]
fn set_output_clutch_hotkey(hotkey: String, state: State<RuntimeState>) -> Result<(), String> {
 let normalized = hotkey.trim();
 if normalized.is_empty() {
  return Err("hotkey must not be empty".to_owned());
 }

 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.snapshot.output_clutch_hotkey = normalized.to_owned();
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);
 persist_runtime_preferences_or_set_error(state.inner());
 Ok(())
}

#[tauri::command]
fn set_output_clutch_hotkey_mode(mode: ClutchHotkeyMode, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.snapshot.output_clutch_hotkey_mode = mode;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);
 persist_runtime_preferences_or_set_error(state.inner());
}

#[tauri::command]
fn set_persist_session_settings(enabled: bool, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_persist_session_settings = enabled;
 guard.snapshot.persist_session_settings = enabled;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);

 persist_runtime_preferences_or_set_error(state.inner());
 if enabled {
  persist_session_settings_if_enabled_or_set_error(state.inner());
 }
}

#[tauri::command]
fn set_output_axis_multipliers(yaw_output_multiplier: f32, pitch_output_multiplier: f32, state: State<RuntimeState>) {
 let clamped_yaw = yaw_output_multiplier.clamp(AXIS_MULTIPLIER_MIN, AXIS_MULTIPLIER_MAX);
 let clamped_pitch = pitch_output_multiplier.clamp(AXIS_MULTIPLIER_MIN, AXIS_MULTIPLIER_MAX);

 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_yaw_output_multiplier = clamped_yaw;
 guard.desired_pitch_output_multiplier = clamped_pitch;
 guard.snapshot.yaw_output_multiplier = clamped_yaw;
 guard.snapshot.pitch_output_multiplier = clamped_pitch;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);

 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn set_output_axis_inversion(invert_yaw: bool, invert_pitch: bool, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_invert_output_yaw = invert_yaw;
 guard.desired_invert_output_pitch = invert_pitch;
 guard.snapshot.invert_output_yaw = invert_yaw;
 guard.snapshot.invert_output_pitch = invert_pitch;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);

 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn set_output_easing(enabled: bool, alpha: f32, state: State<RuntimeState>) {
 let clamped_alpha = alpha.clamp(OUTPUT_EASING_ALPHA_MIN, OUTPUT_EASING_ALPHA_MAX);

 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_output_easing_enabled = enabled;
 guard.desired_output_easing_alpha = clamped_alpha;
 guard.snapshot.output_easing_enabled = enabled;
 guard.snapshot.output_easing_alpha = clamped_alpha;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);

 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn set_spike_rejection_enabled(enabled: bool, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_spike_rejection_enabled = enabled;
 guard.snapshot.spike_rejection_enabled = enabled;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);

 persist_session_settings_if_enabled_or_set_error(state.inner());
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
 drop(guard);
 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn set_vmc_osc_port(port: u16, state: State<RuntimeState>) -> Result<(), String> {
 if port == 0 {
  return Err("port must be in range 1-65535".to_owned());
 }

 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_vmc_osc_port = port;
 guard.snapshot.vmc_osc_port = port;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);
 persist_session_settings_if_enabled_or_set_error(state.inner());
 Ok(())
}

#[tauri::command]
fn set_vmc_osc_passthrough_enabled(enabled: bool, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_vmc_osc_passthrough_enabled = enabled;
 guard.snapshot.vmc_osc_passthrough_enabled = enabled;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);
 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn set_vmc_osc_passthrough_mode(mode: VmcOscPassthroughMode, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_vmc_osc_passthrough_mode = mode;
 guard.snapshot.vmc_osc_passthrough_mode = mode;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);
 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn set_vmc_osc_passthrough_targets(targets: Vec<String>, state: State<RuntimeState>) -> Result<(), String> {
 let normalized_targets = sanitize_passthrough_targets(targets)?;

 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_vmc_osc_passthrough_targets = normalized_targets.clone();
 guard.snapshot.vmc_osc_passthrough_targets = normalized_targets;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);
 persist_session_settings_if_enabled_or_set_error(state.inner());
 Ok(())
}

#[tauri::command]
fn set_output_backend(backend: OutputBackendKind, state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_output_backend = backend;
 guard.snapshot.output_backend = backend;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);
 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn set_output_send_filter(mode: OutputSendFilterMode, process_names: Vec<String>, state: State<RuntimeState>) {
 let normalized_process_names = sanitize_process_names(process_names);
 let send_filter = OutputSendFilterConfig {
  mode,
  process_names: normalized_process_names.clone(),
 };

 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.desired_output_send_filter = send_filter;
 guard.snapshot.output_send_filter_mode = mode;
 guard.snapshot.output_send_filter_process_names = normalized_process_names;
 guard.snapshot.output_send_filter_allowed = matches!(mode, OutputSendFilterMode::Unrestricted);
 guard.snapshot.output_send_filter_active_process = None;
 guard.snapshot.updated_at_ms = now_millis();
 drop(guard);
 persist_session_settings_if_enabled_or_set_error(state.inner());
}

#[tauri::command]
fn list_running_processes() -> Result<Vec<String>, String> {
 list_running_process_names().map_err(|error| format!("failed to query process list: {error}"))
}

#[tauri::command]
fn request_recalibration(state: State<RuntimeState>) {
 let mut guard = state.shared.lock().expect("runtime state lock");
 guard.request_recalibration = true;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
 let (config, config_path) = load_config_or_default();
 let runtime_state = RuntimeState::new(&config, config_path);

 #[cfg(target_os = "windows")]
 let _trackir_dummy_process = {
  if let Err(error) = configure_trackir_compatibility_registry() {
   log::warn!("TrackIR registry setup failed: {error}");
  }

  match start_trackir_dummy_process() {
   Ok(process) => Some(process),
   Err(error) => {
    log::warn!("TrackIR dummy process startup failed: {error}");
    None
   },
  }
 };

 spawn_runtime_loop(runtime_state.shared.clone(), config);

 tauri::Builder::default()
  .manage(runtime_state)
  .invoke_handler(tauri::generate_handler![
   get_runtime_snapshot,
   set_output_enabled,
   set_output_clutch,
   set_output_clutch_hotkey,
   set_output_clutch_hotkey_mode,
   set_persist_session_settings,
   set_output_axis_multipliers,
   set_output_axis_inversion,
   set_output_easing,
   set_paused,
   set_input_source,
   set_vmc_osc_port,
   set_vmc_osc_passthrough_enabled,
   set_vmc_osc_passthrough_mode,
   set_vmc_osc_passthrough_targets,
   set_output_backend,
   set_output_send_filter,
   list_running_processes,
   request_recalibration,
   set_spike_rejection_enabled,
  ])
  .setup(|app| {
   if cfg!(debug_assertions) {
    app
     .handle()
     .plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
   }
   Ok(())
  })
  .run(tauri::generate_context!())
  .expect("error while running tauri application");
}

#[cfg(target_os = "windows")]
struct TrackIrDummyProcess(Child);

#[cfg(target_os = "windows")]
impl Drop for TrackIrDummyProcess {
 fn drop(&mut self) {
  let _ = self.0.kill();
  let _ = self.0.wait();
 }
}

#[cfg(target_os = "windows")]
fn configure_trackir_compatibility_registry() -> Result<(), String> {
 let binary_dir = std::env::current_exe()
  .map_err(|error| format!("failed to resolve current executable path: {error}"))?
  .parent()
  .ok_or_else(|| "failed to resolve executable directory".to_owned())?
  .to_path_buf();

 let npclient64_path = binary_dir.join("NPClient64.dll");
 if !npclient64_path.exists() {
  return Err(format!("NPClient64.dll not found at {}", npclient64_path.display()));
 }

 let npclient_path = binary_dir.join("NPClient.dll");
 if !npclient_path.exists() {
  std::fs::copy(&npclient64_path, &npclient_path).map_err(|error| {
   format!(
    "failed to materialize NPClient.dll from NPClient64.dll ({} -> {}): {error}",
    npclient64_path.display(),
    npclient_path.display()
   )
  })?;
 }

 let mut registry_dir_value = binary_dir.to_string_lossy().to_string();
 if !registry_dir_value.ends_with('\\') {
  registry_dir_value.push('\\');
 }
 let hkcu = RegKey::predef(HKEY_CURRENT_USER);

 write_trackir_registry_path(&hkcu, r"Software\NaturalPoint\NATURALPOINT\NPClient Location", &registry_dir_value)?;
 write_trackir_registry_path(
  &hkcu,
  r"Software\NaturalPoint\NATURALPOINT\NPClient64 Location",
  &registry_dir_value,
 )?;
 write_trackir_registry_path(&hkcu, r"Software\NaturalPoint\NaturalPoint\NPClient Location", &registry_dir_value)?;
 write_trackir_registry_path(
  &hkcu,
  r"Software\NaturalPoint\NaturalPoint\NPClient64 Location",
  &registry_dir_value,
 )?;
 write_trackir_registry_path(&hkcu, r"Software\Freetrack\FreeTrackClient", &registry_dir_value)?;
 write_trackir_registry_path(&hkcu, r"Software\Freetrack\FreetrackClient", &registry_dir_value)?;

 Ok(())
}

#[cfg(target_os = "windows")]
fn write_trackir_registry_path(hkcu: &RegKey, key_path: &str, value: &str) -> Result<(), String> {
 let (key, _) = hkcu
  .create_subkey(key_path)
  .map_err(|error| format!("failed to create registry key {key_path}: {error}"))?;
 key
  .set_value("", &value)
  .map_err(|error| format!("failed to write default value for {key_path}: {error}"))?;
 key
  .set_value(TRACKIR_INSTALLER_REG_VALUE, &value)
  .map_err(|error| format!("failed to write Path value for {key_path}: {error}"))?;

 let path_subkey = format!(r"{key_path}\{TRACKIR_INSTALLER_REG_VALUE}");
 let (path_key, _) = hkcu
  .create_subkey(&path_subkey)
  .map_err(|error| format!("failed to create registry key {path_subkey}: {error}"))?;
 path_key
  .set_value("", &value)
  .map_err(|error| format!("failed to write default value for {path_subkey}: {error}"))?;

 Ok(())
}

#[cfg(target_os = "windows")]
fn start_trackir_dummy_process() -> Result<TrackIrDummyProcess, String> {
 let binary_dir = std::env::current_exe()
  .map_err(|error| format!("failed to resolve current executable path: {error}"))?
  .parent()
  .ok_or_else(|| "failed to resolve executable directory".to_owned())?
  .to_path_buf();

 let trackir_dummy_path = binary_dir.join(TRACKIR_DUMMY_PROCESS_NAME);
 if !trackir_dummy_path.exists() {
  return Err(format!("TrackIR dummy process not found at {}", trackir_dummy_path.display()));
 }

 let child = Command::new(&trackir_dummy_path)
  .stdin(Stdio::null())
  .stdout(Stdio::null())
  .stderr(Stdio::null())
  .spawn()
  .map_err(|error| format!("failed to spawn TrackIR dummy process {}: {error}", trackir_dummy_path.display()))?;

 Ok(TrackIrDummyProcess(child))
}

fn load_config_or_default() -> (AppConfig, PathBuf) {
 let local_path = PathBuf::from("config/unvet.toml");
 let workspace_path = PathBuf::from("../../config/unvet.toml");

 if local_path.exists() {
  if let Ok(config) = AppConfig::load_or_default(&local_path) {
   return (config, local_path);
  }
 }

 if workspace_path.exists() {
  if let Ok(config) = AppConfig::load_or_default(&workspace_path) {
   return (config, workspace_path);
  }
 }

 (AppConfig::default(), local_path)
}

fn spawn_runtime_loop(shared: Arc<Mutex<RuntimeShared>>, config: AppConfig) {
 thread::spawn(move || {
  let mut active_mapping = config.effective_mapping();
  active_mapping.yaw_output_multiplier = active_mapping.yaw_output_multiplier.clamp(AXIS_MULTIPLIER_MIN, AXIS_MULTIPLIER_MAX);
  active_mapping.pitch_output_multiplier = active_mapping
   .pitch_output_multiplier
   .clamp(AXIS_MULTIPLIER_MIN, AXIS_MULTIPLIER_MAX);
  active_mapping.smoothing_alpha = active_mapping
   .smoothing_alpha
   .clamp(OUTPUT_EASING_ALPHA_MIN, OUTPUT_EASING_ALPHA_MAX);
  let mut frame_smoother = OutputFrameSmoother::new(active_mapping.smoothing_alpha);
  let mut frame_stabilizer = TrackingFrameStabilizer::new(config.input_filter.spike_rejection_enabled);
  let mut calibration = build_calibration(&config);

  let mut active_input_config = config.input.clone();
  let mut active_input_passthrough = config.vmc_osc_passthrough.clone();
  let mut receiver = RuntimeInputReceiver::from_input_config(&active_input_config, &active_input_passthrough, active_input_config.source);
  let mut output_layer = OutputBackendLayer::new(&config.output);
  let mut active_input_source = active_input_config.source;
  let mut active_backend = config.output.backend;
  let mut active_send_filter = config.output.send_filter.clone();
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

   if (active_mapping.yaw_output_multiplier - desired.yaw_output_multiplier).abs() > f32::EPSILON {
    active_mapping.yaw_output_multiplier = desired.yaw_output_multiplier;
   }
   if (active_mapping.pitch_output_multiplier - desired.pitch_output_multiplier).abs() > f32::EPSILON {
    active_mapping.pitch_output_multiplier = desired.pitch_output_multiplier;
   }
   if active_mapping.invert_output_yaw != desired.invert_output_yaw {
    active_mapping.invert_output_yaw = desired.invert_output_yaw;
   }
   if active_mapping.invert_output_pitch != desired.invert_output_pitch {
    active_mapping.invert_output_pitch = desired.invert_output_pitch;
   }

   let desired_easing_alpha = desired.output_easing_alpha.clamp(OUTPUT_EASING_ALPHA_MIN, OUTPUT_EASING_ALPHA_MAX);
   if (active_mapping.smoothing_alpha - desired_easing_alpha).abs() > f32::EPSILON {
    active_mapping.smoothing_alpha = desired_easing_alpha;
    frame_smoother.set_alpha(desired_easing_alpha);
   }
   if active_mapping.output_easing_enabled != desired.output_easing_enabled {
    active_mapping.output_easing_enabled = desired.output_easing_enabled;
    if !active_mapping.output_easing_enabled {
     frame_smoother.reset();
    }
   }

   if frame_stabilizer.is_enabled() != desired.spike_rejection_enabled {
    frame_stabilizer.set_enabled(desired.spike_rejection_enabled);
   }

   let mut input_needs_reconnect = false;
   if desired.vmc_osc_port != active_input_config.vmc_osc_port {
    active_input_config.vmc_osc_port = desired.vmc_osc_port;
    if matches!(active_input_source, InputSource::VmcOsc) {
     input_needs_reconnect = true;
    }
   }

   if desired.vmc_osc_passthrough_enabled != active_input_passthrough.enabled {
    active_input_passthrough.enabled = desired.vmc_osc_passthrough_enabled;
    if matches!(active_input_source, InputSource::VmcOsc) {
     input_needs_reconnect = true;
    }
   }

   if desired.vmc_osc_passthrough_mode != active_input_passthrough.mode {
    active_input_passthrough.mode = desired.vmc_osc_passthrough_mode;
    if matches!(active_input_source, InputSource::VmcOsc) {
     input_needs_reconnect = true;
    }
   }

   if desired.vmc_osc_passthrough_targets != active_input_passthrough.targets {
    active_input_passthrough.targets = desired.vmc_osc_passthrough_targets.clone();
    if matches!(active_input_source, InputSource::VmcOsc) {
     input_needs_reconnect = true;
    }
   }

   if desired.input_source != active_input_source {
    active_input_config.source = desired.input_source;
    active_input_source = desired.input_source;
    input_needs_reconnect = true;
   }

   if input_needs_reconnect {
    receiver.disconnect();
    receiver = RuntimeInputReceiver::from_input_config(&active_input_config, &active_input_passthrough, active_input_source);
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

   if desired.output_send_filter != active_send_filter {
    match output_layer.set_send_filter(desired.output_send_filter.clone()) {
     Ok(()) => {
      active_send_filter = desired.output_send_filter.clone();
      clear_snapshot_error(&shared);
     },
     Err(error) => set_snapshot_error(&shared, format!("send filter update failed: {error}")),
    }
   }

   let output_live = desired.output_enabled && !desired.paused && desired.output_clutch_engaged;
   if let Err(error) = output_layer.set_enabled(output_live) {
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

   let output_filter_snapshot = collect_output_filter_snapshot(&output_layer);

   if desired.paused {
    let input_live = receiver.is_active() && last_frame_at.elapsed() < INPUT_LIVE_TIMEOUT;
    if !forced_idle_output {
     if output_live {
      let _ = output_layer.apply(OutputFrame::default());
     }
     forced_idle_output = true;
    }
    refresh_snapshot_metadata(
     &shared,
     input_live,
     desired.output_enabled,
     desired.output_clutch_engaged,
     desired.paused,
     active_input_source,
     active_backend,
     &output_filter_snapshot,
    );
    thread::sleep(POLL_INTERVAL);
    continue;
   }

   if let Some(frame) = receiver.poll_frame() {
    if desired.recalibration_requested {
     calibration.calibrate_from_frame(frame);
    }

    let stable_frame = frame_stabilizer.update(frame);
    let calibrated_frame = calibration.apply(stable_frame);
    let output_frame = build_output_frame(calibrated_frame, &active_mapping, active_input_source);
    let smoothed_output = if active_mapping.output_easing_enabled {
     frame_smoother.update(output_frame)
    } else {
     output_frame
    };

    if output_live {
     if let Err(error) = output_layer.apply(smoothed_output) {
      set_snapshot_error(&shared, format!("output apply failed: {error}"));
     } else {
      clear_snapshot_error(&shared);
     }
    }

    last_frame_at = Instant::now();
    forced_idle_output = false;
    refresh_snapshot_from_frame(
     &shared,
     smoothed_output,
     true,
     desired.output_enabled,
     desired.output_clutch_engaged,
     desired.paused,
     active_input_source,
     active_backend,
     &output_filter_snapshot,
    );
   } else {
    let input_live = receiver.is_active() && last_frame_at.elapsed() < INPUT_LIVE_TIMEOUT;

    if receiver.is_active() && !input_live {
     if let Some(message) = receiver.idle_diagnostic_message() {
      set_snapshot_error(&shared, message);
     }
    }

    let idle_expired = last_frame_at.elapsed() >= IDLE_TIMEOUT;
    let keep_idle_streaming = matches!(active_backend, OutputBackendKind::Ets2);
    if idle_expired && (!forced_idle_output || keep_idle_streaming) {
     if output_live {
      let _ = output_layer.apply(OutputFrame::default());
     }
     forced_idle_output = !keep_idle_streaming;
     refresh_snapshot_from_frame(
      &shared,
      OutputFrame::default(),
      input_live,
      desired.output_enabled,
      desired.output_clutch_engaged,
      desired.paused,
      active_input_source,
      active_backend,
      &output_filter_snapshot,
     );
    } else {
     refresh_snapshot_metadata(
      &shared,
      input_live,
      desired.output_enabled,
      desired.output_clutch_engaged,
      desired.paused,
      active_input_source,
      active_backend,
      &output_filter_snapshot,
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
  vmc_osc_port: guard.desired_vmc_osc_port,
  vmc_osc_passthrough_enabled: guard.desired_vmc_osc_passthrough_enabled,
  vmc_osc_passthrough_mode: guard.desired_vmc_osc_passthrough_mode,
  vmc_osc_passthrough_targets: guard.desired_vmc_osc_passthrough_targets.clone(),
  output_backend: guard.desired_output_backend,
  output_send_filter: guard.desired_output_send_filter.clone(),
  output_enabled: guard.desired_output_enabled,
  output_clutch_engaged: guard.desired_output_clutch_engaged,
  yaw_output_multiplier: guard.desired_yaw_output_multiplier,
  pitch_output_multiplier: guard.desired_pitch_output_multiplier,
  invert_output_yaw: guard.desired_invert_output_yaw,
  invert_output_pitch: guard.desired_invert_output_pitch,
  spike_rejection_enabled: guard.desired_spike_rejection_enabled,
  output_easing_enabled: guard.desired_output_easing_enabled,
  output_easing_alpha: guard.desired_output_easing_alpha,
  paused: guard.desired_paused,
  recalibration_requested,
 }
}

fn refresh_snapshot_from_frame(
 shared: &Arc<Mutex<RuntimeShared>>,
 frame: OutputFrame,
 input_connected: bool,
 output_enabled: bool,
 output_clutch_engaged: bool,
 paused: bool,
 input_source: InputSource,
 output_backend: OutputBackendKind,
 output_filter: &OutputFilterSnapshot,
) {
 let mut guard = shared.lock().expect("runtime state lock");
 guard.snapshot.look_yaw_norm = frame.look_yaw_norm;
 guard.snapshot.look_pitch_norm = frame.look_pitch_norm;
 guard.snapshot.confidence = frame.confidence;
 guard.snapshot.active = frame.active;
 guard.snapshot.input_connected = input_connected;
 guard.snapshot.output_enabled = output_enabled;
 guard.snapshot.output_clutch_engaged = output_clutch_engaged;
 guard.snapshot.paused = paused;
 guard.snapshot.input_source = input_source;
 guard.snapshot.output_backend = output_backend;
 guard.snapshot.output_send_filter_mode = output_filter.mode;
 guard.snapshot.output_send_filter_process_names = output_filter.process_names.clone();
 guard.snapshot.output_send_filter_allowed = output_filter.allowed;
 guard.snapshot.output_send_filter_active_process = output_filter.active_process_name.clone();
 guard.snapshot.updated_at_ms = now_millis();
}

fn refresh_snapshot_metadata(
 shared: &Arc<Mutex<RuntimeShared>>,
 input_connected: bool,
 output_enabled: bool,
 output_clutch_engaged: bool,
 paused: bool,
 input_source: InputSource,
 output_backend: OutputBackendKind,
 output_filter: &OutputFilterSnapshot,
) {
 let mut guard = shared.lock().expect("runtime state lock");
 guard.snapshot.input_connected = input_connected;
 guard.snapshot.output_enabled = output_enabled;
 guard.snapshot.output_clutch_engaged = output_clutch_engaged;
 guard.snapshot.paused = paused;
 guard.snapshot.input_source = input_source;
 guard.snapshot.output_backend = output_backend;
 guard.snapshot.output_send_filter_mode = output_filter.mode;
 guard.snapshot.output_send_filter_process_names = output_filter.process_names.clone();
 guard.snapshot.output_send_filter_allowed = output_filter.allowed;
 guard.snapshot.output_send_filter_active_process = output_filter.active_process_name.clone();
 guard.snapshot.updated_at_ms = now_millis();
}

fn collect_output_filter_snapshot(output_layer: &OutputBackendLayer) -> OutputFilterSnapshot {
 let filter = output_layer.send_filter().clone();
 let status = output_layer.send_filter_status().unwrap_or_else(|_| SendFilterStatus {
  allowed: matches!(filter.mode, OutputSendFilterMode::Unrestricted),
  active_process_name: None,
 });

 OutputFilterSnapshot {
  mode: filter.mode,
  process_names: filter.process_names,
  allowed: status.allowed,
  active_process_name: status.active_process_name,
 }
}

fn persist_runtime_preferences_or_set_error(state: &RuntimeState) {
 if let Err(error) = persist_runtime_preferences(state) {
  set_snapshot_error(&state.shared, format!("settings persistence failed: {error}"));
 }
}

fn persist_session_settings_if_enabled_or_set_error(state: &RuntimeState) {
 if let Err(error) = persist_session_settings_if_enabled(state) {
  set_snapshot_error(&state.shared, format!("settings persistence failed: {error}"));
 }
}

fn persist_runtime_preferences(state: &RuntimeState) -> Result<(), String> {
 let (config_path, persist_session_settings, output_clutch_hotkey, output_clutch_hotkey_mode) = {
  let guard = state.shared.lock().expect("runtime state lock");
  (
   guard.config_path.clone(),
   guard.snapshot.persist_session_settings,
   guard.snapshot.output_clutch_hotkey.clone(),
   guard.snapshot.output_clutch_hotkey_mode,
  )
 };

 let mut config = AppConfig::load_or_default(&config_path).map_err(|error| format!("failed to load config for persistence: {error}"))?;
 config.runtime.persist_session_settings = persist_session_settings;
 config.runtime.hotkey_toggle = output_clutch_hotkey;
 config.runtime.clutch_hotkey_mode = output_clutch_hotkey_mode;
 config
  .save_to_path(&config_path)
  .map_err(|error| format!("failed to save config for persistence: {error}"))
}

fn persist_session_settings_if_enabled(state: &RuntimeState) -> Result<(), String> {
 let (
  config_path,
  persist_session_settings,
  input_source,
  vmc_osc_port,
  vmc_osc_passthrough_enabled,
  vmc_osc_passthrough_mode,
  vmc_osc_passthrough_targets,
  output_backend,
  output_enabled,
  output_send_filter_mode,
  output_send_filter_process_names,
  yaw_output_multiplier,
  pitch_output_multiplier,
  invert_output_yaw,
  invert_output_pitch,
  spike_rejection_enabled,
  output_easing_enabled,
  output_easing_alpha,
 ) = {
  let guard = state.shared.lock().expect("runtime state lock");
  (
   guard.config_path.clone(),
   guard.snapshot.persist_session_settings,
   guard.snapshot.input_source,
   guard.snapshot.vmc_osc_port,
   guard.snapshot.vmc_osc_passthrough_enabled,
   guard.snapshot.vmc_osc_passthrough_mode,
   guard.snapshot.vmc_osc_passthrough_targets.clone(),
   guard.snapshot.output_backend,
   guard.snapshot.output_enabled,
   guard.snapshot.output_send_filter_mode,
   guard.snapshot.output_send_filter_process_names.clone(),
   guard.snapshot.yaw_output_multiplier,
   guard.snapshot.pitch_output_multiplier,
   guard.snapshot.invert_output_yaw,
   guard.snapshot.invert_output_pitch,
   guard.snapshot.spike_rejection_enabled,
   guard.snapshot.output_easing_enabled,
   guard.snapshot.output_easing_alpha,
  )
 };

 if !persist_session_settings {
  return Ok(());
 }

 let mut config = AppConfig::load_or_default(&config_path).map_err(|error| format!("failed to load config for persistence: {error}"))?;
 config.input.source = input_source;
 config.input.vmc_osc_port = vmc_osc_port;
 config.vmc_osc_passthrough.enabled = vmc_osc_passthrough_enabled;
 config.vmc_osc_passthrough.mode = vmc_osc_passthrough_mode;
 config.vmc_osc_passthrough.targets = vmc_osc_passthrough_targets;
 config.output.backend = output_backend;
 config.output.enabled = output_enabled;
 config.output.send_filter = OutputSendFilterConfig {
  mode: output_send_filter_mode,
  process_names: output_send_filter_process_names,
 };
 config.mapping.yaw_output_multiplier = yaw_output_multiplier.clamp(AXIS_MULTIPLIER_MIN, AXIS_MULTIPLIER_MAX);
 config.mapping.pitch_output_multiplier = pitch_output_multiplier.clamp(AXIS_MULTIPLIER_MIN, AXIS_MULTIPLIER_MAX);
 config.mapping.invert_output_yaw = invert_output_yaw;
 config.mapping.invert_output_pitch = invert_output_pitch;
 config.input_filter.spike_rejection_enabled = spike_rejection_enabled;
 config.mapping.output_easing_enabled = output_easing_enabled;
 config.mapping.smoothing_alpha = output_easing_alpha.clamp(OUTPUT_EASING_ALPHA_MIN, OUTPUT_EASING_ALPHA_MAX);
 config
  .save_to_path(&config_path)
  .map_err(|error| format!("failed to save config for persistence: {error}"))
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

fn sanitize_passthrough_targets(targets: Vec<String>) -> Result<Vec<String>, String> {
 let mut normalized = Vec::new();
 let mut seen = HashSet::new();

 for raw in targets {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
   continue;
  }

  let (host, port) = parse_passthrough_target(trimmed)
   .ok_or_else(|| format!("invalid passthrough target `{trimmed}` (expected host:port with port 1-65535)"))?;
  let formatted = format_passthrough_target(&host, port);

  if seen.insert(formatted.to_ascii_lowercase()) {
   normalized.push(formatted);
  }
 }

 Ok(normalized)
}

fn parse_passthrough_target(raw: &str) -> Option<(String, u16)> {
 let text = raw.trim();
 if text.is_empty() {
  return None;
 }

 if let Some(rest) = text.strip_prefix('[') {
  let end = rest.find(']')?;
  let host = rest[..end].trim().to_ascii_lowercase();
  if host.is_empty() {
   return None;
  }

  let port_text = rest[end + 1..].strip_prefix(':')?.trim();
  if port_text.is_empty() || port_text.contains(':') {
   return None;
  }

  let port = port_text.parse::<u16>().ok()?;
  if port == 0 {
   return None;
  }

  return Some((host, port));
 }

 let mut parts = text.rsplitn(2, ':');
 let port_text = parts.next()?.trim();
 let host = parts.next()?.trim().to_ascii_lowercase();
 if host.is_empty() || host.contains(':') {
  return None;
 }

 let port = port_text.parse::<u16>().ok()?;
 if port == 0 {
  return None;
 }

 Some((host, port))
}

fn format_passthrough_target(host: &str, port: u16) -> String {
 if host.contains(':') {
  format!("[{host}]:{port}")
 } else {
  format!("{host}:{port}")
 }
}

fn sanitize_process_names(process_names: Vec<String>) -> Vec<String> {
 let mut normalized = process_names
  .iter()
  .filter_map(|name| normalize_process_name(name))
  .collect::<Vec<_>>();
 normalized.sort();
 normalized.dedup();
 normalized
}

fn normalize_process_name(name: &str) -> Option<String> {
 let trimmed = name.trim();
 if trimmed.is_empty() {
  return None;
 }

 let normalized_path = trimmed.replace('/', "\\");
 let file_name = normalized_path.rsplit('\\').next().unwrap_or(trimmed).trim();
 if file_name.is_empty() {
  return None;
 }

 Some(file_name.to_ascii_lowercase())
}

fn build_calibration(config: &AppConfig) -> NeutralPoseCalibration {
 match config.calibration.offsets() {
  Some(offsets) => NeutralPoseCalibration::from_offsets(config.calibration.enabled, offsets),
  None => NeutralPoseCalibration::new(config.calibration.enabled),
 }
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

 OutputFrame {
  look_yaw_norm: (map_angle_to_normalized(mixed_yaw, yaw_settings)
   * mapping.yaw_output_multiplier
   * if mapping.invert_output_yaw { -1.0 } else { 1.0 })
  .clamp(-1.0, 1.0),
  look_pitch_norm: (map_angle_to_normalized(mixed_pitch, pitch_settings)
   * mapping.pitch_output_multiplier
   * if mapping.invert_output_pitch { -1.0 } else { 1.0 })
  .clamp(-1.0, 1.0),
  confidence: frame.confidence,
  active: frame.active,
 }
}
