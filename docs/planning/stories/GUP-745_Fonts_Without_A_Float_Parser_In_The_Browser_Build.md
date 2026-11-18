---
implementer: opus
implementer-reason:
  a dependency-level choice (upstream change, version, or bypassing a parser
  entry point) with a correctness argument for CFF reals and an outward-facing
  step that needs the owner
touches:
  [
    crates/gup-text/Cargo.toml,
    crates/gup-text/src/font.rs,
    crates/gup-text/src/opentype.rs,
    crates/gup-core/PERF_BUDGETS.md,
  ]
---

# GUP-745: Fonts Without a Float Parser in the Browser Build

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-19

## Context

GUP-744 found the largest removable item in the browser build: `f64::from_str`
(`core::num::dec2flt`, 3,024 B gz of code and a 10,416 B table of powers of
five, 10,135 B gz). Nothing in gup calls it. ttf-parser's CFF DICT reader does
(`tables/cff/dict.rs`, `parse_float`: `s.parse()` on the decoded nibble string),
and `Face::parse` reaches it, called by both fontdue (`Font::from_bytes`) and
gup-text (`OtLayout`, `Font::own`). ttf-parser 0.25.1, which fontdue 0.9.5 uses,
has the same call.

The bundled Inter subsets are TrueType (`glyf`, no CFF table), so the path never
runs for them. Users can still pass CFF fonts (`TextSystem::with_font`,
`add_fallback`), so CFF support stays.

GUP-744 measured the saving with a scratch `[patch]` of the WASM harness
(reverted, never committed) that replaced the parse with a crude loop: over bare
wgpu 356,270 to 340,405 B gz (**−15,865**). A `[patch]` in this workspace does
not reach users who build gup from crates.io, so the harness must not measure
one. The fix has to be in a dependency that users get.

## User Story

> "As a developer shipping a gup chart to the browser, I want the font code to
> carry no decimal-to-float parser it never uses, so that 15 KB of my bundle is
> not spent on CFF real numbers my TrueType fonts never contain."

## Acceptance Criteria

### AC1: Choose the route

- [ ] Compare, with evidence, at least these:
  1. **Upstream ttf-parser**: replace `parse_float`'s `s.parse()` with a small
     exact parser for CFF reals (a nibble string of at most 64 characters: an
     integer mantissa and a power of ten; exact whenever the mantissa fits 2^53
     and the power is within ±22, by Clinger's fast path), or put CFF real
     parsing behind a feature flag. Then a fontdue and ttf-parser version pair
     that carries it.
  2. **A version that already avoids it**: check the newest ttf-parser, fontdue,
     `owned_ttf_parser` and any alternative rasterizer for a CFF reader without
     `f64::from_str`.
  3. **gup-text avoids `Face::parse` for the bundled TrueType fonts**: whether
     fontdue and gup-text's own layout code can be fed parsed tables without the
     CFF reader linked (for example through a parse path that does not pull in
     CFF), and what that costs for user CFF fonts.
- [ ] Record each route's measured `mask wasm-size` saving (a scratch build is
      fine for a route not taken) and its effect on user fonts.

### AC2: Land it (only routes that users get)

- [ ] The chosen route is in a published dependency, or in gup's own code. No
      `[patch.crates-io]` in the workspace or the WASM harnesses.
- [ ] `wasm.over_wgpu.gz_bytes` falls by at least 12,000 B gz from the story's
      base commit, measured with `mask wasm-size` before and after.
- [ ] A CFF font still loads and draws as before (a test with a CFF font, or the
      existing font tests if one is present); the bundled fonts' goldens are
      byte-identical.

## Decisions

- **Opening an upstream PR or issue is outward-facing**: it needs the owner's
  approval. The implementer prepares the patch, its tests and the size evidence
  in the story; the orchestrator asks the owner, and the PR is opened only after
  that.
- No fork published under gup's name without the owner's decision.
- A CFF real parser must give the same `f64` as `str::parse` for every value it
  accepts; a value outside its exact range takes a path that is still exact (or
  the route is rejected).

## Escalation Triggers

- Every route needs an upstream change that is not yet released: stop after AC1
  with the patch ready, and report.
- A route would drop CFF support or change the rendering of any font.

## Out of Scope

- Shrinking the Inter subsets, or font data in general.
- Replacing fontdue's rasterizer.

## Technical Tasks

- [ ] Read ttf-parser's `tables/cff/dict.rs` (0.21.1 and the newest) and
      fontdue's `Font::from_bytes`.
- [ ] Measure each route (AC1).
- [ ] Prepare the upstream patch with tests comparing against `str::parse` over
      CFF-shaped strings (AC1 route 1), for the owner.
- [ ] Land the chosen route and re-measure (AC2).

## Dependencies

### Prerequisite Stories

- GUP-744: The Browser Build Stays Under Its Size Cap ✅ (the finding).

### Enables Stories

- Release 0.1: about 15 KB more headroom under the policy line of 360,000.

## Testing Strategy

- **Unit**: the CFF real parser against `str::parse` on generated nibble strings
  (mantissas, signs, exponents, the 64-character limit).
- **Integration**: gup-text font tests; a CFF font loaded and measured.
- **Visual**: native goldens byte-identical; `mask wasm-browser` pixel counts
  identical.
- **Size**: `mask wasm-size` before and after.

## Success Metrics

- [ ] `wasm.over_wgpu.gz_bytes` down by at least 12,000 B gz, in a build users
      get.

## Risk Assessment

- **Medium**: upstream may be slow or decline. _Mitigation_: route 3, or an
  owner decision on a published fork.
- **Low**: a hand parser misrounds. _Mitigation_: exactness argument plus tests
  against `str::parse`.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-browser` passes with identical pixel counts
- [ ] WASM size: `mask wasm-size` before and after pasted
- [ ] Story status updated in the story file and INDEX.md
- [ ] Retrospective added to story document
