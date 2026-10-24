# Accessibility Testing Guide for Gup Consumers

This guide explains how to validate WCAG 2.1 AA compliance in applications built
with Gup. It covers automated scanning, screen reader testing, keyboard
navigation verification, and colour contrast checking.

## Quick Start

Run the built-in accessibility test suite to verify the library itself:

```bash
cargo test accessibility -- --test-threads=1
```

For your own application, follow the sections below to validate the complete
user experience.

---

## 1. Automated Scanning (Web/WASM Builds)

If your application uses Gup's WASM target with a DOM overlay, use
[axe-core](https://github.com/dequelabs/axe-core) to scan the generated HTML for
accessibility violations.

### Setup

```bash
npm install -g @axe-core/cli
# or use Playwright for headless scanning
npm install -D @playwright/test axe-playwright
```

### Running axe-core

```bash
# Serve your WASM application
cd your-app && npm run serve &

# Scan with axe-core CLI
axe http://localhost:8080 --tags wcag2a,wcag2aa --exit
```

The `--tags wcag2a,wcag2aa` flag restricts checks to WCAG 2.1 Level A and AA
criteria. The `--exit` flag returns a non-zero exit code on violations.

### Interpreting Results

axe-core reports violations with:

- **Impact**: critical, serious, moderate, minor
- **Rule ID**: e.g., `color-contrast`, `aria-roles`, `label`
- **HTML snippet**: The violating element
- **Fix suggestion**: How to resolve

Focus on **critical** and **serious** violations first. These are the most
likely to affect real users.

---

## 2. Screen Reader Testing

Screen reader testing verifies that assistive technologies can announce chart
content meaningfully. Gup generates an ARIA tree that screen readers traverse.

### Recommended Screen Readers

| Platform | Screen Reader  | Notes                                     |
| -------- | -------------- | ----------------------------------------- |
| Windows  | NVDA (free)    | Download from <https://www.nvaccess.org/> |
| Windows  | JAWS           | Commercial; industry standard             |
| macOS    | VoiceOver      | Built-in; Cmd+F5 to toggle                |
| Linux    | Orca           | Included in GNOME; uses AT-SPI2           |
| Web      | NVDA + Firefox | Best combination for web testing          |

### Testing Protocol

This protocol is adapted from Gup's internal screen reader testing (GUP-122).

#### Step 1: Launch with Screen Reader Active

```bash
# Build and run an example (e.g., scatter_plot_demo)
cargo run --example scatter_plot_demo
```

Ensure your screen reader is running before launching the application.

#### Step 2: Navigate the Chart

1. **Tab** into the chart — the screen reader should announce the chart title
   and description (e.g., "Scatter plot: Sales data over time, image")
2. **Tab** through data series — each series should be announced as a list
   (e.g., "Series 1, list, 50 items")
3. **Tab** through data points — each point should announce its value (e.g.,
   "Data point: Revenue $42K, list item")
4. **Arrow keys** for spatial navigation — moving right/left/up/down should
   announce adjacent data points
5. **Escape** to exit the chart context

#### Step 3: Verify Announcements

Check that:

- [ ] Chart title is announced on focus
- [ ] Series names are announced
- [ ] Individual data point values are announced
- [ ] Navigation direction matches visual layout
- [ ] Escape exits the chart context without trapping focus
- [ ] Dynamic updates (if any) are announced via live regions

#### Step 4: Record Results

Document any issues found with:

- Screen reader name and version
- Browser/OS version
- Steps to reproduce
- Expected vs. actual announcement

---

## 3. Keyboard Navigation Testing

Gup provides three keyboard navigation modes. Test each one:

### Sequential Navigation (Tab/Shift-Tab)

1. Press **Tab** to enter the chart
2. Continue pressing **Tab** — focus should move through elements in data order
3. Press **Shift-Tab** to move backwards
4. After the last element, **Tab** should wrap to the first (no trap)
5. **Escape** should exit the chart

### Spatial Navigation (Arrow Keys)

1. Enter the chart and press an **arrow key**
2. Focus should move to the nearest element in that direction
3. All four directions should work
4. Visual focus indicator should be clearly visible

### Data Dimension Navigation

1. If enabled, **Up/Down** arrows cycle through data dimensions
2. **Left/Right** navigate within the current dimension
3. The screen reader should announce the current dimension and value

### What to Check

- [ ] All interactive elements are reachable via keyboard
- [ ] Focus order is logical and predictable
- [ ] Focus indicator is visible (high contrast ring)
- [ ] No keyboard traps — you can always Tab or Escape away
- [ ] Enter/Space activates the focused element

---

## 4. Colour Contrast Checking

Gup provides built-in contrast checking functions. Use them to validate your
application's colour choices.

### Programmatic Checking

```rust
use gup::accessibility::high_contrast::{
    Color, calculate_contrast_ratio, meets_wcag_aa, meets_wcag_aa_large_text,
};

// Check your theme colours
let text = Color::new(0.2, 0.2, 0.2, 1.0);    // Dark gray text
let bg = Color::new(1.0, 1.0, 1.0, 1.0);       // White background

let ratio = calculate_contrast_ratio(text, bg);
println!("Contrast ratio: {:.1}:1", ratio);

assert!(meets_wcag_aa(text, bg),
    "Text colour does not meet WCAG AA (4.5:1)");

// For large text or UI components, use the 3:1 threshold
let accent = Color::new(0.0, 0.48, 0.80, 1.0);  // Blue accent
assert!(meets_wcag_aa_large_text(accent, bg),
    "UI component does not meet WCAG AA (3:1)");
```

### Visual Checking Tools

For checking rendered output:

- **Colour Contrast Analyser** (CCA) —
  <https://www.tpgi.com/color-contrast-checker/> (free, Windows/macOS)
- **WebAIM Contrast Checker** — <https://webaim.org/resources/contrastchecker/>
- **Chrome DevTools** — Inspect element → Styles → contrast ratio display

### Using Gup's Contrast Modes

Test your application with each of Gup's built-in contrast modes:

```rust
use gup::accessibility::{AccessibilitySystem, ContrastMode};

let mut a11y = AccessibilitySystem::new();

// Test each mode
for mode in [
    ContrastMode::Standard,
    ContrastMode::HighContrast,
    ContrastMode::Colorblind,
    ContrastMode::LowVision,
    ContrastMode::Pattern,
] {
    a11y.set_contrast_mode(mode);
    // Render and visually inspect
}
```

---

## 5. Worked Example: Scatter Plot Demo

This example walks through a complete accessibility validation of the
`scatter_plot_demo` example.

### Build and Run

```bash
cargo run --example scatter_plot_demo
```

### Automated Check

```bash
# Run the accessibility unit tests
cargo test accessibility -- --test-threads=1
# Expected: all tests pass
```

### Keyboard Navigation Check

1. Launch the example
2. Press **Tab** — focus should enter the chart
3. Press **Tab** repeatedly — focus moves through data points
4. Press **Arrow Right** — focus moves to the nearest point to the right
5. Press **Escape** — focus exits the chart
6. Verify the focus ring (yellow highlight) is visible on each focused point

### Contrast Check

```rust
use gup::accessibility::high_contrast::{Color, meets_wcag_aa};

// Default scatter plot colours (blue points on white background)
let point_color = Color::new(0.12, 0.47, 0.71, 1.0);  // Steel blue
let background = Color::WHITE;

assert!(meets_wcag_aa(point_color, background));
// ✅ Passes: contrast ratio ≈ 5.2:1
```

### Screen Reader Check

With NVDA or VoiceOver active:

1. Tab to the chart — should announce "Scatter plot, image"
2. Tab to first series — should announce "Series 1, list"
3. Tab to first point — should announce the data value
4. Arrow through points — each should be announced

---

## Checklist Summary

Use this checklist when validating your Gup-powered application:

- [ ] `cargo test accessibility` passes
- [ ] axe-core scan produces zero critical/serious violations (web builds)
- [ ] Screen reader announces chart title, series, and data points
- [ ] All elements reachable via keyboard (Tab, arrows)
- [ ] No keyboard traps
- [ ] Focus indicator clearly visible
- [ ] Text contrast ≥ 4.5:1 (normal) or ≥ 3:1 (large/UI)
- [ ] Information not conveyed by colour alone
- [ ] High contrast mode renders correctly
- [ ] Colorblind mode provides distinguishable palette

---

## Further Resources

- [WCAG 2.1 Specification](https://www.w3.org/TR/WCAG21/)
- [WAI-ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/)
- [Gup WCAG 2.1 AA Audit](./WCAG_2_1_AA_AUDIT.md)
- [Gup Conformance Statement](./WCAG_2_1_AA_CONFORMANCE.md)
- [Gup CI Accessibility Checks](./CI_ACCESSIBILITY.md)
