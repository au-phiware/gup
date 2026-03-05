# GUP-282B: Gallery Config Sync Tool

## Story Overview

**Initiative**: Documentation **Status**: ✅ Complete **Created**: 2025-07-26

## Context

GUP-282 introduced `scripts/gallery_config.toml` as the canonical list of
examples for the gallery. However, `examples/INDEX.md` and the `[[example]]`
entries in `Cargo.toml` are separate sources of truth. When a contributor adds a
new example, they might update one but not the others, causing drift.

This story creates a validation script that compares the three sources and
reports any inconsistencies.

## User Story

> "As a contributor adding a new example, I want CI to warn me if I forgot to
> add it to the gallery config so that the gallery always reflects the full set
> of examples."

## Acceptance Criteria

- [x] A script (`scripts/check_gallery_sync.sh` or equivalent) compares examples
      listed in `gallery_config.toml`, `examples/INDEX.md`, and `Cargo.toml`
      `[[example]]` entries.
- [x] The script reports examples present in one source but missing from
      another.
- [x] The script is integrated into the gallery CI workflow.
- [x] The script exits non-zero if any drift is detected.

## Dependencies

### Prerequisite Stories

- GUP-282: Example Gallery ✅ — provides `gallery_config.toml`.

## Testing Strategy

- Add a deliberately missing example to test detection.
- Run the script against the current codebase to verify zero drift.

## Risk Assessment

- **Low**: Shell-based TOML / Markdown parsing may miss edge cases. Mitigate
  with explicit test cases.

## Definition of Done

- [x] Sync check script passes on current codebase
- [x] Script integrated into gallery CI workflow
- [x] Story status updated to ✅ Complete

## Implementation Summary

**Completed**: 2025-07-27

### What Was Implemented

1. **`scripts/check_gallery_sync.sh`** — Bash script that compares example names
   across three sources: `gallery_config.toml`, `examples/INDEX.md`, and Cargo
   examples (explicit `[[example]]` entries plus auto-discovered `examples/*.rs`
   files). Reports all six pair-wise differences and exits non-zero on drift.

2. **CI integration** — Added a "Check gallery config sync" step to
   `.github/workflows/gallery.yml`, running before thumbnail generation.

3. **Fixed existing drift** — Added 9 missing examples to `gallery_config.toml`
   (5 filesystem examples + 4 tutorial examples) and added 42 missing examples
   to `examples/INDEX.md` with 3 new sections (Export, Intermediate, Tutorials).

### Key Files Changed

- `scripts/check_gallery_sync.sh` (new, 109 lines)
- `scripts/gallery_config.toml` (added 9 entries)
- `examples/INDEX.md` (added 42 entries, 3 new sections)
- `.github/workflows/gallery.yml` (added sync check step)
