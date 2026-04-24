# Clean-Room Compatibility Policy

## Purpose

UNVET is planned as publicly released OSS under MIT terms. Any game interop
component must be implemented as clean-room, black-box compatibility.

This policy is an engineering rule for contributors. It is not legal advice.

## Allowed Reference Sources

- Public protocol descriptions and public API contracts
- Public operating system documentation (for example Win32 API docs)
- Behavioral observation from running software as a black box
- Open-source implementations used as behavioral references only
- Our own logs, tests, and packet/shared-memory observations

## Forbidden Reference Sources

- Proprietary SDK headers, source, or private documentation
- Leaked code, leaked symbols, or reverse-engineered private internals
- Decompiled proprietary binaries used as copy sources
- Any material with unclear licensing/provenance

## Implementation Rules

- Keep module naming vendor-neutral in product code and UI text.
  - Preferred wording: "NPClient-compatible layer" or "head-tracking
    compatibility layer".
- Exported function names required for interoperability are allowed when they
  are part of the public calling surface expected by games.
- Interop must be based on observed call patterns and public behavior only.
- Do not implement bypasses for DRM, anti-cheat, cryptographic protection, or
  signature checks.
- If a field is optional in observed behavior, prefer safe minimal behavior
  (no-op, zero, or explicit unsupported) over speculative emulation.
- In comments, document behavior assumptions and observation basis.

## Review Checklist (Required for Compatibility PRs)

- Confirm no proprietary SDK/leak-derived material was consulted.
- Confirm compatibility is black-box and behavior-driven.
- Confirm all newly added third-party references have clear open licenses.
- Confirm no vendor trademark is used as product branding in UI/docs.
- Confirm tests validate behavior without copying proprietary constants/tables.

## Repository Guardrails

- Changes that touch compatibility code must include a short note in the PR
  body stating:
  - observation source,
  - assumptions made,
  - what was intentionally left unimplemented.
- Run tools/check-clean-room.ps1 before submitting compatibility-related
  changes.
