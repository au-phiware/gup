# GUP-378: Tutorial Examples for Tutorials 2 and 3

## Story Overview

**Initiative**: Documentation **Status**: 📋 Planned **Created**: 2025-07-26

## Context

GUP-352 delivered dedicated examples for tutorials 1, 4, 5, and 6 but
intentionally excluded tutorials 2 (Data Binding) and 3 (Custom Shader
Functions) because the story's scope listed only four examples. Both tutorials
have "Full Example" code blocks that could benefit from standalone runnable
examples for completeness.

## User Story

> "As a developer who has completed Tutorials 2 or 3, I want to run a single
> command and see the tutorial's full example in action."

## Acceptance Criteria

- [ ] `tutorial02_data_binding.rs` exists in `examples/tutorials/` and matches
      the Full Example from Tutorial 2.
- [ ] `tutorial03_shader_functions.rs` exists in `examples/tutorials/` and
      matches the Full Example from Tutorial 3.
- [ ] Both examples compile via `cargo check --examples`.
- [ ] Tutorial 2 and 3 documents are updated to reference the new examples.

## Technical Tasks

- [ ] Implement `tutorial02_data_binding.rs` — data binding example from
      Tutorial 2.
- [ ] Implement `tutorial03_shader_functions.rs` — shader function example from
      Tutorial 3.
- [ ] Register both examples in `Cargo.toml`.
- [ ] Update tutorial documents with links.

## Dependencies

### Prerequisite Stories

- GUP-352: Interactive Tutorial Examples ✅ — Established the
  `examples/tutorials/` directory and pattern.
- GUP-281: Tutorial and Guide Suite ✅ — The tutorials whose examples to create.

## Testing Strategy

- Each example compiles via `cargo check --examples`.
- Unit tests validate data setup and API usage.

## Success Metrics

- [ ] Both tutorial examples exist and compile.
- [ ] Full tutorial-to-example coverage for all 6 tutorials.

## Risk Assessment

- **Low**: Both tutorials have headless Full Examples, so these will likely be
  headless console programs like tutorials 5 and 6.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Story status updated to ✅ Complete
