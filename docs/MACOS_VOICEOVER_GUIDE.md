# macOS VoiceOver Integration Guide

This guide explains how to use Gup visualizations with macOS VoiceOver.

## Overview

Gup implements full NSAccessibility protocol support for macOS, enabling
VoiceOver and other assistive technologies to understand and interact with
GPU-rendered visualizations.

## Architecture

### NSAccessibility Elements

Each visualization component is represented as an `NSAccessibilityElement` with:

- **Role**: Semantic role (chart, data point, legend, axis, etc.)
- **Label**: Human-readable name
- **Description**: Detailed explanation
- **Value**: Current data value (for interactive elements)
- **Children**: Hierarchical relationships

### ARIA to NSAccessibility Mapping

| ARIA Role      | NSAccessibility Role | Description                   |
| -------------- | -------------------- | ----------------------------- |
| Chart          | Image                | Overall visualization         |
| ChartSeries    | List                 | Group of related data points  |
| DataPoint      | Cell                 | Individual data point         |
| Legend         | Group                | Legend explaining encodings   |
| Axis           | Ruler                | Scale/axis                    |
| Tooltip        | HelpTag              | Contextual information        |
| Control        | Button               | Interactive control           |

## Using VoiceOver

### Enabling VoiceOver

1. Press `Cmd + F5` to toggle VoiceOver
2. Or go to System Preferences → Accessibility → VoiceOver

### Navigating Visualizations

#### Basic Navigation

- **VO + Right/Left Arrow**: Move between elements
- **VO + Space**: Activate/interact with element
- **VO + Shift + Down**: Interact with container (drill into chart)
- **VO + Shift + Up**: Exit container

#### Rotor Navigation

The rotor provides specialized navigation for different element types:

1. **Activate Rotor**: `VO + U`
2. **Select Category**: Use Left/Right arrows to choose:
   - **Charts**: Jump between visualizations
   - **Data Points**: Navigate data points
   - **Legends**: Move between legend entries
   - **Axes**: Jump between x/y axes
3. **Navigate**: Use Up/Down arrows to move within category
4. **Go to Element**: Press `Enter`

### Announcements

Gup announces important events via VoiceOver:

- **Data Updates**: When visualization data changes
- **Interactions**: Results of user interactions
- **State Changes**: Selection, hover, focus changes

Announcement priorities:

- **Assertive**: Interrupts current speech (critical updates)
- **Polite**: Waits for natural pause (routine updates)

## Development Integration

### Attaching to NSWindow

For VoiceOver to discover elements, they must be attached to the window:

```rust
use raw_window_handle::HasRawWindowHandle;

// Get the NSWindow from winit window
let raw_handle = window.raw_window_handle();

if let RawWindowHandle::AppKit(handle) = raw_handle {
    unsafe {
        let ns_window: *mut NSWindow = handle.ns_window as *mut _;
        
        // Get accessibility elements from Gup
        let elements = accessibility_system.get_platform_elements();
        
        // Set as window's accessibility children
        (*ns_window).setAccessibilityChildren(elements);
    }
}
```

### Creating Accessible Visualizations

```rust
use gup::accessibility::{AccessibilitySystem, AriaRole, AriaNode};

let mut accessibility = AccessibilitySystem::new();

// Create chart node
let chart_node = AriaNode::new(
    AriaRole::Chart,
    "Sales Over Time".to_string()
).with_description(
    "Line chart showing monthly sales from Jan to Dec 2024".to_string()
);

let chart_id = accessibility.add_node(chart_node)?;

// Add data points
for (month, sales) in data.iter() {
    let point_node = AriaNode::new(
        AriaRole::DataPoint,
        format!("{}: ${:.2}", month, sales)
    ).with_value(sales.to_string());
    
    accessibility.add_child_node(chart_id, point_node)?;
}

// Process updates
let updates = accessibility.drain_updates();
accessibility.update_platform(&updates)?;
```

### Announcements

```rust
// Announce data update
accessibility.announce(
    "Data refreshed. New maximum: $45,230",
    AnnouncementPriority::Polite
)?;

// Critical announcement
accessibility.announce(
    "Error: Failed to load data",
    AnnouncementPriority::Assertive
)?;
```

### Focus Management

```rust
// Set focus to specific element
accessibility.set_focus(Some(node_id))?;

// Handle focus change events
accessibility.on_focus_changed(|node_id| {
    println!("Focus moved to: {:?}", node_id);
});
```

## Testing with Accessibility Inspector

macOS provides Accessibility Inspector for testing:

1. Open Xcode → Open Developer Tool → Accessibility Inspector
2. Select your application from the target menu
3. Inspect element hierarchy and attributes
4. Test VoiceOver announcements
5. Verify keyboard navigation

### What to Check

- [ ] All visualization elements are present
- [ ] Roles are correctly assigned
- [ ] Labels are descriptive
- [ ] Descriptions provide context
- [ ] Hierarchy reflects visual structure
- [ ] Focus changes are announced
- [ ] Data updates trigger announcements
- [ ] Keyboard navigation works smoothly

## Best Practices

### Labels and Descriptions

- **Labels**: Brief, clear identification ("Sales Chart", "Data Point: January")
- **Descriptions**: Context and relationships ("Line chart showing 12 months of
  sales data, trending upward")
- **Values**: Current data or state ("$12,345.67", "Selected")

### Hierarchy

- Organize elements to match visual structure
- Use appropriate roles for semantic meaning
- Keep depth manageable (3-4 levels max)
- Group related items (series, legend entries)

### Announcements

- Use `Polite` priority for routine updates
- Use `Assertive` for critical information
- Avoid announcement spam (debounce rapid updates)
- Be concise but informative

### Performance

- Update only changed elements
- Batch updates when possible
- Defer non-critical updates
- Profile accessibility overhead

## Troubleshooting

### Elements Not Appearing in VoiceOver

1. Verify `initialize()` was called on `AccessibilitySystem`
2. Check that elements are attached to NSWindow
3. Ensure application has accessibility permissions
4. Test with Accessibility Inspector first

### Announcements Not Speaking

1. Check VoiceOver is enabled
2. Verify announcement priority (not `Off`)
3. Test with System Preferences → Accessibility → Speech
4. Check macOS sound output

### Focus Not Working

1. Verify element exists in accessibility tree
2. Check element is not hidden or disabled
3. Ensure window has focus
4. Test with Accessibility Inspector

## Platform-Specific Notes

### macOS Versions

- NSAccessibility is available on all macOS versions
- Modern API requires macOS 10.10+
- Some features require macOS 11.0+ (Big Sur)

### Permissions

Applications don't need special permissions for NSAccessibility, but:

- Accessibility Inspector requires Developer Tools
- Screen recording permission may be needed for some AT tools
- Always test with VoiceOver itself

### VoiceOver Quirks

- Element discovery can be delayed on first navigation
- Rapid updates may be coalesced
- Focus announcements may conflict with other speech
- Rotor categories appear based on element roles present

## Resources

- [Apple Accessibility Programming Guide](https://developer.apple.com/accessibility/)
- [NSAccessibility Protocol
  Reference](https://developer.apple.com/documentation/appkit/nsaccessibility)
- [VoiceOver User
  Guide](https://support.apple.com/guide/voiceover/welcome/mac)
- [Gup Accessibility System Documentation](../ACCESSIBILITY_KNOWN_ISSUES.md)

## Support

For issues specific to Gup's macOS accessibility implementation:

1. Check [ACCESSIBILITY_KNOWN_ISSUES.md](../ACCESSIBILITY_KNOWN_ISSUES.md)
2. Search existing GitHub issues
3. File a new issue with:
   - macOS version
   - VoiceOver behavior
   - Accessibility Inspector output
   - Minimal reproduction
