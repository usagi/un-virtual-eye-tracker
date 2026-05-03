import { invoke } from "@tauri-apps/api/core";

export type InputSource = "ifacialmocap_udp" | "ifacialmocap_tcp" | "vmc_osc";
export type OutputBackendKind =
  | "ets2"
  | "ets2_relative"
  | "mouse"
  | "keyboard"
  | "touch";
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
  yawPosOutputMultiplier: number;
  yawNegOutputMultiplier: number;
  pitchPosOutputMultiplier: number;
  pitchNegOutputMultiplier: number;
  yawPosInputDeadzone: number;
  yawNegInputDeadzone: number;
  pitchPosInputDeadzone: number;
  pitchNegInputDeadzone: number;
  yawPosInputRangeEnd: number;
  yawNegInputRangeEnd: number;
  pitchPosInputRangeEnd: number;
  pitchNegInputRangeEnd: number;
  yawPosOutputRangeStart: number;
  yawNegOutputRangeStart: number;
  pitchPosOutputRangeStart: number;
  pitchNegOutputRangeStart: number;
  ets2RelativeAngularVelocityDegPerSec: number;
  ets2RelativeAccumulationResetEnabled: boolean;
  ets2RelativeAccumulationResetTimeoutSecs: number;
  ets2RelativeAutoReturnAngularVelocityDegPerSec: number;
  invertOutputYaw: boolean;
  invertOutputPitch: boolean;
  spikeRejectionEnabled: boolean;
  outputEasingEnabled: boolean;
  outputEasingAlpha: number;
  lookYawNorm: number;
  lookPitchNorm: number;
  lookYawNormRaw: number;
  lookPitchNormRaw: number;
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

export type AxisRangePayload = {
  yawPosInputStart: number;
  yawPosInputEnd: number;
  yawPosOutputStart: number;
  yawPosOutputEnd: number;
  yawNegInputStart: number;
  yawNegInputEnd: number;
  yawNegOutputStart: number;
  yawNegOutputEnd: number;
  pitchPosInputStart: number;
  pitchPosInputEnd: number;
  pitchPosOutputStart: number;
  pitchPosOutputEnd: number;
  pitchNegInputStart: number;
  pitchNegInputEnd: number;
  pitchNegOutputStart: number;
  pitchNegOutputEnd: number;
};

export const setOutputAxisRanges = (ranges: AxisRangePayload) =>
  invokeRuntime<void>("set_output_axis_ranges", ranges);

export const setEts2RelativeAngularVelocity = (
  angularVelocityDegPerSec: number,
) =>
  invokeRuntime<void>("set_ets2_relative_angular_velocity", {
    angularVelocityDegPerSec,
  });

export const setEts2RelativeAccumulationReset = (
  enabled: boolean,
  timeoutSecs: number,
) =>
  invokeRuntime<void>("set_ets2_relative_accumulation_reset", {
    enabled,
    timeoutSecs,
  });

export const setEts2RelativeAutoReturnAngularVelocity = (
  angularVelocityDegPerSec: number,
) =>
  invokeRuntime<void>("set_ets2_relative_auto_return_angular_velocity", {
    angularVelocityDegPerSec,
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
