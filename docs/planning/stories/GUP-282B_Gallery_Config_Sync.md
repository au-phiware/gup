# GUP-282B: Gallery Config Sync Tool

## Story Overview

**Initiative**: Documentation **Status**: 🚧 In Progress **Created**: 2025-07-26

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

- [ ] A script (`scripts/check_gallery_sync.sh` or equivalent) compares examples
      listed in `gallery_config.toml`, `examples/INDEX.md`, and `Cargo.toml`
      `[[example]]` entries.
- [ ] The script reports examples present in one source but missing from
      another.
- [ ] The script is integrated into the gallery CI workflow.
- [ ] The script exits non-zero if any drift is detected.

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

- [ ] Sync check script passes on current codebase
- [ ] Script integrated into gallery CI workflow
- [ ] Story status updated to ✅ Complete
