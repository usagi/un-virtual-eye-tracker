# UNVET Desktop

Svelte5 + Vite + Tauri based control UI for UNVET.

## Purpose

This app provides epsilon-1 level controls and runtime visibility:

- Input source switching (UDP/TCP)
- Output backend switching (ETS2/Mouse/Keyboard)
- Output enable/disable and pause toggle
- Recalibration trigger
- Live runtime snapshot (yaw/pitch/confidence/connection state)

Core tracking/output processing remains in Rust crates; this desktop app acts as the control plane.

## Development

From `apps/desktop`:

```bash
npm install
npm run check
npm run build
```

Then launch from workspace root:

```bash
cargo run
```

This default path does not require a localhost dev server.

### Optional HMR (Vite + Tauri)

If you want live frontend reload:

```bash
npm run dev:tauri:hmr
```

## Build

```bash
npm run build
npm run build:tauri
```
