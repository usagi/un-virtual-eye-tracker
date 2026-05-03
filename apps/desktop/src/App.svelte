<script lang="ts">
  import { onMount } from "svelte";
  import {
    getRuntimeSnapshot,
    listRunningProcesses,
    requestRecalibration,
    setInputSource,
    setVmcOscPort,
    setVmcOscPassthroughEnabled,
    setVmcOscPassthroughMode,
    setVmcOscPassthroughTargets,
    setOutputBackend,
    setOutputAxisRanges,
    setOutputAxisInversion,
    setEts2RelativeAngularVelocity,
    setEts2RelativeAccumulationReset,
    setEts2RelativeAutoReturnAngularVelocity,
    setOutputClutch,
    setOutputClutchHotkey,
    setOutputClutchHotkeyMode,
    setOutputEasing,
    setOutputEnabled,
    setPersistSessionSettings,
    setOutputSendFilter,
    setSpikeRejectionEnabled,
    type InputSource,
    type OutputBackendKind,
    type OutputSendFilterMode,
    type VmcOscPassthroughMode,
    type ClutchHotkeyMode,
    type RuntimeSnapshot,
  } from "./lib/runtime";

  const INPUT_SOURCES: Array<{ value: InputSource; label: string }> = [
    { value: "vmc_osc", label: "VMC / OSC UDP" },
    { value: "ifacialmocap_udp", label: "iFacialMocap UDP" },
    { value: "ifacialmocap_tcp", label: "iFacialMocap TCP" },
  ];

  const OUTPUT_BACKENDS: Array<{ value: OutputBackendKind; label: string }> = [
    { value: "ets2", label: "ETS2 / ATS (Headtracking-Absolute)" },
    { value: "ets2_relative", label: "ETS2 / ATS (Headtracking-Relative)" },
    { value: "mouse", label: "Pointer (Relative)" },
    { value: "touch", label: "Pointer (Absolute)" },
    { value: "keyboard", label: "Keyboard 4-way" },
  ];

  const OUTPUT_FILTER_MODES: Array<{
    value: OutputSendFilterMode;
    label: string;
  }> = [
    { value: "unrestricted", label: "制限なし（常時送信）" },
    { value: "foreground_process", label: "前面プロセス一致時のみ送信" },
  ];

  const VMC_OSC_PASSTHROUGH_MODES: Array<{
    value: VmcOscPassthroughMode;
    label: string;
  }> = [{ value: "raw_udp_forward", label: "raw_udp_forward" }];

  const CLUTCH_HOTKEY_MODES: Array<{ value: ClutchHotkeyMode; label: string }> =
    [
      { value: "toggle", label: "ON/OFFトグル" },
      { value: "press_on_release_off", label: "プレスON/リリースOFF" },
    ];

  const EMPTY_SNAPSHOT: RuntimeSnapshot = {
    inputConnected: false,
    outputEnabled: false,
    outputClutchEngaged: true,
    outputClutchHotkey: "Ctrl+Shift+E",
    outputClutchHotkeyMode: "toggle" as ClutchHotkeyMode,
    persistSessionSettings: true,
    paused: false,
    inputSource: "vmc_osc",
    vmcOscPort: 39539,
    vmcOscPassthroughEnabled: false,
    vmcOscPassthroughMode: "raw_udp_forward",
    vmcOscPassthroughTargets: [],
    outputBackend: "ets2",
    outputSendFilterMode: "unrestricted",
    outputSendFilterProcessNames: [],
    outputSendFilterAllowed: true,
    outputSendFilterActiveProcess: null,
    yawPosOutputMultiplier: 1,
    yawNegOutputMultiplier: 1,
    pitchPosOutputMultiplier: 1,
    pitchNegOutputMultiplier: 1,
    yawPosInputDeadzone: 0,
    yawNegInputDeadzone: 0,
    pitchPosInputDeadzone: 0,
    pitchNegInputDeadzone: 0,
    yawPosInputRangeEnd: 1,
    yawNegInputRangeEnd: 1,
    pitchPosInputRangeEnd: 1,
    pitchNegInputRangeEnd: 1,
    yawPosOutputRangeStart: 0,
    yawNegOutputRangeStart: 0,
    pitchPosOutputRangeStart: 0,
    pitchNegOutputRangeStart: 0,
    ets2RelativeAngularVelocityDegPerSec: 120,
    ets2RelativeAccumulationResetEnabled: false,
    ets2RelativeAccumulationResetTimeoutSecs: 1.5,
    ets2RelativeAutoReturnAngularVelocityDegPerSec: 60,
    invertOutputYaw: false,
    invertOutputPitch: false,
    spikeRejectionEnabled: false,
    outputEasingEnabled: true,
    outputEasingAlpha: 0.18,
    lookYawNorm: 0,
    lookPitchNorm: 0,
    lookYawNormRaw: 0,
    lookPitchNormRaw: 0,
    confidence: 0,
    active: false,
    lastError: null,
    updatedAtMs: 0,
  };

  type UiLogLevel = "info" | "warn" | "error";
  type UiLogSource = "runtime" | "ui";

  type UiLogEntry = {
    id: number;
    timestampMs: number;
    level: UiLogLevel;
    source: UiLogSource;
    message: string;
  };

  type ParsedShortcut = {
    ctrl: boolean;
    shift: boolean;
    alt: boolean;
    meta: boolean;
    key: string | null;
  };

  const MAX_LOG_ENTRIES = 400;
  const AXIS_OUTPUT_RANGE_MIN = 0.0;
  const AXIS_OUTPUT_RANGE_MAX = 1.0;
  const AXIS_INPUT_RANGE_MIN = 0.0;
  const AXIS_INPUT_RANGE_MAX = 1.0;
  const VMC_OSC_PORT_MIN = 1;
  const VMC_OSC_PORT_MAX = 65535;
  const AXIS_OUTPUT_RANGE_STEP = 0.01;
  const AXIS_INPUT_RANGE_STEP = 0.01;
  const AXIS_APPLY_DEBOUNCE_MS = 100;
  const AXIS_SYNC_GRACE_MS = 450;
  const OUTPUT_EASING_ALPHA_MIN = 0.01;
  const OUTPUT_EASING_ALPHA_MAX = 1.0;
  const OUTPUT_EASING_ALPHA_STEP = 0.01;
  const OUTPUT_EASING_APPLY_DEBOUNCE_MS = 100;
  const OUTPUT_EASING_SYNC_GRACE_MS = 450;
  const ETS2_RELATIVE_ANGULAR_VELOCITY_MIN = 1;
  const ETS2_RELATIVE_ANGULAR_VELOCITY_MAX = 720;
  const ETS2_RELATIVE_ANGULAR_VELOCITY_STEP = 1;
  const ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_MIN = 0.05;
  const ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_MAX = 10.0;
  const ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_STEP = 0.05;

  let snapshot: RuntimeSnapshot = EMPTY_SNAPSHOT;
  let logs: UiLogEntry[] = [];
  let busy = true;
  let actionError = "";
  let nextLogId = 1;
  let copyFeedback = "";

  let processInput = "";
  let runningProcesses: string[] = [];
  let selectedRunningProcess = "";
  let processListBusy = false;

  let clutchHotkeyDraft = "Ctrl+Shift+E";
  let clutchHotkeyDirty = false;

  let vmcOscPortDraft = 39539;
  let vmcOscPortDirty = false;

  let vmcOscPassthroughEnabledDraft = false;
  let vmcOscPassthroughModeDraft: VmcOscPassthroughMode = "raw_udp_forward";
  let vmcOscPassthroughTargetsDraft = "";
  let vmcOscPassthroughTargetsDirty = false;

  let yawPosInputStartDraft = 0;
  let yawPosInputEndDraft = 1;
  let yawPosOutputStartDraft = 0;
  let yawPosOutputEndDraft = 1;
  let yawNegInputStartDraft = 0;
  let yawNegInputEndDraft = 1;
  let yawNegOutputStartDraft = 0;
  let yawNegOutputEndDraft = 1;
  let pitchPosInputStartDraft = 0;
  let pitchPosInputEndDraft = 1;
  let pitchPosOutputStartDraft = 0;
  let pitchPosOutputEndDraft = 1;
  let pitchNegInputStartDraft = 0;
  let pitchNegInputEndDraft = 1;
  let pitchNegOutputStartDraft = 0;
  let pitchNegOutputEndDraft = 1;
  let axisLastEditAt = 0;
  let axisApplyTimer: number | undefined;

  let invertYawDraft = false;
  let invertPitchDraft = false;
  let spikeRejectionEnabledDraft = false;
  let outputEasingEnabledDraft = true;
  let outputEasingAlphaDraft = 0.18;
  let ets2RelativeAngularVelocityDraft = 120;
  let ets2RelativeAccumulationResetEnabledDraft = false;
  let ets2RelativeAccumulationResetTimeoutSecsDraft = 1.5;
  let ets2RelativeAutoReturnAngularVelocityDraft = 60;
  let outputSettingsLastEditAt = 0;
  let outputSettingsApplyTimer: number | undefined;

  let poller: number | undefined;
  let copyFeedbackTimer: number | undefined;

  const confidencePercent = () => Math.round(snapshot.confidence * 100);
  const liveSendEnabled = () =>
    snapshot.outputEnabled && snapshot.outputClutchEngaged;
  const updatedAtLabel = () =>
    snapshot.updatedAtMs <= 0
      ? "-"
      : new Date(snapshot.updatedAtMs).toLocaleTimeString();
  const logTimestampLabel = (timestampMs: number) =>
    new Date(timestampMs).toLocaleTimeString();
  const outputBackendLabel = () =>
    OUTPUT_BACKENDS.find((backend) => backend.value === snapshot.outputBackend)
      ?.label ?? snapshot.outputBackend;

  const normalizeProcessName = (name: string): string | null => {
    const trimmed = name.trim();
    if (trimmed.length === 0) {
      return null;
    }

    const normalizedPath = trimmed.replace(/\//g, "\\");
    const parts = normalizedPath.split("\\");
    const fileName = parts[parts.length - 1]?.trim();
    if (!fileName || fileName.length === 0) {
      return null;
    }

    return fileName.toLowerCase();
  };

  function parseShortcut(raw: string): ParsedShortcut | null {
    const tokens = raw
      .split("+")
      .map((token) => token.trim().toLowerCase())
      .filter((token) => token.length > 0);

    if (tokens.length === 0) {
      return null;
    }

    const parsed: ParsedShortcut = {
      ctrl: false,
      shift: false,
      alt: false,
      meta: false,
      key: null,
    };

    for (const token of tokens) {
      if (
        token === "ctrl" ||
        token === "control" ||
        token === "commandorcontrol" ||
        token === "cmdorctrl"
      ) {
        parsed.ctrl = true;
        continue;
      }
      if (token === "shift") {
        parsed.shift = true;
        continue;
      }
      if (token === "alt" || token === "option") {
        parsed.alt = true;
        continue;
      }
      if (
        token === "meta" ||
        token === "cmd" ||
        token === "command" ||
        token === "super" ||
        token === "win" ||
        token === "windows"
      ) {
        parsed.meta = true;
        continue;
      }

      parsed.key = token;
    }

    return parsed.key ? parsed : null;
  }

  function normalizeKeyboardKey(key: string): string {
    if (key === " ") {
      return "space";
    }
    return key.toLowerCase();
  }

  function shortcutMatchesEvent(
    event: KeyboardEvent,
    shortcut: ParsedShortcut,
  ): boolean {
    if (event.ctrlKey !== shortcut.ctrl) {
      return false;
    }
    if (event.shiftKey !== shortcut.shift) {
      return false;
    }
    if (event.altKey !== shortcut.alt) {
      return false;
    }
    if (event.metaKey !== shortcut.meta) {
      return false;
    }

    return normalizeKeyboardKey(event.key) === shortcut.key;
  }

  function isTextInputTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) {
      return false;
    }

    const tagName = target.tagName.toLowerCase();
    if (tagName === "input" || tagName === "textarea" || tagName === "select") {
      return true;
    }

    return target.isContentEditable;
  }

  function shouldIgnoreHotkeyForTextInput(
    event: KeyboardEvent,
    shortcut: ParsedShortcut,
  ): boolean {
    if (!isTextInputTarget(event.target)) {
      return false;
    }

    return !shortcut.ctrl && !shortcut.alt && !shortcut.meta;
  }

  function clampAxisInputRange(value: number): number {
    return Math.min(
      AXIS_INPUT_RANGE_MAX,
      Math.max(AXIS_INPUT_RANGE_MIN, value),
    );
  }

  function clampAxisOutputRange(value: number): number {
    return Math.min(
      AXIS_OUTPUT_RANGE_MAX,
      Math.max(AXIS_OUTPUT_RANGE_MIN, value),
    );
  }

  function orderedRange(start: number, end: number): [number, number] {
    return start <= end ? [start, end] : [end, start];
  }

  function clampEts2RelativeAngularVelocity(value: number): number {
    return Math.min(
      ETS2_RELATIVE_ANGULAR_VELOCITY_MAX,
      Math.max(ETS2_RELATIVE_ANGULAR_VELOCITY_MIN, value),
    );
  }

  function clampEts2RelativeAccumulationResetTimeout(value: number): number {
    return Math.min(
      ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_MAX,
      Math.max(ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_MIN, value),
    );
  }

  const axisIsActive = (
    value: number,
    start: number,
    end: number,
    direction: "positive" | "negative",
  ) => {
    const [rangeStart, rangeEnd] = orderedRange(start, end);
    const magnitude = direction === "positive" ? value : -value;
    return magnitude >= rangeStart && magnitude <= rangeEnd && magnitude > 0;
  };

  const formatRange = (start: number, end: number, digits = 2) => {
    const [rangeStart, rangeEnd] = orderedRange(start, end);
    return `[${rangeStart.toFixed(digits)}..${rangeEnd.toFixed(digits)}]`;
  };

  function rangePercent(value: number, min: number, max: number): number {
    if (max <= min) {
      return 0;
    }

    const clamped = Math.min(max, Math.max(min, value));
    return ((clamped - min) / (max - min)) * 100;
  }

  function axisRangeStyle(
    start: number,
    end: number,
    min: number,
    max: number,
  ): string {
    const [rangeStart, rangeEnd] = orderedRange(start, end);
    return `--range-start: ${rangePercent(rangeStart, min, max).toFixed(2)}%; --range-end: ${rangePercent(rangeEnd, min, max).toFixed(2)}%`;
  }

  function clampVmcOscPort(value: number): number {
    const rounded = Math.round(value);
    return Math.min(VMC_OSC_PORT_MAX, Math.max(VMC_OSC_PORT_MIN, rounded));
  }

  function parsePassthroughTarget(raw: string): string | null {
    const text = raw.trim();
    if (text.length === 0) {
      return null;
    }

    if (text.startsWith("[")) {
      const end = text.indexOf("]");
      if (end <= 1) {
        return null;
      }

      const host = text.slice(1, end).trim().toLowerCase();
      const remainder = text.slice(end + 1).trim();
      if (host.length === 0 || !remainder.startsWith(":")) {
        return null;
      }

      const port = Number(remainder.slice(1).trim());
      if (!Number.isInteger(port) || port < 1 || port > 65535) {
        return null;
      }

      return `[${host}]:${port}`;
    }

    const parts = text.split(":");
    if (parts.length !== 2) {
      return null;
    }

    const host = parts[0].trim().toLowerCase();
    const port = Number(parts[1].trim());
    if (host.length === 0 || host.includes(":")) {
      return null;
    }
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      return null;
    }

    return `${host}:${port}`;
  }

  function parsePassthroughTargetsInput(raw: string): {
    targets: string[];
    invalidLine: string | null;
  } {
    const unique = new Set<string>();
    const targets: string[] = [];

    for (const line of raw.split(/\r?\n/)) {
      const trimmed = line.trim();
      if (trimmed.length === 0) {
        continue;
      }

      const normalized = parsePassthroughTarget(trimmed);
      if (!normalized) {
        return {
          targets,
          invalidLine: trimmed,
        };
      }

      const dedupKey = normalized.toLowerCase();
      if (unique.has(dedupKey)) {
        continue;
      }

      unique.add(dedupKey);
      targets.push(normalized);
    }

    return {
      targets,
      invalidLine: null,
    };
  }

  function sameStringList(left: string[], right: string[]): boolean {
    if (left.length !== right.length) {
      return false;
    }

    for (let index = 0; index < left.length; index += 1) {
      if (left[index] !== right[index]) {
        return false;
      }
    }

    return true;
  }

  function clampOutputEasingAlpha(value: number): number {
    return Math.min(
      OUTPUT_EASING_ALPHA_MAX,
      Math.max(OUTPUT_EASING_ALPHA_MIN, value),
    );
  }

  function nextProcessListWith(entry: string) {
    const next = [...snapshot.outputSendFilterProcessNames, entry];
    const deduped = Array.from(new Set(next.map((name) => name.toLowerCase())));
    deduped.sort();
    return deduped;
  }

  function pushLog(level: UiLogLevel, source: UiLogSource, message: string) {
    const normalizedMessage = message.trim();
    if (normalizedMessage.length === 0) {
      return;
    }

    const now = Date.now();
    const last = logs[logs.length - 1];
    if (
      last &&
      last.level === level &&
      last.source === source &&
      last.message === normalizedMessage &&
      now - last.timestampMs < 500
    ) {
      return;
    }

    logs = [
      ...logs,
      {
        id: nextLogId,
        timestampMs: now,
        level,
        source,
        message: normalizedMessage,
      },
    ];
    nextLogId += 1;

    if (logs.length > MAX_LOG_ENTRIES) {
      logs = logs.slice(logs.length - MAX_LOG_ENTRIES);
    }
  }

  function setCopyFeedback(message: string) {
    copyFeedback = message;
    if (copyFeedbackTimer !== undefined) {
      window.clearTimeout(copyFeedbackTimer);
    }
    copyFeedbackTimer = window.setTimeout(() => {
      copyFeedback = "";
      copyFeedbackTimer = undefined;
    }, 2200);
  }

  function clearLogs() {
    logs = [];
    nextLogId = 1;
    setCopyFeedback("Logs cleared");
  }

  function exportLogsText() {
    return logs
      .map(
        (entry) =>
          `[${new Date(entry.timestampMs).toISOString()}] [${entry.level.toUpperCase()}] [${entry.source}] ${entry.message}`,
      )
      .join("\n");
  }

  async function copyLogs() {
    const content = exportLogsText();
    if (content.length === 0) {
      setCopyFeedback("No logs to copy");
      return;
    }

    try {
      if (!navigator.clipboard || !navigator.clipboard.writeText) {
        throw new Error("clipboard API unavailable");
      }
      await navigator.clipboard.writeText(content);
      setCopyFeedback("Logs copied");
    } catch (error) {
      const message = String(error);
      pushLog("error", "ui", `Copy logs failed: ${message}`);
      setCopyFeedback("Copy failed");
    }
  }

  async function refreshSnapshot() {
    const previousSnapshot = snapshot;
    const previousError = snapshot.lastError;

    try {
      const latest = await getRuntimeSnapshot();
      snapshot = latest;
      busy = false;

      if (!clutchHotkeyDirty) {
        clutchHotkeyDraft = latest.outputClutchHotkey;
      }

      if (!vmcOscPortDirty) {
        vmcOscPortDraft = latest.vmcOscPort;
      }

      vmcOscPassthroughEnabledDraft = latest.vmcOscPassthroughEnabled;
      vmcOscPassthroughModeDraft = latest.vmcOscPassthroughMode;
      if (!vmcOscPassthroughTargetsDirty) {
        vmcOscPassthroughTargetsDraft =
          latest.vmcOscPassthroughTargets.join("\n");
      }

      if (Date.now() - axisLastEditAt > AXIS_SYNC_GRACE_MS) {
        yawPosInputStartDraft = latest.yawPosInputDeadzone;
        yawPosInputEndDraft = latest.yawPosInputRangeEnd;
        yawPosOutputStartDraft = latest.yawPosOutputRangeStart;
        yawPosOutputEndDraft = latest.yawPosOutputMultiplier;
        yawNegInputStartDraft = latest.yawNegInputDeadzone;
        yawNegInputEndDraft = latest.yawNegInputRangeEnd;
        yawNegOutputStartDraft = latest.yawNegOutputRangeStart;
        yawNegOutputEndDraft = latest.yawNegOutputMultiplier;
        pitchPosInputStartDraft = latest.pitchPosInputDeadzone;
        pitchPosInputEndDraft = latest.pitchPosInputRangeEnd;
        pitchPosOutputStartDraft = latest.pitchPosOutputRangeStart;
        pitchPosOutputEndDraft = latest.pitchPosOutputMultiplier;
        pitchNegInputStartDraft = latest.pitchNegInputDeadzone;
        pitchNegInputEndDraft = latest.pitchNegInputRangeEnd;
        pitchNegOutputStartDraft = latest.pitchNegOutputRangeStart;
        pitchNegOutputEndDraft = latest.pitchNegOutputMultiplier;
      }

      if (Date.now() - outputSettingsLastEditAt > OUTPUT_EASING_SYNC_GRACE_MS) {
        invertYawDraft = latest.invertOutputYaw;
        invertPitchDraft = latest.invertOutputPitch;
        spikeRejectionEnabledDraft = latest.spikeRejectionEnabled;
        outputEasingEnabledDraft = latest.outputEasingEnabled;
        outputEasingAlphaDraft = latest.outputEasingAlpha;
        ets2RelativeAngularVelocityDraft =
          latest.ets2RelativeAngularVelocityDegPerSec;
        ets2RelativeAccumulationResetEnabledDraft =
          latest.ets2RelativeAccumulationResetEnabled;
        ets2RelativeAccumulationResetTimeoutSecsDraft =
          latest.ets2RelativeAccumulationResetTimeoutSecs;
        ets2RelativeAutoReturnAngularVelocityDraft =
          latest.ets2RelativeAutoReturnAngularVelocityDegPerSec;
      }

      if (!previousSnapshot.inputConnected && latest.inputConnected) {
        pushLog("info", "runtime", "Input link connected");
      }
      if (previousSnapshot.inputConnected && !latest.inputConnected) {
        pushLog("warn", "runtime", "Input link disconnected");
      }
      if (previousSnapshot.active !== latest.active) {
        pushLog(
          "info",
          "runtime",
          latest.active ? "Tracking active" : "Tracking idle",
        );
      }
      if (
        previousSnapshot.outputSendFilterMode !== latest.outputSendFilterMode
      ) {
        pushLog(
          "info",
          "runtime",
          `Send filter mode: ${latest.outputSendFilterMode}`,
        );
      }
      if (
        previousSnapshot.outputSendFilterAllowed !==
        latest.outputSendFilterAllowed
      ) {
        pushLog(
          latest.outputSendFilterAllowed ? "info" : "warn",
          "runtime",
          latest.outputSendFilterAllowed
            ? "Send filter gate opened"
            : `Send filter gate closed${latest.outputSendFilterActiveProcess ? ` (active: ${latest.outputSendFilterActiveProcess})` : ""}`,
        );
      }

      if (
        previousSnapshot.vmcOscPassthroughEnabled !==
        latest.vmcOscPassthroughEnabled
      ) {
        pushLog(
          latest.vmcOscPassthroughEnabled ? "info" : "warn",
          "runtime",
          latest.vmcOscPassthroughEnabled
            ? "VMC/OSC passthrough enabled"
            : "VMC/OSC passthrough disabled",
        );
      }

      if (
        !sameStringList(
          previousSnapshot.vmcOscPassthroughTargets,
          latest.vmcOscPassthroughTargets,
        )
      ) {
        pushLog(
          "info",
          "runtime",
          `VMC/OSC passthrough targets: ${latest.vmcOscPassthroughTargets.length}`,
        );
      }

      const wasLiveSend =
        previousSnapshot.outputEnabled && previousSnapshot.outputClutchEngaged;
      const isLiveSend = latest.outputEnabled && latest.outputClutchEngaged;
      if (wasLiveSend !== isLiveSend) {
        pushLog(
          isLiveSend ? "info" : "warn",
          "runtime",
          isLiveSend ? "Live send enabled" : "Live send disabled",
        );
      }

      if (latest.lastError && latest.lastError !== previousError) {
        pushLog("warn", "runtime", latest.lastError);
      }
    } catch (error) {
      const message = String(error);
      actionError = message;
      pushLog("error", "runtime", `Snapshot refresh failed: ${message}`);
    }
  }

  async function applyAction(label: string, action: () => Promise<void>) {
    try {
      actionError = "";
      await action();
      pushLog("info", "ui", `${label} succeeded`);
      await refreshSnapshot();
    } catch (error) {
      const message = String(error);
      actionError = message;
      pushLog("error", "ui", `${label} failed: ${message}`);
    }
  }

  async function applyAxisRangesLive() {
    yawPosInputStartDraft = clampAxisInputRange(yawPosInputStartDraft);
    yawPosInputEndDraft = clampAxisInputRange(yawPosInputEndDraft);
    yawPosOutputStartDraft = clampAxisOutputRange(yawPosOutputStartDraft);
    yawPosOutputEndDraft = clampAxisOutputRange(yawPosOutputEndDraft);
    yawNegInputStartDraft = clampAxisInputRange(yawNegInputStartDraft);
    yawNegInputEndDraft = clampAxisInputRange(yawNegInputEndDraft);
    yawNegOutputStartDraft = clampAxisOutputRange(yawNegOutputStartDraft);
    yawNegOutputEndDraft = clampAxisOutputRange(yawNegOutputEndDraft);
    pitchPosInputStartDraft = clampAxisInputRange(pitchPosInputStartDraft);
    pitchPosInputEndDraft = clampAxisInputRange(pitchPosInputEndDraft);
    pitchPosOutputStartDraft = clampAxisOutputRange(pitchPosOutputStartDraft);
    pitchPosOutputEndDraft = clampAxisOutputRange(pitchPosOutputEndDraft);
    pitchNegInputStartDraft = clampAxisInputRange(pitchNegInputStartDraft);
    pitchNegInputEndDraft = clampAxisInputRange(pitchNegInputEndDraft);
    pitchNegOutputStartDraft = clampAxisOutputRange(pitchNegOutputStartDraft);
    pitchNegOutputEndDraft = clampAxisOutputRange(pitchNegOutputEndDraft);

    try {
      actionError = "";
      await setOutputAxisRanges({
        yawPosInputStart: yawPosInputStartDraft,
        yawPosInputEnd: yawPosInputEndDraft,
        yawPosOutputStart: yawPosOutputStartDraft,
        yawPosOutputEnd: yawPosOutputEndDraft,
        yawNegInputStart: yawNegInputStartDraft,
        yawNegInputEnd: yawNegInputEndDraft,
        yawNegOutputStart: yawNegOutputStartDraft,
        yawNegOutputEnd: yawNegOutputEndDraft,
        pitchPosInputStart: pitchPosInputStartDraft,
        pitchPosInputEnd: pitchPosInputEndDraft,
        pitchPosOutputStart: pitchPosOutputStartDraft,
        pitchPosOutputEnd: pitchPosOutputEndDraft,
        pitchNegInputStart: pitchNegInputStartDraft,
        pitchNegInputEnd: pitchNegInputEndDraft,
        pitchNegOutputStart: pitchNegOutputStartDraft,
        pitchNegOutputEnd: pitchNegOutputEndDraft,
      });
    } catch (error) {
      const message = String(error);
      actionError = message;
      pushLog("error", "ui", `Set axis ranges failed: ${message}`);
    }
  }

  function queueAxisMultiplierApply() {
    axisLastEditAt = Date.now();

    if (axisApplyTimer !== undefined) {
      window.clearTimeout(axisApplyTimer);
    }

    axisApplyTimer = window.setTimeout(() => {
      axisApplyTimer = undefined;
      void applyAxisRangesLive();
    }, AXIS_APPLY_DEBOUNCE_MS);
  }

  async function applyOutputAxisInversionLive() {
    try {
      actionError = "";
      await setOutputAxisInversion(invertYawDraft, invertPitchDraft);
    } catch (error) {
      const message = String(error);
      actionError = message;
      pushLog("error", "ui", `Set output axis inversion failed: ${message}`);
    }
  }

  async function applyOutputMappingSettings() {
    const alpha = clampOutputEasingAlpha(outputEasingAlphaDraft);
    outputEasingAlphaDraft = alpha;

    try {
      actionError = "";
      await setOutputEasing(outputEasingEnabledDraft, alpha);
      if (snapshot.outputBackend === "ets2_relative") {
        ets2RelativeAngularVelocityDraft = clampEts2RelativeAngularVelocity(
          ets2RelativeAngularVelocityDraft,
        );
        await setEts2RelativeAngularVelocity(ets2RelativeAngularVelocityDraft);
        ets2RelativeAccumulationResetTimeoutSecsDraft =
          clampEts2RelativeAccumulationResetTimeout(
            ets2RelativeAccumulationResetTimeoutSecsDraft,
          );
        await setEts2RelativeAccumulationReset(
          ets2RelativeAccumulationResetEnabledDraft,
          ets2RelativeAccumulationResetTimeoutSecsDraft,
        );
        ets2RelativeAutoReturnAngularVelocityDraft =
          clampEts2RelativeAngularVelocity(
            ets2RelativeAutoReturnAngularVelocityDraft,
          );
        await setEts2RelativeAutoReturnAngularVelocity(
          ets2RelativeAutoReturnAngularVelocityDraft,
        );
      }
    } catch (error) {
      const message = String(error);
      actionError = message;
      pushLog("error", "ui", `Set output mapping settings failed: ${message}`);
    }
  }

  function queueOutputSettingsApply() {
    outputSettingsLastEditAt = Date.now();

    if (outputSettingsApplyTimer !== undefined) {
      window.clearTimeout(outputSettingsApplyTimer);
    }

    outputSettingsApplyTimer = window.setTimeout(() => {
      outputSettingsApplyTimer = undefined;
      void applyOutputMappingSettings();
    }, OUTPUT_EASING_APPLY_DEBOUNCE_MS);
  }

  function onInvertYawToggle(event: Event) {
    invertYawDraft = (event.currentTarget as HTMLInputElement).checked;
    outputSettingsLastEditAt = Date.now();
    void applyOutputAxisInversionLive();
  }

  function onInvertPitchToggle(event: Event) {
    invertPitchDraft = (event.currentTarget as HTMLInputElement).checked;
    outputSettingsLastEditAt = Date.now();
    void applyOutputAxisInversionLive();
  }

  async function onSpikeRejectionEnabledToggle(event: Event) {
    spikeRejectionEnabledDraft = (event.currentTarget as HTMLInputElement)
      .checked;
    try {
      actionError = "";
      await setSpikeRejectionEnabled(spikeRejectionEnabledDraft);
    } catch (error) {
      const message = String(error);
      actionError = message;
      pushLog("error", "ui", `Set spike rejection failed: ${message}`);
    }
  }

  function onOutputEasingEnabledToggle(event: Event) {
    outputEasingEnabledDraft = (event.currentTarget as HTMLInputElement)
      .checked;
    queueOutputSettingsApply();
  }

  function onOutputEasingAlphaInput(event: Event) {
    const parsed = Number((event.currentTarget as HTMLInputElement).value);
    if (!Number.isFinite(parsed)) {
      return;
    }

    outputEasingAlphaDraft = clampOutputEasingAlpha(parsed);
    queueOutputSettingsApply();
  }

  function onAxisInputRangeInput(
    event: Event,
    assign: (value: number) => void,
  ) {
    const parsed = Number((event.currentTarget as HTMLInputElement).value);
    if (!Number.isFinite(parsed)) {
      return;
    }

    assign(clampAxisInputRange(parsed));
    queueAxisMultiplierApply();
  }

  function onAxisOutputRangeInput(
    event: Event,
    assign: (value: number) => void,
  ) {
    const parsed = Number((event.currentTarget as HTMLInputElement).value);
    if (!Number.isFinite(parsed)) {
      return;
    }

    assign(clampAxisOutputRange(parsed));
    queueAxisMultiplierApply();
  }

  function onEts2RelativeAngularVelocityInput(event: Event) {
    const parsed = Number((event.currentTarget as HTMLInputElement).value);
    if (!Number.isFinite(parsed)) {
      return;
    }

    ets2RelativeAngularVelocityDraft = clampEts2RelativeAngularVelocity(parsed);
    queueOutputSettingsApply();
  }

  function onEts2RelativeAccumulationResetEnabledToggle(event: Event) {
    ets2RelativeAccumulationResetEnabledDraft = (
      event.currentTarget as HTMLInputElement
    ).checked;
    outputSettingsLastEditAt = Date.now();
    queueOutputSettingsApply();
  }

  function onEts2RelativeAccumulationResetTimeoutInput(event: Event) {
    const parsed = Number((event.currentTarget as HTMLInputElement).value);
    if (!Number.isFinite(parsed)) {
      return;
    }

    ets2RelativeAccumulationResetTimeoutSecsDraft =
      clampEts2RelativeAccumulationResetTimeout(parsed);
    queueOutputSettingsApply();
  }

  function onEts2RelativeAutoReturnAngularVelocityInput(event: Event) {
    const parsed = Number((event.currentTarget as HTMLInputElement).value);
    if (!Number.isFinite(parsed)) {
      return;
    }

    ets2RelativeAutoReturnAngularVelocityDraft =
      clampEts2RelativeAngularVelocity(parsed);
    queueOutputSettingsApply();
  }

  function onLiveSendToggle(event: Event) {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    void applyAction(`Set live send=${enabled}`, async () => {
      await setOutputEnabled(enabled);
      await setOutputClutch(enabled);
    });
  }

  function onPersistSessionSettingsToggle(event: Event) {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    void applyAction(`Set persist session settings=${enabled}`, () =>
      setPersistSessionSettings(enabled),
    );
  }

  function onInputSourceChange(event: Event) {
    const source = (event.currentTarget as HTMLSelectElement)
      .value as InputSource;
    void applyAction(`Set input source=${source}`, () =>
      setInputSource(source),
    );
  }

  function onVmcOscPortInput(event: Event) {
    const parsed = Number((event.currentTarget as HTMLInputElement).value);
    if (!Number.isFinite(parsed)) {
      return;
    }

    vmcOscPortDraft = clampVmcOscPort(parsed);
    vmcOscPortDirty = vmcOscPortDraft !== snapshot.vmcOscPort;
  }

  function onApplyVmcOscPort() {
    const port = clampVmcOscPort(vmcOscPortDraft);
    vmcOscPortDraft = port;

    if (port === snapshot.vmcOscPort) {
      vmcOscPortDirty = false;
      return;
    }

    void applyAction(`Set VMC/OSC UDP port=${port}`, () =>
      setVmcOscPort(port),
    ).finally(() => {
      vmcOscPortDirty = vmcOscPortDraft !== snapshot.vmcOscPort;
    });
  }

  function onVmcOscPassthroughEnabledToggle(event: Event) {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    vmcOscPassthroughEnabledDraft = enabled;
    void applyAction(`Set VMC/OSC passthrough enabled=${enabled}`, () =>
      setVmcOscPassthroughEnabled(enabled),
    );
  }

  function onVmcOscPassthroughModeChange(event: Event) {
    const mode = (event.currentTarget as HTMLSelectElement)
      .value as VmcOscPassthroughMode;
    vmcOscPassthroughModeDraft = mode;
    void applyAction(`Set VMC/OSC passthrough mode=${mode}`, () =>
      setVmcOscPassthroughMode(mode),
    );
  }

  function onVmcOscPassthroughTargetsInput(event: Event) {
    vmcOscPassthroughTargetsDraft = (event.currentTarget as HTMLTextAreaElement)
      .value;
    const parsed = parsePassthroughTargetsInput(vmcOscPassthroughTargetsDraft);
    vmcOscPassthroughTargetsDirty =
      parsed.invalidLine !== null ||
      !sameStringList(parsed.targets, snapshot.vmcOscPassthroughTargets);
  }

  function onApplyVmcOscPassthroughTargets() {
    const parsed = parsePassthroughTargetsInput(vmcOscPassthroughTargetsDraft);
    if (parsed.invalidLine) {
      pushLog(
        "warn",
        "ui",
        `Invalid passthrough target: ${parsed.invalidLine}`,
      );
      return;
    }

    if (sameStringList(parsed.targets, snapshot.vmcOscPassthroughTargets)) {
      vmcOscPassthroughTargetsDirty = false;
      return;
    }

    void applyAction(
      `Set VMC/OSC passthrough targets=${parsed.targets.length}`,
      () => setVmcOscPassthroughTargets(parsed.targets),
    ).finally(() => {
      const latestParsed = parsePassthroughTargetsInput(
        vmcOscPassthroughTargetsDraft,
      );
      vmcOscPassthroughTargetsDirty =
        latestParsed.invalidLine !== null ||
        !sameStringList(
          latestParsed.targets,
          snapshot.vmcOscPassthroughTargets,
        );
    });
  }

  function onOutputBackendChange(event: Event) {
    const backend = (event.currentTarget as HTMLSelectElement)
      .value as OutputBackendKind;
    void applyAction(`Set output backend=${backend}`, () =>
      setOutputBackend(backend),
    );
  }

  function onFilterModeChange(event: Event) {
    const mode = (event.currentTarget as HTMLSelectElement)
      .value as OutputSendFilterMode;
    void applyAction(`Set send filter mode=${mode}`, () =>
      setOutputSendFilter(mode, snapshot.outputSendFilterProcessNames),
    );
  }

  function addFilterProcess(entryRaw: string, sourceLabel: string) {
    const normalized = normalizeProcessName(entryRaw);
    if (!normalized) {
      pushLog("warn", "ui", `Process name from ${sourceLabel} is empty`);
      return;
    }

    if (snapshot.outputSendFilterProcessNames.includes(normalized)) {
      pushLog("info", "ui", `Process already registered: ${normalized}`);
      return;
    }

    const nextNames = nextProcessListWith(normalized);
    void applyAction(`Add filter process=${normalized}`, () =>
      setOutputSendFilter(snapshot.outputSendFilterMode, nextNames),
    );
  }

  function removeFilterProcess(name: string) {
    const nextNames = snapshot.outputSendFilterProcessNames.filter(
      (entry) => entry !== name,
    );
    void applyAction(`Remove filter process=${name}`, () =>
      setOutputSendFilter(snapshot.outputSendFilterMode, nextNames),
    );
  }

  function onAddManualProcess() {
    addFilterProcess(processInput, "manual input");
    processInput = "";
  }

  function onAddRunningProcess() {
    if (!selectedRunningProcess) {
      pushLog("warn", "ui", "No running process selected");
      return;
    }

    addFilterProcess(selectedRunningProcess, "running process list");
  }

  async function refreshRunningProcessList() {
    processListBusy = true;
    try {
      const names = await listRunningProcesses();
      runningProcesses = names;
      if (
        !selectedRunningProcess ||
        !runningProcesses.includes(selectedRunningProcess)
      ) {
        selectedRunningProcess = runningProcesses[0] ?? "";
      }
      pushLog(
        "info",
        "ui",
        `Loaded ${runningProcesses.length} running process names`,
      );
    } catch (error) {
      const message = String(error);
      actionError = message;
      pushLog("error", "ui", `Process list refresh failed: ${message}`);
    } finally {
      processListBusy = false;
    }
  }

  function onClutchHotkeyInput(event: Event) {
    clutchHotkeyDraft = (event.currentTarget as HTMLInputElement).value;
    clutchHotkeyDirty =
      clutchHotkeyDraft.trim() !== snapshot.outputClutchHotkey;
  }

  function onApplyClutchHotkey() {
    const hotkey = clutchHotkeyDraft.trim();
    if (!parseShortcut(hotkey)) {
      pushLog(
        "warn",
        "ui",
        `Invalid clutch shortcut format: ${hotkey || "(empty)"}`,
      );
      return;
    }

    void applyAction(`Set clutch hotkey=${hotkey}`, () =>
      setOutputClutchHotkey(hotkey),
    ).finally(() => {
      clutchHotkeyDirty =
        clutchHotkeyDraft.trim() !== snapshot.outputClutchHotkey;
    });
  }

  function onRecalibrate() {
    void applyAction("Request recalibration", () => requestRecalibration());
  }

  function toggleLiveSendFromHotkey() {
    const next = !liveSendEnabled();
    void applyAction(`Set live send=${next} (hotkey)`, async () => {
      await setOutputEnabled(next);
      await setOutputClutch(next);
    });
  }

  function onClutchHotkeyModeChange(event: Event) {
    const mode = (event.currentTarget as HTMLSelectElement)
      .value as ClutchHotkeyMode;
    void applyAction(`Set clutch hotkey mode=${mode}`, () =>
      setOutputClutchHotkeyMode(mode),
    );
  }

  onMount(() => {
    pushLog("info", "ui", "UNVET control deck started");
    void refreshSnapshot();
    void refreshRunningProcessList();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat) {
        return;
      }

      const parsedShortcut = parseShortcut(snapshot.outputClutchHotkey);
      if (!parsedShortcut) {
        return;
      }
      if (shouldIgnoreHotkeyForTextInput(event, parsedShortcut)) {
        return;
      }
      if (!shortcutMatchesEvent(event, parsedShortcut)) {
        return;
      }

      event.preventDefault();

      if (snapshot.outputClutchHotkeyMode === "press_on_release_off") {
        void applyAction("Set live send=true (hotkey press)", async () => {
          await setOutputEnabled(true);
          await setOutputClutch(true);
        });
      } else {
        toggleLiveSendFromHotkey();
      }
    };

    const onKeyUp = (event: KeyboardEvent) => {
      if (snapshot.outputClutchHotkeyMode !== "press_on_release_off") {
        return;
      }

      const parsedShortcut = parseShortcut(snapshot.outputClutchHotkey);
      if (!parsedShortcut) {
        return;
      }
      if (shouldIgnoreHotkeyForTextInput(event, parsedShortcut)) {
        return;
      }

      const keyMatches = normalizeKeyboardKey(event.key) === parsedShortcut.key;
      if (!keyMatches) {
        return;
      }

      event.preventDefault();
      void applyAction("Set live send=false (hotkey release)", async () => {
        await setOutputEnabled(false);
        await setOutputClutch(false);
      });
    };

    window.addEventListener("keydown", onKeyDown, { capture: true });
    window.addEventListener("keyup", onKeyUp, { capture: true });

    poller = window.setInterval(() => {
      void refreshSnapshot();
    }, 120);

    return () => {
      if (poller !== undefined) {
        window.clearInterval(poller);
      }
      if (axisApplyTimer !== undefined) {
        window.clearTimeout(axisApplyTimer);
      }
      if (outputSettingsApplyTimer !== undefined) {
        window.clearTimeout(outputSettingsApplyTimer);
      }
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
      if (copyFeedbackTimer !== undefined) {
        window.clearTimeout(copyFeedbackTimer);
      }
    };
  });
</script>

<main class="shell">
  <header class="hero">
    <div class="hero-copy">
      <p class="eyebrow">UNVET DESKTOP</p>
      <h1>UNVET Control Deck</h1>
      <p class="summary">
        USAGI.NETWORK Virtual Eye Tracker ―
        アバター向け視線トラッキングをトラックの運転へ
      </p>
    </div>
    <div class="hero-meta">
      <p class={`pill ${liveSendEnabled() ? "ok" : "warn"}`}>
        {liveSendEnabled() ? "Live Send ON" : "Live Send OFF"}
      </p>
      <span class="meta-line">Updated {updatedAtLabel()}</span>
      <span class="meta-line">Confidence {confidencePercent()}%</span>
    </div>
  </header>

  <section class="workspace">
    <section class="control-column" aria-live="polite">
      <section class="status-grid">
        <article class="status-card">
          <h2>Input Link</h2>
          <p class={`pill ${snapshot.inputConnected ? "ok" : "warn"}`}>
            {snapshot.inputConnected ? "Connected" : "Waiting"}
          </p>
          <span>{snapshot.inputSource}</span>
        </article>

        <article class="status-card">
          <h2>Output Backend</h2>
          <p class={`pill ${snapshot.outputSendFilterAllowed ? "ok" : "warn"}`}>
            {snapshot.outputSendFilterAllowed ? "Send Allowed" : "Send Blocked"}
          </p>
          <span>{outputBackendLabel()}</span>
          <span
            >Active process: {snapshot.outputSendFilterActiveProcess ??
              "-"}</span
          >
        </article>

        <article class="status-card">
          <h2>Tracking</h2>
          <p class={`pill ${snapshot.active ? "ok" : "warn"}`}>
            {snapshot.active ? "Active" : "Idle"}
          </p>
          <span
            >Yaw {snapshot.lookYawNormRaw.toFixed(3)} → {snapshot.lookYawNorm.toFixed(
              3,
            )}</span
          >
          <span
            >Pitch {snapshot.lookPitchNormRaw.toFixed(3)} → {snapshot.lookPitchNorm.toFixed(
              3,
            )}</span
          >
        </article>
      </section>

      <section class="deck-section runtime-panel">
        <h2>Runtime Controls</h2>

        <div class="toggle-stack">
          <label class="switch">
            <input
              type="checkbox"
              checked={liveSendEnabled()}
              on:change={onLiveSendToggle}
            />
            <span>Live Send</span>
          </label>
          <p class="hint">Clutch Shortcut: {snapshot.outputClutchHotkey}</p>

          <label class="switch">
            <input
              type="checkbox"
              checked={snapshot.persistSessionSettings}
              on:change={onPersistSessionSettingsToggle}
            />
            <span>終了時/起動時に設定値を保持/復元する</span>
          </label>
        </div>

        <div class="field-grid">
          <div class="control">
            <label for="input-source">Input Source</label>
            <select
              id="input-source"
              value={snapshot.inputSource}
              on:change={onInputSourceChange}
            >
              {#each INPUT_SOURCES as option}
                <option value={option.value}>{option.label}</option>
              {/each}
            </select>
          </div>

          <div class="control">
            <label for="output-backend">Output Backend</label>
            <select
              id="output-backend"
              value={snapshot.outputBackend}
              on:change={onOutputBackendChange}
            >
              {#each OUTPUT_BACKENDS as option}
                <option value={option.value}>{option.label}</option>
              {/each}
            </select>
          </div>
        </div>

        <div class="control compact">
          <label for="clutch-hotkey-input">Clutch Shortcut</label>
          <div class="row-inline">
            <input
              id="clutch-hotkey-input"
              type="text"
              placeholder="Ctrl+Shift+E"
              value={clutchHotkeyDraft}
              on:input={onClutchHotkeyInput}
            />
            <button
              type="button"
              class="action"
              disabled={!clutchHotkeyDirty}
              on:click={onApplyClutchHotkey}
            >
              Apply
            </button>
          </div>
        </div>

        <div class="control compact">
          <label for="clutch-hotkey-mode">Clutch Shortcut Mode</label>
          <select
            id="clutch-hotkey-mode"
            value={snapshot.outputClutchHotkeyMode}
            on:change={onClutchHotkeyModeChange}
          >
            {#each CLUTCH_HOTKEY_MODES as option}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
        </div>

        {#if snapshot.inputSource === "vmc_osc"}
          <div class="control compact">
            <label for="vmc-osc-port-input">VMC/OSC UDP Port</label>
            <div class="row-inline">
              <input
                id="vmc-osc-port-input"
                type="number"
                min={VMC_OSC_PORT_MIN}
                max={VMC_OSC_PORT_MAX}
                step="1"
                value={vmcOscPortDraft}
                on:input={onVmcOscPortInput}
              />
              <button
                type="button"
                class="action"
                disabled={!vmcOscPortDirty}
                on:click={onApplyVmcOscPort}
              >
                Apply
              </button>
            </div>
            <p class="hint">Current runtime port: {snapshot.vmcOscPort}</p>
          </div>

          <div class="control compact">
            <label class="switch">
              <input
                type="checkbox"
                checked={vmcOscPassthroughEnabledDraft}
                on:change={onVmcOscPassthroughEnabledToggle}
              />
              <span>VMC / OSC Passthrough</span>
            </label>
            <p class="hint">
              受信したVMC/OSC UDP datagramを複数ターゲットへそのまま転送
            </p>
          </div>

          <div class="control compact">
            <label for="vmc-osc-passthrough-mode"
              >VMC/OSC Passthrough Mode</label
            >
            <select
              id="vmc-osc-passthrough-mode"
              value={vmcOscPassthroughModeDraft}
              on:change={onVmcOscPassthroughModeChange}
            >
              {#each VMC_OSC_PASSTHROUGH_MODES as option}
                <option value={option.value}>{option.label}</option>
              {/each}
            </select>
          </div>

          <div class="control compact">
            <label for="vmc-osc-passthrough-targets"
              >VMC/OSC Passthrough Targets (one host:port per line)</label
            >
            <textarea
              id="vmc-osc-passthrough-targets"
              rows="3"
              placeholder="127.0.0.1:39539&#10;127.0.0.1:39541"
              value={vmcOscPassthroughTargetsDraft}
              on:input={onVmcOscPassthroughTargetsInput}
            ></textarea>
            <div class="row-inline">
              <p class="hint">
                Current targets: {snapshot.vmcOscPassthroughTargets.length}
              </p>
              <button
                type="button"
                class="action"
                disabled={!vmcOscPassthroughTargetsDirty}
                on:click={onApplyVmcOscPassthroughTargets}
              >
                Apply
              </button>
            </div>
          </div>
        {/if}

        <button class="recalibrate" disabled={busy} on:click={onRecalibrate}
          >Recalibrate Neutral Pose</button
        >
      </section>

      <section class="deck-section telemetry-panel">
        <h2>Axis Tuning (Instant Apply)</h2>

        <div class="axis-grid">
          <article
            class={`axis-card pitch-pos ${axisIsActive(snapshot.lookPitchNormRaw, snapshot.pitchPosInputDeadzone, snapshot.pitchPosInputRangeEnd, "positive") ? "axis-active" : "axis-inactive"}`}
          >
            <div class="axis-head">
              <span>Pitch+ (Up)</span>
              <output
                >{snapshot.lookPitchNormRaw.toFixed(3)} → {snapshot.lookPitchNorm.toFixed(
                  3,
                )}</output
              >
            </div>
            <div class="axis-range-control">
              <span class="axis-field-label">Input</span>
              <div
                class="axis-dual-range"
                style={axisRangeStyle(
                  pitchPosInputStartDraft,
                  pitchPosInputEndDraft,
                  AXIS_INPUT_RANGE_MIN,
                  AXIS_INPUT_RANGE_MAX,
                )}
              >
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={pitchPosInputStartDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (pitchPosInputStartDraft = value),
                    )}
                />
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={pitchPosInputEndDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (pitchPosInputEndDraft = value),
                    )}
                />
              </div>
              <div class="axis-number-pair">
                <input
                  class="axis-number"
                  type="number"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={pitchPosInputStartDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (pitchPosInputStartDraft = value),
                    )}
                /><input
                  class="axis-number"
                  type="number"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={pitchPosInputEndDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (pitchPosInputEndDraft = value),
                    )}
                />
              </div>
              <span class="axis-field-label">Output</span>
              <div
                class="axis-dual-range"
                style={axisRangeStyle(
                  pitchPosOutputStartDraft,
                  pitchPosOutputEndDraft,
                  AXIS_OUTPUT_RANGE_MIN,
                  AXIS_OUTPUT_RANGE_MAX,
                )}
              >
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={pitchPosOutputStartDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (pitchPosOutputStartDraft = value),
                    )}
                />
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={pitchPosOutputEndDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (pitchPosOutputEndDraft = value),
                    )}
                />
              </div>
              <div class="axis-number-pair">
                <input
                  class="axis-number"
                  type="number"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={pitchPosOutputStartDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (pitchPosOutputStartDraft = value),
                    )}
                /><input
                  class="axis-number"
                  type="number"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={pitchPosOutputEndDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (pitchPosOutputEndDraft = value),
                    )}
                />
              </div>
            </div>
            <p class="axis-caption">
              Input {formatRange(
                snapshot.pitchPosInputDeadzone,
                snapshot.pitchPosInputRangeEnd,
              )} → Output {formatRange(
                snapshot.pitchPosOutputRangeStart,
                snapshot.pitchPosOutputMultiplier,
                1,
              )}
            </p>
          </article>

          <article
            class={`axis-card yaw-neg ${axisIsActive(snapshot.lookYawNormRaw, snapshot.yawNegInputDeadzone, snapshot.yawNegInputRangeEnd, "negative") ? "axis-active" : "axis-inactive"}`}
          >
            <div class="axis-head">
              <span>Yaw- (Left)</span>
              <output
                >{snapshot.lookYawNormRaw.toFixed(3)} → {snapshot.lookYawNorm.toFixed(
                  3,
                )}</output
              >
            </div>
            <div class="axis-range-control">
              <span class="axis-field-label">Input</span>
              <div
                class="axis-dual-range"
                style={axisRangeStyle(
                  yawNegInputStartDraft,
                  yawNegInputEndDraft,
                  AXIS_INPUT_RANGE_MIN,
                  AXIS_INPUT_RANGE_MAX,
                )}
              >
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={yawNegInputStartDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (yawNegInputStartDraft = value),
                    )}
                /><input
                  class="axis-slider"
                  type="range"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={yawNegInputEndDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (yawNegInputEndDraft = value),
                    )}
                />
              </div>
              <div class="axis-number-pair">
                <input
                  class="axis-number"
                  type="number"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={yawNegInputStartDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (yawNegInputStartDraft = value),
                    )}
                /><input
                  class="axis-number"
                  type="number"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={yawNegInputEndDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (yawNegInputEndDraft = value),
                    )}
                />
              </div>
              <span class="axis-field-label">Output</span>
              <div
                class="axis-dual-range"
                style={axisRangeStyle(
                  yawNegOutputStartDraft,
                  yawNegOutputEndDraft,
                  AXIS_OUTPUT_RANGE_MIN,
                  AXIS_OUTPUT_RANGE_MAX,
                )}
              >
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={yawNegOutputStartDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (yawNegOutputStartDraft = value),
                    )}
                /><input
                  class="axis-slider"
                  type="range"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={yawNegOutputEndDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (yawNegOutputEndDraft = value),
                    )}
                />
              </div>
              <div class="axis-number-pair">
                <input
                  class="axis-number"
                  type="number"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={yawNegOutputStartDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (yawNegOutputStartDraft = value),
                    )}
                /><input
                  class="axis-number"
                  type="number"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={yawNegOutputEndDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (yawNegOutputEndDraft = value),
                    )}
                />
              </div>
            </div>
            <p class="axis-caption">
              Input {formatRange(
                snapshot.yawNegInputDeadzone,
                snapshot.yawNegInputRangeEnd,
              )} → Output {formatRange(
                snapshot.yawNegOutputRangeStart,
                snapshot.yawNegOutputMultiplier,
                1,
              )}
            </p>
          </article>

          <article
            class={`axis-card yaw-pos ${axisIsActive(snapshot.lookYawNormRaw, snapshot.yawPosInputDeadzone, snapshot.yawPosInputRangeEnd, "positive") ? "axis-active" : "axis-inactive"}`}
          >
            <div class="axis-head">
              <span>Yaw+ (Right)</span>
              <output
                >{snapshot.lookYawNormRaw.toFixed(3)} → {snapshot.lookYawNorm.toFixed(
                  3,
                )}</output
              >
            </div>
            <div class="axis-range-control">
              <span class="axis-field-label">Input</span>
              <div
                class="axis-dual-range"
                style={axisRangeStyle(
                  yawPosInputStartDraft,
                  yawPosInputEndDraft,
                  AXIS_INPUT_RANGE_MIN,
                  AXIS_INPUT_RANGE_MAX,
                )}
              >
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={yawPosInputStartDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (yawPosInputStartDraft = value),
                    )}
                /><input
                  class="axis-slider"
                  type="range"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={yawPosInputEndDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (yawPosInputEndDraft = value),
                    )}
                />
              </div>
              <div class="axis-number-pair">
                <input
                  class="axis-number"
                  type="number"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={yawPosInputStartDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (yawPosInputStartDraft = value),
                    )}
                /><input
                  class="axis-number"
                  type="number"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={yawPosInputEndDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (yawPosInputEndDraft = value),
                    )}
                />
              </div>
              <span class="axis-field-label">Output</span>
              <div
                class="axis-dual-range"
                style={axisRangeStyle(
                  yawPosOutputStartDraft,
                  yawPosOutputEndDraft,
                  AXIS_OUTPUT_RANGE_MIN,
                  AXIS_OUTPUT_RANGE_MAX,
                )}
              >
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={yawPosOutputStartDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (yawPosOutputStartDraft = value),
                    )}
                /><input
                  class="axis-slider"
                  type="range"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={yawPosOutputEndDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (yawPosOutputEndDraft = value),
                    )}
                />
              </div>
              <div class="axis-number-pair">
                <input
                  class="axis-number"
                  type="number"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={yawPosOutputStartDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (yawPosOutputStartDraft = value),
                    )}
                /><input
                  class="axis-number"
                  type="number"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={yawPosOutputEndDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (yawPosOutputEndDraft = value),
                    )}
                />
              </div>
            </div>
            <p class="axis-caption">
              Input {formatRange(
                snapshot.yawPosInputDeadzone,
                snapshot.yawPosInputRangeEnd,
              )} → Output {formatRange(
                snapshot.yawPosOutputRangeStart,
                snapshot.yawPosOutputMultiplier,
                1,
              )}
            </p>
          </article>

          <article
            class={`axis-card pitch-neg ${axisIsActive(snapshot.lookPitchNormRaw, snapshot.pitchNegInputDeadzone, snapshot.pitchNegInputRangeEnd, "negative") ? "axis-active" : "axis-inactive"}`}
          >
            <div class="axis-head">
              <span>Pitch- (Down)</span>
              <output
                >{snapshot.lookPitchNormRaw.toFixed(3)} → {snapshot.lookPitchNorm.toFixed(
                  3,
                )}</output
              >
            </div>
            <div class="axis-range-control">
              <span class="axis-field-label">Input</span>
              <div
                class="axis-dual-range"
                style={axisRangeStyle(
                  pitchNegInputStartDraft,
                  pitchNegInputEndDraft,
                  AXIS_INPUT_RANGE_MIN,
                  AXIS_INPUT_RANGE_MAX,
                )}
              >
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={pitchNegInputStartDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (pitchNegInputStartDraft = value),
                    )}
                /><input
                  class="axis-slider"
                  type="range"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={pitchNegInputEndDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (pitchNegInputEndDraft = value),
                    )}
                />
              </div>
              <div class="axis-number-pair">
                <input
                  class="axis-number"
                  type="number"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={pitchNegInputStartDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (pitchNegInputStartDraft = value),
                    )}
                /><input
                  class="axis-number"
                  type="number"
                  min={AXIS_INPUT_RANGE_MIN}
                  max={AXIS_INPUT_RANGE_MAX}
                  step={AXIS_INPUT_RANGE_STEP}
                  value={pitchNegInputEndDraft}
                  on:input={(event) =>
                    onAxisInputRangeInput(
                      event,
                      (value) => (pitchNegInputEndDraft = value),
                    )}
                />
              </div>
              <span class="axis-field-label">Output</span>
              <div
                class="axis-dual-range"
                style={axisRangeStyle(
                  pitchNegOutputStartDraft,
                  pitchNegOutputEndDraft,
                  AXIS_OUTPUT_RANGE_MIN,
                  AXIS_OUTPUT_RANGE_MAX,
                )}
              >
                <input
                  class="axis-slider"
                  type="range"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={pitchNegOutputStartDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (pitchNegOutputStartDraft = value),
                    )}
                /><input
                  class="axis-slider"
                  type="range"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={pitchNegOutputEndDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (pitchNegOutputEndDraft = value),
                    )}
                />
              </div>
              <div class="axis-number-pair">
                <input
                  class="axis-number"
                  type="number"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={pitchNegOutputStartDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (pitchNegOutputStartDraft = value),
                    )}
                /><input
                  class="axis-number"
                  type="number"
                  min={AXIS_OUTPUT_RANGE_MIN}
                  max={AXIS_OUTPUT_RANGE_MAX}
                  step={AXIS_OUTPUT_RANGE_STEP}
                  value={pitchNegOutputEndDraft}
                  on:input={(event) =>
                    onAxisOutputRangeInput(
                      event,
                      (value) => (pitchNegOutputEndDraft = value),
                    )}
                />
              </div>
            </div>
            <p class="axis-caption">
              Input {formatRange(
                snapshot.pitchNegInputDeadzone,
                snapshot.pitchNegInputRangeEnd,
              )} → Output {formatRange(
                snapshot.pitchNegOutputRangeStart,
                snapshot.pitchNegOutputMultiplier,
                1,
              )}
            </p>
          </article>
        </div>

        <div class="axis-advanced-grid">
          <article class="axis-advanced-card">
            <h3>Axis Direction</h3>
            <label class="switch">
              <input
                type="checkbox"
                checked={invertYawDraft}
                on:change={onInvertYawToggle}
              />
              <span>Yaw invert</span>
            </label>
            <label class="switch">
              <input
                type="checkbox"
                checked={invertPitchDraft}
                on:change={onInvertPitchToggle}
              />
              <span>Pitch invert</span>
            </label>
            <p class="axis-caption">
              入力方向を反転します。Pitch が逆向きに感じる場合は Pitch invert
              を有効化
            </p>
          </article>

          <article class="axis-advanced-card">
            <h3>Output Easing</h3>
            <label class="switch">
              <input
                type="checkbox"
                checked={outputEasingEnabledDraft}
                on:change={onOutputEasingEnabledToggle}
              />
              <span>Easing enabled</span>
            </label>
            <div class="axis-editor">
              <span class="axis-field-label">Alpha</span>
              <input
                id="output-easing-alpha-range"
                class="axis-slider"
                type="range"
                min={OUTPUT_EASING_ALPHA_MIN}
                max={OUTPUT_EASING_ALPHA_MAX}
                step={OUTPUT_EASING_ALPHA_STEP}
                value={outputEasingAlphaDraft}
                disabled={!outputEasingEnabledDraft}
                on:input={onOutputEasingAlphaInput}
              />
              <input
                id="output-easing-alpha-number"
                class="axis-number"
                type="number"
                min={OUTPUT_EASING_ALPHA_MIN}
                max={OUTPUT_EASING_ALPHA_MAX}
                step={OUTPUT_EASING_ALPHA_STEP}
                value={outputEasingAlphaDraft}
                disabled={!outputEasingEnabledDraft}
                on:input={onOutputEasingAlphaInput}
              />
            </div>
            <p class="axis-caption">
              alpha {snapshot.outputEasingAlpha.toFixed(
                2,
              )}（低いほど滑らか/遅い, 高いほど追従/速い）
            </p>
          </article>
        </div>

        <article class="axis-advanced-card">
          <h3>ETS2 / ATS Relative</h3>
          <div class="ets2-relative-grid">
            <div>
              <div class="axis-editor">
                <span class="axis-field-label">Speed</span>
                <input
                  id="ets2-relative-angular-velocity-range"
                  class="axis-slider"
                  type="range"
                  min={ETS2_RELATIVE_ANGULAR_VELOCITY_MIN}
                  max={ETS2_RELATIVE_ANGULAR_VELOCITY_MAX}
                  step={ETS2_RELATIVE_ANGULAR_VELOCITY_STEP}
                  value={ets2RelativeAngularVelocityDraft}
                  disabled={snapshot.outputBackend !== "ets2_relative"}
                  on:input={onEts2RelativeAngularVelocityInput}
                />
                <input
                  id="ets2-relative-angular-velocity-number"
                  class="axis-number"
                  type="number"
                  min={ETS2_RELATIVE_ANGULAR_VELOCITY_MIN}
                  max={ETS2_RELATIVE_ANGULAR_VELOCITY_MAX}
                  step={ETS2_RELATIVE_ANGULAR_VELOCITY_STEP}
                  value={ets2RelativeAngularVelocityDraft}
                  disabled={snapshot.outputBackend !== "ets2_relative"}
                  on:input={onEts2RelativeAngularVelocityInput}
                />
              </div>
              <p class="axis-caption">
                {snapshot.ets2RelativeAngularVelocityDegPerSec.toFixed(0)} deg/sec
              </p>
            </div>
            <div>
              <label class="switch">
                <input
                  type="checkbox"
                  checked={ets2RelativeAccumulationResetEnabledDraft}
                  disabled={snapshot.outputBackend !== "ets2_relative"}
                  on:change={onEts2RelativeAccumulationResetEnabledToggle}
                />
                <span>Auto return to center</span>
              </label>
              <div class="axis-editor">
                <span class="axis-field-label">Timeout</span>
                <input
                  id="ets2-relative-accumulation-reset-timeout-range"
                  class="axis-slider"
                  type="range"
                  min={ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_MIN}
                  max={ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_MAX}
                  step={ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_STEP}
                  value={ets2RelativeAccumulationResetTimeoutSecsDraft}
                  disabled={!ets2RelativeAccumulationResetEnabledDraft ||
                    snapshot.outputBackend !== "ets2_relative"}
                  on:input={onEts2RelativeAccumulationResetTimeoutInput}
                />
                <input
                  id="ets2-relative-accumulation-reset-timeout-number"
                  class="axis-number"
                  type="number"
                  min={ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_MIN}
                  max={ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_MAX}
                  step={ETS2_RELATIVE_ACCUMULATION_RESET_TIMEOUT_STEP}
                  value={ets2RelativeAccumulationResetTimeoutSecsDraft}
                  disabled={!ets2RelativeAccumulationResetEnabledDraft ||
                    snapshot.outputBackend !== "ets2_relative"}
                  on:input={onEts2RelativeAccumulationResetTimeoutInput}
                />
              </div>
              <p class="axis-caption">
                {snapshot.ets2RelativeAccumulationResetEnabled
                  ? `自動センター復帰: ON (${snapshot.ets2RelativeAccumulationResetTimeoutSecs.toFixed(2)}s)`
                  : "自動センター復帰: OFF"}
              </p>
              <div class="axis-editor">
                <span class="axis-field-label">Return speed</span>
                <input
                  id="ets2-relative-auto-return-angular-velocity-range"
                  class="axis-slider"
                  type="range"
                  min={ETS2_RELATIVE_ANGULAR_VELOCITY_MIN}
                  max={ETS2_RELATIVE_ANGULAR_VELOCITY_MAX}
                  step={ETS2_RELATIVE_ANGULAR_VELOCITY_STEP}
                  value={ets2RelativeAutoReturnAngularVelocityDraft}
                  disabled={!ets2RelativeAccumulationResetEnabledDraft ||
                    snapshot.outputBackend !== "ets2_relative"}
                  on:input={onEts2RelativeAutoReturnAngularVelocityInput}
                />
                <input
                  id="ets2-relative-auto-return-angular-velocity-number"
                  class="axis-number"
                  type="number"
                  min={ETS2_RELATIVE_ANGULAR_VELOCITY_MIN}
                  max={ETS2_RELATIVE_ANGULAR_VELOCITY_MAX}
                  step={ETS2_RELATIVE_ANGULAR_VELOCITY_STEP}
                  value={ets2RelativeAutoReturnAngularVelocityDraft}
                  disabled={!ets2RelativeAccumulationResetEnabledDraft ||
                    snapshot.outputBackend !== "ets2_relative"}
                  on:input={onEts2RelativeAutoReturnAngularVelocityInput}
                />
              </div>
              <p class="axis-caption">
                {snapshot.ets2RelativeAutoReturnAngularVelocityDegPerSec.toFixed(0)} deg/sec
              </p>
            </div>
          </div>
        </article>

        <article class="axis-advanced-card">
          <h3>Input Spike Rejection</h3>
          <label class="switch">
            <input
              type="checkbox"
              checked={spikeRejectionEnabledDraft}
              on:change={onSpikeRejectionEnabledToggle}
            />
            <span>Spike rejection enabled</span>
          </label>
          <p class="axis-caption">
            トラッキング失敗時に原点へ戻るガタツキを抑制（MediaPipe / Warudo
            環境で有効）
          </p>
        </article>
      </section>

      <section class="deck-section filter-panel">
        <div class="filter-head">
          <h2>Output Send Filter</h2>
          <button
            type="button"
            class="action ghost"
            disabled={processListBusy}
            on:click={refreshRunningProcessList}
          >
            {processListBusy ? "Loading..." : "Refresh Process List"}
          </button>
        </div>

        <div class="control">
          <label for="send-filter-mode">Mode</label>
          <select
            id="send-filter-mode"
            value={snapshot.outputSendFilterMode}
            on:change={onFilterModeChange}
          >
            {#each OUTPUT_FILTER_MODES as option}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
        </div>

        <div class="process-targets">
          <p class="label-mini">Allowed process names</p>
          {#if snapshot.outputSendFilterProcessNames.length === 0}
            <p class="process-empty">No process names configured.</p>
          {:else}
            <div class="process-chip-list">
              {#each snapshot.outputSendFilterProcessNames as processName (processName)}
                <button
                  type="button"
                  class="process-chip"
                  on:click={() => removeFilterProcess(processName)}
                  title="Remove process from send filter"
                >
                  <span>{processName}</span>
                  <span aria-hidden="true">x</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>

        <div class="process-add-row">
          <div class="control compact">
            <label for="running-process-select">Add from running process</label>
            <div class="row-inline">
              <select
                id="running-process-select"
                value={selectedRunningProcess}
                on:change={(event) =>
                  (selectedRunningProcess = (
                    event.currentTarget as HTMLSelectElement
                  ).value)}
              >
                <option value="" disabled={runningProcesses.length > 0}
                  >Choose process</option
                >
                {#each runningProcesses as processName}
                  <option value={processName}>{processName}</option>
                {/each}
              </select>
              <button
                type="button"
                class="action"
                on:click={onAddRunningProcess}>Add</button
              >
            </div>
          </div>

          <div class="control compact">
            <label for="manual-process-name">Add manual process name</label>
            <div class="row-inline">
              <input
                id="manual-process-name"
                type="text"
                placeholder="eurotrucks2.exe"
                value={processInput}
                on:input={(event) =>
                  (processInput = (event.currentTarget as HTMLInputElement)
                    .value)}
              />
              <button type="button" class="action" on:click={onAddManualProcess}
                >Add</button
              >
            </div>
          </div>
        </div>
      </section>

      {#if snapshot.lastError}
        <p class="error">Runtime warning: {snapshot.lastError}</p>
      {/if}

      {#if actionError}
        <p class="error">UI action failed: {actionError}</p>
      {/if}
    </section>

    <aside class="console-column">
      <section class="log-panel" aria-live="polite">
        <div class="log-toolbar">
          <p class="log-count">{logs.length} entries</p>
          <div class="log-actions">
            <button type="button" class="action" on:click={copyLogs}
              >Copy</button
            >
            <button type="button" class="action ghost" on:click={clearLogs}
              >Clear</button
            >
          </div>
        </div>

        {#if copyFeedback}
          <p class="copy-feedback">{copyFeedback}</p>
        {/if}

        <div class="log-list" role="log" aria-label="runtime logs">
          {#if logs.length === 0}
            <p class="log-empty">No log entries yet.</p>
          {:else}
            {#each [...logs].reverse() as entry (entry.id)}
              <article class={`log-entry ${entry.level}`}>
                <div class="log-meta">
                  <time>{logTimestampLabel(entry.timestampMs)}</time>
                  <span class={`log-level ${entry.level}`}
                    >{entry.level.toUpperCase()}</span
                  >
                  <span class="log-source">{entry.source}</span>
                </div>
                <p class="log-message">{entry.message}</p>
              </article>
            {/each}
          {/if}
        </div>
      </section>
    </aside>
  </section>
</main>
