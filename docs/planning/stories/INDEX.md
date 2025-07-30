# Gup Stories Index

This index provides an overview of all stories in the Gup project, organized by
epic and status.

## Epic 1: Core GPU Primitives and Selection API

### Phase 1 - Foundation (Stories 1-15)

| Story                                            | Title                      | Status      | Priority | Points |
| ------------------------------------------------ | -------------------------- | ----------- | -------- | ------ |
| [GUP-001](GUP-001_Build_Mixable_Trait.md)        | Build Mixable Trait        | ✅ Complete | High     | 3      |
| [GUP-002](GUP-002_Core_Selection_Type.md)        | Core Selection Type        | 📋 Planned  | High     | 2      |
| [GUP-003](GUP-003_GPU_Buffer_Management.md)      | GPU Buffer Management      | 📋 Planned  | High     | 4      |
| [GUP-004](GUP-004_Basic_Render_Context.md)       | Basic Render Context       | 📋 Planned  | High     | 3      |
| [GUP-005](GUP-005_Shader_Function_Trait.md)      | Shader Function Trait      | 📋 Planned  | Medium   | 4      |
| [GUP-006](GUP-006_WGSL_Function_Macro.md)        | WGSL Function Macro        | 📋 Planned  | Medium   | 3      |
| [GUP-007](GUP-007_Shader_Pipeline_Builder.md)    | Shader Pipeline Builder    | 📋 Planned  | High     | 5      |
| [GUP-008](GUP-008_Type_System_Integration.md)    | Type System Integration    | 📋 Planned  | Medium   | 4      |
| [GUP-009](GUP-009_Core_Mark_Trait.md)            | Core Mark Trait            | 📋 Planned  | High     | 3      |
| [GUP-010](GUP-010_Basic_Mark_Implementations.md) | Basic Mark Implementations | 📋 Planned  | High     | 4      |
| [GUP-011](GUP-011_Mark_Shader_Integration.md)    | Mark Shader Integration    | 📋 Planned  | High     | 5      |
| [GUP-012](GUP-012_GPU_Interaction_System.md)     | GPU Interaction System     | 📋 Planned  | Medium   | 4      |
| [GUP-013](GUP-013_Event_Handling_System.md)      | Event Handling System      | 📋 Planned  | Medium   | 3      |
| [GUP-014](GUP-014_Performance_Validation.md)     | Performance Validation     | 📋 Planned  | High     | 3      |
| [GUP-015](GUP-015_Real_Time_Data_Streaming.md)   | Real Time Data Streaming   | 📋 Planned  | Medium   | 5      |

### Phase 2 - Advanced Features (Stories 16-25)

| Story                                                          | Title                                    | Status      | Priority | Points |
| -------------------------------------------------------------- | ---------------------------------------- | ----------- | -------- | ------ |
| [GUP-016](GUP-016_Core_Accessibility_System.md)                | Core Accessibility System                | 📋 Planned  | High     | 4      |
| [GUP-017](GUP-017_Error_Handling_Framework.md)                 | Error Handling Framework                 | 📋 Planned  | High     | 3      |
| [GUP-018](GUP-018_Observable_Plot_Chart_Builders.md)           | Observable Plot Chart Builders           | 📋 Planned  | Medium   | 6      |
| [GUP-019](GUP-019_Mixable_Performance_Validation.md)           | Mixable Performance Validation           | 📋 Planned  | High     | 3      |
| [GUP-020](GUP-020_WebGPU_Integration_RenderContext.md)         | WebGPU Integration RenderContext         | ✅ Complete | High     | 5      |
| [GUP-021](GUP-021_Advanced_Composition_Mode_Implementation.md) | Advanced Composition Mode Implementation | ✅ Complete | High     | 6      |
| [GUP-022](GUP-022_Deep_Composition_Chain_Optimization.md)      | Deep Composition Chain Optimization      | 📋 Planned  | Medium   | 4      |
| [GUP-023](GUP-023_Mixable_Trait_Ecosystem_Integration.md)      | Mixable Trait Ecosystem Integration      | 📋 Planned  | Medium   | 5      |
| [GUP-024](GUP-024_Composition_Error_Recovery_Diagnostics.md)   | Composition Error Recovery Diagnostics   | 📋 Planned  | High     | 3      |
| [GUP-025](GUP-025_Async_Streaming_Composition_Support.md)      | Async Streaming Composition Support      | 📋 Planned  | Medium   | 5      |

### Future Improvements - Post GUP-021 (Stories 26+)

| Story                                                      | Title                                | Status | Priority | Points |
| ---------------------------------------------------------- | ------------------------------------ | ------ | -------- | ------ |
| [GUP-026](GUP-026_Data_Source_Merge_Implementation.md)     | Data Source Merge Implementation     | 💡 New | Medium   | 5      |
| [GUP-027](GUP-027_GPU_Blend_State_Integration.md)          | GPU Blend State Integration          | 💡 New | High     | 3      |
| [GUP-028](GUP-028_Composition_Performance_Optimization.md) | Composition Performance Optimization | 💡 New | Medium   | 4      |

## Story Status Legend

- ✅ **Complete**: Story fully implemented and tested
- 🚧 **In Progress**: Currently being worked on
- 📋 **Planned**: Ready for implementation
- 💡 **New**: Recently identified, needs planning
- ❌ **Blocked**: Cannot proceed due to dependencies
- 🔄 **On Hold**: Temporarily paused

## Dependencies Map

### Critical Path Stories

1. **GUP-001** → **GUP-020** → **GUP-021** ✅ _Foundation complete_
2. **GUP-021** → **GUP-027** → **GUP-028** 📋 _Next priority sequence_
3. **GUP-021** → **GUP-026** 📋 _Parallel development track_

### Foundation Dependencies

- **GUP-002, GUP-003, GUP-004**: Core infrastructure for most other stories
- **GUP-009, GUP-010, GUP-011**: Mark system foundation
- **GUP-007, GUP-008**: Shader system prerequisites

## Epic Progress

### Phase 1 Foundation

- **Completed**: 2/15 stories (13%)
- **Critical Path**: GUP-001 ✅, GUP-020 ✅, GUP-021 ✅

### Post-GUP-021 Improvements

- **Identified**: 3 new stories based on learnings
- **Next Priority**: GUP-027 (GPU Blend State Integration)

## Story Point Summary

- **Total Planned**: ~110 story points across all stories
- **Completed**: 14 story points (GUP-001: 3pts, GUP-020: 5pts, GUP-021: 6pts)
- **Progress**: ~13% of total scope

## Recent Additions (Post GUP-021)

The following stories were created based on learnings from implementing GUP-021:

### GUP-026: Data Source Merge Implementation

**Key Learning**: Current merge mode is placeholder - needs actual data
combination **Dependencies**: GUP-021 complete **Impact**: Enables true unified
visualizations from multiple datasets

### GUP-027: GPU Blend State Integration

**Key Learning**: BlendMode enum exists but not connected to GPU state
**Dependencies**: GUP-020, GUP-021 complete **Impact**: Proper alpha blending
for overlay compositions

### GUP-028: Composition Performance Optimization

**Key Learning**: Composition overhead can accumulate in complex scenarios
**Dependencies**: GUP-021, GUP-027 complete **Impact**: Maintains performance
for complex nested compositions

## Development Conventions

See [CONVENTIONS.md](../../CONVENTIONS.md) for key learnings and patterns
discovered during story implementation.

---

_Last Updated: After completion of GUP-021_ _Next Stories: GUP-027 (High
Priority), GUP-026 (Medium Priority), GUP-028 (Medium Priority)_
