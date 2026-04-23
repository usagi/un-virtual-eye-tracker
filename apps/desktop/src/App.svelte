<script lang="ts">
 import { onMount } from 'svelte'
 import {
  getRuntimeSnapshot,
  requestRecalibration,
  setInputSource,
  setOutputBackend,
  setOutputEnabled,
  setPaused,
  type InputSource,
  type OutputBackendKind,
  type RuntimeSnapshot,
 } from './lib/runtime'

 const INPUT_SOURCES: Array<{ value: InputSource; label: string }> = [
  { value: 'ifacialmocap_udp', label: 'iFacialMocap UDP' },
  { value: 'ifacialmocap_tcp', label: 'iFacialMocap TCP' },
 ]

 const OUTPUT_BACKENDS: Array<{ value: OutputBackendKind; label: string }> = [
  { value: 'ets2', label: 'ETS2 / ATS' },
  { value: 'mouse', label: 'Relative Mouse' },
  { value: 'keyboard', label: 'Keyboard 4-way' },
 ]

 const EMPTY_SNAPSHOT: RuntimeSnapshot = {
  inputConnected: false,
  outputEnabled: false,
  paused: false,
  inputSource: 'ifacialmocap_udp',
  outputBackend: 'ets2',
  lookYawNorm: 0,
  lookPitchNorm: 0,
  confidence: 0,
  active: false,
  lastError: null,
  updatedAtMs: 0,
 }

 let snapshot: RuntimeSnapshot = EMPTY_SNAPSHOT
 let busy = true
 let actionError = ''
 let poller: number | undefined

 const confidencePercent = () => Math.round(snapshot.confidence * 100)
 const updatedAtLabel = () => {
  if (snapshot.updatedAtMs <= 0) {
   return '-'
  }
  return new Date(snapshot.updatedAtMs).toLocaleTimeString()
 }

 async function refreshSnapshot() {
  try {
   snapshot = await getRuntimeSnapshot()
   busy = false
  } catch (error) {
   actionError = String(error)
  }
 }

 async function applyAction(action: () => Promise<void>) {
  try {
   actionError = ''
   await action()
   await refreshSnapshot()
  } catch (error) {
   actionError = String(error)
  }
 }

 function onOutputToggle(event: Event) {
  const enabled = (event.currentTarget as HTMLInputElement).checked
  void applyAction(() => setOutputEnabled(enabled))
 }

 function onPauseToggle(event: Event) {
  const paused = (event.currentTarget as HTMLInputElement).checked
  void applyAction(() => setPaused(paused))
 }

 function onInputSourceChange(event: Event) {
  const source = (event.currentTarget as HTMLSelectElement).value as InputSource
  void applyAction(() => setInputSource(source))
 }

 function onOutputBackendChange(event: Event) {
  const backend = (event.currentTarget as HTMLSelectElement).value as OutputBackendKind
  void applyAction(() => setOutputBackend(backend))
 }

 function onRecalibrate() {
  void applyAction(() => requestRecalibration())
 }

 onMount(() => {
  void refreshSnapshot()
  poller = window.setInterval(() => {
   void refreshSnapshot()
  }, 120)

  return () => {
   if (poller !== undefined) {
    window.clearInterval(poller)
   }
  }
 })
</script>

<main class="shell">
 <header class="hero">
  <p class="eyebrow">UNVET Desktop</p>
  <h1>Eyes-on Runtime Console</h1>
  <p class="summary">
   ランタイム状態を監視しつつ、入力ソース・出力バックエンド・ON/OFF を即時切替できます。
  </p>
 </header>

 <section class="status-grid" aria-live="polite">
  <article class="status-card">
   <h2>Input Link</h2>
   <p class={`pill ${snapshot.inputConnected ? 'ok' : 'warn'}`}>
    {snapshot.inputConnected ? 'Connected' : 'Waiting'}
   </p>
   <span>Source: {snapshot.inputSource}</span>
  </article>

  <article class="status-card">
   <h2>Output</h2>
   <p class={`pill ${snapshot.outputEnabled ? 'ok' : 'muted'}`}>
    {snapshot.outputEnabled ? 'Enabled' : 'Disabled'}
   </p>
   <span>Backend: {snapshot.outputBackend}</span>
  </article>

  <article class="status-card">
   <h2>Tracking</h2>
   <p class={`pill ${snapshot.active ? 'ok' : 'warn'}`}>
    {snapshot.active ? 'Active' : 'Idle'}
   </p>
   <span>Confidence: {confidencePercent()}%</span>
  </article>
 </section>

 <section class="telemetry">
  <div class="metric">
     <span class="metric-label">Yaw</span>
   <output>{snapshot.lookYawNorm.toFixed(3)}</output>
  </div>
  <div class="metric">
     <span class="metric-label">Pitch</span>
   <output>{snapshot.lookPitchNorm.toFixed(3)}</output>
  </div>
  <div class="metric">
     <span class="metric-label">Updated</span>
   <output>{updatedAtLabel()}</output>
  </div>
 </section>

 <section class="controls">
  <div class="control">
   <label for="input-source">Input Source</label>
   <select id="input-source" value={snapshot.inputSource} on:change={onInputSourceChange}>
    {#each INPUT_SOURCES as option}
     <option value={option.value}>{option.label}</option>
    {/each}
   </select>
  </div>

  <div class="control">
   <label for="output-backend">Output Backend</label>
   <select id="output-backend" value={snapshot.outputBackend} on:change={onOutputBackendChange}>
    {#each OUTPUT_BACKENDS as option}
     <option value={option.value}>{option.label}</option>
    {/each}
   </select>
  </div>

  <div class="toggles">
   <label class="switch">
    <input type="checkbox" checked={snapshot.outputEnabled} on:change={onOutputToggle} />
    <span>Output Enabled</span>
   </label>

   <label class="switch">
    <input type="checkbox" checked={snapshot.paused} on:change={onPauseToggle} />
    <span>Pause Runtime</span>
   </label>
  </div>

  <button class="recalibrate" disabled={busy} on:click={onRecalibrate}>
   Recalibrate Neutral Pose
  </button>
 </section>

 {#if snapshot.lastError}
  <p class="error">Runtime warning: {snapshot.lastError}</p>
 {/if}

 {#if actionError}
  <p class="error">UI action failed: {actionError}</p>
 {/if}
</main>
