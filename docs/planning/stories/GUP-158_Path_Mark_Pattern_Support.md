# GUP-158: Path Mark Pattern Support

## Story Overview

**Title**: Extend Pattern Rendering to Path Marks  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Low  
**Story Points**: 3  
**Status**: 📋 Planned

## Context

GUP-157 added pattern support to Circle, Rectangle, Line, and BoxPlot marks, but Path marks were deferred because they use generated shaders rather than hand-written shaders. Path marks are important for complex visualizations (geographic boundaries, custom shapes, SVG-like graphics) and should support pattern rendering for complete accessibility coverage.

## User Story

**As a** data visualization developer  
**I want** pattern support on Path marks  
**So that** complex vector graphics can be accessible to colorblind users

## Acceptance Criteria

### AC1: Pattern Integration Architecture

- [ ] Decide approach: hand-written shaders vs pattern-aware shader generation
- [ ] Integrate pattern bind group with Path rendering pipeline
- [ ] Handle pattern world positions for tessellated paths
- [ ] Ensure pattern continuity across path segments

### AC2: Pattern Rendering Quality

- [ ] Patterns render correctly on straight path segments
- [ ] Patterns render correctly on curved path segments
- [ ] Pattern orientation follows path direction (optional enhancement)
- [ ] No visual artifacts at path joins or endpoints

### AC3: Testing & Validation

- [ ] Unit tests for Path pattern pipeline creation
- [ ] Tests for all pattern types on Path marks
- [ ] Visual validation of pattern rendering on paths
- [ ] Performance validation (pattern paths vs standard paths)

## Dependencies

### Prerequisite Stories

- GUP-113: Pattern-Based Rendering Implementation ✅
- GUP-157: Multi-Mark Pattern Support ✅

## Technical Tasks

- [ ] Analyze Path shader generation system
- [ ] Choose implementation approach (generated vs hand-written)
- [ ] Create path_pattern.frag.wgsl (if hand-written approach)
- [ ] Integrate pattern functions into generated shaders (if generated approach)
- [ ] Add PATTERN_FRAGMENT_SHADER to Path Mark implementation
- [ ] Handle tessellation coordinates for pattern world positions
- [ ] Add tests for Path pattern rendering
- [ ] Visual validation of pattern paths

## Success Metrics

- Path marks render patterns correctly
- Pattern appearance consistent with other mark types
- No performance regression vs standard path rendering
- Test coverage equivalent to other mark types

## Definition of Done

- [ ] Path marks support pattern rendering
- [ ] All pattern types work on paths
- [ ] Tests added and passing
- [ ] Performance validated
- [ ] Documentation updated

## Risk Assessment

**Technical Risks**:
- Path tessellation may complicate pattern world position calculation
- Generated shader system may need significant changes for pattern support
- Curved paths may show pattern artifacts

**Mitigation**:
- Start with simple straight-line paths to prove concept
- Consider hand-written shaders if generation proves too complex
- Use higher tessellation for curves if patterns show artifacts
