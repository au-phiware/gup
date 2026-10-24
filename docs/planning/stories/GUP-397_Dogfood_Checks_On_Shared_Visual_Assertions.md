---
implementer: sonnet
implementer-reason:
  a verbatim port between two crates with an equivalence proof; no thresholds
  change
touches: [dogfood/, crates/gup-visual-regression/]
---

# GUP-397: Dogfood Checks on Shared Visual Assertions

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 🚧 In Progress
**Created**: 2026-10-04 **Revised**: 2026-10-24

## Context

**Revised 2026-10-24** for the code after the RFC-001 flip. The premise still
holds: the repository has two pixel-measurement modules that do not know about
each other.

- `dogfood/src/pixels.rs` (529 lines, GUP-394) measures decoded
  `image::RgbaImage`s over fractional `Region`s: `background`,
  `non_background_fraction`, `count_near`, `blob_centroids`, `count_ink`,
  `tooltip_panel` (with `Panel`), `count_hue`, `count_grey`, `count_runs`,
  `count_saturated` and `hue_of`. `dogfood/src/suite.rs`'s `Measure::evaluate`
  (lines 177–290) is its only user. The suite is 10 PASS, 0 XFAIL, 0 FAIL.
- `crates/gup-visual-regression` (GUP-388) holds the golden-image harness used
  by `gup-core`'s, `gup-egui`'s and `gup-bevy`'s tests: its own `RgbaImage` (PNG
  decode via the `png` crate), `PxRect`, `LayoutMetadata` and pass/fail
  `checks`, but no counting API. It has no `gup` dependency and no workspace
  inheritance, so the detached dogfood crate can depend on it by path.

This story moves dogfood's measurements into the shared crate as a public
`measure` module, so there is one tested home for them and other tests can use
them. It moves them **unchanged**: the same arithmetic and thresholds, proved
equal on every dogfood output, so no check can flip.

Two things from the 2026-10-04 version are deliberately dropped. Re-deriving
dogfood's thresholds on ΔE and rebuilding `checks` on top of `measure` would
change what both suites measure, and the gain is small. Replacing the manifest's
fractional regions with `LayoutMetadata` would need the plot rectangle, which
dogfood, as an external user, does not get from the public API.

## Read First

- `dogfood/src/pixels.rs` (all of it: the code moves).
- `dogfood/src/suite.rs` lines 1–30 (imports, `OUT_DIR`), 145–290 (`Measured`,
  `evaluate_file`, `evaluate`) and 1056–1130 (tests).
- `dogfood/src/lib.rs` (module list, `save_screenshot`).
- `crates/gup-visual-regression/src/lib.rs` (module list and re-exports) and
  `src/image.rs` lines 55–240 (`RgbaImage`: `new`, `width`, `height`, `pixel`,
  `set_pixel`, `fill_rect`, `pixels`, `from_png`, `load_png`, `save_png`).
- GUP-388's retrospective, "How GUP-397 should consume this harness"
  (`docs/planning/stories/GUP-388_Visual_Regression_Harness.md` lines 507–540),
  for background only: this story overrides its ΔE and `checks` advice (see
  Decisions).

## User Story

> "As a maintainer, I want the dogfood suite's pixel measurements to live in the
> shared visual-regression crate, tested once, so that the golden tests and the
> external-user suite draw on one module and a measurement fix lands in both."

## Acceptance Criteria

### AC1: A public `measure` module

- [ ] `crates/gup-visual-regression/src/measure.rs` exists, declared
      `pub mod measure;` in `lib.rs` (nothing from it is re-exported at the
      crate root). It has a copyright header and a module doc saying it holds
      counting functions over fractional regions, ported from dogfood (GUP-394,
      GUP-397), and how its "ink" (luma contrast, dark text) differs from
      `checks`' (ΔE from the background, any ink).
- [ ] It provides every public item of `dogfood/src/pixels.rs` with the same
      name, parameters and return type, except that images are
      `&gup_visual_regression::RgbaImage`: `Region` (with `ALL`, `new`),
      `background`, `non_background_fraction`, `count_near`, `blob_centroids`,
      `count_ink`, `Panel`, `tooltip_panel`, `count_hue`, `count_grey`,
      `count_runs`, `count_saturated`, `hue_of`. Private helpers (`rgb`,
      `max_channel_diff`, `mode`, `segment_distance`, `luma`, `hue_sat`,
      `hue_distance`, `Region::bounds`, `Region::pixels`) stay private.
- [ ] Each function's body computes the same thing as before, line for line,
      with `img.pixel(x, y)` (an `Rgba8`) in place of `img.get_pixel(x, y).0`.
      No constant, threshold, rounding or iteration order changes.
- [ ] `pixels.rs`'s 11 unit tests move into `measure.rs` on the harness's
      `RgbaImage` (`RgbaImage::filled`, `set_pixel`, `fill_rect`) and pass:
      `scripts/with_cargo_env.sh cargo test -p gup-visual-regression`.
- [ ] Every public item has a doc comment (copied from `pixels.rs`), and
      `measure` has one runnable doctest (for example `count_near` on a filled
      image).

### AC2: Dogfood uses it, and only it

- [ ] `dogfood/Cargo.toml` adds
      `gup-visual-regression = { path = "../crates/gup-visual-regression" }` and
      drops `image`; `dogfood/Cargo.lock` is updated
      (`cargo update -p     gup-visual-regression --manifest-path dogfood/Cargo.toml`
      or a build).
- [ ] `dogfood/src/pixels.rs` is deleted; `lib.rs` drops `pub mod pixels;` and
      its doc names `gup_visual_regression::measure` instead.
- [ ] `suite.rs` imports `gup_visual_regression::measure::{self, Region}` and
      `gup_visual_regression::RgbaImage`; `evaluate_file` loads PNGs with
      `RgbaImage::load_png`, and `evaluate` takes `&RgbaImage`.
- [ ] `save_screenshot` writes with
      `RgbaImage::new(w, h, bytes).and_then(|i| i.save_png(path)).unwrap()` (it
      panics on error today too).
- [ ] `measures_work_on_a_synthetic_image` builds its image with
      `RgbaImage::filled` and `fill_rect`, and passes.
- [ ] `git grep -n "image::" dogfood/src` prints nothing, and
      `git grep -n "pixels" dogfood/README.md` points at the new module (update
      the sentence at line 59).

### AC3: Proof that no measurement changed

- [ ] Before deleting `pixels.rs`: run `mask dogfood` on the unchanged suite and
      save its stdout as `/tmp/gup397-before.txt`. Then, with both modules
      present, add a temporary dogfood test, `measure_matches_pixels`, that
      loads every `*.png` in `/tmp/gup-dogfood/` twice (`image::open` for
      `pixels`, `RgbaImage::load_png` for `measure`) and asserts equal results
      from each function pair with the arguments `suite.rs` uses (for every
      `Region` in the manifest's checks on that file, and `Region::ALL`). Run
      it, record the number of files and comparisons in the retrospective, then
      delete the test along with `pixels.rs`.
- [ ] After the port: `mask dogfood` again, stdout saved as
      `/tmp/gup397-after.txt`; `diff` of the two, ignoring timing lines, shows
      no change to any check's status or `[detail]`. Paste the summary line
      (`dogfood: OK`, counts) in the retrospective.

## Decisions

- **Home**: `gup_visual_regression::measure`, not a new crate; the harness
  already has the right dependencies and is detached-friendly (GUP-388).
- **Port verbatim; no re-derivation.** Same maths and thresholds, so AC3 can
  demand identical output. ΔE-based tolerances (GUP-388's suggestion) are not
  adopted.
- **`checks` untouched.** `checks::is_ink`/`is_glyph` stay private and as they
  are; making `checks` call `measure` would change golden-test behaviour. The
  two "ink" definitions answer different questions and are documented side by
  side in `measure`'s module doc.
- **`Region` stays fractional** and lives in `measure` (not re-exported), so it
  is not confused with the harness's pixel `PxRect`.
- **One PNG decoder.** Dogfood drops the `image` crate; `RgbaImage::from_png`
  normalises 8- and 16-bit colour types to RGBA8, which covers every file
  dogfood writes (gup's RGBA8 PNGs and egui screenshots saved as RGBA8).
- **Manifest unchanged.** No region, threshold, `Gap` or task changes in
  `suite.rs`; only types and imports.

## Escalation Triggers

- `measure_matches_pixels` finds any difference, or the after-run differs from
  the before-run in any status or detail.
- `RgbaImage::load_png` rejects a file `image::open` read (for example an
  indexed PNG).
- `dogfood/Cargo.lock` needs a crate version not already in `~/.cargo/registry`
  and the network is unavailable.
- The before-run is not 10 PASS, 0 XFAIL, 0 FAIL (the suite changed under you;
  report, do not fix).

## Out of Scope

- ΔE thresholds for dogfood, and rebuilding `checks` on `measure`.
- `LayoutMetadata`-based regions in the manifest.
- New dogfood tasks or checks, including the dropped tick formats (N2,
  builders).
- `gup-core`'s own test helpers (`near`, `pixel` in `builders.rs`).

## Technical Tasks

- [ ] Run `mask dogfood`; save `/tmp/gup397-before.txt`.
- [ ] Write `measure.rs`; move and convert the unit tests; add the doctest.
- [ ] Add the dependency; write and run `measure_matches_pixels`.
- [ ] Switch `suite.rs` and `lib.rs`; delete `pixels.rs` and the temporary test;
      drop `image`; update `README.md`.
- [ ] Run `mask dogfood`; diff with the before-run.

## Dependencies

### Prerequisite Stories

- GUP-394: Dogfood Suite in Repo and CI ✅
- GUP-388: Visual Regression Harness ✅
- GUP-501: Headless Environment Script ✅ — `mask dogfood` runs headless.

### Enables Stories

- None.

## Testing Strategy

- **Unit tests**: the moved `pixels.rs` tests and the doctest, in
  `gup-visual-regression`.
- **Integration tests**: `cargo test --manifest-path dogfood/Cargo.toml --lib`;
  the temporary equivalence test over real outputs.
- **End to end**: `mask dogfood` before and after, diffed.

## Success Metrics

- [ ] `dogfood/src/pixels.rs` is gone, and dogfood has no `image` dependency.
- [ ] Every dogfood check reports the same status and detail as before.

## Risk Assessment

- **Low**: `image::open` and `RgbaImage::load_png` decode a file differently
  (gamma chunks, 16-bit rounding). _Mitigation_: AC3's equivalence test compares
  on the real files; escalate on any difference.
- **Low**: `background` breaks colour-count ties through `HashMap` order, which
  could differ between runs. _Mitigation_: keep it verbatim; a tie in the
  most-common colour of a chart is not expected, and AC3 would show it.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix` (it lints and formats `dogfood/`)
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not applicable (no rendering change; AC3
      proves identical measurements)
- [ ] `mask wasm-browser`: not needed (no layout, input or wasm code changes)
- [ ] WASM size: unchanged: no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
