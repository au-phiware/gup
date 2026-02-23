# Windows UI Automation Integration Guide

This guide explains how to use Gup visualizations with Windows screen readers
like NVDA and JAWS.

## Overview

Gup implements Windows UI Automation API support, enabling NVDA, JAWS, and other
assistive technologies on Windows to understand and interact with GPU-rendered
visualizations.

## Architecture

### UI Automation Elements

Each visualization component is represented as a UI Automation provider with:

- **Control Type**: Semantic type (Image, List, DataItem, Group, etc.)
- **Name**: Human-readable label
- **AutomationId**: Unique identifier for programmatic access
- **HelpText**: Detailed description
- **Value**: Current data value (for data elements)
- **Children**: Hierarchical relationships

### ARIA to UI Automation Mapping

| ARIA Role   | UIA Control Type | Description                  |
| ----------- | ---------------- | ---------------------------- |
| Chart       | Image            | Overall visualization        |
| ChartSeries | List             | Group of related data points |
| DataPoint   | DataItem         | Individual data point        |
| Legend      | Group            | Legend explaining encodings  |
| Axis        | Separator        | Scale/axis                   |
| Tooltip     | ToolTip          | Contextual information       |
| Control     | Button           | Interactive control          |

## Using NVDA

NVDA (NonVisual Desktop Access) is a free, open-source screen reader for
Windows.

### Installation

1. Download from <https://www.nvaccess.org/>
2. Run installer and follow prompts
3. NVDA starts automatically after installation

### Navigating Visualizations

#### Basic Navigation

- **Down/Up Arrow**: Move between elements
- **Tab**: Move to next focusable element
- **Shift + Tab**: Move to previous focusable element
- **Enter**: Activate element
- **NVDA + Space**: Browse mode toggle

#### Object Navigation

- **NVDA + Numpad 8**: Move to parent object
- **NVDA + Numpad 2**: Move to first child object
- **NVDA + Numpad 4**: Move to previous object
- **NVDA + Numpad 6**: Move to next object

#### Element List

1. Press **NVDA + F7** to open Elements List
2. Select element type (buttons, links, etc.)
3. Use arrows to navigate list
4. Press **Enter** to jump to element

### Announcements

Gup uses UI Automation notification events for announcements:

- **Data Updates**: Notified when visualization data changes
- **Interactions**: Results of user interactions announced
- **State Changes**: Focus, selection changes

## Using JAWS

JAWS (Job Access With Speech) is a commercial screen reader for Windows.

### Navigation

#### Quick Navigation Keys

- **D**: Next data region
- **G**: Next graphic
- **B**: Next button
- **T**: Next table
- **F**: Next form field

#### JAWS Cursor

- **Insert + Numpad 4/6**: Move JAWS cursor left/right
- **Insert + Numpad 8/2**: Move JAWS cursor up/down
- **Insert + Numpad 5**: Activate JAWS cursor item
- **Insert + Shift + M**: Move to element

### Speech Output Control

- **Ctrl**: Stop speech
- **Insert + Down Arrow**: Say all (continuous reading)
- **Insert + Up Arrow**: Read current line

## Development Integration

### Window Integration

For screen readers to discover UI Automation elements, they must be attached to
the window's automation tree:

```rust
use raw_window_handle::HasRawWindowHandle;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::UiaReturnRawElementProvider;

// Get HWND from winit window
let raw_handle = window.raw_window_handle();

if let RawWindowHandle::Win32(handle) = raw_handle {
    let hwnd = HWND(handle.hwnd.get() as isize);
    
    // In WM_GETOBJECT handler:
    // Return provider for this HWND
    let provider = accessibility_system.get_uia_provider();
    
    unsafe {
        UiaReturnRawElementProvider(hwnd, wparam, lparam, provider);
    }
}
```

### Creating Custom Providers

Implement `IRawElementProviderSimple` for each element:

```rust
use windows::Win32::UI::Accessibility::*;
use windows::core::*;

#[implement(IRawElementProviderSimple)]
struct ChartProvider {
    name: String,
    control_type: i32,
    automation_id: String,
}

#[allow(non_snake_case)]
impl IRawElementProviderSimple_Impl for ChartProvider {
    fn GetPatternProvider(&self, pattern_id: UIA_PATTERN_ID) -> Result<IUnknown> {
        // Return pattern implementations (ITextProvider, IValueProvider, etc.)
        Ok(None)
    }
    
    fn GetPropertyValue(&self, property_id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        match property_id {
            UIA_NamePropertyId => {
                // Return element name
                Ok(VARIANT::from(&self.name))
            }
            UIA_ControlTypePropertyId => {
                // Return control type
                Ok(VARIANT::from(self.control_type))
            }
            UIA_AutomationIdPropertyId => {
                // Return automation ID
                Ok(VARIANT::from(&self.automation_id))
            }
            _ => Ok(VARIANT::default())
        }
    }
    
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions::ServerSideProvider)
    }
    
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        Ok(None)
    }
}
```

### Raising Notification Events

Use `UiaRaiseNotificationEvent` for announcements:

```rust
use windows::Win32::UI::Accessibility::*;

pub fn announce(provider: &IRawElementProviderSimple, message: &str, priority: NotificationPriority) {
    unsafe {
        UiaRaiseNotificationEvent(
            provider,
            NotificationKind::ActionCompleted,
            NotificationProcessing::CurrentThenMostRecent,
            message,
            format!("gup-notification-{}", uuid::Uuid::new_v4()),
        );
    }
}
```

### Focus Events

Raise focus change events when focus moves:

```rust
unsafe {
    UiaRaiseAutomationEvent(
        provider,
        UIA_AutomationFocusChangedEventId,
    );
}
```

## Testing and Validation

### Inspect.exe (Windows SDK)

The Windows SDK includes Inspect.exe for testing UI Automation:

1. Install Windows SDK
2. Run `Inspect.exe` from SDK bin folder
3. Hover over your application
4. Verify:
   - Control types are correct
   - Names and descriptions are meaningful
   - Element hierarchy is logical
   - Properties are set correctly

### AccChecker (Accessibility Checker)

AccChecker validates against Windows accessibility guidelines:

1. Download from Microsoft
2. Run `AccChecker.exe`
3. Select your application
4. Review validation report
5. Fix any issues reported

### Manual Testing Checklist

- [ ] NVDA announces all chart elements correctly
- [ ] NVDA object navigation works through hierarchy
- [ ] NVDA Elements List shows visualization components
- [ ] JAWS announces all chart elements correctly
- [ ] JAWS quick navigation keys work
- [ ] Focus management works with keyboard
- [ ] Notifications announce at correct priority
- [ ] Control types map appropriately
- [ ] Element names are descriptive
- [ ] Hierarchy is logical and navigable

## Troubleshooting

### Screen Reader Not Announcing Elements

1. Verify UI Automation is initialized (`initialize()` called)
2. Check that providers are returned in `WM_GETOBJECT` handler
3. Use Inspect.exe to verify elements exist
4. Check Windows Event Viewer for UI Automation errors

### Elements Not Appearing in Hierarchy

1. Ensure parent-child relationships are set correctly
2. Verify `GetChildren()` returns correct array
3. Check that providers have correct `ProviderOptions`

### Performance Issues

1. Cache property values instead of computing on each query
2. Use `UiaDisconnectProvider` when elements are removed
3. Avoid creating providers for invisible elements
4. Consider lazy provider creation for large datasets

## Compatibility

- **Windows Vista and later**: Full UI Automation support
- **NVDA**: Version 2019.1+ recommended
- **JAWS**: Version 2019+ recommended
- **Windows Narrator**: Built-in support
- **Other screen readers**: Should work via UI Automation API

## Resources

- [UI Automation Overview](https://docs.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32)
- [NVDA User Guide](https://www.nvaccess.org/files/nvda/documentation/userGuide.html)
- [JAWS Documentation](https://www.freedomscientific.com/training/jaws/)
- [Windows Accessibility Guidelines](https://docs.microsoft.com/en-us/windows/apps/design/accessibility/accessibility)

## Implementation Status

The current implementation provides:

- ✅ Core UI Automation element structure
- ✅ ARIA to UIA control type mapping
- ✅ Element creation and lifecycle management
- ✅ Notification and focus event architecture
- 🚧 Full IRawElementProviderSimple COM implementation (requires COM interop)
- 🚧 Window message handling for WM_GETOBJECT
- 🚧 Pattern implementations (ITextProvider, IValueProvider, etc.)

The architecture is complete and ready for full COM integration when needed for
production Windows deployments.
