# Contributing

Thank you for contributing to UNVET.

## Ground Rules

- Keep changes small and focused.
- Prefer one functional objective per commit.
- Follow clean-room rules for compatibility code. Read `docs/clean-room-compatibility.md`.
- Do not include proprietary SDK headers, leaked code, or unclear-license sources.

## Development Setup

### Prerequisites

- Rust stable toolchain
- Node.js 20+
- Windows is the primary target platform

### Install frontend dependencies

```powershell
cd apps/desktop
npm ci
```

## Required Local Checks

Run these before opening a pull request.

```powershell
cargo fmt --all -- --check
cargo test --workspace
./tools/check-clean-room.ps1
```

```powershell
cd apps/desktop
npm run check
```

Optional package validation:

```powershell
cd ..\..
./tools/make-release-package.ps1
```

## Pull Request Checklist

- Explain what changed and why.
- Link related issue(s) if available.
- Include test evidence (command output summary is enough).
- For compatibility-layer changes, include:
  - observation source,
  - assumptions made,
  - intentionally unimplemented items.

## Commit Guidance

The repository roadmap defines preferred commit granularity in `docs/roadmap.md`.

## Code Style

- Rust formatting is enforced with `cargo fmt`.
- Keep module boundaries clear: input, core mapping/filtering, output backends, UI/runtime.
- Add tests when behavior changes or bug fixes are introduced.
