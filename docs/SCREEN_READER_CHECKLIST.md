# Screen Reader Testing Quick Reference Checklist

Use this checklist while performing screen reader tests. Print or view
side-by-side with the visualization under test.

---

## Pre-Test Setup

- [ ] Screen reader installed and configured
- [ ] Browser compatible with screen reader
- [ ] Example built and served (`mask serve web_accessibility_demo`)
- [ ] Audio recording software ready (optional but recommended)
- [ ] Notebook/doc ready for taking notes

---

## Test Scenario 1: Basic Navigation

**Goal**: All elements are reachable and announced correctly

- [ ] Tab to first element - announced with role and label
- [ ] Tab through all elements - logical order
- [ ] Each data point announced with value
- [ ] Can Tab to last element
- [ ] Shift+Tab works in reverse
- [ ] Visual focus indicator visible (for sighted testers)
- [ ] No keyboard traps

**Pass Criteria**: All elements reachable, clear announcements, logical order

---

## Test Scenario 2: Screen Reader Commands

**Goal**: Screen reader-specific navigation works

**NVDA/JAWS**:
- [ ] `H` key: Navigate by headings (if present)
- [ ] `R` key: Navigate by regions
- [ ] `B` key: Navigate by buttons/interactive elements
- [ ] `D` key: Navigate by landmarks

**VoiceOver (macOS)**:
- [ ] `VO+Cmd+H`: Navigate by headings
- [ ] `VO+Cmd+J`: Navigate by form controls
- [ ] `VO+Right Arrow`: Explore item by item

**VoiceOver (iOS)**:
- [ ] Rotor gesture: Access navigation types
- [ ] Swipe up/down: Navigate by selected type
- [ ] Touch exploration: Drag finger across screen

**Pass Criteria**: All navigation methods work correctly

---

## Test Scenario 3: Data Exploration

**Goal**: Data values are clear and have context

- [ ] Navigate to first data point
- [ ] Announcement includes: Role + Label + Value
- [ ] Context is clear (not just "10")
- [ ] Navigate to next data point
- [ ] Announcements not overly repetitive
- [ ] Can distinguish between data points
- [ ] Values are accurate

**Pass Criteria**: Data is comprehensible from announcements alone

---

## Test Scenario 4: Interaction

**Goal**: Activating elements works and provides feedback

- [ ] Navigate to interactive element
- [ ] Press Enter - activation works
- [ ] Hear feedback announcement (live region)
- [ ] Press Space - also works
- [ ] Visual state change announced
- [ ] Can continue navigating after activation
- [ ] Can undo/deselect with Escape (if applicable)

**Pass Criteria**: All interactions work, feedback is clear

---

## Test Scenario 5: Keyboard Shortcuts

**Goal**: All keyboard shortcuts work with screen reader

- [ ] Tab: Next element
- [ ] Shift+Tab: Previous element
- [ ] Arrow keys: Navigate within chart (if implemented)
- [ ] Enter: Activate
- [ ] Space: Activate
- [ ] Escape: Cancel/deselect
- [ ] No conflicts with screen reader commands
- [ ] Shortcuts documented somewhere

**Pass Criteria**: All shortcuts work, no conflicts

---

## Test Scenario 6: Dynamic Updates

**Goal**: Live regions announce changes appropriately

- [ ] Trigger data update (if example supports this)
- [ ] Live region announcement heard
- [ ] Announcement is clear and concise
- [ ] Update doesn't interrupt current reading
- [ ] Can navigate to updated elements
- [ ] Updated values are correct

**Pass Criteria**: Updates are announced clearly and timely

---

## Test Scenario 7: Touch (iOS only)

**Goal**: Touch exploration works on mobile

- [ ] Drag finger - elements announced as touched
- [ ] Touch areas appropriately sized
- [ ] Double-tap activates element
- [ ] Swipe right: Next element
- [ ] Swipe left: Previous element
- [ ] Three-finger swipe: Navigate sections
- [ ] Rotor accessible

**Pass Criteria**: Full touch navigation works

---

## Common Issues to Watch For

### Critical Issues
- [ ] Elements not announced at all
- [ ] Cannot navigate to key elements
- [ ] Screen reader crashes or freezes
- [ ] Keyboard trap (cannot navigate away)
- [ ] Data values never announced

### Major Issues
- [ ] Confusing announcements
- [ ] Missing role or label
- [ ] Wrong navigation order
- [ ] Live regions not working
- [ ] Interactions don't provide feedback

### Minor Issues
- [ ] Overly verbose announcements
- [ ] Inconsistent terminology
- [ ] Focus indicator hard to see
- [ ] Slightly awkward navigation
- [ ] Delayed announcements

---

## Issue Recording Template

```
Issue: [Brief description]
Severity: Critical / Major / Minor
Screen Reader: [Name + Version]
Browser: [Name + Version]
OS: [Name + Version]
Scenario: [Which test scenario]
Steps to Reproduce:
1. [Step 1]
2. [Step 2]
Expected: [What should happen]
Actual: [What actually happened]
Recording: [Link if available]
```

---

## Quick ARIA Reference

### Expected Roles
- `region` - Chart container
- `datapoint` (custom) or `button` - Interactive data elements
- `status` or `log` - Live region for announcements

### Expected Attributes
- `aria-label` - Human-readable label
- `aria-describedby` - Additional description
- `aria-live` - Live region behavior
- `tabindex` - Keyboard focus control

---

## Screen Reader Keyboard Shortcuts

### NVDA (Windows)
- `NVDA+N` - Open NVDA menu
- `NVDA+Q` - Quit NVDA
- `Insert` = NVDA key
- `NVDA+S` - Toggle speech mode
- `Ctrl` - Stop speaking

### JAWS (Windows)
- `INSERT+F2` - JAWS menu
- `INSERT+F4` - Close JAWS
- `Insert` = JAWS key
- `Ctrl` - Stop speaking

### VoiceOver (macOS)
- `Cmd+F5` - Toggle VoiceOver
- `VO+H` - VoiceOver help
- `VO` = `Ctrl+Option`
- `Ctrl` - Stop speaking

### VoiceOver (iOS)
- Triple-click home/side button - Toggle VoiceOver
- Two-finger tap - Stop speaking
- Three-finger triple tap - Screen curtain
- Four-finger tap top - First item
- Four-finger tap bottom - Last item

---

## Browser Testing Matrix

Test in these combinations (in order of priority):

**High Priority**:
1. [ ] NVDA + Chrome (Windows)
2. [ ] VoiceOver + Safari (macOS)
3. [ ] JAWS + Chrome (Windows)
4. [ ] VoiceOver + Safari (iOS)

**Medium Priority**:
5. [ ] NVDA + Firefox (Windows)
6. [ ] VoiceOver + Chrome (macOS)
7. [ ] Orca + Firefox (Linux)

---

## Results Summary

After testing, complete this summary:

**Date**: _____________
**Tester**: _____________
**Screen Reader**: _____________ Version: _____________
**Browser**: _____________ Version: _____________
**OS**: _____________ Version: _____________

**Overall Result**: ☐ Pass  ☐ Pass with minor issues  ☐ Fail

**Scenarios Passed**: ___/7

**Critical Issues Found**: ___
**Major Issues Found**: ___
**Minor Issues Found**: ___

**Would you recommend this for production use?**: ☐ Yes  ☐ With fixes  ☐ No

**Additional Notes**:
_________________________________________________________________
_________________________________________________________________
_________________________________________________________________

---

## Next Steps After Testing

1. [ ] Update compatibility matrix (`docs/ACCESSIBILITY_COMPATIBILITY.md`)
2. [ ] Document issues (`docs/ACCESSIBILITY_KNOWN_ISSUES.md`)
3. [ ] Update story (`docs/planning/stories/GUP-121_Screen_Reader_Manual_Testing.md`)
4. [ ] Create GitHub issues for critical/major problems
5. [ ] Share findings with team

---

**Quick Reference Links**:

- Full Testing Guide: `docs/SCREEN_READER_TESTING.md`
- Compatibility Matrix: `docs/ACCESSIBILITY_COMPATIBILITY.md`
- Known Issues: `docs/ACCESSIBILITY_KNOWN_ISSUES.md`
- Story: `docs/planning/stories/GUP-121_Screen_Reader_Manual_Testing.md`

---

**Version**: 1.0
**Last Updated**: 2025-01-24
