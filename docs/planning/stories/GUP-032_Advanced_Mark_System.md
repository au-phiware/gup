# GUP-032: Advanced Mark System with Custom Shapes

## Story Overview

**Title**: Extend Mark System Beyond Basic Circles to Support Complex Shapes  
**Epic**: Phase 1 Initiative 3 - Visual Mark Diversity  
**Priority**: Medium  
**Story Points**: 8

## Context

GUP-002 implemented a basic `Circle` mark for proof-of-concept. Real
visualizations need diverse mark types: rectangles, lines, complex paths, custom
SVG-like shapes, and procedurally generated marks.

## User Story

**As a** visualization developer  
**I want** to use diverse mark types (rectangles, lines, paths, custom shapes)  
**So that** I can create rich, varied visualizations beyond simple scatter plots

## Acceptance Criteria

### AC1: Core Mark Types

- [ ] `Rectangle` mark with width/height/corner radius
- [ ] `Line` mark with start/end points and stroke properties
- [ ] `Path` mark supporting SVG-like path commands
- [ ] `Text` mark with font, size, and alignment options

### AC2: Mark Composition System

- [ ] `CompositeMark` for combining multiple marks
- [ ] Mark transformation pipeline (scale, rotate, translate)
- [ ] Hierarchical mark relationships
- [ ] Mark templates and reusable components

### AC3: Custom Mark Definition

- [ ] Trait system for user-defined marks
- [ ] WGSL shader generation for custom marks
- [ ] Instancing support for performance
- [ ] Attribute mapping for custom properties

### AC4: Performance Optimization

- [ ] Instanced rendering for identical marks
- [ ] GPU-based mark culling and LOD
- [ ] Batch rendering of heterogeneous marks
- [ ] Memory-efficient mark storage

## Technical Requirements

- Support for both 2D and 3D marks
- Efficient GPU representation
- Antialiased rendering quality
- Consistent attribute mapping system

## Dependencies

- **Requires**: GUP-002 (Core Selection Type) - ✅ Complete
- **Requires**: GUP-029 (WGSL Shader Code Generation)
- **Enables**: Rich visualization diversity

## Success Metrics

- [ ] Support 8+ distinct mark types
- [ ] Custom mark definition in <50 lines of code
- [ ] Rendering performance within 10% of Circle mark
- [ ] High-quality antialiased output

## Risk Assessment

**Medium Risk**: Complex marks may require sophisticated WGSL shader generation
and GPU resource management.

---

_Created from GUP-002 retrospective learnings about Mark trait system
extensibility needs._
