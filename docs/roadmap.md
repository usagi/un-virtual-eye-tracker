# UNVET Implementation Roadmap

## Phase alpha

- [x] alpha-4 docs: define architecture and module boundaries
- [x] alpha-4 chore: scaffold workspace crates

## Phase beta

- [x] beta-1 chore: initialize workspace and base crates
- [x] beta-1 feat(core): add config loading
- [x] beta-1 feat(core): add structured logging
- [x] beta-1 feat(core): add unified error handling

## Next queue

- [x] beta-2 feat(input): implement iFacialMocap UDP receiver
- [x] beta-2 feat(input): add receiver state management
- [x] beta-2 test(input): add UDP frame parsing tests

- [x] beta-3 feat(input): implement iFacialMocap TCP receiver
- [x] beta-3 feat(input): add TCP frame reassembly
- [x] beta-3 test(input): add TCP stream parsing tests

- [x] beta-4 feat(core): add normalized TrackingFrame model updates
- [x] beta-4 feat(core): derive eye yaw/pitch from raw eye angles
- [x] beta-4 feat(core): add frame validity and confidence logic

- [x] gamma-1 feat(calib): add neutral pose calibration
- [x] gamma-1 feat(calib): persist calibration data
- [x] gamma-2 feat(map): add axis sensitivity and deadzone
- [x] gamma-2 feat(map): add response curve presets
- [x] gamma-3 feat(filter): add exponential smoothing
- [x] gamma-4 feat(map): add head-eye blend presets
- [x] gamma-4 feat(config): add per-game mapping profiles
- [x] delta-1 feat(output-mouse): implement relative mouse backend
- [x] delta-1 feat(output-mouse): add speed mapping and clamp
- [x] delta-2 feat(output-keyboard): implement directional key backend
- [x] delta-3 feat(output-ets2): implement ets2/ats dedicated backend
- [x] delta-3 feat(output-ets2): add truck sim response presets
- [x] delta-3 test(output-ets2): add backend mapping tests
- [x] delta-4 feat(output): add backend abstraction layer

- [x] epsilon-1 feat(ui): scaffold tauri + svelte desktop shell
- [x] epsilon-1 feat(ui): add runtime snapshot commands for UI polling
- [x] epsilon-1 feat(ui): add minimal runtime control panel

## Commit Unit Convention

- Prefix with phase id: `alpha-4`, `beta-1`, `gamma-2`
- Keep one functional objective per commit

## Compliance Gate (Compatibility Work)

- [x] legal-1 chore(docs): add clean-room compatibility policy
- [ ] legal-2 chore(process): require black-box observation note in compatibility PRs
- [x] legal-3 test(process): add lightweight guard to block proprietary-source references

## Recent Completed Work

- [x] zeta-1 feat(input): add VMC/OSC UDP receiver crate and runtime wiring
- [x] zeta-1 feat(ui): add VMC / OSC UDP input source selection in desktop UI
- [x] zeta-2 feat(ui): add runtime-editable VMC / OSC UDP port in desktop UI
- [x] eta-1 feat(compat): add NPClient-compatible shim and TrackIR helper bootstrap for ETS2/ATS
- [x] eta-1 feat(build): emit NPClient64.dll, NPClient.dll, and TrackIR.exe in desktop build outputs
- [x] eta-2 chore(release): add portable package script `tools/make-release-package.ps1`
- [x] eta-3 feat(compat): add `unvet-uninstall-compatible-layers.exe` for compatibility-layer cleanup

## Planned Next Release (v1.1.0)

- [ ] theta-1 feat(input): add VMC/OSC passthrough (raw UDP fan-out for multiple local targets)
- [ ] theta-1 feat(ui): add Desktop GUI editor for passthrough enabled/mode/targets
- Plan detail: `docs/release-1.1.0.md`
