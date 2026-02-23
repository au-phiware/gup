# Event Forwarding from DOM Overlay to GPU Interaction System

## Overview

The Web DOM Overlay provides event forwarding capabilities that bridge browser
DOM events to Gup's GPU-accelerated interaction system. This enables accessible
touch, pointer, and keyboard interactions while maintaining high performance.

## Architecture

```
┌─────────────────┐
│   Browser DOM   │
│  (User Input)   │
└────────┬────────┘
         │
         v
┌─────────────────────┐
│  WebDomOverlay      │
│  Event Handlers     │
│  - Pointer Events   │
│  - Touch Events     │
│  - Keyboard Events  │
└────────┬────────────┘
         │
         v
┌─────────────────────┐
│ DomInteractionEvent │
│  - Screen Coords    │
│  - Canvas Coords    │
│  - Event Metadata   │
└────────┬────────────┘
         │
         v
┌─────────────────────┐
│  InteractionSystem  │
│  GPU Hit Testing    │
│  Element Selection  │
└─────────────────────┘
```

## Usage

### Basic Setup

```rust
use gup::accessibility::web_overlay::{WebDomOverlay, DomOverlayConfig};
use gup::interaction::InteractionSystem;

// Create overlay with default configuration
let mut overlay = WebDomOverlay::new()?;

// Initialize the overlay (creates DOM elements)
overlay.initialize()?;

// Set up event forwarding callback
overlay.set_event_forward_callback(move |event| {
    // Forward to visualization system
    handle_dom_event(event);
});
```

### Custom Configuration

```rust
// Configure overlay behavior
let config = DomOverlayConfig {
    container_id: "my-overlay".to_string(),
    canvas_id: "my-canvas".to_string(),
    keyboard_enabled: true,
    pointer_enabled: true,
    show_focus_indicators: true,
    z_index: 1000,
    forward_events: true,         // Enable event forwarding
    deduplicate_events: true,     // Prevent duplicate events
};

let mut overlay = WebDomOverlay::with_config(config)?;
```

### Integrating with InteractionSystem

```rust
use gup::interaction::{InteractionSystem, Vec2};

async fn handle_dom_event(event: DomInteractionEvent) {
    match event.event_type.as_str() {
        "pointerdown" | "touchstart" => {
            // Query GPU for elements at this position
            let position = Vec2::new(event.canvas_x, event.canvas_y);
            let hits = interaction_system.query_point(position, &selections).await?;

            // Handle selection
            for hit in hits {
                println!("Selected element {} in selection {}",
                    hit.element_id, hit.selection_id);
            }
        }
        "pointermove" | "touchmove" => {
            // Handle hover/drag
            update_hover_state(event.canvas_x, event.canvas_y);
        }
        "pointerup" | "touchend" => {
            // Handle release
            finalize_interaction();
        }
        _ => {}
    }
}
```

## Event Types

### DomInteractionEvent

All forwarded events are wrapped in a `DomInteractionEvent` structure:

```rust
pub struct DomInteractionEvent {
    /// Event type: "pointerdown", "pointermove", "pointerup", etc.
    pub event_type: String,

    /// Screen coordinates (client X/Y from the DOM event)
    pub screen_x: f32,
    pub screen_y: f32,

    /// Canvas-relative coordinates (accounting for canvas position)
    pub canvas_x: f32,
    pub canvas_y: f32,

    /// Pointer type: "mouse", "pen", "touch"
    pub pointer_type: String,

    /// Pointer ID for multi-touch tracking
    pub pointer_id: i32,

    /// Button state for pointer events
    pub button: i16,

    /// Timestamp of the event
    pub timestamp: f64,
}
```

### Pointer Events

- `pointerdown` - Pointer/touch pressed
- `pointermove` - Pointer/touch moved
- `pointerup` - Pointer/touch released
- `pointerenter` - Pointer entered element
- `pointerleave` - Pointer left element

### Touch Events

- `touchstart` - Touch began
- `touchmove` - Touch moved
- `touchend` - Touch ended

## Coordinate Mapping

The overlay automatically maps DOM coordinates to canvas coordinates:

1. **Screen Coordinates** (`screen_x`, `screen_y`): Raw client coordinates from
   the browser event
2. **Canvas Coordinates** (`canvas_x`, `canvas_y`): Coordinates relative to the
   canvas element

This mapping accounts for:

- Canvas position within the page
- Scroll offsets
- Any CSS transformations

## Event Deduplication

When enabled, the overlay prevents duplicate events that might occur when both
the canvas and overlay receive the same user action:

```rust
// Deduplication logic:
// - Events within 50ms at the same coordinates are considered duplicates
// - Only the first event is forwarded
// - Subsequent duplicates are logged but ignored
```

Configure deduplication:

```rust
let mut config = DomOverlayConfig::default();
config.deduplicate_events = true;  // Enable (default)
config.deduplicate_events = false; // Disable
```

## Touch Target Size

For accessibility, the overlay ensures touch targets meet WCAG 2.1 AAA minimum
size requirements (44x44px). This is automatically handled by the CSS styling.

## Multi-Touch Support

The overlay tracks individual touch points by `pointer_id`. For multi-touch
gestures, you can:

1. Track touch points by ID
2. Calculate distances and angles between touches
3. Recognize pinch, rotate, and pan gestures

Example:

```rust
let mut touch_points = HashMap::new();

overlay.set_event_forward_callback(move |event| {
    match event.event_type.as_str() {
        "touchstart" => {
            touch_points.insert(event.pointer_id, (event.canvas_x, event.canvas_y));
        }
        "touchmove" => {
            if let Some(pos) = touch_points.get_mut(&event.pointer_id) {
                *pos = (event.canvas_x, event.canvas_y);
            }

            // Calculate gesture from touch_points
            if touch_points.len() == 2 {
                calculate_pinch_gesture(&touch_points);
            }
        }
        "touchend" => {
            touch_points.remove(&event.pointer_id);
        }
        _ => {}
    }
});
```

## Performance Considerations

1. **Event Forwarding**: Minimal overhead - events are forwarded through a
   zero-cost closure
2. **Coordinate Mapping**: Computed once per event using cached canvas bounds
3. **Deduplication**: Simple timestamp and coordinate comparison (<1µs)
4. **GPU Hit Testing**: Asynchronous, doesn't block event handling

## Testing

Tests are located in `tests/event_forwarding_tests.rs`:

```bash
# Run tests (native only, requires manual testing for wasm)
cargo test --test event_forwarding_tests

# Run wasm tests in browser
wasm-pack test --headless --firefox
```

## Accessibility Benefits

The event forwarding system enables:

1. **Keyboard Navigation**: Arrow keys, Tab, Enter/Space for element selection
2. **Touch/Pointer Access**: Native browser touch handling with proper target
   sizes
3. **Screen Reader Support**: ARIA tree synchronized with visual state
4. **Focus Management**: Visible focus indicators and proper focus order

## See Also

- [GUP-119 Story](../docs/planning/stories/GUP-119_Interactive_Event_Forwarding.md)
- [GUP-117: Web Accessibility DOM Overlay](../docs/planning/stories/GUP-117_Web_Accessibility_DOM_Overlay.md)
- [GUP-012: GPU Interaction System](../docs/planning/stories/GUP-012_GPU_Interaction_System.md)
