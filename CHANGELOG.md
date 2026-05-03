# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project follows Semantic Versioning.

## [Unreleased]

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
