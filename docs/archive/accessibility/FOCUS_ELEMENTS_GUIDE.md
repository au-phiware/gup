# Focus Elements for Data Points - Usage Guide

This document explains how to use the focus element system (GUP-127) for
keyboard-accessible data point navigation.

## Overview

GUP-127 provides keyboard navigation for data points with:

1. **SelectionFocusBridge** — Maps Selection data to focusable elements
2. **MarkFocusHelper** — Low-level position-to-focusable-element conversion
3. **FocusRingRenderer** — GPU-accelerated focus ring visualization
4. **DataDimension navigation** — Sort-based exploration of data dimensions
5. **ARIA integration** — Screen reader support via ARIA tree nodes

## Architecture

```text
Selection<T, M>  ─────────────────────────────────────┐
       │                                               │
       │ register_focus_elements()                     │
       ▼                                               ▼
SelectionFocusBridge ── sync_focus_elements ──► FocusManager
       │                                         │         │
       │ sync_focus_elements_with_aria           │         │
       ▼                                         ▼         ▼
   AriaTree                               Sequential  Spatial
  (DataPoint nodes)                        (Tab)      (Arrow)
                                                │
                                                ▼
                                         FocusRingRenderer
```

## Quick Start with Selection

```rust
use gup::accessibility::selection_focus::{
    SelectionFocusBridge, FocusPointDescriptor,
};
use gup::accessibility::FocusManager;
use gup::selection::Selection;
use gup::mark::Circle;

// Create a Selection with data.
let selection: Selection<MyData, Circle> = Selection::from_data(data);

// Create the bridge and focus manager.
let mut bridge = SelectionFocusBridge::new(Default::default());
let mut fm = FocusManager::new();

// Register data points as focusable elements.
selection.register_focus_elements(&mut bridge, &mut fm, |item, idx| {
    FocusPointDescriptor {
        position: [item.x, item.y],
        label: format!("Point {}: value={:.1}", idx, item.value),
        value: Some(item.value as f64),
    }
});
```

## Handling Keyboard Input

```rust
use gup::accessibility::keyboard::{KeyEvent, AccessibilityAction};

// In your event loop:
if let Some(action) = fm.handle_key_input(key_event) {
    match action {
        AccessibilityAction::FocusChanged => {
            if let Some(desc) = fm.describe_current_focus() {
                println!("Focused: {}", desc);
            }
        }
        AccessibilityAction::DimensionCycleRequested { forward } => {
            // Cycle the active dimension and re-sort.
            let next = if forward { next_dimension } else { prev_dimension };
            bridge.sort_by_dimension(&mut fm, next);
        }
        _ => {}
    }
}
```

## Data Dimension Navigation

Sort focus elements by data dimensions so sequential navigation follows a
meaningful order:

```rust
use gup::accessibility::selection_focus::DataDimension;

// Sort by X position (left to right).
bridge.sort_by_dimension(&mut fm, DataDimension::X);

// Sort by Y position (top to bottom).
bridge.sort_by_dimension(&mut fm, DataDimension::Y);

// Sort by numeric value.
bridge.sort_by_dimension(&mut fm, DataDimension::Value);
```

In `DataDimension` navigation mode, Arrow Up/Down emit `DimensionCycleRequested`
events so the application can switch the active dimension.

## ARIA Integration

Register focus elements with an ARIA tree for screen reader support:

```rust
use gup::accessibility::AccessibilitySystem;

let mut system = AccessibilitySystem::new();
let chart_id = system.aria_tree.create_chart_node(
    "Sales Chart".to_string(),
    Some("Q4 2024 revenue by region".to_string()),
);

bridge.sync_focus_elements_with_aria(
    selection.data(),
    &mut system.focus_manager,
    &mut system.aria_tree,
    chart_id,
    |item, idx| FocusPointDescriptor {
        position: [item.x, item.y],
        label: format!("Region {}: ${:.0}k", item.region, item.revenue),
        value: Some(item.revenue),
    },
);
```

## Rendering Focus Rings

```rust
use gup::accessibility::FocusRingRenderer;
use gup::accessibility::FocusRingStyle;

let mut renderer = FocusRingRenderer::with_style(
    FocusRingStyle::high_contrast() // WCAG AAA compliant
);

// In your render loop:
renderer.update(delta_time);

if let Some(focused) = fm.get_focused_element() {
    renderer.render_focus_ring(device, &mut render_pass, focused.bounds)?;
}
```

### Focus Ring Styles

```rust
let default_style = FocusRingStyle::default();       // Blue, 2px
let high_contrast = FocusRingStyle::high_contrast();  // Yellow, 3px
let animated = FocusRingStyle::animated();             // Dashed, animated

let custom = FocusRingStyle {
    color: [1.0, 0.0, 0.0, 1.0], // Red
    width: 3.0,
    dash_pattern: vec![10.0, 5.0],
    animation_speed: 0.5,
};
```

## Handling Data Changes

When the underlying data changes, re-sync the focus elements:

```rust
// After calling selection.set_data(new_data):
if bridge.needs_sync(selection.len()) {
    selection.register_focus_elements(&mut bridge, &mut fm, descriptor_fn);
}
```

## Performance

- **Registration**: <50ms for 1000 elements
- **Navigation**: <0.1ms per key event for 1000 elements
- **Max elements**: Configurable (default 1000) to prevent degradation
- **Focus ring rendering**: Single GPU draw call via instanced rendering

### Large Datasets

```rust
use gup::accessibility::selection_focus::SelectionFocusConfig;
use gup::accessibility::FocusElementConfig;

let config = SelectionFocusConfig {
    element_config: FocusElementConfig {
        max_elements: 500,
        include_offscreen: false,
        ..Default::default()
    },
    ..Default::default()
};
```

## WCAG 2.1 AA Compliance

- **SC 2.1.1 (Keyboard)**: All data points navigable via keyboard
- **SC 2.4.7 (Focus Visible)**: Clear focus ring indicators
- **SC 1.4.11 (Non-text Contrast)**: High contrast focus rings available

## Testing

```bash
# Unit tests
cargo test accessibility::selection_focus -- --test-threads=1
cargo test accessibility::focus_elements -- --test-threads=1
cargo test accessibility::focus_ring -- --test-threads=1
cargo test accessibility::keyboard -- --test-threads=1

# Integration tests
cargo test --test accessibility_integration -- --test-threads=1
```
