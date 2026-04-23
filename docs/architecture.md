# UNVET Architecture v0.1

## Scope

- Windows single application
- Input first: iFacialMocap UDP and TCP
- Output first: ETS2/ATS, mouse, keyboard
- Future extension: VMC input and optional virtual XInput

## Layering

```text
Input Receiver -> Normalize Pipeline -> Output Backend -> UI/Config/Logging
```

### crates/app

- Process entrypoint
- Runtime wiring
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

### crates/output-ets2

- ETS2/ATS tuned mapping backend

### crates/output-mouse

- Relative mouse output backend

### crates/output-keyboard

- 4-direction key output backend

### crates/ui

- UI state models
- Future window/tray layer

## Dependency Direction

- `app` depends on all feature crates
- `config`, `input-*`, `output-*`, `ui` depend on `core`
- `core` depends only on low-level libraries
- Feature crates do not depend on each other directly

## Commit Granularity Rule

- Phase is the major work unit
- Sub-phase item (`alpha-1`, `beta-2`) is the commit unit
- One commit should change one behavior topic only