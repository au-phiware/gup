---
implementer: sonnet
implementer-reason: an investigation with a seeded test
touches: [scripts/]
---

# GUP-605: Reproduce the Publish Check Shared Directory Failure

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 💡 New
**Created**: 2026-11-04

## Context

GUP-603 made `mask publish-check` use a build directory private to the checkout,
after `no hash listed for gup-core-macros v0.1.0` appeared twice with the shared
one. The failure could not be reproduced: stale or cksum-less index entries, a
second tree with the same version and other contents, and concurrent runs all
passed. The cause is probable, not proven. Do this only if the failure returns
or the private directory's cost matters.

## User Story

> "As the maintainer, I want to know why the shared directory failed, so that I
> know the private one is the right fix."

## Acceptance Criteria

- [ ] The cause is found and written down, with a seeded test that fails on the
      old command and passes on `scripts/publish_check.sh`, or the story is
      closed as not reproducible.

## Dependencies

### Prerequisite Stories

- GUP-603 ✅

## Testing Strategy

Try a changed `Cargo.lock`, a cargo upgrade between runs, and two users.

## Risk Assessment

- **Low**: release tooling only.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` clean
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
