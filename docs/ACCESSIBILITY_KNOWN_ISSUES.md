# Gup Accessibility Known Issues and Workarounds

This document tracks known accessibility issues discovered during screen reader
testing, along with their workarounds and status.

**Last Updated**: 2025-01-24 **Status**: 🚧 In Progress - Issues will be
documented as testing proceeds

---

## Issue Tracking Summary

| Status      | Count | Description                       |
| ----------- | ----- | --------------------------------- |
| 🔥 Critical | 0     | Core functionality broken         |
| ⚠️ Major    | 0     | Significant usability issues      |
| ℹ️ Minor    | 0     | Small issues, doesn't block usage |
| ✅ Resolved | 0     | Fixed in current version          |
| 📋 Planned  | 0     | Fix scheduled for future release  |

---

## Critical Issues

> **None reported yet** - Will be updated as testing proceeds

<!--
### Template for Critical Issue:

### ❌ [Issue ID]: [Short Title]

**Severity**: Critical
**Discovered**: YYYY-MM-DD
**Screen Readers Affected**: [NVDA / JAWS / VoiceOver / etc.]
**Platforms**: [Windows / macOS / iOS / Linux]
**Status**: 🔥 Open / 🔧 In Progress / ✅ Resolved

**Description**:
[Detailed description of what doesn't work]

**Impact**:
[How this affects users - e.g., "Cannot navigate any data points"]

**Reproduction Steps**:
1. [Step 1]
2. [Step 2]
3. [Observe issue]

**Expected Behavior**:
[What should happen]

**Actual Behavior**:
[What actually happens]

**Workaround**:
[If any workaround exists, describe it here]
or
**Workaround**: None available - feature completely inaccessible

**Resolution**:
[Once fixed, describe what was done]

**Related**:
- Issue: [GitHub issue #]
- Story: [GUP-XXX]
- Commit: [commit hash]

---
-->

## Major Issues

> **None reported yet** - Will be updated as testing proceeds

<!--
### Template for Major Issue:

### ⚠️ [Issue ID]: [Short Title]

**Severity**: Major
**Discovered**: YYYY-MM-DD
**Screen Readers Affected**: [NVDA / JAWS / VoiceOver / etc.]
**Platforms**: [Windows / macOS / iOS / Linux]
**Status**: ⚠️ Open / 🔧 In Progress / ✅ Resolved

**Description**:
[Detailed description]

**Impact**:
[How this affects users]

**Reproduction Steps**:
1. [Step 1]
2. [Step 2]
3. [Observe issue]

**Expected Behavior**:
[What should happen]

**Actual Behavior**:
[What actually happens]

**Workaround**:
[Describe workaround if available]

**Resolution**:
[Once fixed, describe fix]

**Related**:
- Issue: [GitHub issue #]
- Story: [GUP-XXX]

---
-->

## Minor Issues

> **None reported yet** - Will be updated as testing proceeds

<!--
### Template for Minor Issue:

### ℹ️ [Issue ID]: [Short Title]

**Severity**: Minor
**Discovered**: YYYY-MM-DD
**Screen Readers Affected**: [NVDA / JAWS / VoiceOver / etc.]
**Platforms**: [Windows / macOS / iOS / Linux]
**Status**: ℹ️ Open / 📋 Planned / ✅ Resolved

**Description**:
[Brief description]

**Impact**:
[How this affects users - minor impact]

**Workaround**:
[Usually not needed for minor issues, but document if relevant]

**Resolution Plan**:
[If planned, when/how it will be fixed]

---
-->

## Resolved Issues

> **None yet** - Will be populated as issues are fixed

<!--
### Template for Resolved Issue:

### ✅ [Issue ID]: [Short Title]

**Severity**: [Critical / Major / Minor]
**Discovered**: YYYY-MM-DD
**Resolved**: YYYY-MM-DD
**Screen Readers Affected**: [List]
**Platforms**: [List]

**Description**:
[What the issue was]

**Resolution**:
[How it was fixed]

**Fixed In**:
- Version: [0.1.x]
- Commit: [hash]
- PR: [#xxx]

**Verification**:
[How fix was verified - e.g., "Tested with NVDA 2023.3 and VoiceOver on macOS 14"]

---
-->

## Platform-Specific Quirks

These are not bugs but expected behaviors that differ across platforms:

### VoiceOver Role Announcements

**Behavior**: VoiceOver may announce custom ARIA roles differently than
NVDA/JAWS

**Example**:

- ARIA role: `role="datapoint"`
- NVDA announces: "Data point"
- VoiceOver announces: "Button"

**Explanation**: VoiceOver maps custom roles to closest standard HTML role

**Recommendation**: Accept this behavior; ensure semantic meaning is preserved

**Impact**: Minor - users understand the element is interactive

### NVDA Live Region Delays

**Behavior**: NVDA may queue live region announcements behind current speech

**Example**:

- User is navigating through data points
- Data updates trigger live region announcement
- Announcement happens after current item finishes reading

**Explanation**: NVDA prioritizes user-initiated navigation over live updates

**Recommendation**: Use `aria-live="assertive"` only for critical updates

**Impact**: Minor - updates are announced, just with slight delay

### JAWS Verbosity

**Behavior**: JAWS provides more detailed announcements than other screen
readers

**Example**:

- NVDA: "Data point 1: 10, clickable"
- JAWS: "Data point 1: 10, clickable button, to activate press spacebar"

**Explanation**: JAWS default verbosity setting is higher

**Recommendation**: Don't add extra verbose aria-descriptions; JAWS provides its
own help

**Impact**: None - JAWS users are accustomed to this verbosity

---

## Browser-Specific Issues

### Chrome: Live Region Announcement Timing

**Status**: Under investigation

**Description**: In Chrome, live region announcements may be delayed compared to
Firefox

**Affected**: All screen readers in Chrome

**Workaround**: None needed - announcements still work, just timing varies

**Recommendation**: Test in multiple browsers

### Safari iOS: Focus Management on Dynamic Updates

**Status**: Under investigation

**Description**: Focus may reset to top after dynamic content updates on iOS
Safari

**Affected**: VoiceOver on iOS

**Workaround**: Manually restore focus after updates

**Impact**: Medium - requires code change to handle properly

---

## Future Enhancements

These aren't issues but potential improvements identified during testing:

### Enhancement: Data Table Navigation

**Priority**: Low **Description**: Add table navigation patterns for grid-based
visualizations **Benefit**: Easier navigation of 2D data layouts **Effort**:
Medium

### Enhancement: Keyboard Shortcut Customization

**Priority**: Low **Description**: Allow users to customize keyboard shortcuts
**Benefit**: Accommodates different workflow preferences **Effort**: High

### Enhancement: Audio Sonification

**Priority**: Low **Description**: Add audio cues for data values (sonification)
**Benefit**: Additional data exploration method **Effort**: Very High

---

## Testing Methodology

### How Issues Are Categorized

**Critical**: Core functionality completely broken or inaccessible

- Cannot navigate to elements
- Screen reader crashes
- No announcements whatsoever
- Data values never announced

**Major**: Significant usability problems but workarounds exist

- Confusing or incorrect announcements
- Missing context that makes data hard to understand
- Keyboard traps that are difficult to escape
- Inconsistent behavior across elements

**Minor**: Small issues that don't significantly impact usage

- Verbose or repetitive announcements
- Visual focus indicator could be clearer
- Slightly awkward navigation order
- Inconsistent terminology

### How to Report Issues

1. **Verify Issue**: Test in multiple browsers/versions if possible
2. **Search Existing**: Check if already reported in this document
3. **Document Thoroughly**: Use issue templates above
4. **Assign Severity**: Be honest about impact
5. **Provide Recording**: Screen/audio recording if possible
6. **Test Workarounds**: Try to find workarounds before reporting

### Issue Lifecycle

```
Discovered → Documented → Prioritized → Assigned → Fixed → Verified → Resolved
     ↓           ↓            ↓           ↓         ↓        ↓           ↓
   Testing   This Doc    Planning     Story    Impl.    Test      Close Issue
```

---

## Related Documents

- **Testing Guide**: `docs/SCREEN_READER_TESTING.md`
- **Compatibility Matrix**: `docs/ACCESSIBILITY_COMPATIBILITY.md`
- **Story**: `docs/planning/stories/GUP-121_Screen_Reader_Manual_Testing.md`

---

## Contributing

If you discover an accessibility issue:

1. **For Critical Issues**:
   - Open GitHub issue immediately with label `accessibility` and `critical`
   - Notify maintainers
   - Document in this file

2. **For Major/Minor Issues**:
   - Document in this file first
   - Open GitHub issue with appropriate labels
   - Include link to this documentation

3. **For Quirks/Enhancements**:
   - Document in appropriate section of this file
   - Discuss in GitHub Discussions if significant
   - Can be addressed in future releases

---

**Status**: 🚧 This document will be updated continuously as screen reader
testing proceeds and as issues are discovered in production use.

**Next Review**: After GUP-121 completion (manual screen reader testing)
