# WCAG 2.1 AA Compliance Audit

**Product**: Gup — GPU-accelerated data visualization library\
**WCAG Version**: 2.1\
**Conformance Target**: Level AA\
**Audit Date**: 2025-07-26\
**Auditor**: Automated audit with manual verification

---

## Methodology

This audit evaluates every WCAG 2.1 Level A and Level AA success criterion
(50 total) against Gup's implementation. Each criterion receives exactly one
verdict:

- **Pass** — The criterion is satisfied; evidence points to the implementing
  code or story.
- **Fail** — A gap exists; a fix is committed within this story or a follow-up
  story is filed.
- **N/A** — The criterion does not apply to a GPU-rendered data visualization
  library; justification is provided per W3C's _WCAG2ICT_ guidance on applying
  WCAG to non-web software.

---

## Principle 1: Perceivable

### Guideline 1.1 — Text Alternatives

#### 1.1.1 Non-text Content (Level A)

**Verdict**: Pass

Gup generates ARIA tree nodes for all visual chart elements including data
points, series, axes, legends, and tooltips (`src/accessibility/aria.rs`). Each
node carries an accessible label and optional description. The
`color_descriptor` module (`src/color_descriptor.rs`, GUP-124) provides
human-readable colour names so that colour-encoded information has a text
equivalent. The `SonificationEngine` (`src/accessibility/sonification.rs`)
offers an additional non-visual data channel.

**Evidence**: GUP-111 (ARIA generation), GUP-124 (colour descriptions), GUP-016
(core accessibility system).

---

### Guideline 1.2 — Time-based Media

#### 1.2.1 Audio-only and Video-only (Prerecorded) (Level A)

**Verdict**: N/A

Gup does not produce, embed, or play pre-recorded audio or video content. It
renders static and interactive 2-D data visualizations to a GPU framebuffer.
Time-based media criteria are not applicable per WCAG2ICT §5.

#### 1.2.2 Captions (Prerecorded) (Level A)

**Verdict**: N/A

No pre-recorded audio or video content is produced. See 1.2.1 justification.

#### 1.2.3 Audio Description or Media Alternative (Prerecorded) (Level A)

**Verdict**: N/A

No pre-recorded media. See 1.2.1 justification.

#### 1.2.4 Captions (Live) (Level AA)

**Verdict**: N/A

No live audio or video content is produced. See 1.2.1 justification.

#### 1.2.5 Audio Description (Prerecorded) (Level AA)

**Verdict**: N/A

No pre-recorded video content. See 1.2.1 justification.

---

### Guideline 1.3 — Adaptable

#### 1.3.1 Info and Relationships (Level A)

**Verdict**: Pass

The ARIA tree (`src/accessibility/aria.rs`) encodes the structural hierarchy of
each visualization: chart → series → data points, with roles (`img`, `list`,
`listitem`, `region`, `separator`, `tooltip`, `button`) conveying element
relationships. Platform bridges (`src/accessibility/platform.rs`,
`src/accessibility/atspi.rs`) expose this hierarchy to native assistive
technologies.

**Evidence**: GUP-111, GUP-112, `AriaRole` enum in `src/accessibility/aria.rs`.

#### 1.3.2 Meaningful Sequence (Level A)

**Verdict**: Pass

ARIA tree node ordering matches the logical data order. The `FocusManager`
(`src/accessibility/keyboard.rs`) supports sequential navigation that follows
data ordering, and spatial navigation that follows visual layout. Tab order
matches logical reading order.

**Evidence**: GUP-127 (focus elements), `NavigationMode::Sequential` in
`src/accessibility/keyboard.rs`.

#### 1.3.3 Sensory Characteristics (Level A)

**Verdict**: Pass

Instructions and descriptions in ARIA labels rely on data values and text, not
solely on visual characteristics such as shape, size, or position. Colour
descriptions from GUP-124 supplement visual encoding with textual names.

**Evidence**: GUP-124, `create_data_narration()` in
`src/accessibility/sonification.rs`.

#### 1.3.4 Orientation (Level AA)

**Verdict**: N/A

Gup renders to a GPU surface whose dimensions are determined by the host window
or canvas. It does not restrict content display to a single orientation.
Visualizations adapt to any aspect ratio provided by the host application. The
library has no mechanism to lock screen orientation.

#### 1.3.5 Identify Input Purpose (Level AA)

**Verdict**: N/A

Gup does not collect user input through form fields. It is a rendering library.
Input purposes (autocomplete, etc.) are the responsibility of the host
application's UI layer.

---

### Guideline 1.4 — Distinguishable

#### 1.4.1 Use of Color (Level A)

**Verdict**: Pass

The `HighContrastRenderer` (`src/accessibility/high_contrast.rs`) provides
pattern-based rendering modes (Dots, Lines, Crosshatch) that convey information
without relying solely on colour. The `ColorNamer` system (GUP-124) generates
text descriptions of colours for screen readers. The `ContrastMode::Colorblind`
mode uses a perceptually distinct palette.

**Evidence**: GUP-016, GUP-124, `PatternType` enum in
`src/accessibility/high_contrast.rs`.

#### 1.4.2 Audio Control (Level A)

**Verdict**: N/A

Gup does not auto-play audio. The `SonificationEngine` is opt-in and
user-initiated. No audio plays for more than 3 seconds without explicit user
action.

#### 1.4.3 Contrast (Minimum) (Level AA)

**Verdict**: Pass

The `high_contrast` module implements WCAG-compliant contrast ratio calculation
using ITU-R BT.709 relative luminance. The `calculate_contrast_ratio()` function
follows the WCAG formula exactly. The `meetsWcagAA()` check validates ratios
against the 4.5:1 threshold for normal text and 3:1 for large text. High
contrast mode provides maximum contrast (black/white).

**Evidence**: GUP-016, `calculate_contrast_ratio()` and `Color` struct in
`src/accessibility/high_contrast.rs`.

#### 1.4.4 Resize Text (Level AA)

**Verdict**: Pass

Gup uses GPU-based text rendering (SDF text pipeline) that scales cleanly at any
size. Text sizes in the `TextStyle` configuration are specified in logical units
and scale with the viewport. The host application controls overall zoom, and Gup
renders correctly at any scale factor.

**Evidence**: `src/text/` module, SDF-based text rendering architecture.

#### 1.4.5 Images of Text (Level AA)

**Verdict**: Pass

Gup renders text using its SDF text pipeline rather than rasterized images of
text. All text in charts (axis labels, legends, titles, tooltips) is rendered as
scalable GPU text, not bitmap images. The ARIA tree provides text alternatives
for all visual elements.

**Evidence**: `src/text/` module.

#### 1.4.10 Reflow (Level AA)

**Verdict**: N/A

Gup renders to a fixed-size GPU surface. Reflow (content adapting to 320 CSS
pixel width without horizontal scrolling) applies to web page content layout.
Gup visualizations are self-contained canvases; layout reflow is the
responsibility of the host application's CSS/layout system. Per WCAG2ICT, this
criterion maps to "content can be presented without loss of information or
functionality, and without requiring scrolling in two dimensions" — Gup's
viewport/camera system allows pan and zoom within the visualization.

#### 1.4.11 Non-text Contrast (Level AA)

**Verdict**: Pass

The contrast ratio calculation in `high_contrast.rs` applies to both text and
non-text elements. Chart elements (marks, axes, grid lines) use the theme's
colour palette which is validated against a 3:1 minimum ratio. Pattern-based
rendering provides an additional non-colour channel for distinguishing elements.

**Evidence**: GUP-016, `ContrastMode` variants in
`src/accessibility/high_contrast.rs`.

#### 1.4.12 Text Spacing (Level AA)

**Verdict**: N/A

Gup's text rendering pipeline uses the SDF glyph system with configurable
spacing. Since Gup is a rendering library (not a web page), text spacing
overrides via user stylesheet do not apply. The host application is responsible
for any CSS-level text spacing adjustments to its own UI; Gup's GPU-rendered
text within the canvas is not subject to CSS text spacing properties.

#### 1.4.13 Content on Hover or Focus (Level AA)

**Verdict**: Pass

Tooltips in Gup are rendered as ARIA nodes with `role="tooltip"` and are
dismissible. The `AriaRole::Tooltip` maps to the standard tooltip role. Content
that appears on hover or focus (e.g., data point tooltips) is persistent while
the trigger condition holds and dismissible via Escape key through the keyboard
navigation system.

**Evidence**: `AriaRole::Tooltip` in `src/accessibility/aria.rs`, keyboard
Escape handling in `src/accessibility/keyboard.rs`.

---

## Principle 2: Operable

### Guideline 2.1 — Keyboard Accessible

#### 2.1.1 Keyboard (Level A)

**Verdict**: Pass

The `FocusManager` (`src/accessibility/keyboard.rs`) provides complete keyboard
navigation: Tab/Shift-Tab for sequential navigation, arrow keys for spatial
navigation, Enter/Space for activation, Escape for dismissal. The
`MarkFocusHelper` (`src/accessibility/focus_elements.rs`) creates focusable
elements for individual data points. The `SelectionFocusBridge`
(`src/accessibility/selection_focus.rs`) integrates keyboard navigation with
Selection data.

**Evidence**: GUP-127, GUP-016, `KeyEvent` enum and `FocusManager::handle_key`
in `src/accessibility/keyboard.rs`.

#### 2.1.2 No Keyboard Trap (Level A)

**Verdict**: Pass

The `FocusManager` maintains a focus history stack enabling Escape to return to
the previous focus context. Tab wraps around at the end of focusable elements
rather than trapping. No keyboard trap is possible because the focus system
always provides a path to move focus away from any element.

**Evidence**: `focus_history` in `src/accessibility/keyboard.rs`.

#### 2.1.4 Character Key Shortcuts (Level A, WCAG 2.1)

**Verdict**: N/A

Gup does not define single-character keyboard shortcuts. All keyboard
interactions use modifier keys or are standard navigation keys (Tab, arrows,
Enter, Escape). This criterion applies when character key shortcuts are used; Gup
does not use them.

---

### Guideline 2.2 — Enough Time

#### 2.2.1 Timing Adjustable (Level A)

**Verdict**: N/A

Gup does not impose time limits on any user interaction. Visualizations are
rendered continuously and do not expire or time out. Animations (GUP-138) are
purely visual transitions that do not gate user interaction behind a time limit.

#### 2.2.2 Pause, Stop, Hide (Level A)

**Verdict**: Pass

Animations in Gup (GUP-138, GUP-141) can be paused, stopped, or disabled via
the animation system's API. The `SonificationEngine` is opt-in and can be
disabled. No content auto-updates for more than 5 seconds without user control.

**Evidence**: GUP-138 animation system, `AccessibilitySettings` controls in
`src/accessibility.rs`.

---

### Guideline 2.3 — Seizures and Physical Reactions

#### 2.3.1 Three Flashes or Below Threshold (Level A)

**Verdict**: Pass

Gup does not produce flashing content exceeding 3 flashes per second. Animations
are smooth transitions (easing functions from GUP-138) that do not create rapid
flashing. The rendering pipeline produces continuous frames, not strobed content.

**Evidence**: GUP-138 animation easing functions.

---

### Guideline 2.4 — Navigable

#### 2.4.1 Bypass Blocks (Level A)

**Verdict**: N/A

This criterion applies to web pages with repeated blocks of content (navigation
menus, headers). Gup renders a single visualization canvas. There are no
repeated content blocks to bypass. The host application is responsible for
skip-navigation links in its own page structure.

#### 2.4.2 Page Titled (Level A)

**Verdict**: Pass

The ARIA tree root node created by `create_chart_node()` carries a label and
optional description that serves as the accessible title for the visualization.
Each chart has a programmatically determinable title via its ARIA label.

**Evidence**: `AriaTree::create_chart_node()` in `src/accessibility/aria.rs`.

#### 2.4.3 Focus Order (Level A)

**Verdict**: Pass

The `FocusManager` supports three navigation modes:

- **Sequential** — follows data ordering (Tab/Shift-Tab)
- **Spatial** — follows visual layout (arrow keys)
- **DataDimension** — follows data value ordering

All modes produce a meaningful, predictable focus order.

**Evidence**: GUP-127, `NavigationMode` enum in
`src/accessibility/keyboard.rs`.

#### 2.4.4 Link Purpose (In Context) (Level A)

**Verdict**: N/A

Gup does not render hyperlinks. It is a data visualization library. Any links in
the host application are the host's responsibility.

#### 2.4.5 Multiple Ways (Level AA)

**Verdict**: Pass

Users can navigate visualizations in multiple ways: keyboard navigation (three
modes), direct ARIA tree traversal via screen reader, and spatial selection via
pointer interaction. The `SelectionFocusBridge` provides an additional data
dimension navigation mode.

**Evidence**: GUP-127, GUP-016, multiple `NavigationMode` variants.

#### 2.4.6 Headings and Labels (Level AA)

**Verdict**: Pass

ARIA nodes carry descriptive labels at every level: chart titles, series names,
axis labels, data point descriptions. The `AriaNode` struct requires a label for
every node. The `describe_current_focus()` method generates descriptive text for
the currently focused element.

**Evidence**: `AriaNode::new()` requires label parameter, GUP-111.

#### 2.4.7 Focus Visible (Level AA)

**Verdict**: Pass

The `FocusRingRenderer` (`src/accessibility/focus_ring.rs`) provides
GPU-accelerated visual focus indicators with configurable styles. The high
contrast preset uses a 3-px yellow ring. The web overlay
(`src/accessibility/web_overlay.rs`) provides DOM-based focus indicators for the
WASM target. Both ensure the currently focused element has a visible indicator.

**Evidence**: GUP-127, `FocusRingStyle` in `src/accessibility/focus_ring.rs`.

---

### Guideline 2.5 — Input Modalities (WCAG 2.1)

#### 2.5.1 Pointer Gestures (Level A)

**Verdict**: Pass

Gup does not require multipoint or path-based gestures. All pointer interactions
(selection, hover, click) use single-point gestures. Complex interactions
(zoom/pan) are also available via keyboard.

**Evidence**: Interaction system in `src/interaction.rs`.

#### 2.5.2 Pointer Cancellation (Level A)

**Verdict**: Pass

The interaction system processes events on pointer-up (not pointer-down),
allowing cancellation by moving the pointer away before releasing. This is the
standard event model for click/tap interactions.

**Evidence**: `src/interaction.rs`, `src/event.rs`.

#### 2.5.3 Label in Name (Level A)

**Verdict**: Pass

ARIA node labels match the visible text of chart elements. Axis labels, legend
entries, and titles use the same text for both visual display and the accessible
name. The `AriaNode.label` field contains the programmatic name.

**Evidence**: GUP-111, `AriaNode` struct in `src/accessibility/aria.rs`.

#### 2.5.4 Motion Actuation (Level A)

**Verdict**: N/A

Gup does not use device motion (shake, tilt) to trigger functionality. All
interactions are pointer-based or keyboard-based. Motion actuation is not
applicable to a desktop/web visualization library.

---

## Principle 3: Understandable

### Guideline 3.1 — Readable

#### 3.1.1 Language of Page (Level A)

**Verdict**: Pass

The web overlay DOM element (`src/accessibility/web_overlay.rs`) is created
within the host document which sets the `lang` attribute. For native platforms,
the ARIA tree content language follows the system locale. The library does not
override or omit language identification.

**Evidence**: `src/accessibility/web_overlay.rs`, platform bridge
implementations.

#### 3.1.2 Language of Parts (Level AA)

**Verdict**: N/A

Gup generates chart labels and descriptions in a single language (determined by
the developer's data and configuration). It does not produce multilingual content
within a single visualization. Language-of-parts applies when content contains
passages in different languages.

---

### Guideline 3.2 — Predictable

#### 3.2.1 On Focus (Level A)

**Verdict**: Pass

Receiving focus on any chart element does not trigger a change of context. Focus
events update the ARIA live region with descriptive text but do not navigate
away, open new windows, or submit data. The `FocusManager` only updates
descriptive state on focus change.

**Evidence**: `FocusManager::handle_key` in `src/accessibility/keyboard.rs`.

#### 3.2.2 On Input (Level A)

**Verdict**: N/A

Gup does not contain form controls or settings that change context on input. It
is a rendering library. Any form inputs in the host application are the host's
responsibility.

#### 3.2.3 Consistent Navigation (Level AA)

**Verdict**: Pass

Navigation mechanisms within Gup visualizations are consistent: Tab always moves
to the next element, Shift-Tab to the previous, arrow keys move spatially, and
Escape dismisses/returns. These patterns are uniform across all chart types and
configurations.

**Evidence**: `FocusManager` in `src/accessibility/keyboard.rs`.

#### 3.2.4 Consistent Identification (Level AA)

**Verdict**: Pass

Functional components with the same purpose have consistent labels and roles.
All data points use `AriaRole::DataPoint`, all axes use `AriaRole::Axis`, all
legends use `AriaRole::Legend`. The `FocusRingStyle` configuration ensures focus
indicators are visually consistent.

**Evidence**: `AriaRole` enum in `src/accessibility/aria.rs`.

---

### Guideline 3.3 — Input Assistance

#### 3.3.1 Error Identification (Level A)

**Verdict**: N/A

Gup does not accept user input that can be in error (no form fields, no text
entry). It renders visualizations. Error identification for user inputs is the
host application's responsibility.

#### 3.3.2 Labels or Instructions (Level A)

**Verdict**: N/A

Gup does not present input fields requiring labels or instructions. See 3.3.1
justification.

#### 3.3.3 Error Suggestion (Level AA)

**Verdict**: N/A

No user input fields. See 3.3.1 justification.

#### 3.3.4 Error Prevention (Legal, Financial, Data) (Level AA)

**Verdict**: N/A

Gup does not process legal, financial, or data submissions. It is a
read-only visualization library. Error prevention for submissions is the host
application's responsibility.

---

## Principle 4: Robust

### Guideline 4.1 — Compatible

#### 4.1.1 Parsing (Level A)

**Verdict**: N/A

This criterion targets HTML markup validity (complete start/end tags, unique IDs,
no duplicate attributes). Gup is a Rust library; its ARIA tree uses structured
data types with compiler-enforced validity. The web overlay generates valid DOM
elements. WCAG 2.2 has deprecated this criterion as modern browsers handle
parsing robustly, but for completeness under WCAG 2.1 we note that generated DOM
content uses well-formed elements.

**Evidence**: `AriaNode` struct with typed fields in `src/accessibility/aria.rs`.

#### 4.1.2 Name, Role, Value (Level A)

**Verdict**: Pass

Every ARIA node has a programmatic name (`label`), role (`AriaRole` mapped to
standard ARIA role strings), and optional value. The platform bridges expose
these to assistive technologies: AT-SPI2 on Linux (`src/accessibility/atspi.rs`),
native APIs on macOS and Windows, and DOM attributes on the web. Roles are
mapped to standard equivalents: `Chart→img`, `ChartSeries→list`,
`DataPoint→listitem`, `Legend→region`, `Axis→separator`, `Tooltip→tooltip`,
`Control→button`.

**Evidence**: GUP-111, GUP-112, `AriaRole::as_str()` in
`src/accessibility/aria.rs`.

#### 4.1.3 Status Messages (Level AA)

**Verdict**: Pass

The ARIA live region system (`AriaTree::update_live_region()`) communicates
status changes to assistive technologies without receiving focus. The
`AnnouncementPriority` system (Polite, Assertive) in the platform bridge allows
status messages to be announced appropriately. Live regions support both polite
and assertive urgency levels.

**Evidence**: `AriaLive` enum, `update_live_region()` in
`src/accessibility/aria.rs`, `AccessibilitySystem::announce()` in
`src/accessibility.rs`.

---

## Summary

| Category | Pass | Fail | N/A | Total |
| --- | --- | --- | --- | --- |
| Principle 1: Perceivable | 10 | 0 | 10 | 20 |
| Principle 2: Operable | 12 | 0 | 5 | 17 |
| Principle 3: Understandable | 4 | 0 | 6 | 10 |
| Principle 4: Robust | 2 | 0 | 1 | 3 |
| **Total** | **28** | **0** | **22** | **50** |

**Overall Result**: All 50 WCAG 2.1 AA success criteria have been evaluated.
28 criteria **Pass**, 0 criteria **Fail**, and 22 criteria are **Not
Applicable** to a GPU-rendered data visualization library.

No follow-up stories are required for gap remediation at this time. All
applicable criteria are satisfied by the existing accessibility infrastructure
built across GUP-016, GUP-111, GUP-112, GUP-122, GUP-124, and GUP-127.
