---
implementer: sonnet
implementer-reason:
  every colour and size is chosen in Decisions with its measured contrast; the
  work follows Theme::dark (GUP-553) and its tests
touches:
  [
    crates/gup-core/src/theme.rs,
    crates/gup-core/src/scale/categorical.rs,
    crates/gup-core/src/scale/mod.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/tests/theme_png.rs,
  ]
---

# GUP-625: High-Contrast Theme

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: 📋 Planned **Created**:
2026-11-04

## Context

Gup has `Theme::light()` (the default) and `Theme::dark()` (GUP-553). The light
theme's grid (`#E5E5E5`, 1.3:1 on white) and default palette (Okabe-Ito, where
orange, sky blue and yellow are 2.3:1, 2.3:1 and 1.3:1 on white) suit most
readers, but fail WCAG 1.4.11 (3:1 for graphics needed to understand content)
for low-vision readers. The November review lists a high-contrast theme in track
N4. A theme is a plain struct (`theme.rs`); the default categorical palette
follows `Theme::palette` (GUP-553), so a theme can carry its own palette.

The palette below was chosen by the scribe with a search over colours of at
least 3:1 on white, keeping Okabe-Ito's five colours that already pass and
maximising the smallest pairwise CIEDE2000 difference under normal vision and
simulated deuteranopia and protanopia (Machado 2009, severity 1). Its weakest
pair is `#0072B2`/`#CC79A7` under protanopia, ΔE00 12.3.

## Read First

- `crates/gup-core/src/theme.rs` (all; `Theme::dark` and its contrast test are
  the pattern).
- `crates/gup-core/src/scale/categorical.rs` lines 25–60 (`OKABE_ITO`,
  `OKABE_ITO_DARK`).
- `crates/gup-visual-regression/src/color.rs` (`srgb_to_lab`, `delta_e_2000`),
  usable from `gup-core`'s tests.

## User Story

> "As a low-vision reader, I want a chart whose text, axes, grid and every
> series meet contrast guidelines, so that I can read it without straining."

## Acceptance Criteria

### AC1: `HIGH_CONTRAST` palette

- [ ] `pub const HIGH_CONTRAST: [Color; 8]` in `scale/categorical.rs` with the
      colours of Decisions in that order, re-exported where `OKABE_ITO_DARK` is
      (`scale` and the crate root), with a doc comment giving the contrast rule
      and the weakest pair.
- [ ] A test asserts each colour's WCAG contrast on white is ≥ 3.0, and the
      smallest pairwise CIEDE2000 difference over normal, deuteranopia and
      protanopia views (Machado 2009 severity-1 matrices applied in linear sRGB,
      as in Decisions) is ≥ 12.0.

### AC2: `Theme::high_contrast()`

- [ ] `Theme::high_contrast()` returns the values of Decisions; doc comment with
      the contrast figures and a doctest like `Theme::dark`'s.
- [ ] A test in `theme.rs` asserts: label, title, axis and tooltip text ≥ 7:1;
      subtitle ≥ 7:1; grid ≥ 3:1; null colour ≥ 4.5:1; every palette colour ≥
      3:1, all against the background (and tooltip text against the tooltip
      background).

### AC3: Seen

- [ ] A golden `gup_core/theme_high_contrast`: a scatter with four colour keys,
      a title, a subtitle, axis titles and `grid(Axes::Both)`, 640 × 400. Read
      by eye: black text and axes, a grid clearly visible but lighter than the
      axes, four distinct dark colours, larger text than the light theme, and no
      clipped tick label.
- [ ] The same chart with `Theme::high_contrast()` renders in SVG with the same
      colours (the existing SVG parity helper used for the dark theme, if one
      exists; otherwise assert the SVG contains the four palette hex codes).

## Decisions

- **Palette** (`HIGH_CONTRAST`), in order: `#0072B2`, `#D55E00`, `#009E73`,
  `#000000`, `#CC79A7`, `#4411DD`, `#115500`, `#221155`. Contrast on white:
  5.19, 3.87, 3.42, 21.0, 3.06, 8.86, 9.04, 16.41. Blue first because it is the
  marks' default colour.
- **Theme values** (fields not listed keep `Theme::default()`'s): `axis`,
  `label`, `title`, `tooltip_text`, `tooltip_border` and `brush_stroke`
  `#000000`; `subtitle` `#333333` (12.6:1); `grid` `#949494` (3.03:1);
  `null_color` `#767676` (4.54:1); `title_size` 20, `subtitle_size` 15,
  `axis_title_size` 15, `tick_size` 14, `tooltip_size` 14 (px); `tick_len` 6;
  `x_tick_spacing` 96 and `y_tick_spacing` 52 (room for the larger labels);
  `brush_fill` `#0072B2` at alpha 0.2; `unbrushed` `Unbrushed::Grey(#949494)`
  (dimmed rows stay at 3:1 but lose their colour); `palette` `HIGH_CONTRAST`.
- **CVD simulation for the test**: Machado, Oliveira and Fernandes (2009),
  severity 1, applied to linear RGB, results clamped to 0..1 and re-encoded to
  sRGB before Lab:

  ```rust
  const DEUTERANOPIA: [[f64; 3]; 3] = [
      [0.367322, 0.860646, -0.227968],
      [0.280085, 0.672501, 0.047413],
      [-0.011820, 0.042940, 0.968881],
  ];
  const PROTANOPIA: [[f64; 3]; 3] = [
      [0.152286, 1.052583, -0.204868],
      [0.114503, 0.786281, 0.099216],
      [-0.003882, -0.048116, 1.051998],
  ];
  ```

- **No dark high-contrast variant**: one theme; a dark one waits for a request.
- **Not themed**: mark sizes and line widths (themes carry colours, text and
  spacing only); sequential and diverging palettes.

## Escalation Triggers

- The larger tick text makes an existing layout test or golden fail when the
  theme is applied in it (none apply it today, so this would be a new test's
  finding: report it rather than change spacing rules).
- AC1's ΔE test gives a smallest difference below 12.0 with the test's colour
  code (the scribe's number came from a separate script; report both).

## Out of Scope

- Patterns or textures as a second channel for colour.
- Following the OS high-contrast or `prefers-contrast` setting (hosts could; not
  here).

## Technical Tasks

- [ ] `HIGH_CONTRAST`, exports, palette test.
- [ ] `Theme::high_contrast`, contrast test, doctest.
- [ ] Golden and SVG check.

## Dependencies

### Prerequisite Stories

- GUP-553: Dark Theme ✅ — theme palettes and the contrast test pattern.

## Testing Strategy

- **Unit tests**: contrast and ΔE assertions.
- **Visual validation**: the golden.

## Success Metrics

- [ ] Every colour the theme draws meets its WCAG 2 contrast target, by test.

## Risk Assessment

- **Low**: a new constructor and constant. _Mitigation_: no default changes;
  existing goldens cannot move.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the golden)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code change
- [ ] WASM size: `mask wasm-size` scatter gz bytes (expect no change: unused
      constructors are not linked)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
