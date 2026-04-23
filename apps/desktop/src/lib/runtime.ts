import { invoke } from '@tauri-apps/api/core'

export type InputSource = 'ifacialmocap_udp' | 'ifacialmocap_tcp'
export type OutputBackendKind = 'ets2' | 'mouse' | 'keyboard'

export type RuntimeSnapshot = {
  inputConnected: boolean
  outputEnabled: boolean
  paused: boolean
  inputSource: InputSource
  outputBackend: OutputBackendKind
  lookYawNorm: number
  lookPitchNorm: number
  confidence: number
  active: boolean
  lastError: string | null
  updatedAtMs: number
}

const invokeRuntime = <T>(command: string, args?: Record<string, unknown>) =>
  invoke<T>(command, args)

export const getRuntimeSnapshot = () =>
  invokeRuntime<RuntimeSnapshot>('get_runtime_snapshot')

export const setOutputEnabled = (enabled: boolean) =>
  invokeRuntime<void>('set_output_enabled', { enabled })

export const setPaused = (paused: boolean) =>
  invokeRuntime<void>('set_paused', { paused })

export const setInputSource = (source: InputSource) =>
  invokeRuntime<void>('set_input_source', { source })

export const setOutputBackend = (backend: OutputBackendKind) =>
  invokeRuntime<void>('set_output_backend', { backend })

export const requestRecalibration = () =>
  invokeRuntime<void>('request_recalibration')
