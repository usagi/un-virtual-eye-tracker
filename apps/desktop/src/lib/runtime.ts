import { invoke } from "@tauri-apps/api/core";

export type InputSource = "ifacialmocap_udp" | "ifacialmocap_tcp";
export type OutputBackendKind = "ets2" | "mouse" | "keyboard" | "touch";
export type OutputSendFilterMode = "unrestricted" | "foreground_process";

export type RuntimeSnapshot = {
  inputConnected: boolean;
  outputEnabled: boolean;
  outputClutchEngaged: boolean;
  outputClutchHotkey: string;
  persistSessionSettings: boolean;
  paused: boolean;
  inputSource: InputSource;
  outputBackend: OutputBackendKind;
  outputSendFilterMode: OutputSendFilterMode;
  outputSendFilterProcessNames: string[];
  outputSendFilterAllowed: boolean;
  outputSendFilterActiveProcess: string | null;
  yawOutputMultiplier: number;
  pitchOutputMultiplier: number;
  invertOutputYaw: boolean;
  invertOutputPitch: boolean;
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
