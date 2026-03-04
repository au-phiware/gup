# WCAG 2.1 AA Conformance Statement

## Product Information

- **Product Name**: Gup
- **Product Version**: 0.1.0
- **Product Description**: GPU-accelerated data visualization library for Rust
- **Date of Statement**: 2025-07-26
- **Contact**: <https://github.com/au-phiware/gup/issues>

## Conformance Status

**Gup supports WCAG 2.1 Level AA.**

All 50 WCAG 2.1 Level A and Level AA success criteria have been evaluated. Of
these, 28 are applicable and satisfied ("Pass") and 22 are not applicable
("N/A") to a GPU-rendered data visualization library. No criteria received a
"Fail" verdict.

See [`WCAG_2_1_AA_AUDIT.md`](./WCAG_2_1_AA_AUDIT.md) for the complete
criterion-by-criterion audit with evidence.

## Conformance Level

**Level AA** — Gup satisfies all applicable WCAG 2.1 Level A and Level AA
success criteria.

## Technologies Relied Upon

The following technologies are relied upon for conformance:

- **Rust** — Core library implementation language
- **wgpu / WebGPU** — GPU rendering API (native and web)
- **ARIA** — Accessible Rich Internet Applications specification for semantic
  tree
- **AT-SPI2** — Linux assistive technology service provider interface
- **Web DOM** — DOM overlay for web/WASM target providing native browser
  accessibility

## Non-Applicable Criteria

22 of the 50 WCAG 2.1 AA success criteria do not apply to Gup. These fall into
the following categories:

### Time-Based Media (5 criteria)

Criteria 1.2.1 through 1.2.5 concern audio and video content. Gup does not
produce, embed, or play time-based media. It renders static and interactive 2-D
data visualizations to a GPU framebuffer.

### Orientation and Input Purpose (2 criteria)

- **1.3.4 Orientation** — Gup renders to a GPU surface whose dimensions are set
  by the host window. It does not restrict orientation.
- **1.3.5 Identify Input Purpose** — Gup does not collect user input via form
  fields.

### Audio, Text Spacing, and Reflow (3 criteria)

- **1.4.2 Audio Control** — Gup does not auto-play audio.
- **1.4.10 Reflow** — Layout reflow is the host application's responsibility.
- **1.4.12 Text Spacing** — GPU-rendered text is not subject to CSS text spacing
  overrides.

### Keyboard Shortcuts, Timing, and Motion (4 criteria)

- **2.1.4 Character Key Shortcuts** — Gup does not define single-character
  shortcuts.
- **2.2.1 Timing Adjustable** — No time limits are imposed.
- **2.4.1 Bypass Blocks** — Single visualization canvas; no repeated blocks.
- **2.5.4 Motion Actuation** — No device motion input.

### Links and Navigation (1 criterion)

- **2.4.4 Link Purpose** — Gup does not render hyperlinks.

### Language, Input, and Forms (7 criteria)

- **3.1.2 Language of Parts** — Single-language content.
- **3.2.2 On Input** — No form controls.
- **3.3.1, 3.3.2, 3.3.3, 3.3.4** — No user input fields or data submissions.
- **4.1.1 Parsing** — Rust compiler enforces structural validity; deprecated in
  WCAG 2.2.

Full justifications for each N/A verdict are documented in the audit.

## Known Limitations

Gup is a visualization library, not a complete application. The following
accessibility responsibilities fall on the host application that integrates Gup:

1. **Page-level structure** — Skip-navigation links, page titles, language
   attributes, and heading hierarchy are the host application's responsibility.
2. **Form inputs** — Any input controls, labels, and error messages surrounding
   the visualization are provided by the host.
3. **Layout and reflow** — Responsive layout and CSS reflow of the
   visualization container are managed by the host.
4. **Platform audio** — While Gup provides a `SonificationEngine` API for data
   sonification, the host application must implement the actual audio synthesis.

## Third-Party Content

Gup does not embed or render third-party content. All visual elements are
generated from user-supplied data and configuration. Accessibility of the data
content depends on the labels, descriptions, and structure provided by the
developer.

## Accessibility Features

Gup provides the following built-in accessibility features:

| Feature | Description |
| --- | --- |
| **Screen reader support** | Automatic ARIA tree generation with roles, labels, and descriptions for all chart elements |
| **Keyboard navigation** | Three modes: Sequential (Tab), Spatial (arrows), Data Dimension (value-ordered) |
| **Focus indicators** | GPU-accelerated focus rings with high-contrast presets; DOM-based indicators on web |
| **Contrast modes** | Standard, High Contrast, Low Vision, Colorblind (Tol palette), Pattern-based |
| **Colour descriptions** | Human-readable colour names for screen readers (e.g., "dark grayish-blue") |
| **Platform integration** | Native AT-SPI2 (Linux), macOS, Windows, and web DOM accessibility APIs |
| **Live regions** | ARIA live region updates for dynamic data changes with urgency control |
| **Data sonification** | API for mapping data dimensions to audio parameters |
| **Pattern rendering** | Dots, Lines, Crosshatch patterns as alternatives to colour encoding |

## Evaluation Methods

- **Automated**: ARIA tree structure validation, contrast ratio calculation
  tests, role mapping verification
- **Manual**: Screen reader testing protocol (GUP-122) with NVDA and VoiceOver
- **Code review**: Systematic mapping of all 50 WCAG 2.1 AA criteria to source
  code

## Feedback

Accessibility issues and feedback can be reported at:
<https://github.com/au-phiware/gup/issues>

We are committed to maintaining and improving accessibility. Please include
"accessibility" in the issue title for prioritised handling.
