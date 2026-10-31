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

The rest of N0, rewritten 2026-10-24. GUP-392 and GUP-523 share no hot file and
may run together; GUP-397 touches only `dogfood/` and the harness crate. Run
`mask merge-check --dogfood` after merging GUP-392 and GUP-397.

- [GUP-392](GUP-392_Correct_Visual_Defaults.md) ✅ — Its four original items
  hold in `gup-core` (evidence in the story); what is left: Okabe-Ito reordered
  so yellow (1.32:1 on white) comes seventh, the marks' default blue taken from
  the palette, and linear-family tick and ramp labels grouped (`10,000`) when a
  tick set reaches 10,000. Deps: GUP-388 ✅, GUP-474 ✅.
- [GUP-397](GUP-397_Dogfood_Checks_On_Shared_Visual_Assertions.md) ✅ — Move
  dogfood's `pixels.rs` verbatim into `gup_visual_regression::measure`, drop
  dogfood's `image` dependency, and prove every check's status and detail
  unchanged on real outputs. Deps: GUP-394 ✅, GUP-388 ✅, GUP-501 ✅.
- [GUP-523](GUP-523_Attr_Path_Column_Value_Message.md) ✅ — Make `f64` the one
  recommended `ColumnValue` impl, so a string accessor passed to `attr` gets
  "implemented for `f64`" in both errors instead of an `IntoEncoding` hint, and
  restore the `attr` form of `borrowing_accessor` with a seeded-stable snapshot.
  Deps: GUP-511 ✅.

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

- [GUP-530](GUP-530_Tested_Tutorials_Getting_Started.md) ✅ — Tutorials 1
  (getting started with the builders) and 2 (plots and selections) rewritten as
  doctests of `gup`, with their images as goldens in `tests/tutorials.rs`, and
  the conventions the other tutorial stories follow. Deps: GUP-537 📋.
- [GUP-531](GUP-531_Tutorials_Shader_Functions_And_Custom_Marks.md) ✅ —
  Tutorials 3 (`#[wgsl_function]`, `then`) and 6 (`#[derive(Mark)]`, the custom
  mark guide) as doctests, and a "Deriving" reference on the `Mark` trait. Deps:
  GUP-530 📋.
- [GUP-532](GUP-532_Tutorials_Interaction_And_Streaming.md) ✅ — Tutorials 4
  (tooltips, `on_row`, zoom, pan, brush, tested by sending `Input`) and 5
  (appenders, `Window::LastRows`) as doctests; the last banner and stale assets
  removed. Deps: GUP-530 📋.
- [GUP-533](GUP-533_Documentation_Lint_Gates.md) ✅ Complete —
  `#![warn(missing_docs)]` in every crate (all pass today) and `mask doc` with
  `RUSTDOCFLAGS="-D warnings"` over seven crates in `mask all-check`.
- [GUP-534](GUP-534_Honest_README_And_Tested_Quick_Start.md) ✅ — README tagline
  and "Why Gup" cut to `PERF_BUDGETS.md` figures, a "Not yet" list, a
  git-dependency Quick Start, and the README compiled as a doctest. Deps:
  GUP-530 📋 (soft), GUP-537 📋 (soft).
- [GUP-535](GUP-535_Compiled_Integration_Guides.md) ✅ — The egui and Bevy
  guides compiled as `no_run` doctests of `gup-egui` and `gup-bevy`. Deps:
  GUP-483 ✅, GUP-484 ✅.
- [GUP-536](GUP-536_Tutorial_Tour_Of_Chart_Types.md) 📋 — Tutorial 7: every
  chart type with a tested block of at most 20 lines and a golden image. Deps:
  GUP-530 📋, GUP-537 📋, GUP-542 📋, GUP-544 📋, GUP-547 📋, GUP-548 📋,
  GUP-549 📋; GUP-550–554 📋 (soft).
- [GUP-564](GUP-564_Derive_Role_Message_And_Stale_Gallery.md) 📋 — The
  `#[derive(Mark)]` role error lists all six roles (with a compile-fail case),
  the gallery page is regenerated (96 of 101 source links were dead), and
  `all-check` runs the gallery link check. Edits `maskfile.md`. Deps: GUP-531
  ✅, GUP-499 ✅.

## Track N2: Chart Vocabulary

Track N2 from the [November 2026 review](../STRATEGIC_REVIEW_2026-11.md): the
chart types and text a real user expects, for release 0.1 (owner decision 2).
Minimal accessibility is track N3, not here (owner decision 4). Most stories
edit `crates/gup-core/src/lib.rs`, `plot.rs`, `selection.rs` or
`builder/mod.rs`, so N2 runs mostly one story at a time, with an N1 or a
non-conflicting N2 story beside it (two build-heavy agents at most; check
`touches:`). Suggested pairs, in order: GUP-540 with GUP-533; GUP-537 with
GUP-535 (before GUP-541 and GUP-530: it edits `builder/`, `dogfood/` and
`src/lib.rs`); GUP-541 with GUP-530; GUP-542 (Opus) with GUP-531; GUP-543 with
GUP-532; GUP-555 (alone among `plot.rs` stories); GUP-544 with GUP-553; GUP-545
with GUP-534; GUP-546 with GUP-405; GUP-547 with GUP-535; GUP-548; GUP-549;
GUP-550 (Opus) with GUP-552; GUP-551 (Opus) with GUP-416; GUP-439 (Opus) with
GUP-554; then GUP-536. Deferred from N2: curved and horizontal areas, heatmap
cell labels, error bars, pie charts and facets.

- [GUP-537](GUP-537_Owned_Category_Keys_In_Builders.md) ✅ — Builders' key
  setters take owned keys (`|r| r.year`, an enum, `r.name.clone()`) through
  `DictionaryKey`, compared by label, with a "not a category key" diagnostic and
  a snapshot of the borrowed-key lifetime error; Plot-layer `encode_key`
  unchanged. Deps: GUP-419 ✅, GUP-464 ✅.
- [GUP-540](GUP-540_Explicit_Domain_Edge_Inset.md) ✅ — An explicit domain's
  near end is inset by the marks' overhang, so a point at `(0, 0)` or the first
  constant-width bar is drawn whole; bars still sit on the axis.
- [GUP-528](GUP-528_Data_Aware_Edge_Insets.md) ✅ — Each end of an explicit axis
  is inset by the overhang of the axis-bound marks that reach it (exact from CPU
  columns, chunk stats otherwise), never more than GUP-540's; run it alone among
  `plot.rs` stories. Deps: GUP-540 ✅.
- [GUP-529](GUP-529_Shared_Scale_Overhang_Clipping.md) ✅ — Circles on shared
  automatic scales (tutorial 2: `plot.line` + a raw `Selection`, `Px(6.0)`
  radius) are clipped at the plot edges: find why widening is lost and fix it.
  Deps: GUP-530 ✅. Runs after GUP-528 (both edit `plot.rs`).
- [GUP-541](GUP-541_Builder_Tick_Formats.md) ✅ — `x_tick_format`/
  `y_tick_format` on `Plot` and every builder, surviving a scale change, and the
  dogfood `%Y-%m` and `$Nk` formats restored with SVG text checks. Deps: GUP-474
  ✅.
- [GUP-542](GUP-542_Text_Layer_For_Annotations.md) ✅ — `Plot::text(rows)`: a
  CPU-laid-out `TextLayer` at data or band positions with anchors, offsets and
  `within` spans, clipped, in every target and SVG. Deps: GUP-540 📋, GUP-541 📋
  (soft, `plot.rs`), GUP-537 📋 (soft).
- [GUP-555](GUP-555_Text_Labels_Make_Room_At_The_Plot_Edge.md) ✅ — Text layers
  report per-side label extents, so automatic axes widen and explicit ones inset
  only where labels reach (capped at a quarter of the axis) and edge labels are
  drawn whole; label boxes measured once and cached. Not beside GUP-541 or
  GUP-552 (`plot.rs`). Deps: GUP-542 ✅, GUP-528 ✅, GUP-529 ✅.
- [GUP-526](GUP-526_Text_Layers_On_The_Right_Axis.md) 💡 — A text layer's `y`
  mapped through the plot's right y axis, so dual-axis charts can label both
  series. Deps: GUP-542 ✅.
- [GUP-527](GUP-527_Label_Halos_And_Overlap_Culling.md) 💡 — `halo(Px)` and
  `hide_overlaps(true)` on text layers, so labels stay legible over dense marks.
  Deps: GUP-542 ✅, GUP-405 📋 (soft).
- [GUP-543](GUP-543_Horizontal_Bars_And_Group_Padding.md) ✅ — `gup::barh`
  (`BarChart<T, Horizontal>`, categories top to bottom) and
  `BarChart::group_padding`. Deps: GUP-537 📋.
- [GUP-544](GUP-544_Bar_Value_Labels.md) ✅ — `value_labels()` and
  `value_labels_with(f)` on every bar variant: above or beside bar ends with
  room from GUP-555, centred and contrast-coloured in stacks, hidden where they
  do not fit. Deps: GUP-542 ✅, GUP-543 ✅, GUP-555 📋, GUP-541 📋 (soft),
  GUP-537 ✅ (soft).
- [GUP-545](GUP-545_Area_Mark.md) ✅ — The `Area` mark (`Plot::area`): MSAA
  trapezoids on the `Polyline` geometry, split where the top crosses the
  baseline, seam-free when translucent, written to SVG as paths.
- [GUP-546](GUP-546_Area_Picking.md) ✅ — `HitShape::Area` on the CPU and GPU,
  so areas get hover, tooltips and brushing. Deps: GUP-545 📋.
- [GUP-547](GUP-547_Area_Builder_And_Stacked_Areas.md) ✅ — `gup::area` with
  overlapping series, stacking aligned on the union of x values, `outline` and
  tooltips. Deps: GUP-545 📋, GUP-546 📋, GUP-537 📋.
- [GUP-548](GUP-548_Histogram_Builder_And_Bin_Transform.md) ✅ —
  `transform::bin` (Sturges or a count on a 1–2–5 step, a width, or edges) and
  `gup::histogram` with edges on round numbers, 1 px gaps, stacking by key and a
  "Count" axis title. Deps: GUP-537 📋, GUP-540 📋 (soft).
- [GUP-549](GUP-549_Heatmap_Builder.md) ✅ — `gup::heatmap` on two band axes
  with `color_value`, a ramp legend and gapped cells; `color_value_scale` takes
  `Sequential` or `Diverging` on the scatter too. Deps: GUP-537 📋.
- [GUP-550](GUP-550_Line_Curves_And_Steps.md) ✅ — `Curve` and
  `.curve(Curve::StepAfter)` on lines: step pieces drawn exactly in the shader
  through a per-layer vertex count and glue constant, straight lines unchanged.
- [GUP-551](GUP-551_Monotone_Line_Curves.md) ✅ — `Curve::MonotoneX`: Steffen
  tangents on the CPU as Bezier control columns, tessellated per segment, never
  overshooting. Deps: GUP-550 📋.
- [GUP-576](GUP-576_Monotone_Piece_Count_From_Density.md) 💡 — Monotone curves
  choose their piece count from pixel density, so 100K-row curves cost about
  what straight lines cost (16 pieces cost 18x today). Deps: GUP-551 ✅.
- [GUP-552](GUP-552_Chart_Subtitle.md) ✅ — `subtitle` on `Plot` and the
  builders, laid out under the title with its own theme size and colour. Deps:
  GUP-541 📋 (soft).
- [GUP-553](GUP-553_Dark_Theme.md) ✅ — `Theme::dark()` with contrast-checked
  colours and a theme palette (`OKABE_ITO_DARK`, black replaced) that
  `Categorical::okabe_ito()` follows. Deps: GUP-552 📋 (soft).
- [GUP-524](GUP-524_Svg_Unbrushed_Dimming.md) 💡 — `SvgTarget` dims unbrushed
  rows as the GPU does, so brushed goldens get the SVG parity check. Deps:
  GUP-553 ✅.
- [GUP-554](GUP-554_Box_Plot_Builder.md) 🚧 — `transform::box_stats` (type-7
  quartiles, 1.5 IQR whiskers) and `gup::boxplot` with outliers and tooltips.
  Deps: GUP-537 📋, GUP-548 📋.
- [GUP-405](GUP-405_Gup_Text_OpenType_Shaping.md) 📋 — Shape `gup-text` runs
  with rustybuzz: Inter's GPOS kerning and `tnum` tabular figures for tick
  labels, with a 25 KB gz WASM stop rule. Deps: GUP-400 ✅, GUP-542 📋 (soft).
- [GUP-416](GUP-416_Gup_Text_Font_Fallback.md) 📋 — A user-supplied fallback
  chain (`Context::add_fallback_font`), so characters outside the bundled subset
  draw from another font instead of as boxes; fallbacks parse lazily and
  `SvgTarget` embeds the fonts it uses. Deps: GUP-407 ✅, GUP-405 📋.
- [GUP-439](GUP-439_Line_Translucent_Joins.md) 📋 — Translucent lines with even
  opacity at rows, curve pieces and self-crossings (a per-layer coverage pass),
  opaque lines unchanged. Deps: GUP-432 ✅, GUP-550 📋, GUP-551 📋.

Follow-ups from reviewing N1 and N2 (2026-10-26). GUP-560, GUP-561 and GUP-563
all edit `plot.rs`, so they run one at a time; GUP-562 follows GUP-561 (it
reuses its `Linear::step`) and edits `encoding.rs`, as GUP-566 does; GUP-565
touches no hot file and runs beside any of them.

- [GUP-560](GUP-560_Bar_Labels_Follow_The_Plot_Tick_Format.md) ✅ — Bar value
  labels read the plot's stored tick format through a shared slot, so a format
  set via `plot_mut()` (before or after a resolve) labels the bars too; the
  builder's copy is removed. Deps: GUP-544 ✅, GUP-541 ✅.
- [GUP-561](GUP-561_Axis_Ends_Fit_Bars_And_Bands.md) ✅ — Bar and histogram
  value axes round out to their own tick step (`PositionScale::nice_to`), so
  every bar ends at or inside a labelled tick, and band and point axes lose the
  far-end `plot_padding` that overran heatmap cells. Deps: GUP-543 ✅, GUP-548
  ✅, GUP-549 ✅.
- [GUP-562](GUP-562_Integer_Tick_Steps_For_Integer_Data.md) 📋 —
  `ColumnStats::integral` reaches the scale with the extent, and a `Linear` axis
  fed only whole numbers (span at least 2) ticks in steps of at least 1:
  tutorial 6's Week axis reads 0–3. Deps: GUP-561 📋.
- [GUP-563](GUP-563_Input_Positions_From_Tuples.md) ✅ — `(f32, f32)` into
  `geom::Point`, `Input::pointer_moved` and friends, and `impl Into<Point>` on
  `zoom`, `invert` and every `pick`, so tutorials stop importing
  `gup::geom::Point`. Deps: GUP-532 ✅.
- [GUP-565](GUP-565_Text_Check_Against_The_Local_Background.md) 📋 — The
  harness's `text_present` check measures each region against its sampled local
  background, so labels on coloured fills pass the standard check; the stacked
  bar golden's workaround is removed. Deps: GUP-544 ✅.

## Track N5: Performance at Scale

Track N5 from the [November 2026 review](../STRATEGIC_REVIEW_2026-11.md):
columnar input, LOD, GPU sort and the 100M-row demonstration are not written
yet. These two are measured regressions found in N2; each commits a release
benchmark, and the orchestrator, not the worker, re-records `PERF_BUDGETS.md`.
GUP-566 edits `encoding.rs` (not beside GUP-562); GUP-567 edits `selection.rs`.

- [GUP-566](GUP-566_Cheaper_Owned_Category_Keys.md) 📋 — A keyed first-resolve
  benchmark, then builder keys encoded from a borrowed label (no second
  allocation) and a `foldhash` dictionary, each kept only if it helps: owned-key
  overhead from 52 to at most 30 ns per row. Deps: GUP-537 ✅.
- [GUP-567](GUP-567_Cached_Edge_Extremes_For_Zoom.md) 📋 — Sorted per-chunk
  values cached for the edge-inset solve, so a programmatic zoom answers by
  binary search with identical insets: `zoom_culled.cpu` from 1.24 back under
  0.90 ms. Deps: GUP-528 ✅.

## Open Backlog

Stories still worth doing that no active initiative owns, with the track that
will pick each up.

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
- [GUP-492](GUP-492_Windowed_Size_Order_Draws.md) 💡 — bounded draw calls for a
  windowed layer sized by a column whose sizes do not follow data order
  (per-chunk sort runs draw about one instance per draw today). Deps: GUP-482
  ✅. Track: N5 (GPU sort or index buffer).
- [GUP-376](GUP-376_GupApp_Event_Callbacks.md) 📋 — Add optional `.on_key()` and
  `.on_mouse()` callbacks to `GupApp` so examples with simple custom event
  handling can also use the shell. Deps: GUP-318 ✅. Track: N4 (keyboard).
