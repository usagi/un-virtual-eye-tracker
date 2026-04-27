import { invoke } from "@tauri-apps/api/core";

export type InputSource = "ifacialmocap_udp" | "ifacialmocap_tcp" | "vmc_osc";
export type OutputBackendKind = "ets2" | "mouse" | "keyboard" | "touch";
export type OutputSendFilterMode = "unrestricted" | "foreground_process";
export type VmcOscPassthroughMode = "raw_udp_forward";
export type ClutchHotkeyMode = "toggle" | "press_on_release_off";

export type RuntimeSnapshot = {
  inputConnected: boolean;
  outputEnabled: boolean;
  outputClutchEngaged: boolean;
  outputClutchHotkey: string;
  outputClutchHotkeyMode: ClutchHotkeyMode;
  persistSessionSettings: boolean;
  paused: boolean;
  inputSource: InputSource;
  vmcOscPort: number;
  vmcOscPassthroughEnabled: boolean;
  vmcOscPassthroughMode: VmcOscPassthroughMode;
  vmcOscPassthroughTargets: string[];
  outputBackend: OutputBackendKind;
  outputSendFilterMode: OutputSendFilterMode;
  outputSendFilterProcessNames: string[];
  outputSendFilterAllowed: boolean;
  outputSendFilterActiveProcess: string | null;
  yawOutputMultiplier: number;
  pitchOutputMultiplier: number;
  invertOutputYaw: boolean;
  invertOutputPitch: boolean;
  spikeRejectionEnabled: boolean;
  outputEasingEnabled: boolean;
  outputEasingAlpha: number;
  lookYawNorm: number;
  lookPitchNorm: number;
  confidence: number;
  active: boolean;
  lastError: string | null;
  updatedAtMs: number;
};

const invokeRuntime = <T>(command: string, args?: Record<string, unknown>) =>
  invoke<T>(command, args);

export const getRuntimeSnapshot = () =>
  invokeRuntime<RuntimeSnapshot>("get_runtime_snapshot");

export const setOutputEnabled = (enabled: boolean) =>
  invokeRuntime<void>("set_output_enabled", { enabled });

export const setOutputClutch = (engaged: boolean) =>
  invokeRuntime<void>("set_output_clutch", { engaged });

export const setOutputClutchHotkey = (hotkey: string) =>
  invokeRuntime<void>("set_output_clutch_hotkey", { hotkey });

export const setOutputClutchHotkeyMode = (mode: ClutchHotkeyMode) =>
  invokeRuntime<void>("set_output_clutch_hotkey_mode", { mode });

export const setPersistSessionSettings = (enabled: boolean) =>
  invokeRuntime<void>("set_persist_session_settings", { enabled });

export const setOutputAxisMultipliers = (
  yawOutputMultiplier: number,
  pitchOutputMultiplier: number,
) =>
  invokeRuntime<void>("set_output_axis_multipliers", {
    yawOutputMultiplier,
    pitchOutputMultiplier,
  });

export const setOutputAxisInversion = (
  invertYaw: boolean,
  invertPitch: boolean,
) =>
  invokeRuntime<void>("set_output_axis_inversion", {
    invertYaw,
    invertPitch,
  });

export const setOutputEasing = (enabled: boolean, alpha: number) =>
  invokeRuntime<void>("set_output_easing", {
    enabled,
    alpha,
  });

export const setPaused = (paused: boolean) =>
  invokeRuntime<void>("set_paused", { paused });

export const setInputSource = (source: InputSource) =>
  invokeRuntime<void>("set_input_source", { source });

export const setVmcOscPort = (port: number) =>
  invokeRuntime<void>("set_vmc_osc_port", { port });

export const setVmcOscPassthroughEnabled = (enabled: boolean) =>
  invokeRuntime<void>("set_vmc_osc_passthrough_enabled", { enabled });

export const setVmcOscPassthroughMode = (mode: VmcOscPassthroughMode) =>
  invokeRuntime<void>("set_vmc_osc_passthrough_mode", { mode });

export const setVmcOscPassthroughTargets = (targets: string[]) =>
  invokeRuntime<void>("set_vmc_osc_passthrough_targets", { targets });

export const setOutputBackend = (backend: OutputBackendKind) =>
  invokeRuntime<void>("set_output_backend", { backend });

export const setOutputSendFilter = (
  mode: OutputSendFilterMode,
  processNames: string[],
) => invokeRuntime<void>("set_output_send_filter", { mode, processNames });

export const listRunningProcesses = () =>
  invokeRuntime<string[]>("list_running_processes");

export const requestRecalibration = () =>
  invokeRuntime<void>("request_recalibration");

export const setSpikeRejectionEnabled = (enabled: boolean) =>
  invokeRuntime<void>("set_spike_rejection_enabled", { enabled });
