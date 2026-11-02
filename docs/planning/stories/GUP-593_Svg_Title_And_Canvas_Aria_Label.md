---
implementer: sonnet
implementer-reason:
  writes GUP-592's description into two existing outputs at points named below,
  with the markup fixed in Decisions and a browser check
touches:
  [
    crates/gup-core/src/svg.rs,
    crates/gup-core/src/web.rs,
    crates/gup-core/wasm-size/,
    scripts/browser_smoke.mjs,
  ]
---

# GUP-593: SVG Title and Description, Canvas `aria-label`

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-02

## Context

[GUP-592](GUP-592_Plot_Describe_Text_Alternative.md) gives every resolved
`Scene` a `description: Option<Description>` with a `title` and a `text`. Owner
decision 4 asks for that text in the two outputs a screen reader meets: an SVG
file, and a chart on a web page.

`SvgTarget::write` (`crates/gup-core/src/svg.rs`, from about line 233) opens the
document with
`<svg xmlns="http://www.w3.org/2000/svg" width=".." height=".." viewBox="..">`,
then `<defs>`. SVG has its own accessible-name elements: per the SVG
Accessibility API Mappings, a `<title>` that is the root's first child gives the
name, and a `<desc>` gives the description. `role="img"` makes browsers announce
the whole document as one image instead of walking its shapes.

`web::CanvasApp` (`crates/gup-core/src/web.rs`) draws through `State::draw`
(line 256), which resolves the chart each frame. A `<canvas>` has no accessible
name unless the page gives it one. A chart's canvas should carry `role="img"`
and an `aria-label` of the description's `alt_text`, updated when the chart's
description changes (a new title, appended rows). A page that set its own
`aria-label` keeps it.

## Read First

- `crates/gup-core/src/svg.rs` lines 233–262 (the start of `write`) and `escape`
  (the XML escaper it already uses for text).
- `crates/gup-core/src/web.rs` lines 230–270 (`State`, `State::draw`) and
  420–470 (`CanvasApp::new`).
- `crates/gup-core/wasm-size/scatter/index.html` lines 160–190: where
  `show_scatter(canvas)` is called and steps report through `interactive`.
- GUP-592's Decisions (`Description`, `alt_text`, the no-title wording).
- `crates/gup-core/tests/svg.rs`: tests that match the document's first lines.

## User Story

> "As a reader using a screen reader, I want a Gup chart in an SVG file or on a
> web page to announce its description, so that I am not told only 'image'."

## Acceptance Criteria

### AC1: SVG

- [ ] When `scene.description` is `Some(d)`, the root element gains
      `role="img"`, and its first two children are `<title>{d.title}</title>`
      and `<desc>{d.text}</desc>`, both XML-escaped with `escape`, before
      `<defs>`. When it is `None` (a hand-built `Scene`), the output is byte for
      byte what it was.
- [ ] Tests in `crates/gup-core/tests/svg.rs`: (a) a builder chart titled
      `Sales & costs <2026>` writes
      `<title>Sales &amp; costs &lt;2026&gt;</title>` and a `<desc>` equal to
      the escaped `describe().unwrap().text`; (b) a `Scene::new` scene has no
      `<title>`, `<desc>` or `role`; (c) `Scene::guides()` of a resolved scene
      keeps both.
- [ ] Every existing SVG test and SVG parity check passes. A test that matched
      the old first line or an exact document is updated to the new prefix, and
      listed in the Implementation Summary.
- [ ] Rasterising with resvg is unchanged: the `svg_marks` parity checks pass
      without new tolerance.

### AC2: Canvas

- [ ] `CanvasApp::new` records whether the canvas already had an `aria-label`.
      If it did not, after each successful `State::draw` it sets `role="img"`
      (once) and sets `aria-label` to `description.alt_text()` whenever that
      string differs from the last one set. It never writes the DOM when nothing
      changed. If the page had set `aria-label`, `CanvasApp` sets neither
      attribute.
- [ ] The browser page checks both, as two new steps reported through
      `interactive`. Step `aria-label`: after `show_scatter(canvas)`, the canvas
      has `role="img"`, and its `aria-label` starts with
      `Chart. 6 data rows in 1 layer.` (the interactive scatter has no title;
      GUP-592's no-title wording). Step `aria-kept`: a third canvas given
      `aria-label="Mine"` before `show_scatter` still reads `Mine` after two
      frames, and has no `role`.
- [ ] `mask wasm-browser` passes, with both steps.

### AC3: Size

- [ ] `mask wasm-size`: `wasm.scatter.gz_bytes` grows by at most 1,000 B over
      the parent commit. Report both numbers. The orchestrator re-records the
      budget.

## Decisions

- **No `id`s, no `aria-labelledby`.** Ids would collide when several Gup SVGs
  are inlined into one HTML page. The `<title>` and `<desc>` first-child rule is
  the standard mapping, and current Chrome, Firefox and Safari follow it.
- **The SVG root line becomes**
  `<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img">`
  followed by `<title>…</title>` and `<desc>…</desc>`, one per line, and then
  `<defs>` as before. With no description, the root line has no `role`.
- **The `<desc>` holds `text` only**; the name is `<title>`. Screen readers read
  both.
- **Canvas label is `alt_text()`** (title and text in one string), because a
  canvas has no separate description element. `aria-describedby` would need an
  element in the page, which belongs to N4's hidden data table.
- **DOM writes**: `Element::set_attribute` through the `Element` web-sys
  feature, already enabled. Keep the last label in `State` as `String`.
- **The harness page's third canvas** reuses `show_scatter`; it needs no new
  Rust export.

## Escalation Triggers

- An existing SVG test compares whole documents in a way that would need more
  than a prefix update, such as a stored expected-SVG fixture file.
- Setting attributes in `State::draw` borrows the canvas while the
  `RefCell<State>` is held, and this conflicts with an event listener's borrow.
- `wasm.scatter.gz_bytes` grows by more than 1,000 B.

## Out of Scope

- Generating the text: GUP-592.
- A hidden data table, keyboard focus, `aria-live` announcements of hover: track
  N4.
- egui and Bevy accessibility (AccessKit): track N4.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] SVG root, title and desc; tests (AC1). Commit.
- [ ] Canvas attributes in `CanvasApp`; page steps; `mask wasm-browser` (AC2).
      Commit.
- [ ] `mask wasm-size` (AC3); DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-592: `Plot::describe` 📋 — the text and `Scene::description`.
- GUP-557: SvgTarget text out of the WASM build 📋 — edits the same `svg.rs`
  functions and the harness; it must merge first.

### Enables Stories

- GUP-597: README for release 0.1.

## Testing Strategy

- **Unit and integration tests**: SVG markup in `tests/svg.rs`.
- **Browser**: the two page steps under `mask wasm-browser`.
- **Visual validation**: none; nothing renders differently. The SVG parity
  checks pass.

## Success Metrics

- [ ] Every SVG Gup writes from a plot, and every canvas `CanvasApp` drives, has
      an accessible name and description.

## Risk Assessment

- **Low**: an SVG viewer shows `<title>` as a tooltip over the whole image. That
  is the standard behaviour, and it is expected.
- **Low**: an older screen reader ignores the SVG `<title>`. _Mitigation_:
  `role="img"` plus `<title>` is the most widely supported pattern without ids.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not applicable; the SVG parity checks
      pass
- [ ] `mask wasm-browser`: required (canvas attributes and page steps)
- [ ] WASM size: `mask wasm-size` scatter gz bytes, parent and after
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
