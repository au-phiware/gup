# GUP-129: GPU Interaction Debug Visualization Tool

**Status**: 💡 New

## Story Overview

**Title**: Create GPU Interaction Debug Visualization Tool  
**Epic**: Phase 1 Initiative 4 - Interaction System  
**Priority**: Medium  
**Story Points**: 8

## Context

During GUP-128 debugging, it was challenging to visualize what data was being
uploaded to GPU buffers and whether hit testing was working correctly. Debug
logging helped identify the issue, but a visual tool would have made diagnosis
much faster.

## User Story

**As a** Gup developer debugging GPU interaction issues  
**I want** a visual debugging tool that shows element positions, query
locations, and hit test results  
**So that** I can quickly diagnose GPU interaction problems without extensive
logging

## Acceptance Criteria

### AC1: Element Buffer Visualization

- [ ] Display all elements uploaded to GPU with their positions and sizes
- [ ] Color-code elements by mark type (Circle=blue, Rectangle=green, Line=red)
- [ ] Show element IDs and selection IDs on hover
- [ ] Update visualization in real-time as buffers change

### AC2: Query Visualization

- [ ] Overlay query positions on the element visualization
- [ ] Show query type (point vs region) and parameters
- [ ] Display query radius/region as semi-transparent overlay
- [ ] Highlight elements within query range

### AC3: Hit Test Results Display

- [ ] Show hit test results color-coded by distance
- [ ] Display hits in green, misses in red
- [ ] Show intersection points for hits
- [ ] Provide result summary (X hits from Y elements)

### AC4: Buffer Inspector

- [ ] Raw buffer view showing byte-level data
- [ ] Structured view showing parsed ElementData/InteractionResult
- [ ] Diff view comparing before/after buffer states
- [ ] Export buffer contents to JSON for analysis

## Technical Tasks

### 1. Debug Visualization Window

- [ ] Create debug-only feature flag `debug-viz`
- [ ] Implement imgui or egui-based debug window
- [ ] Add toggle to enable/disable visualization
- [ ] Layout with element view, query view, results panel

### 2. Element Rendering

- [ ] Render circles with correct radius
- [ ] Render rectangles with correct size
- [ ] Render lines with thickness
- [ ] Add grid overlay for position reference

### 3. Query Overlay

- [ ] Render point queries as crosshairs
- [ ] Render region queries as rectangles
- [ ] Show query parameters in tooltip
- [ ] Animate active query

### 4. Hit Test Visualization

- [ ] Color elements based on hit test results
- [ ] Draw lines from query point to hit intersections
- [ ] Display distance/hit information
- [ ] Show result buffer contents in table

### 5. Buffer Inspector

- [ ] Parse ElementData buffer and display
- [ ] Parse InteractionResult buffer and display
- [ ] Implement hex dump view
- [ ] Add JSON export functionality

## Dependencies

- **Requires**: GUP-128 (GPU Hit Test Debug) - ✅ Complete
- **Optional**: egui or imgui integration
- **Enables**: Faster GPU debugging for future issues

## Success Metrics

- [ ] Visual tool helps identify GPU interaction issues in <5 minutes
- [ ] Buffer inspector shows accurate data for 100K+ elements
- [ ] Debug window adds <5% overhead when enabled
- [ ] Zero overhead when debug feature is disabled

## Risk Assessment

**Low Risk**: Debug-only tool with feature flag. No impact on production code or
performance.

## Implementation Notes

### Recommended Approach

Use egui for the debug UI since it's pure Rust and integrates well with wgpu:

```rust
#[cfg(feature = "debug-viz")]
pub mod debug_viz {
    use crate::interaction::{ElementData, InteractionResult};

    pub struct InteractionDebugger {
        elements: Vec<ElementData>,
        queries: Vec<GpuInteractionQuery>,
        results: Vec<InteractionResult>,
        enabled: bool,
    }

    impl InteractionDebugger {
        pub fn update(&mut self, elements: &[ElementData],
                     queries: &[GpuInteractionQuery],
                     results: &[InteractionResult]) {
            if !self.enabled {
                return;
            }
            self.elements = elements.to_vec();
            self.queries = queries.to_vec();
            self.results = results.to_vec();
        }

        pub fn render_ui(&mut self, ctx: &egui::Context) {
            egui::Window::new("GPU Interaction Debugger")
                .show(ctx, |ui| {
                    self.render_element_view(ui);
                    self.render_query_view(ui);
                    self.render_results_view(ui);
                });
        }
    }
}
```

### Feature Flag

Add to `Cargo.toml`:

```toml
[features]
debug-viz = ["dep:egui", "dep:egui-wgpu"]
```

---

_Created from GUP-128 retrospective - identified need for GPU debugging
visualization._
