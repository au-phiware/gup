# Screen Reader Testing Guide for Gup

This guide provides comprehensive instructions for testing Gup visualizations
with screen readers to ensure accessibility compliance.

## Overview

Gup's Web accessibility layer provides:

- ARIA-compliant DOM overlay for keyboard navigation
- Hidden ARIA tree for detailed screen reader descriptions
- Live region announcements for dynamic updates
- Keyboard shortcuts for navigation and interaction

Testing with actual screen readers validates that these features work correctly
with real assistive technologies.

## Supported Screen Readers

### Primary Support (Tier 1)

- **NVDA** (Windows) - Free, open-source
- **JAWS** (Windows) - Commercial, industry standard
- **VoiceOver** (macOS/iOS) - Built-in, widely used

### Secondary Support (Tier 2)

- **Orca** (Linux) - Free, open-source
- **TalkBack** (Android) - Built-in for Android

## Test Environment Setup

### NVDA Setup (Windows)

1. **Download NVDA**:
   - Visit https://www.nvaccess.org/download/
   - Download the latest stable version
   - Run installer (no admin rights needed for portable version)

2. **Configure NVDA for Testing**:
   - Press `NVDA+N` to open NVDA menu
   - Go to Preferences → Settings
   - Speech category: Set rate to comfortable level
   - Keyboard category: Enable "Speak typed characters"
   - Browse Mode category: Enable "Automatic focus mode for focus changes"

3. **Browser Setup**:
   - Use Chrome, Firefox, or Edge (all supported)
   - Ensure JavaScript is enabled
   - Disable ad blockers that might interfere with ARIA

4. **Test Files**:
   - Build the web_accessibility_demo example:
     ```bash
     cd examples
     wasm-pack build --target web --out-dir ../pkg/web_accessibility_demo web_accessibility_demo
     ```
   - Serve the example:
     ```bash
     mask serve web_accessibility_demo
     ```
   - Open in browser with NVDA running

### JAWS Setup (Windows)

1. **Download JAWS**:
   - Visit https://www.freedomscientific.com/products/software/jaws/
   - Download trial version (40-minute sessions) or purchase license
   - Run installer (requires admin rights)

2. **Configure JAWS for Testing**:
   - Press `INSERT+F2` to open JAWS menu
   - Go to Options → Settings Center
   - Set verbosity to "Intermediate" or "Advanced" for testing
   - Ensure "Announce ARIA Roles" is enabled

3. **Browser Setup**:
   - JAWS works best with Chrome and Firefox
   - Same browser setup as NVDA

4. **Test Files**:
   - Same as NVDA setup above

### VoiceOver Setup (macOS)

1. **Enable VoiceOver**:
   - Press `Cmd+F5` to toggle VoiceOver
   - Or go to System Settings → Accessibility → VoiceOver → Enable

2. **Configure VoiceOver for Testing**:
   - Open VoiceOver Utility (`Cmd+F8` while VoiceOver is on)
   - Set speech rate to comfortable level
   - Enable "Speak notifications"
   - Web category: Enable "Speak table headers"

3. **Browser Setup**:
   - Safari is recommended (best VoiceOver integration)
   - Chrome and Firefox also work but may have different behavior

4. **Test Files**:
   - Build and serve same as NVDA setup
   - Open http://localhost:8080 in Safari

### VoiceOver Setup (iOS)

1. **Enable VoiceOver**:
   - Go to Settings → Accessibility → VoiceOver → On
   - Or use Siri: "Hey Siri, turn on VoiceOver"
   - Practice: Triple-click home button (or side button) to toggle

2. **Learn VoiceOver Gestures**:
   - Single tap: Select item
   - Double tap: Activate selected item
   - Swipe right: Next item
   - Swipe left: Previous item
   - Two-finger tap: Stop speaking
   - Three-finger swipe right: Next page/screen

3. **Test Files**:
   - Ensure mobile-responsive version is served
   - Open Safari on iOS device
   - Navigate to http://[your-ip]:8080

### Orca Setup (Linux)

1. **Install Orca**:

   ```bash
   # Debian/Ubuntu
   sudo apt install orca

   # Fedora
   sudo dnf install orca
   ```

2. **Configure Orca**:
   - Run `orca` from terminal
   - Press `INSERT+Space` to open Orca preferences
   - Set speech rate and voice
   - Enable "Speak blank lines"

3. **Browser Setup**:
   - Firefox recommended for Linux
   - Chrome/Chromium also supported

4. **Test Files**:
   - Same build process as above
   - Ensure WebGPU is enabled in browser

## Test Scenarios

### Scenario 1: Basic Navigation

**Objective**: Verify screen reader can navigate through all elements

**Steps**:

1. Load the web accessibility demo
2. With screen reader active, press Tab to navigate
3. Listen to announcements for each element
4. Verify:
   - [ ] Chart title is announced with role
   - [ ] Each data point is announced with role and value
   - [ ] Navigation order is logical (top to bottom, left to right)
   - [ ] Focus indicators are visible (for sighted users)

**Expected Announcements** (NVDA/JAWS):

```
"Sample Accessible Chart, region"
"Data Point 1: 10, clickable"
"Data Point 2: 25, clickable"
"Data Point 3: 15, clickable"
...
```

**Expected Announcements** (VoiceOver):

```
"Sample Accessible Chart, region"
"Data Point 1: 10, button"
"Data Point 2: 25, button"
...
```

**Notes**:

- JAWS may announce more context
- VoiceOver uses different terminology ("button" vs "clickable")
- Orca follows NVDA patterns generally

### Scenario 2: Screen Reader Navigation Commands

**Objective**: Verify screen reader-specific navigation works

**NVDA/JAWS Steps**:

1. Press `H` to navigate by headings (if any)
2. Press `R` to navigate by regions
3. Press `B` to navigate by buttons/interactive elements
4. Press `D` to navigate by landmarks
5. Verify:
   - [ ] Headings are announced correctly
   - [ ] Regions are identified
   - [ ] Interactive elements are found
   - [ ] Live regions are announced when updated

**VoiceOver Steps** (macOS):

1. Press `VO+Cmd+H` to navigate by headings
2. Press `VO+Cmd+J` to navigate by form controls
3. Press `VO+Right Arrow` to explore by item
4. Verify same as above

**VoiceOver Steps** (iOS):

1. Use rotor (two fingers rotate on screen)
2. Select "Headings", "Links", or "Form Controls"
3. Swipe up/down to navigate by selected type
4. Verify same as above

### Scenario 3: Data Exploration

**Objective**: Verify data values are announced clearly with context

**Steps**:

1. Navigate to first data point
2. Listen to full announcement
3. Move to next data point
4. Verify:
   - [ ] Data point role is announced ("Data Point")
   - [ ] Data value is announced clearly ("10")
   - [ ] Additional context provided if available (label, description)
   - [ ] Announcements are not repetitive or verbose

**Expected Announcements**:

```
"Data Point 1: 10, clickable"
[Move to next]
"Data Point 2: 25, clickable"
```

**Anti-patterns** (what to avoid):

- "Data Point Data Point 1 10 clickable clickable" (repetitive)
- "10" (no context)
- Long aria-descriptions that obscure the value

### Scenario 4: Interaction Testing

**Objective**: Verify selecting and activating elements works

**Steps**:

1. Navigate to a data point
2. Press Enter or Space to activate
3. Listen for feedback announcement
4. Verify:
   - [ ] Activation is announced
   - [ ] Feedback is provided via live region
   - [ ] Visual state changes are announced (if any)
   - [ ] User can continue navigating after interaction

**Expected Behavior**:

- Press Enter on "Data Point 1: 10"
- Hear: "Selected Data Point 1" (via live region)
- Visual: Data point changes appearance
- Screen reader: Announces selection state

**Test Keyboard Shortcuts**:

- [ ] Tab: Next element (forward)
- [ ] Shift+Tab: Previous element (backward)
- [ ] Arrow keys: Navigate within chart (if implemented)
- [ ] Enter: Activate/select element
- [ ] Space: Activate/select element
- [ ] Escape: Cancel or deselect (if applicable)

### Scenario 5: Dynamic Updates

**Objective**: Verify live regions announce changes

**Steps**:

1. Load visualization with dynamic data
2. Trigger a data update (filter, sort, add/remove data)
3. Listen for announcement
4. Verify:
   - [ ] Update is announced via live region
   - [ ] Announcement is clear and concise
   - [ ] Update doesn't interrupt current reading
   - [ ] Announcement timing is appropriate (not too fast/slow)

**Expected Announcements**:

```
"Data updated: 5 points"
```

or

```
"Filtered to: 10 of 50 points"
```

**Test Cases**:

- [ ] Add data points
- [ ] Remove data points
- [ ] Update existing point values
- [ ] Filter/sort data
- [ ] Change visualization type (if supported)

### Scenario 6: Keyboard-Only Navigation

**Objective**: Verify full functionality without mouse

**Steps**:

1. Unplug mouse or ignore it entirely
2. Navigate entire visualization using only keyboard
3. Perform all interactions
4. Verify:
   - [ ] All elements are reachable via keyboard
   - [ ] Tab order is logical
   - [ ] No keyboard traps (can always navigate away)
   - [ ] Focus indicators are always visible
   - [ ] All functions work (selection, activation, etc.)

**Common Issues to Check**:

- Elements that only respond to mouse hover
- Custom controls without keyboard handlers
- Focus lost after interaction
- Keyboard trap in nested controls

### Scenario 7: Touch Exploration (iOS VoiceOver)

**Objective**: Verify touch exploration works on mobile

**Steps**:

1. Enable VoiceOver on iOS device
2. Open visualization in Safari
3. Drag finger across screen to explore
4. Verify:
   - [ ] Elements are announced when touched
   - [ ] Touch areas are appropriately sized
   - [ ] Announcements don't overlap
   - [ ] Double-tap activates elements
   - [ ] Swipe gestures work for navigation

**Mobile-Specific Tests**:

- [ ] Portrait orientation
- [ ] Landscape orientation
- [ ] Different screen sizes (iPhone, iPad)
- [ ] Pinch-to-zoom (if supported)

## Recording Test Results

### Test Result Template

For each screen reader tested, record results using this template:

```markdown
## [Screen Reader Name] [Version] - [Date]

**Tester**: [Your Name] **Environment**:

- OS: [Windows 10/macOS 14.2/etc.]
- Browser: [Chrome 120/Safari 17/etc.]
- Example: [web_accessibility_demo]

### Test Results Summary

- **Overall Pass Rate**: X/Y tests passed (Z%)
- **Critical Issues**: N
- **Major Issues**: N
- **Minor Issues**: N

### Scenario Results

#### Scenario 1: Basic Navigation

- **Status**: ✅ Pass / ⚠️ Partial / ❌ Fail
- **Notes**: [Any observations, issues, or exceptional behavior]

#### Scenario 2: Screen Reader Navigation

- **Status**: ✅ Pass / ⚠️ Partial / ❌ Fail
- **Notes**: [...]

[Continue for all scenarios]

### Issues Found

#### Issue #1: [Title]

- **Severity**: Critical / Major / Minor
- **Scenario**: [Which scenario]
- **Description**: [What happened]
- **Expected**: [What should happen]
- **Reproduction**: [Steps to reproduce]
- **Screenshot/Recording**: [If available]

### Recommendations

[Any suggestions for improvements, patterns that worked well, etc.]
```

### Issue Severity Levels

**Critical**:

- Functionality completely inaccessible
- Screen reader crashes or freezes
- Navigation completely broken
- No announcements for key elements

**Major**:

- Functionality difficult to use
- Confusing or missing announcements
- Inconsistent behavior
- Keyboard trap in some scenarios

**Minor**:

- Suboptimal announcements (verbose, repetitive)
- Visual focus indicator could be improved
- Inconsistent terminology
- Minor navigation quirks

## Screen Reader Compatibility Matrix

Document compatibility with this matrix format:

| Feature            | NVDA | JAWS | VoiceOver (macOS) | VoiceOver (iOS) | Orca |
| ------------------ | ---- | ---- | ----------------- | --------------- | ---- |
| Basic Navigation   | ✅   | ✅   | ✅                | ⚠️              | ❓   |
| ARIA Announcements | ✅   | ✅   | ⚠️                | ⚠️              | ❓   |
| Keyboard Shortcuts | ✅   | ✅   | ✅                | N/A             | ❓   |
| Live Regions       | ⚠️   | ✅   | ⚠️                | ⚠️              | ❓   |
| Touch Exploration  | N/A  | N/A  | N/A               | ✅              | N/A  |

**Legend**:

- ✅ Fully supported
- ⚠️ Partially supported (note issues)
- ❌ Not supported
- ❓ Not yet tested
- N/A Not applicable

## Known Issues and Workarounds

### Issue: VoiceOver Announces Button Instead of Data Point

**Symptoms**: VoiceOver on macOS announces data points as "button" even though
ARIA role is "datapoint"

**Cause**: VoiceOver maps custom ARIA roles to nearest standard role

**Workaround**: Accept this behavior; "button" is semantically correct for
interactive elements

**Status**: Not a bug, expected behavior

### Issue: Live Regions Not Announced Immediately

**Symptoms**: NVDA sometimes delays live region announcements

**Cause**: Live region announcements queue behind current speech

**Workaround**: Use `aria-live="assertive"` for critical updates,
`aria-live="polite"` for informational

**Status**: Known NVDA behavior, working as designed

### Issue: Focus Lost After Dynamic Update

**Symptoms**: After data update, keyboard focus returns to top of page

**Cause**: DOM regeneration without focus management

**Workaround**: Track focused element before update, restore after update

**Status**: Implementation issue, should be fixed

## Automated Validation

While manual testing is required, use automated tools first to catch obvious
issues:

### axe DevTools

```bash
# Install browser extension
# Chrome: https://chrome.google.com/webstore/detail/axe-devtools/lhdoppojpmngadmnindnejefpokejbdd
# Firefox: https://addons.mozilla.org/en-US/firefox/addon/axe-devtools/

# Or use CLI
npm install -g @axe-core/cli
axe http://localhost:8080 --exit
```

### WAVE Browser Extension

- Chrome:
  https://chrome.google.com/webstore/detail/wave-evaluation-tool/jbbplnpkjmmeebjpijfedlgcdilocofh
- Firefox:
  https://addons.mozilla.org/en-US/firefox/addon/wave-accessibility-tool/

### Lighthouse Accessibility Audit

```bash
# In Chrome DevTools
# 1. Open DevTools (F12)
# 2. Go to Lighthouse tab
# 3. Select "Accessibility" category
# 4. Click "Generate report"
```

## Best Practices

### DO:

- ✅ Test with at least 2 different screen readers
- ✅ Test with keyboard only first
- ✅ Record audio/video of tests for reference
- ✅ Test on actual devices (especially mobile)
- ✅ Test with real users with disabilities if possible
- ✅ Document all issues, even minor ones
- ✅ Verify fixes with original screen reader

### DON'T:

- ❌ Rely only on automated tools
- ❌ Test only in virtual machines (performance issues)
- ❌ Skip testing after "minor" code changes
- ❌ Assume one screen reader = all screen readers
- ❌ Ignore user feedback about "quirky" behavior
- ❌ Test with muted screen reader (audio is key!)

## Resources

### Screen Reader Documentation

- **NVDA User Guide**:
  https://www.nvaccess.org/files/nvda/documentation/userGuide.html
- **JAWS Documentation**:
  https://support.freedomscientific.com/Documentation/JAWS
- **VoiceOver User Guide** (macOS):
  https://support.apple.com/guide/voiceover/welcome/mac
- **VoiceOver User Guide** (iOS):
  https://support.apple.com/guide/iphone/turn-on-and-practice-voiceover-iph3e2e415f/ios
- **Orca Documentation**: https://help.gnome.org/users/orca/stable/

### ARIA Best Practices

- **WAI-ARIA Authoring Practices**: https://www.w3.org/WAI/ARIA/apg/
- **ARIA Data Grid Pattern**: https://www.w3.org/WAI/ARIA/apg/patterns/grid/
- **ARIA Live Regions**: https://www.w3.org/WAI/ARIA/apg/practices/live-regions/

### Testing Resources

- **WebAIM Screen Reader Testing**:
  https://webaim.org/articles/screenreader_testing/
- **Deque Screen Reader Testing**:
  https://www.deque.com/blog/basic-screen-reader-commands-for-accessibility-testing/
- **A11y Project**: https://www.a11yproject.com/

## Reporting Test Results

After completing tests, update the following files:

1. **Compatibility Matrix**: `docs/ACCESSIBILITY_COMPATIBILITY.md`
   - Update matrix with test results
   - Note any browser-specific issues

2. **Known Issues**: `docs/ACCESSIBILITY_KNOWN_ISSUES.md`
   - Document any issues found
   - Include workarounds if available

3. **Story Document**:
   `docs/planning/stories/GUP-121_Screen_Reader_Manual_Testing.md`
   - Check off acceptance criteria
   - Add Implementation Summary section
   - List which screen readers were tested

## Contact

If you discover critical accessibility issues, please:

1. Open a GitHub issue with label `accessibility`
2. Include screen reader name and version
3. Attach screen recording if possible
4. Tag with severity level (critical/major/minor)

---

**Last Updated**: 2025-01-24 **Version**: 1.0
