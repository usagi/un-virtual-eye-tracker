# UNVET Architecture v0.1

## Scope

- Windows desktop application (Tauri + Rust runtime)
- Input: VMC/OSC UDP, iFacialMocap UDP/TCP
- Output: ETS2/ATS, relative pointer, absolute pointer, keyboard
- Compatibility: NPClient/TrackIR-compatible interop for ETS2/ATS
- Future extension: optional virtual XInput backend

## Layering

```text
Input Receiver -> Normalize Pipeline -> Output Backend -> UI/Config/Logging
```

Compatibility path (Windows ETS2/ATS):

```text
Runtime bootstrap -> NPClient-compatible shim + TrackIR helper -> Game interop
```

### apps/desktop/src-tauri

- Main desktop runtime loop
- Tauri commands (runtime snapshot and controls)
- Windows compatibility bootstrap (registry + helper process)
- Build hooks for compatibility binaries

### crates/unvet-cli

- CLI/runtime entrypoint for non-desktop execution paths
- Runtime wiring and polling loop integration
- CLI argument handling (config path)

### crates/core

- Common models (`TrackingFrame`, `OutputFrame`)
- Cross-crate traits (`InputReceiver`, `OutputBackend`)
- Shared error type
- Logging bootstrap

### crates/config

- Configuration schema
- Load/save/validation
- Profile/preset loading base

### crates/input-ifacialmocap

- iFacialMocap receiver state machine
- UDP/TCP receiver entry point
- Parser integration entry point

### crates/input-vmc-osc

- VMC/OSC UDP receiver
- Bone/BlendShape pose extraction and normalization bridge

### crates/output-ets2

- ETS2/ATS tuned mapping backend

### crates/output-mouse

- Relative mouse output backend

### crates/output-touch

- Absolute pointer output backend

### crates/output-keyboard

- 4-direction key output backend

### crates/output

- Unified output backend facade
- Send filter mode/process gating

### crates/ui

- Shared UI-facing model helpers (minimal)

## Dependency Direction

- `apps/desktop/src-tauri` and `unvet-cli` depend on feature crates
- `config`, `input-*`, `output-*`, `ui` depend on `core`
- `core` depends only on low-level libraries
- Feature crates coordinate through shared models/traits and `output` facade

## Clean-Room Interop Boundary

- Compatibility layers must be implemented from public behavior and open
 documentation only.
- No proprietary SDK headers or leaked material may be used as implementation
 sources.
- Vendor-specific exported symbol names may be implemented only where required
 for interoperability.
- Product/UI naming should stay vendor-neutral.
- See docs/clean-room-compatibility.md for the full policy and checklist.

## Release Packaging

- Portable package generation is automated by `tools/make-release-package.ps1`.
- Output format is `unvet-<version>.zip`, built from `target/release` artifacts.
- Compatibility artifacts in the package include `NPClient64.dll`, `NPClient.dll`, `TrackIR.exe`, and `unvet-uninstall-compatible-layers.exe`.

## Commit Granularity Rule

- Phase is the major work unit
- Sub-phase item (`alpha-1`, `beta-2`) is the commit unit
- One commit should change one behavior topic only
