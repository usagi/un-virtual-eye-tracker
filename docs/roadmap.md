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
- [ ] delta-4 feat(output): add backend abstraction layer

## Commit Unit Convention

- Prefix with phase id: `alpha-4`, `beta-1`, `gamma-2`
- Keep one functional objective per commit
