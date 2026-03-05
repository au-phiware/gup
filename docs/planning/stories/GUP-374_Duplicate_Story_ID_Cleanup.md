# GUP-374: Duplicate Story ID Cleanup

## Story Overview

**Initiative**: Project Maintenance **Status**: 📋 Planned **Created**:
2025-07-22

## Context

During implementation of GUP-315 (3D Axis and Grid), a duplicate story ID was
discovered: GUP-315 is used by both "Graph Node Label Rendering" and "3D Axis
and Grid". The INDEX.md has two entries with the same GUP-315 identifier. This
creates confusion and should be resolved by renumbering one of the stories.

## User Story

> "As a project maintainer, I want unique story IDs so that references are
> unambiguous and the story index is reliable."

## Acceptance Criteria

- [ ] All story IDs in INDEX.md are unique
- [ ] The renamed story file is updated with the new ID
- [ ] Any cross-references to the renamed story are updated
- [ ] No broken links in INDEX.md

## Technical Tasks

- [ ] Identify which GUP-315 to renumber (likely "Graph Node Label Rendering"
      since "3D Axis and Grid" is now complete)
- [ ] Rename the story file and update its header
- [ ] Update INDEX.md entries
- [ ] Search for and update any cross-references in other story files

## Dependencies

None.

## Testing Strategy

- Verify INDEX.md has no duplicate IDs
- Verify all story file links resolve

## Risk Assessment

- **Low**: Simple renaming operation. Main risk is missing a cross-reference.

## Definition of Done

- [ ] All story IDs are unique
- [ ] No broken links
- [ ] Story status updated in INDEX.md
