# Golden images

Checked-in baseline renders for the visual regression harness (GUP-388). The
harness itself is the `gup-visual-regression` crate
(`crates/gup-visual-regression/`). It has no dependency on `gup`: every check
takes a plain `RgbaImage` plus `LayoutMetadata` (plot rect, text regions, guide
regions, configured colours).

| Directory   | Produced by                                                                                                      |
| ----------- | ---------------------------------------------------------------------------------------------------------------- |
| `gup_core/` | `crates/gup-core/tests/scatter_png.rs` (RFC-001 core: headless scatter, layout metadata from `gup_core::Layout`) |

## What a case checks

Each case runs every applicable check:

| Check            | Fails when                                                                                  |
| ---------------- | ------------------------------------------------------------------------------------------- |
| `render`         | the adapter could not produce an image (error or panic)                                     |
| `not_blank`      | the image is a single solid colour, or background everywhere                                |
| `marks_present`  | the plot rect has fewer than 20 data-ink pixels outside axis/tick geometry                  |
| `text_present`   | an expected title/tick-label region has fewer than 8 pixels that look like its text colour  |
| `marks_confined` | any ink lies outside the plot rect (plus the case's mark overhang), guides and text regions |
| `color_present`  | an explicitly configured colour has no pixel within CIEDE2000 ΔE 3                          |
| `golden`         | the render differs from the golden image beyond the perceptual tolerance                    |

**Golden images are baselines of current output, not certified-correct images.**
Correctness is asserted by the structural checks. A golden that shows
known-broken output is tracked in
[`../visual_regression/expected_failures.toml`](../visual_regression/expected_failures.toml),
so the golden catches any _change_ while the tracked entry records what is
wrong. When a fix lands, the structural check starts passing (XPASS), which
fails the test until the entry is removed. The golden changes at the same time
and must be re-blessed.

## Perceptual-diff tolerance

A render matches its golden image when both rules hold:

- **Fraction rule**: a pixel differs when its CIEDE2000 distance from the golden
  pixel exceeds **3.0**; at most **0.25%** of pixels may differ. This absorbs
  antialiasing and glyph noise across backends.
- **Strong rule** (GUP-687): a pixel is a strong difference when its distance
  exceeds **12.0** (`STRONG_DELTA_E`); at most **8** pixels may be strong
  (`MAX_STRONG_PIXELS`). This catches a thin, high-contrast feature that is too
  small in count for the fraction rule: GUP-676's 16 px hover ring changed 52
  pixels (0.02% of a 640 by 400 render), each by more than ΔE 40, and passed the
  fraction rule alone.

How the strong constants were chosen (2026-11-09): `M`, the largest number of
strong pixels on any platform against the checked-in goldens, was 0. In nine
Platforms runs (49 to 55 tour, tutorial and gallery goldens each) no pixel
differed from its golden by more than ΔE 2.7 on Windows WARP or 1.2 on macOS
Metal; on Linux lavapipe all 117 goldens were byte-equal to their renders. A
pixel moved by one antialiasing coverage level (±8 per channel) is at most ΔE
3.0. The limit is `max(2 * M, 8)` rounded up to a multiple of 4, so 8. To
re-derive: run the golden tests with `--nocapture` on each platform (the
Platforms workflow does), read `N strongly` and `max ΔE` from every
`PASS golden` line, and apply the same rule; if `M` exceeds 24 the rule would no
longer catch a ring and needs a new design, not a larger limit.

A mismatch message names the broken rule, the counts, and whether the fraction
rule alone would have passed. Byte-exact comparison is too brittle across GPU
backends and drivers. Tighten these (`DiffTolerance` in
`crates/gup-visual-regression/src/diff.rs`) only after observing stability
across backends. Never loosen them, or any structural threshold, to make a
known-broken case pass; add an expected-failure entry instead.

## Running

```bash
cargo test --lib visual_regression -- --test-threads=1
# show the per-check report (PASS / XFAIL / BLESS) for passing cases too
cargo test --lib visual_regression -- --test-threads=1 --nocapture
```

The latest render of every case is written to
`target/visual-regression/<case>.png` (or under `$CARGO_TARGET_DIR`). On a
golden mismatch a `<case>.diff.png` is written next to it, with differing pixels
in magenta.

## Blessing (intentional visual changes)

```bash
GUP_BLESS=1 cargo test --lib visual_regression -- --test-threads=1
git add tests/golden/
```

`GUP_BLESS=1` (also `true` or `yes`) rewrites the golden image of every case
that ran, whether it was missing, changed size, differed beyond tolerance or was
within tolerance, and reads each one back to check it equals the render. The
committed PNGs therefore equal the current renders, and
`git diff --stat tests/golden` lists exactly the goldens whose bytes changed.
Read each one. Never delete goldens to force a rewrite. Blessing never hides a
structural failure: those still fail unless tracked.

Before committing re-blessed goldens:

1. Open each changed golden and confirm it shows what you intended.
2. Describe the rendered result in the story's Definition-of-Done evidence.
3. Update `expected_failures.toml` if your change fixed (XPASS) or introduced a
   tracked failure.
