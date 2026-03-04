# GUP-273: cbindgen Integration for iOS/Android FFI

## Story Overview

**Initiative**: Mobile **Status**: 📋 Planned **Created**: 2025-07-24

## Context

GUP-270 (iOS) and GUP-271 (Android, planned) define C ABI entry points in Rust
that are consumed by Swift (@\_silgen_name) and Kotlin/Java (JNI). The bridge
declarations are manually written and must be kept in sync with the Rust
`#[unsafe(no_mangle)]` functions.

This story introduces `cbindgen` to auto-generate a C header file from the Rust
FFI crates, ensuring type-safety and eliminating manual synchronisation errors.

## User Story

> "As a mobile platform integrator, I want the C header for the Gup FFI to be
> auto-generated from the Rust source so that I never have a mismatch between
> the Rust functions and the Swift/Kotlin declarations."

## Acceptance Criteria

- [ ] `cbindgen` is configured for `gup-ios` (and `gup-android` when it exists)
- [ ] A `gup_ios.h` header is generated as part of the build
- [ ] The Swift bridge imports the generated header via a module map
- [ ] CI verifies the generated header is up-to-date

## Dependencies

### Prerequisite Stories

- GUP-270: iOS Platform Support ✅
- GUP-271: Android Platform Support 📋 (optional — can proceed with iOS only)

## Testing Strategy

- CI step that runs `cbindgen` and diffs against committed header
- Swift package builds against the generated header

## Definition of Done

- [ ] `cbindgen.toml` configured and committed
- [ ] Generated header committed and CI-verified
- [ ] Swift package uses generated header instead of `@_silgen_name`
