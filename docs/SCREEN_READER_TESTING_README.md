# Screen Reader Testing Infrastructure

This directory contains comprehensive documentation and tools for manual screen
reader testing of Gup visualizations.

## Quick Start

1. **Review the Testing Guide**: `SCREEN_READER_TESTING.md`
   - Complete setup instructions for NVDA, JAWS, VoiceOver, Orca
   - 7 detailed test scenarios
   - Recording templates and best practices

2. **Run the Pre-Check Script**:
   ```bash
   ./scripts/screen_reader_precheck.sh web_accessibility_demo
   ```
   This validates your test environment is ready.

3. **Print the Checklist**: `SCREEN_READER_CHECKLIST.md`
   - Quick reference for testing sessions
   - Printable format with checkboxes

4. **Build and Serve Example**:
   ```bash
   # From repository root
   cd examples
   wasm-pack build --target web --out-dir ../pkg/web_accessibility_demo web_accessibility_demo
   cd ..
   mask serve web_accessibility_demo
   ```

5. **Test with Screen Reader**:
   - Enable your screen reader (NVDA, JAWS, VoiceOver)
   - Open http://localhost:8080 in your browser
   - Follow test scenarios from the guide

6. **Record Results**:
   - Update: `ACCESSIBILITY_COMPATIBILITY.md` with pass/fail status
   - Document issues in: `ACCESSIBILITY_KNOWN_ISSUES.md`
   - Update story: `planning/stories/GUP-121_Screen_Reader_Manual_Testing.md`

## Documentation Files

### `SCREEN_READER_TESTING.md` (16 KB)

The **comprehensive testing guide** with:
- Screen reader setup for NVDA, JAWS, VoiceOver (macOS/iOS), Orca
- 7 detailed test scenarios with expected announcements
- Test result recording templates
- Issue severity classification
- Best practices and resources

**When to use**: Your primary reference for conducting tests

### `ACCESSIBILITY_COMPATIBILITY.md` (10 KB)

The **compatibility matrix** tracking:
- Feature compatibility across all screen readers
- Browser compatibility per platform
- Platform-specific considerations
- Test status (✅/⚠️/❌/❓)

**When to use**: Recording test results and checking what's been tested

### `ACCESSIBILITY_KNOWN_ISSUES.md` (9 KB)

The **issue tracker** with:
- Critical, Major, and Minor issue templates
- Platform-specific quirks
- Browser-specific issues
- Issue lifecycle tracking

**When to use**: Documenting bugs and workarounds discovered during testing

### `SCREEN_READER_CHECKLIST.md` (7 KB)

The **quick reference checklist** with:
- 7 test scenarios as checklists
- Common issues to watch for
- Screen reader keyboard shortcuts
- Results summary template

**When to use**: During active testing sessions (print or view side-by-side)

## Scripts

### `scripts/screen_reader_precheck.sh`

Automated validation script that checks:
- ✓ WASM package is built
- ✓ Server is running
- ✓ Accessibility features are enabled
- ✓ HTML structure is correct
- ✓ Example compiles
- ✓ Documentation exists
- ✓ Runs axe-core if available

**Usage**:
```bash
./scripts/screen_reader_precheck.sh [example_name] [port]
```

**Example**:
```bash
./scripts/screen_reader_precheck.sh web_accessibility_demo 8080
```

## Test Scenarios Overview

1. **Basic Navigation** - Tab through elements, verify announcements
2. **Screen Reader Commands** - H/R/B/D keys, VO commands, iOS rotor
3. **Data Exploration** - Verify data values with context
4. **Interaction** - Test Enter/Space activation, verify feedback
5. **Keyboard Shortcuts** - Test all shortcuts, check conflicts
6. **Dynamic Updates** - Verify live region announcements
7. **Touch Exploration** - iOS VoiceOver gestures (optional)

Each scenario has detailed steps, expected announcements, and pass criteria.

## Screen Readers Supported

| Screen Reader | Platform | Cost | Priority | Status |
|---------------|----------|------|----------|--------|
| NVDA | Windows | Free | High | 📋 Ready to test |
| JAWS | Windows | Commercial | High | 📋 Ready to test |
| VoiceOver | macOS | Built-in | High | 📋 Ready to test |
| VoiceOver | iOS | Built-in | Medium | 📋 Ready to test |
| Orca | Linux | Free | Low | 📋 Ready to test |

## Browser Compatibility

### Windows (NVDA + JAWS)
- Chrome ✓
- Firefox ✓
- Edge ✓

### macOS (VoiceOver)
- Safari ✓ (recommended)
- Chrome ✓
- Firefox ✓

### iOS (VoiceOver)
- Safari ✓

### Linux (Orca)
- Firefox ✓ (recommended)
- Chrome/Chromium ✓

## Testing Workflow

```
┌─────────────────────────────────────────────────┐
│ 1. Review SCREEN_READER_TESTING.md             │
│    (Understand what to test and how)            │
└───────────────────┬─────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ 2. Run scripts/screen_reader_precheck.sh       │
│    (Validate environment)                       │
└───────────────────┬─────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ 3. Print SCREEN_READER_CHECKLIST.md            │
│    (Quick reference during testing)             │
└───────────────────┬─────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ 4. Enable Screen Reader + Open Browser         │
│    (Start testing)                              │
└───────────────────┬─────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ 5. Follow Test Scenarios                       │
│    (Execute 7 scenarios, take notes)            │
└───────────────────┬─────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ 6. Update ACCESSIBILITY_COMPATIBILITY.md       │
│    (Record pass/fail for each feature)          │
└───────────────────┬─────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ 7. Document Issues in KNOWN_ISSUES.md          │
│    (Log bugs, workarounds, quirks)              │
└───────────────────┬─────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ 8. Update Story GUP-121                         │
│    (Check off AC, add summary)                  │
└─────────────────────────────────────────────────┘
```

## Automated Validation (Pre-Test)

Before manual testing, run automated checks:

### axe-core (Recommended)
```bash
npm install -g @axe-core/cli
axe http://localhost:8080 --exit
```

### Lighthouse (Chrome DevTools)
1. Open DevTools (F12)
2. Go to Lighthouse tab
3. Select "Accessibility" category
4. Click "Generate report"

### WAVE Browser Extension
- Chrome: https://chrome.google.com/webstore/detail/wave-evaluation-tool/
- Firefox: https://addons.mozilla.org/firefox/addon/wave-accessibility-tool/

## Success Criteria

A screen reader is considered **fully supported** when:
- ✅ All 7 test scenarios pass
- ✅ No critical issues found
- ✅ Data is comprehensible from audio alone
- ✅ All interactions work via keyboard
- ✅ Live regions announce updates appropriately

## Getting Help

### Screen Reader Not Working?

1. Check `SCREEN_READER_TESTING.md` setup section
2. Run `scripts/screen_reader_precheck.sh`
3. Verify browser is compatible
4. Test with a different example first
5. Check screen reader audio output settings

### Found a Bug?

1. Document in `ACCESSIBILITY_KNOWN_ISSUES.md` using templates
2. Classify severity: Critical/Major/Minor
3. Include screen reader name/version, browser, OS
4. Record steps to reproduce
5. Create GitHub issue with `accessibility` label

### Need More Examples?

The current test suite uses:
- `examples/web_accessibility_demo.rs` - Basic chart with 5 data points

Additional examples will be added in future stories.

## Testing Status

**Overall**: 🎯 Ready for Manual Testing

| Screen Reader | Status | Tester | Date |
|---------------|--------|--------|------|
| NVDA | ❓ Not tested | - | - |
| JAWS | ❓ Not tested | - | - |
| VoiceOver (macOS) | ❓ Not tested | - | - |
| VoiceOver (iOS) | ❓ Not tested | - | - |
| Orca | ❓ Not tested | - | - |

**Legend**:
- ✅ Fully tested, all scenarios pass
- ⚠️ Tested with issues
- ❌ Critical issues found
- ❓ Not yet tested

## Contributing Test Results

When you complete testing:

1. **Update Compatibility Matrix**: Change ❓ to ✅/⚠️/❌ in
   `ACCESSIBILITY_COMPATIBILITY.md`
2. **Document Issues**: Add any problems found to
   `ACCESSIBILITY_KNOWN_ISSUES.md`
3. **Update Story**: Check off applicable boxes in
   `planning/stories/GUP-121_Screen_Reader_Manual_Testing.md`
4. **Commit Changes**: Create PR with your test results
5. **Share Findings**: Comment on GUP-121 story with summary

## Resources

### Official Documentation
- NVDA: https://www.nvaccess.org/files/nvda/documentation/userGuide.html
- JAWS: https://support.freedomscientific.com/Documentation/JAWS
- VoiceOver (Mac): https://support.apple.com/guide/voiceover/welcome/mac
- VoiceOver (iOS): https://support.apple.com/guide/iphone/voiceover/
- Orca: https://help.gnome.org/users/orca/stable/

### ARIA Best Practices
- WAI-ARIA Authoring Practices: https://www.w3.org/WAI/ARIA/apg/
- ARIA Live Regions: https://www.w3.org/WAI/ARIA/apg/practices/live-regions/

### Testing Guides
- WebAIM: https://webaim.org/articles/screenreader_testing/
- Deque: https://www.deque.com/blog/screen-reader-testing/

## Story Tracking

**Story**: GUP-121: Screen Reader Manual Testing  
**Status**: 🎯 Ready for Manual Testing  
**Location**: `docs/planning/stories/GUP-121_Screen_Reader_Manual_Testing.md`

**Remaining Work**:
- Manual testing with NVDA (Windows)
- Manual testing with JAWS (Windows) - if available
- Manual testing with VoiceOver (macOS)
- Manual testing with VoiceOver (iOS) - if available
- Documenting test results
- Fixing any critical/major issues found

---

**Version**: 1.0  
**Last Updated**: 2025-01-24  
**Maintained By**: Gup Accessibility Team
