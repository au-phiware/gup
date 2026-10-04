# Golden images

Checked-in baseline renders for the visual regression harness (GUP-388). The
harness itself is the `gup-visual-regression` crate
(`crates/gup-visual-regression/`). It has no dependency on `gup`: every check
takes a plain `RgbaImage` plus `LayoutMetadata` (plot rect, text regions, guide
regions, configured colours).

| Directory         | Produced by                                                                                        |
| ----------------- | -------------------------------------------------------------------------------------------------- |
| `chart_builders/` | `src/chart_builder/visual_regression/builder_cases.rs` (one case per builder, plus scale variants) |

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
Correctness is asserted by the structural checks. Several current goldens show
known-broken output (the area-chart fan, the solid-fill density plot, the blank
heatmap). Each of those is tracked in
[`../visual_regression/expected_failures.toml`](../visual_regression/expected_failures.toml),
so the golden catches any _change_ while the tracked entry records what is
wrong. When a fix lands, the structural check starts passing (XPASS), which
fails the test until the entry is removed. The golden changes at the same time
and must be re-blessed.

## Perceptual-diff tolerance

A pixel differs when its CIEDE2000 distance from the golden pixel exceeds
**3.0**. A render matches when at most **0.25%** of pixels differ. Byte-exact
comparison is too brittle across GPU backends and drivers. Tighten these
(`DiffTolerance` in `crates/gup-visual-regression/src/diff.rs`) only after
observing stability across backends. Never loosen them, or any structural
threshold, to make a known-broken case pass; add an expected-failure entry
instead.

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
that ran and was missing, changed size or differed beyond tolerance. Goldens
within tolerance are left alone to avoid churn from backend noise. Blessing
never hides a structural failure: those still fail unless tracked.

Before committing re-blessed goldens:

1. Open each changed golden and confirm it shows what you intended.
2. Describe the rendered result in the story's Definition-of-Done evidence.
3. Update `expected_failures.toml` if your change fixed (XPASS) or introduced a
   tracked failure.
