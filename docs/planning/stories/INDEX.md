# Stories

> Stories are the unit of work. Dependencies, not phases, determine order.
> Status: ✅ Complete · 🚧 In Progress · 📋 Planned · 💡 New · ⏸ Parked · 🚫
> Closed

The history before 2026-11 is in [INDEX_ARCHIVE.md](INDEX_ARCHIVE.md); grep it,
do not read it whole.

## Strategic Review 2026-11

Track N0 (hygiene) from the
[November 2026 strategic review](../STRATEGIC_REVIEW_2026-11.md), with the
owner's decisions of 2026-10-24: keep GPL-3.0-or-later, delete the mobile and
Tauri crates, minimal accessibility in release 0.1 (track N3), and at most two
build-heavy agents plus `scripts/checkpoint.sh`. Each story names the hot files
it edits in a `touches:` frontmatter line; two stories that share one never run
together. Suggested order: GUP-496 first; then GUP-497 with GUP-498; GUP-500
with GUP-499 (docs only); GUP-501; GUP-502; GUP-503 alone (it rewrites this
file); GUP-504.

- [GUP-496](GUP-496_Checkpoint_Script.md) ✅ — `scripts/checkpoint.sh` snapshots
  the working tree (untracked files included) to `refs/checkpoints/<branch>`
  with no hook and no branch move, with restore, `mask checkpoint` tasks, a
  seeded test, the worker's 20–30 minute rule, and `touches:` plus the standing
  DoD lines in the scribe's template.
- [GUP-497](GUP-497_Delete_Mobile_And_Tauri.md) 📋 — Delete `gup-ios`,
  `gup-android`, the Tauri example, `pkg/`, their two workflows, mask tasks and
  docs, keeping the hook's foreign-crate case tested with an on-the-fly fixture.
  Deps: GUP-496 📋 (soft).
- [GUP-498](GUP-498_Current_Gup_Core_Crate_Docs.md) ✅ — Rewrite `gup-core`'s
  crate docs to the crate as it is (four marks, layers, legends, `gup` facade,
  SVG marks) with two new running doctests, fix `SvgTarget`'s doc line and the
  Cargo description, and bring `CLAUDE.md` to wgpu 27 and the real wasm tasks.
  Deps: GUP-489 ✅, GUP-496 📋 (soft).
- [GUP-499](GUP-499_Archive_Stale_Docs.md) ✅ — Delete 18 guides of deleted
  code, archive the accessibility and research docs to `docs/archive/`, make
  `PERFORMANCE_GUIDE.md` a pointer to `PERF_BUDGETS.md`, banner the tutorials
  for N1, and move "what is next" to tracks N0–N6. Deps: GUP-497 📋.
- [GUP-500](GUP-500_Stable_Compile_Fail_Snapshots.md) ✅ — Stop trybuild
  `.stderr` churn: `#[diagnostic::do_not_recommend]` on concrete `CpuMirror` and
  `ColumnValue` impls (a spike with a rule) and "did you mean" instead of module
  lists; plus a fixed-recipe spike on the shared-build-dir "multiple versions of
  crate wgpu" doctest error. Deps: GUP-496 📋 (soft). Done: `CpuMirror` only
  (`ColumnValue` attributes swap in an unrelated hint); doctest error not
  reproduced.
- [GUP-511](GUP-511_Stable_Column_Value_Snapshot.md) ✅ — Stop a new
  `ColumnValue` type from changing `borrowing_accessor.stderr`.
- [GUP-501](GUP-501_Headless_Environment_Script.md) ✅ —
  `scripts/headless_env.sh` (lavapipe ICD, no `WAYLAND_DISPLAY`, 1920x1080 Xvfb)
  used by `run_all.sh` and the mask tasks, the egui screenshot test run for real
  in CI via `mask egui-screenshot`, and a lint against reasonless `#[ignore]`.
  Deps: GUP-438 ✅, GUP-483 ✅, GUP-496 📋 (soft).
- [GUP-502](GUP-502_Merge_Check_Task.md) ✅ — `mask merge-check`: gup-core
  tests, `wasm-size` and `wasm-browser` (and dogfood with `--dogfood`), each run
  regardless of failures, with one summary table. Deps: GUP-501 📋.
- [GUP-503](GUP-503_Archive_Story_Index_History.md) ✅ — Move this file's
  history to `INDEX_ARCHIVE.md`, keep this section and a nine-story Open Backlog
  here, close 27 obsolete or superseded stories with a new 🚫 status, and point
  the agent docs at the small index. Deps: GUP-496 📋 (soft).
- [GUP-504](GUP-504_Story_ID_Guard.md) ✅ — `scripts/check_story_ids.sh`: IDs
  above the legacy baseline (GUP-315) unique across files and both index files,
  run by the hook, `all-check` and `merge-check`, with a seeded test. Deps:
  GUP-503 ✅, GUP-502 ✅.
- [GUP-517](GUP-517_Shellcheck_Scripts_Gate.md) ✅ — `mask all-check` runs
  shellcheck over `scripts/*.sh` and `dogfood/run_all.sh` (only workflow `run:`
  blocks are linted today). Deps: GUP-502 ✅.

Backlog triage for later tracks (notes only; their stories are not written):

- **N2, bars**: end-bar padding on numeric axes, `group_padding`, horizontal
  bars and bar value labels are one bar-completeness story; value labels need
  N2's text mark first.
- **N2, lines and areas**: line curves together with GUP-439's translucent joins
  (the same polyline geometry), then area and stacked area, whose outlines reuse
  it.
- **N2, builders**: the dogfood tick formats dropped in GUP-465 and GUP-485 (t1
  `%Y-%m`, t3 `$Nk`) need `x_tick_format`/`y_tick_format` on the builders, whose
  `x_scale` takes a `PositionScale`, not a `ScaleRef` (checked 2026-10-24);
  restore the formats in that story.
- **N5**: GUP-492 (windowed size-order draws), through a GPU sort or an index
  buffer.
- **Parked**: GUP-428 until the next toolchain or `Cargo.lock` bump, or a disk
  alert.
- **N3 after N0**: release 0.1's minimal accessibility (`Plot::describe()`, an
  SVG title and description, a canvas `aria-label`) starts from the docs GUP-499
  archives; full N4 follows the release.

## Track N1: Docs and Tutorials

Track N1 from the [November 2026 review](../STRATEGIC_REVIEW_2026-11.md):
tutorials as tested Markdown, strict doc gates, an honest README. Release 0.1's
scope is "vocabulary + docs" (owner decision 2). These stories edit docs and
`src/lib.rs` only, so one runs beside a track N2 story. Suggested order: GUP-533
(beside GUP-540 or GUP-541, before any N2 story touches
`crates/gup-core/src/lib.rs`); then GUP-530, GUP-531, GUP-532, GUP-534 and
GUP-535 in turn (they share `src/lib.rs`); GUP-536 after N2's builders.

- [GUP-530](GUP-530_Tested_Tutorials_Getting_Started.md) 📋 — Tutorials 1
  (getting started with the builders) and 2 (plots and selections) rewritten as
  doctests of `gup`, with their images as goldens in `tests/tutorials.rs`, and
  the conventions the other tutorial stories follow.
- [GUP-531](GUP-531_Tutorials_Shader_Functions_And_Custom_Marks.md) 📋 —
  Tutorials 3 (`#[wgsl_function]`, `then`) and 6 (`#[derive(Mark)]`, the custom
  mark guide) as doctests, and a "Deriving" reference on the `Mark` trait. Deps:
  GUP-530 📋.
- [GUP-532](GUP-532_Tutorials_Interaction_And_Streaming.md) 📋 — Tutorials 4
  (tooltips, `on_row`, zoom, pan, brush, tested by sending `Input`) and 5
  (appenders, `Window::LastRows`) as doctests; the last banner and stale assets
  removed. Deps: GUP-530 📋.
- [GUP-533](GUP-533_Documentation_Lint_Gates.md) 📋 — `#![warn(missing_docs)]`
  in every crate (all pass today) and `mask doc` with
  `RUSTDOCFLAGS="-D warnings"` over seven crates in `mask all-check`.
- [GUP-534](GUP-534_Honest_README_And_Tested_Quick_Start.md) 📋 — README tagline
  and "Why Gup" cut to `PERF_BUDGETS.md` figures, a "Not yet" list, a
  git-dependency Quick Start, and the README compiled as a doctest. Deps:
  GUP-530 📋 (soft).
- [GUP-535](GUP-535_Compiled_Integration_Guides.md) 📋 — The egui and Bevy
  guides compiled as `no_run` doctests of `gup-egui` and `gup-bevy`. Deps:
  GUP-483 ✅, GUP-484 ✅.

## Track N2: Chart Vocabulary

Track N2 from the [November 2026 review](../STRATEGIC_REVIEW_2026-11.md): the
chart types and text a real user expects, for release 0.1 (owner decision 2).
Minimal accessibility is track N3, not here (owner decision 4). Most stories
edit `crates/gup-core/src/lib.rs`, `plot.rs`, `selection.rs` or
`builder/mod.rs`, so they run one at a time; the story frontmatter's `touches:`
says which.

- [GUP-540](GUP-540_Explicit_Domain_Edge_Inset.md) 📋 — An explicit domain's
  near end is inset by the marks' overhang, so a point at `(0, 0)` or the first
  constant-width bar is drawn whole; bars still sit on the axis.
- [GUP-541](GUP-541_Builder_Tick_Formats.md) 📋 — `x_tick_format`/
  `y_tick_format` on `Plot` and every builder, surviving a scale change, and the
  dogfood `%Y-%m` and `$Nk` formats restored with SVG text checks. Deps: GUP-474
  ✅.
- [GUP-542](GUP-542_Text_Layer_For_Annotations.md) 📋 — `Plot::text(rows)`: a
  CPU-laid-out `TextLayer` at data or band positions with anchors, offsets and
  `within` spans, clipped, in every target and SVG. Deps: GUP-540 📋, GUP-541 📋
  (soft, `plot.rs`).
- [GUP-543](GUP-543_Horizontal_Bars_And_Group_Padding.md) 📋 — `gup::barh`
  (`BarChart<T, Horizontal>`, categories top to bottom) and
  `BarChart::group_padding`.
- [GUP-544](GUP-544_Bar_Value_Labels.md) 📋 — `value_labels()` and
  `value_labels_with(f)` on every bar variant: above or beside bar ends with
  headroom, centred and contrast-coloured in stacks, hidden where they do not
  fit. Deps: GUP-542 📋, GUP-543 📋, GUP-541 📋 (soft).
- [GUP-545](GUP-545_Area_Mark.md) 📋 — The `Area` mark (`Plot::area`): MSAA
  trapezoids on the `Polyline` geometry, split where the top crosses the
  baseline, seam-free when translucent, written to SVG as paths.
- [GUP-546](GUP-546_Area_Picking.md) 📋 — `HitShape::Area` on the CPU and GPU,
  so areas get hover, tooltips and brushing. Deps: GUP-545 📋.
- [GUP-547](GUP-547_Area_Builder_And_Stacked_Areas.md) 📋 — `gup::area` with
  overlapping series, stacking aligned on the union of x values, `outline` and
  tooltips. Deps: GUP-545 📋, GUP-546 📋.

## Open Backlog

Stories still worth doing that no active initiative owns, with the track that
will pick each up.

- [GUP-392](GUP-392_Correct_Visual_Defaults.md) 📋 — Trimmed to what survives
  RFC-001: a single Okabe-Ito palette data source, Inter as the default font
  (with OFL licence), tick-label precision derived from the tick step, and muted
  default theme colours. Deps: GUP-388 ✅. Track: N0, verify against `gup-core`
  (Inter, Okabe-Ito, step-derived tick precision, muted theme) and close.
- [GUP-397](GUP-397_Dogfood_Checks_On_Shared_Visual_Assertions.md) 📋 — Replace
  the dogfood suite's local pixel measurements with GUP-388's shared
  visual-assertion module and layout metadata, without loosening any tracked
  gap. Deps: GUP-394 ✅, GUP-388 ✅. Track: N0.
- [GUP-413](GUP-413_Per_Checkout_Clippy_Artifacts.md) 💡 — Clippy artifacts are
  still shared between checkouts (GUP-411 made them re-lint on a switch, ~90 s
  instead of 26 s for `mask all-check`). Decide with measurements whether a
  per-checkout member `codegen-units` is worth its disk, and if so add it to
  `scripts/cargo_env.sh`. Deps: GUP-411 ✅. Related to GUP-427 (disk growth),
  not superseded by it. No track; keep.
- [GUP-428](GUP-428_Prune_Superseded_Shared_Units.md) 💡 — Prune dependency and
  clippy units nobody can use any more (left by `Cargo.lock` or toolchain bumps,
  deleted targets, pre-GUP-427 checkouts), which GUP-427's owner-based prune
  never touches. Deps: GUP-427 ✅. Parked until the next toolchain or
  `Cargo.lock` bump, or a disk alert.
- [GUP-405](GUP-405_Gup_Text_OpenType_Shaping.md) 📋 — Shape `gup-text` runs
  with rustybuzz: Inter's GPOS kerning and `tnum` tabular figures for tick
  labels. Deps: GUP-400 ✅. Track: N2 (text quality).
- [GUP-416](GUP-416_Gup_Text_Font_Fallback.md) 💡 — A user-supplied fallback
  chain in `TextSystem`, so characters outside the bundled subset (Cyrillic,
  CJK, emoji) draw from another font instead of as boxes. Fallbacks parse
  lazily, nothing new is bundled, and `SvgTarget` lists and embeds the fonts it
  uses. Deps: GUP-407 ✅, GUP-405 📋 (soft). Track: N2 (text quality).
- [GUP-439](GUP-439_Line_Translucent_Joins.md) 💡 — translucent `Line`s with
  even opacity: joins and self-crossings blended once (a per-layer coverage
  pass), opaque lines unchanged. Deps: GUP-432 ✅. Track: N2, with line curves
  and area outlines (same polyline geometry).
- [GUP-492](GUP-492_Windowed_Size_Order_Draws.md) 💡 — bounded draw calls for a
  windowed layer sized by a column whose sizes do not follow data order
  (per-chunk sort runs draw about one instance per draw today). Deps: GUP-482
  ✅. Track: N5 (GPU sort or index buffer).
- [GUP-376](GUP-376_GupApp_Event_Callbacks.md) 📋 — Add optional `.on_key()` and
  `.on_mouse()` callbacks to `GupApp` so examples with simple custom event
  handling can also use the shell. Deps: GUP-318 ✅. Track: N4 (keyboard).
