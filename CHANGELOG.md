# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project follows Semantic Versioning.

## [Unreleased]

### Added

- ETS2 / ATS Relative output backend now exposes a dedicated "Auto return speed" angular velocity setting that controls the speed at which the accumulated yaw/pitch are returned toward center when Auto return to center fires. The default is 60 deg/sec, slower than the regular Speed (120 deg/sec by default), avoiding the abrupt center-snap that could feel disorienting in v1.4.0.

### Fixed

- ETS2 / ATS Relative: the auto-return-to-center motion is no longer scaled to the full `Speed` value (which was much faster than the user's typical head-shake input speed). The auto-return now uses its own configurable angular velocity.

## [1.4.0] - 2026-05-03

### Changed

- Release current desktop and CLI build as v1.4.0.

## [1.1.0] - 2026-04-25

### Added

- VMC/OSC raw UDP passthrough feature (configurable targets, runtime controls, desktop GUI support).

### Changed

- VMC/OSC receive path now uses dedicated reader threading for smoother ingestion under load.
- Runtime output path avoids unnecessary backend apply work when output is not live.

## [1.0.0] - 2026-04-25

### Added

- Initial public OSS release baseline.
- Desktop runtime support for both iFacialMocap (UDP/TCP) and VMC/OSC input.
- Standalone CLI runtime binary: `unvet-cli`.
- Windows compatibility bootstrap for NPClient-compatible and TrackIR-compatible interop.
- Compatibility layer uninstall utility: `unvet-uninstall-compatible-layers.exe`.
- Clean-room compatibility policy documentation and guard script.
- Release packaging script to produce `unvet-<version>.zip` in `release-packages`.
- Basic CI workflows for Rust checks, frontend checks, and package builds.

### Changed

- Workspace crate entrypoint moved from the old `unvet-app` crate to `unvet-cli`.
- Default input source now targets VMC/OSC for initial runtime configuration.
- Packaging output defaults to repository-level `release-packages`.

### Removed

- Legacy `crates/app` entrypoint crate.
