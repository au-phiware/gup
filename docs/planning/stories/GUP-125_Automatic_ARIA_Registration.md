# GUP-125: Automatic ARIA Registration

## Story Overview

**Title**: Automatic ARIA Registration for Selections  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Medium  
**Story Points**: 3  
**Status**: 💡 New

## Context

GUP-111 implemented `generate_aria_tree()` method for Selections but requires
manual registration with AccessibilitySystem. This adds developer friction and
may lead to visualizations without accessible descriptions if developers forget
to register.

Following the "accessibility by default" principle, Selections should
automatically register their ARIA trees with the accessibility system when
rendered, ensuring all visualizations are accessible without additional effort.

## User Story

**As a** developer using Gup  
**I want** Selections to automatically register ARIA trees  
**So that** my visualizations are accessible without manual registration code

## Acceptance Criteria

### AC1: Automatic Registration on Render

- [ ] Selection registers ARIA tree on first render
- [ ] Registration updates when data changes
- [ ] No duplicate registrations for same selection
- [ ] Opt-out mechanism for developers who need manual control

### AC2: Lifecycle Management

- [ ] ARIA tree de-registered when selection is dropped
- [ ] Updates propagate to registered tree
- [ ] Handle selection cloning correctly
- [ ] Clean up on error/panic

### AC3: Integration with RenderContext

- [ ] RenderContext provides access to AccessibilitySystem
- [ ] Thread-safe registration (Arc/Mutex as needed)
- [ ] No performance regression (<5ms overhead)
- [ ] Works with composition (ComposedVisualization)

## Dependencies

### Prerequisite Stories

- GUP-111: Automatic ARIA Generation ✅
- GUP-016: Core Accessibility System ✅

### Enables Stories

- True "accessibility by default" for all visualizations
- Simplified developer experience

## Technical Tasks

- [ ] Add `AccessibilitySystem` reference to `RenderContext`
- [ ] Implement auto-registration in `Selection::render()`
- [ ] Add selection lifetime tracking (weak references or IDs)
- [ ] Implement ARIA tree update on data changes
- [ ] Add opt-out flag/method for manual control
- [ ] Update all marks to work with auto-registration

## Success Metrics

- Zero manual ARIA registration in examples
- No memory leaks from registration lifecycle
- <5ms overhead for registration
- 100% of rendered visualizations have ARIA

## Definition of Done

- [ ] Auto-registration works on render
- [ ] Lifecycle management prevents leaks
- [ ] Opt-out mechanism available
- [ ] Tests validate registration and cleanup
- [ ] Examples updated to remove manual registration
- [ ] Performance benchmarks show acceptable overhead
