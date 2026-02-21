# Gup Accessibility Compatibility Matrix

This document tracks the compatibility status of Gup's accessibility features
across different screen readers, browsers, and platforms.

## Testing Status: In Progress

**Last Updated**: 2025-01-24
**Test Coverage**: 0% (Manual testing in progress)

---

## Feature Compatibility Matrix

### Core Navigation Features

| Feature                    | NVDA (Win) | JAWS (Win) | VoiceOver (macOS) | VoiceOver (iOS) | Orca (Linux) | Notes |
|----------------------------|------------|------------|-------------------|-----------------|--------------|-------|
| Tab Navigation             | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Arrow Key Navigation       | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Enter/Space Activation     | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Escape to Cancel           | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Focus Indicators           | ❓         | ❓         | ❓                | ❓              | ❓           | Visual feature |
| Focus Management           | ❓         | ❓         | ❓                | ❓              | ❓           |       |

### ARIA Announcements

| Feature                    | NVDA (Win) | JAWS (Win) | VoiceOver (macOS) | VoiceOver (iOS) | Orca (Linux) | Notes |
|----------------------------|------------|------------|-------------------|-----------------|--------------|-------|
| Role Announcements         | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Label Announcements        | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Description Announcements  | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Value Announcements        | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| State Changes              | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Live Regions (Polite)      | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Live Regions (Assertive)   | ❓         | ❓         | ❓                | ❓              | ❓           |       |

### Screen Reader Navigation

| Feature                    | NVDA (Win) | JAWS (Win) | VoiceOver (macOS) | VoiceOver (iOS) | Orca (Linux) | Notes |
|----------------------------|------------|------------|-------------------|-----------------|--------------|-------|
| Heading Navigation (H)     | ❓         | ❓         | VO+Cmd+H          | Rotor           | ❓           |       |
| Region Navigation (R)      | ❓         | ❓         | VO+Right Arrow    | Rotor           | ❓           |       |
| Button Navigation (B)      | ❓         | ❓         | VO+Cmd+J          | Rotor           | ❓           |       |
| Landmark Navigation (D)    | ❓         | ❓         | N/A               | Rotor           | ❓           |       |
| Forms Mode                 | ❓         | ❓         | N/A               | N/A             | ❓           |       |

### Touch/Mobile Features

| Feature                    | NVDA (Win) | JAWS (Win) | VoiceOver (macOS) | VoiceOver (iOS) | Orca (Linux) | Notes |
|----------------------------|------------|------------|-------------------|-----------------|--------------|-------|
| Touch Exploration          | N/A        | N/A        | N/A               | ❓              | N/A          | iOS only |
| Touch Activation           | N/A        | N/A        | N/A               | ❓              | N/A          | iOS only |
| Swipe Navigation           | N/A        | N/A        | N/A               | ❓              | N/A          | iOS only |
| Rotor Control              | N/A        | N/A        | N/A               | ❓              | N/A          | iOS only |
| Pinch to Zoom              | N/A        | N/A        | N/A               | ❓              | N/A          | If supported |

### Advanced Features

| Feature                    | NVDA (Win) | JAWS (Win) | VoiceOver (macOS) | VoiceOver (iOS) | Orca (Linux) | Notes |
|----------------------------|------------|------------|-------------------|-----------------|--------------|-------|
| Data Table Navigation      | ❓         | ❓         | ❓                | ❓              | ❓           | If applicable |
| Multi-level Navigation     | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Search/Filter              | ❓         | ❓         | ❓                | ❓              | ❓           |       |
| Keyboard Shortcuts         | ❓         | ❓         | ❓                | N/A             | ❓           |       |
| Context Menus              | ❓         | ❓         | ❓                | ❓              | ❓           | If implemented |

---

## Browser Compatibility

### Windows (NVDA + JAWS)

| Browser           | Version Tested | NVDA Status | JAWS Status | Notes |
|-------------------|----------------|-------------|-------------|-------|
| Chrome            | ❓             | ❓          | ❓          |       |
| Firefox           | ❓             | ❓          | ❓          |       |
| Edge              | ❓             | ❓          | ❓          |       |
| Opera             | ❓             | ❓          | ❓          | Low priority |

### macOS (VoiceOver)

| Browser           | Version Tested | VoiceOver Status | Notes |
|-------------------|----------------|------------------|-------|
| Safari            | ❓             | ❓               | Recommended |
| Chrome            | ❓             | ❓               |       |
| Firefox           | ❓             | ❓               |       |

### iOS (VoiceOver)

| Browser           | Version Tested | VoiceOver Status | Notes |
|-------------------|----------------|------------------|-------|
| Safari            | ❓             | ❓               | iOS default |
| Chrome (iOS)      | ❓             | ❓               | Uses WebKit |

### Linux (Orca)

| Browser           | Version Tested | Orca Status | Notes |
|-------------------|----------------|-------------|-------|
| Firefox           | ❓             | ❓          | Recommended |
| Chrome/Chromium   | ❓             | ❓          |       |

---

## Platform-Specific Considerations

### Windows

**NVDA**:
- Free and open-source
- Generally most compatible with modern web standards
- Updates frequently
- Recommended for initial testing

**JAWS**:
- Commercial (expensive)
- Industry standard in enterprise
- More verbose announcements
- Some proprietary features

**Known Issues**:
- None yet (testing in progress)

### macOS

**VoiceOver**:
- Built-in, no installation needed
- Different terminology than Windows screen readers
- Best with Safari
- Trackpad gestures require learning

**Known Issues**:
- None yet (testing in progress)

### iOS

**VoiceOver**:
- Built-in mobile screen reader
- Touch-based navigation
- Different interaction model than desktop
- Requires physical device testing

**Known Issues**:
- None yet (testing in progress)

### Linux

**Orca**:
- Free and open-source
- Smaller user base
- Generally follows NVDA patterns
- Can be quirky with some web content

**Known Issues**:
- None yet (testing in progress)

---

## Test Configuration

### Test Examples

All screen reader tests should be performed against:

1. **web_accessibility_demo**: Basic chart with 5 data points
   - File: `examples/web_accessibility_demo.rs`
   - Build: `wasm-pack build --target web --out-dir ../pkg/web_accessibility_demo web_accessibility_demo`
   - Serve: `mask serve web_accessibility_demo`

2. **Additional examples** (as implemented):
   - TBD: Interactive scatter plot
   - TBD: Dynamic data updates
   - TBD: Complex multi-chart layout

### Test Scenarios

Reference: See `docs/SCREEN_READER_TESTING.md` for detailed test scenarios

Core scenarios to test:
1. Basic Navigation
2. Screen Reader Navigation Commands
3. Data Exploration
4. Interaction Testing
5. Dynamic Updates
6. Keyboard-Only Navigation
7. Touch Exploration (iOS)

---

## Legend

- **✅ Fully Supported**: Feature works as expected with no issues
- **⚠️ Partially Supported**: Feature works but has minor issues or limitations
- **❌ Not Supported**: Feature doesn't work or has critical issues
- **❓ Not Yet Tested**: Testing not yet performed
- **N/A**: Feature not applicable to this platform/screen reader

---

## Issue Severity

When updating this matrix with test results, use these severity guidelines:

- **Critical** (❌): Core functionality broken, unusable
- **Major** (⚠️): Significant usability issues, workarounds available
- **Minor** (⚠️): Small issues, doesn't prevent usage
- **Enhancement**: Works correctly but could be improved

---

## Testing Priority

**High Priority** (Test First):
1. NVDA + Chrome (Windows) - Most common configuration
2. VoiceOver + Safari (macOS) - Second most common
3. JAWS + Chrome (Windows) - Enterprise standard
4. VoiceOver + Safari (iOS) - Mobile testing

**Medium Priority**:
5. NVDA + Firefox (Windows)
6. VoiceOver + Chrome (macOS)
7. Orca + Firefox (Linux)

**Low Priority**:
8. Other browser combinations
9. Older screen reader versions

---

## Update Instructions

When completing screen reader tests, update this file:

1. Change ❓ to ✅, ⚠️, or ❌ based on test results
2. Add version numbers for tested software
3. Add notes for any issues or special behaviors
4. Update "Last Updated" date at top
5. Increment test coverage percentage

**Example**:
```markdown
| Tab Navigation | ✅ | ✅ | ⚠️ | ❓ | ❓ | VoiceOver: Focus ring not visible |
```

---

## Related Documents

- **Testing Guide**: `docs/SCREEN_READER_TESTING.md` - Detailed testing instructions
- **Known Issues**: `docs/ACCESSIBILITY_KNOWN_ISSUES.md` - Issue tracking and workarounds
- **ARIA Implementation**: `src/accessibility/` - Source code for ARIA features
- **Story**: `docs/planning/stories/GUP-121_Screen_Reader_Manual_Testing.md` - This testing initiative

---

**Next Steps**:

1. ⏳ Perform NVDA testing on Windows
2. ⏳ Perform JAWS testing on Windows (if available)
3. ⏳ Perform VoiceOver testing on macOS
4. ⏳ Perform VoiceOver testing on iOS (if available)
5. ⏳ Document all test results in this matrix
6. ⏳ Create known issues document for any problems found
7. ⏳ Update GUP-121 story with final results

---

**Status**: 🚧 Testing in progress - Matrix will be populated as tests are completed
