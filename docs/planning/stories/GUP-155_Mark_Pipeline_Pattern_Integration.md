# GUP-155: Mark Pipeline Pattern Integration

## Story Overview

**Title**: Integrate Pattern Rendering into Mark Pipeline  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: High  
**Story Points**: 3  
**Status**: 📋 Planned

## Context

GUP-113 implemented the pattern rendering infrastructure (PatternUniforms, PatternRenderer, WGSL shaders), but patterns are not yet integrated into the actual mark rendering pipeline. Marks need to use pattern bind groups in their render passes for patterns to appear in visualizations.

## User Story

**As a** developer using pattern-based rendering  
**I want** patterns to actually appear on rendered marks  
**So that** colorblind users can distinguish data categories visually

## Acceptance Criteria

### AC1: Render Pass Integration

- [ ] PatternRenderer bind group added to mark render passes
- [ ] Pattern uniforms updated before each render
- [ ] Pattern mode toggleable per mark instance
- [ ] Fallback to standard rendering when patterns disabled

### AC2: Pipeline Configuration

- [ ] Mark pipelines include pattern bind group layout
- [ ] Pattern shader variants compiled and cached
- [ ] Shader switching between pattern and standard modes
- [ ] Performance overhead minimal (<5ms)

### AC3: API Integration

- [ ] Marks expose pattern configuration API
- [ ] ContrastMode::Pattern triggers pattern rendering
- [ ] Pattern parameters configurable per mark
- [ ] Documentation for pattern usage

## Dependencies

### Prerequisite Stories

- GUP-113: Pattern-Based Rendering Implementation ✅

## Technical Tasks

- [ ] Add pattern bind group to mark render passes
- [ ] Update mark shaders to use pattern uniforms
- [ ] Implement pattern mode switching in renderer
- [ ] Add pattern configuration to mark API
- [ ] Test pattern rendering with different mark types
- [ ] Validate performance targets

## Success Metrics

- Patterns render correctly on all mark types
- <5ms rendering overhead for pattern mode
- Seamless switching between pattern/standard modes
- Zero visual artifacts

## Definition of Done

- [ ] Patterns appear on rendered marks
- [ ] All mark types support patterns
- [ ] Pattern mode switching works
- [ ] Tests for pattern integration
- [ ] Example showing pattern rendering
- [ ] Performance validation complete
