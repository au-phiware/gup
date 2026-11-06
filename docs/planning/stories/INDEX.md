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

Unparked 2026-11-01 when the shared build directory filled the disk (184 GB).
GUP-428 edits only `scripts/`, `maskfile.md` and `CLAUDE.md`.

- [GUP-428](GUP-428_Prune_Superseded_Shared_Units.md) ✅ — Rule 5 for
  `prune_build_dir.sh`: an 80 GB cap on the build directory, enforced by
  evicting least recently used units (last use propagated through the dependency
  graph in cargo's fingerprints), plus `--report` and a seeded bound test across
  removed worktrees. Deps: GUP-427 ✅.

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
- [GUP-564](GUP-564_Derive_Role_Message_And_Stale_Gallery.md) ✅ — The
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
- [GUP-526](GUP-526_Text_Layers_On_The_Right_Axis.md) ✅ — `TextLayer::y_right`
  and `y_right_key` map labels through the plot's right y axis, with GUP-555's
  edge room on that axis, so dual-axis charts label both series. Edits
  `plot.rs`. Deps: GUP-542 ✅, GUP-555 ✅.
- [GUP-527](GUP-527_Label_Halos_And_Overlap_Culling.md) ✅ — `halo(Px)` (dilated
  glyph bitmaps in the same atlas; SVG stroke with `paint-order="stroke"`) in
  the theme's background colour, and `hide_overlaps(true)` (greedy in row order,
  grid of kept boxes, +0.05 ms per 1,000 labels) on text layers. Default: no
  halo. Deps: GUP-542 ✅, GUP-405 ✅ (soft).
- [GUP-611](GUP-611_Label_Placement_Tries_Other_Anchors.md) 💡 — A ranked list
  of anchors per label, tried against GUP-527's grid before a label is hidden.
  Deps: GUP-527 ✅.
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
- [GUP-576](GUP-576_Monotone_Piece_Count_From_Density.md) ✅ — Monotone curves
  choose their piece count from pixel density, so 100K-row curves cost about
  what straight lines cost (16 pieces cost 18x today). Deps: GUP-551 ✅.
- [GUP-552](GUP-552_Chart_Subtitle.md) ✅ — `subtitle` on `Plot` and the
  builders, laid out under the title with its own theme size and colour. Deps:
  GUP-541 📋 (soft).
- [GUP-553](GUP-553_Dark_Theme.md) ✅ — `Theme::dark()` with contrast-checked
  colours and a theme palette (`OKABE_ITO_DARK`, black replaced) that
  `Categorical::okabe_ito()` follows. Deps: GUP-552 📋 (soft).
- [GUP-524](GUP-524_Svg_Unbrushed_Dimming.md) ✅ — `SvgTarget` dims unbrushed
  rows as the GPU does, through a CPU mirror of the brush state shared with the
  cached scene, so the three brushed goldens get the SVG parity check. Edits
  `selection.rs`. Deps: GUP-553 ✅.
- [GUP-554](GUP-554_Box_Plot_Builder.md) ✅ — `transform::box_stats` (type-7
  quartiles, 1.5 IQR whiskers) and `gup::boxplot` with outliers and tooltips.
  Deps: GUP-537 📋, GUP-548 📋.
- [GUP-405](GUP-405_Gup_Text_OpenType_Shaping.md) ✅ — `gup-text` lays out by
  glyph index and kerns every run with Inter's GPOS `kern` (an in-house walk on
  fontdue's `ttf-parser`, checked against rustybuzz as a dev-dependency;
  rustybuzz itself would add 210 KB gz), adds `Features::TNUM`, and re-blesses
  the goldens, within +6,000 B gz. Deps: GUP-400 ✅, GUP-407 ✅, GUP-542 ✅.
- [GUP-579](GUP-579_Tabular_Figures_For_Tick_Labels.md) ✅ — Tick labels request
  `tnum` (measured and drawn), so four-digit axes keep their margin at every
  zoom, and `SvgTarget` writes `font-variant-numeric:tabular-nums` on them.
  Edits `plot.rs`. Deps: GUP-405 ✅.
- [GUP-416](GUP-416_Gup_Text_Font_Fallback.md) ✅ — `TextSystem::add_fallback`
  and `Font::with_fallbacks`: an ordered chain for characters the primary lacks,
  with fonts validated at `from_bytes` and parsed on first use (Noto Sans JP
  takes 259 ms), in `gup-text` only. Deps: GUP-407 ✅, GUP-405 ✅.
- [GUP-580](GUP-580_Fallback_Fonts_In_Gup_Core_And_Svg.md) ✅ —
  `Context::add_fallback_font`, the `gup_core::Font` re-export, the scene's
  fallback list, SVG `font-family` and embedding of the fallbacks a scene uses,
  and a mixed-script title golden. Edits `lib.rs` and `plot.rs`. Deps: GUP-416
  ✅.
- [GUP-557](GUP-557_Svg_Text_Path_Out_Of_Wasm.md) ✅ — The size harness's SVG
  call moves behind an `svg-export` feature (its own `wasm.svg_export` metric),
  so `wasm.scatter` measures a browser chart; then GUP-580's fallback code is
  ungated, so every build writes the same SVG text. Deps: GUP-580 ✅.
- [GUP-439](GUP-439_Line_Translucent_Joins.md) ✅ — Translucent lines with even
  opacity at rows, curve pieces and self-crossings (a per-layer coverage pass),
  opaque lines unchanged. Deps: GUP-432 ✅, GUP-550 📋, GUP-551 📋.
- [GUP-584](GUP-584_Translucent_Lines_In_Host_Passes.md) ✅ — A public
  `Prepared::encode` so hosts drawing in their own pass get GUP-439's offscreen
  pass (`draw` stays correct without it), and `zoom_bench --translucent`. Edits
  `render.rs`. Deps: GUP-439 ✅.
- [GUP-585](GUP-585_Translucent_Lines_Per_Series.md) 💡 — Translucent lines
  blend once per series, not per layer, so overlapping series show density; plus
  browser coverage of the composite shader. Deps: GUP-439 ✅.

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
- [GUP-562](GUP-562_Integer_Tick_Steps_For_Integer_Data.md) ✅ —
  `ColumnStats::integral` reaches the scale with the extent, and a `Linear` axis
  fed only whole numbers (span at least 2) ticks in steps of at least 1:
  tutorial 6's Week axis reads 0–3. Deps: GUP-561 📋.
- [GUP-563](GUP-563_Input_Positions_From_Tuples.md) ✅ — `(f32, f32)` into
  `geom::Point`, `Input::pointer_moved` and friends, and `impl Into<Point>` on
  `zoom`, `invert` and every `pick`, so tutorials stop importing
  `gup::geom::Point`. Deps: GUP-532 ✅.
- [GUP-565](GUP-565_Text_Check_Against_The_Local_Background.md) ✅ — The
  harness's `text_present` check measures each region against its sampled local
  background, so labels on coloured fills pass the standard check; the stacked
  bar golden's workaround is removed. Deps: GUP-544 ✅.

## Track N3: Release 0.1

Track N3 from the [November 2026 review](../STRATEGIC_REVIEW_2026-11.md), with
the owner's decisions of 2026-10-24: GPL-3.0-or-later kept, minimal
accessibility in 0.1. Owner steps and the run order follow the list. GUP-557
(track N2) is part of the run order because GUP-593 edits the same `svg.rs` code
and WASM harness after it.

- [GUP-588](GUP-588_Publish_Metadata_And_Licence_Headers.md) ✅ — Workspace
  metadata and `=0.1.0` internal versions, plain descriptions, crate READMEs,
  licence symlinks, a header check in `all-check`, and a root `include` list, so
  all seven crates package.
- [GUP-589](GUP-589_Publish_Dry_Run_And_Docs_Rs_In_CI.md) ✅ — `gup` forwards
  `window`, `svg` and `web`; docs.rs tables; `mask publish-check`,
  `mask docs-rs` and a Release check workflow on every push. Deps: GUP-588 📋.
- [GUP-590](GUP-590_Windows_Warp_Render_Smoke_Job.md) ✅ — A `platforms.yml`
  Windows job on WARP (DX12): an adapter spike, then the root crate's golden
  tests, diff images uploaded on failure. Deps: GUP-594 📋 (soft).
- [GUP-591](GUP-591_Macos_Metal_Smoke_Spike.md) ✅ — The same job on `macos-15`
  Metal, kept: the free runner has a Metal GPU. Deps: GUP-590 ✅.
- [GUP-592](GUP-592_Plot_Describe_Text_Alternative.md) ✅ — `Description`,
  `Plot::describe`, `Chart::describe` and `.description(text)`: a text
  alternative built at resolve from the title, kind, row counts, drawn tick
  labels and legends, stored in `Scene`. Deps: GUP-552 ✅, GUP-554 ✅.
- [GUP-593](GUP-593_Svg_Title_And_Canvas_Aria_Label.md) ✅ — SVG `role="img"`,
  `<title>` and `<desc>`; `CanvasApp` sets `role` and `aria-label` unless the
  page set its own, checked in the browser test. Deps: GUP-592 📋, GUP-557 📋.
- [GUP-594](GUP-594_Flagship_Gallery_Part_One.md) ✅ — Five finished gallery
  examples (bubbles, monotone lines, labelled grouped bars, ranked `barh`,
  stacked areas), each a runnable example, a CI thumbnail and a golden in
  `tests/gallery.rs`. Deps: GUP-536 📋.
- [GUP-595](GUP-595_Flagship_Gallery_Part_Two_And_Hero.md) ✅ — Five more
  (stacked histogram, heatmap, box plot, annotated `Plot`, dark lines) and the
  README hero as a 1200 × 630 golden. Deps: GUP-594 📋.
- [GUP-596](GUP-596_Changelog_And_Semver_Policy.md) ✅ — `CHANGELOG.md` (Keep a
  Changelog) with 0.1.0's contents and limitations, and the 0.x policy: lockstep
  versions, breaking means minor, wgpu and MSRV bumps are breaking. Deps:
  GUP-592 📋 (soft), GUP-593 📋 (soft).
- [GUP-597](GUP-597_Readme_For_Release_0_1.md) ✅ — The README at release
  candidate: hero image, eight builders, browser features, a current "Not yet"
  and platform line, CHANGELOG link, published-crates column. Deps: GUP-589 📋,
  GUP-590 📋, GUP-591 📋, GUP-593 📋, GUP-596 📋.
- [GUP-602](GUP-602_Feature_Combinations_Compile.md) ✅ — Every supported
  feature combination builds and tests (`window`-only lib tests fail today at
  `scope.rs:338`); `mask feature-matrix` in CI. Deps: GUP-589 ✅.
- [GUP-598](GUP-598_Release_Checklist.md) ✅ — `mask release-check` (every gate,
  publish dry run, docs.rs, one-version and slug checks, seeded) and
  `docs/RELEASING.md`; ends at "ready to publish". Deps: GUP-536 📋, GUP-588–597
  📋.
- [GUP-599](GUP-599_Switch_Docs_To_Crates_Io.md) 📋 — After the owner publishes:
  README, tutorial 1 and the egui and Bevy guides depend on `"0.1"` from
  crates.io, proven by an external build. Deps: GUP-598 📋, owner publish.
- [GUP-600](GUP-600_Describe_Composite_Marks.md) ✅ — Chart descriptions merge
  same-named layers and name a box plot by its boxes. Deps: GUP-592 ✅.
- [GUP-601](GUP-601_Describe_Vertical_Band_Order.md) ✅ — Chart descriptions
  read vertical band axes top to bottom. Deps: GUP-592 ✅.
- [GUP-603](GUP-603_Publish_Check_Private_Package_Dir.md) ✅ —
  `mask publish-check` fails on a stale shared package directory. Deps: GUP-598
  ✅.
- [GUP-604](GUP-604_Release_Check_Workflow_Conclusions.md) ✅ — `release-check`
  reads the latest workflow job conclusions. Deps: GUP-598 ✅.
- [GUP-605](GUP-605_Reproduce_Publish_Check_Shared_Dir_Failure.md) 💡 — find the
  cause of the shared-directory publish-check failure. Deps: GUP-603 ✅.
- [GUP-606](GUP-606_Release_Check_Branch_Workflow_Runs.md) 💡 — `release-check`
  `--ci-ref` to read a branch's runs. Deps: GUP-604 ✅.

Owner steps (not worker stories), in GUP-598's `docs/RELEASING.md`: a crates.io
token (`cargo login`); re-confirm the seven names are free (all were on
2026-11-02); enable GitHub Pages and set `DEPLOY_GALLERY=true`; date the
CHANGELOG; `cargo publish --workspace`; tag `v0.1.0` and a GitHub release.
Billed macOS `-xlarge` runners are an owner decision only if GUP-591 finds the
standard runner cannot draw.

Run order (at most two build-heavy stories at once; hot files: Cargo.toml files,
`README.md`, `.github/workflows/`, `src/lib.rs`, `maskfile.md`, `plot.rs`,
`selection.rs`, `svg.rs`, the WASM scenes, this file):

1. GUP-588 beside GUP-592; GUP-596 may run as a docs-only third.
2. GUP-589 (after GUP-588) beside GUP-592, if still running, or GUP-557.
3. GUP-590 (workflows) beside GUP-557 (`svg.rs`, WASM scenes).
4. GUP-594 (root `Cargo.toml`; after GUP-536 and GUP-589) beside GUP-593 (after
   GUP-592 and GUP-557).
5. GUP-595 beside GUP-591 (after GUP-590).
6. GUP-597 alone among `README.md` stories; then GUP-598 alone.
7. The owner publishes; then GUP-599.

## Track N4: Accessibility

Track N4 from the [November 2026 review](../STRATEGIC_REVIEW_2026-11.md): the
rest of accessibility after release 0.1's text alternative (GUP-592, GUP-593;
owner decision 4). Keyboard focus over rows (a keyboard-driven hover with a
focus ring), keyboard zoom, an announced text per row, hosts that forward keys,
a hidden data table in the browser, a high-contrast theme and a guide. The
archived docs in `docs/archive/accessibility/` describe the deleted system and
are background only. GUP-619, GUP-620, GUP-621 and GUP-624 all edit `plot.rs`
and run one at a time; the run order is at the end of "Polish after 0.1".

- [GUP-619](GUP-619_Keyboard_Focus_Over_Rows.md) ✅ — `Input::KeyDown` with
  semantic `Key`s and `Modifiers`; arrow keys walk rows in view (x order within
  a series, Up/Down between series), Enter clicks, Escape clears, a focus ring,
  and `Selection::focusable(false)` for composite layers. Deps: GUP-563 ✅,
  GUP-546 ✅.
- [GUP-376](GUP-376_GupApp_Event_Callbacks.md) 🚫 — Add optional `.on_key()` and
  `.on_mouse()` callbacks to `GupApp` so examples with simple custom event
  handling can also use the shell. Deps: GUP-318 ✅. Legacy (2025): GUP-622
  forwards the window's keys to the chart itself, which covers the keyboard
  need; proposed for closing as superseded once GUP-622 lands.
- [GUP-620](GUP-620_Keyboard_Zoom_Pan_And_Reset.md) 📋 — `ZoomIn`/`ZoomOut` (two
  wheel lines about the plot centre), Shift+arrows (pan a tenth of the plot) and
  `ResetView`, through the wheel's and drag's paths, keeping focus while its row
  stays in view. Deps: GUP-619 📋.
- [GUP-621](GUP-621_Row_Announcements.md) 📋 — `Plot::row_fields`, `row_text`
  and `focus_text` (and `Chart::focus_text`): a row's series, x, y and colour
  value with axis names, in the axes' formats (ISO dates on `Time`), with hooks
  for bars, bins and boxes. Deps: GUP-619 📋, GUP-615 📋.
- [GUP-622](GUP-622_Window_And_Egui_Hosts_Forward_Keys.md) ✅ — The window app
  and the egui widget map arrows, Home/End, Enter/Space, Escape, `+`/`=`, `-`
  and `0` to `Input::KeyDown`; the window closes on an Escape the chart did not
  use, and the egui widget takes and locks focus. Deps: GUP-619 📋, GUP-620 📋
  (soft).
- [GUP-623](GUP-623_Browser_Keyboard_And_Live_Region.md) 📋 — `CanvasApp` makes
  the canvas focusable, forwards mapped keys (preventing only their default) and
  speaks `focus_text()` through a hidden `aria-live` region, checked by new
  browser smoke steps. Deps: GUP-619 📋, GUP-621 📋, GUP-622 📋 (soft).
- [GUP-624](GUP-624_Hidden_Data_Table_In_The_Browser.md) 📋 — `DataTable`,
  `Plot::data_table` and `Chart::data_table` from GUP-621's row fields, written
  by `CanvasApp` as a visually hidden `<table>` (at most 1,000 rows) and rebuilt
  when the description changes. Deps: GUP-621 📋, GUP-623 📋.
- [GUP-625](GUP-625_High_Contrast_Theme.md) 🚧 — `Theme::high_contrast()` (black
  text and axes, a 3:1 grid, larger text) and a `HIGH_CONTRAST` palette of eight
  colours, each at least 3:1 on white and at least ΔE00 12 apart under normal,
  deutan and protan vision, all by test. Deps: GUP-553 ✅.
- [GUP-626](GUP-626_Accessibility_Guide.md) 📋 — `docs/ACCESSIBILITY.md`: what
  Gup does per output, keys, screen readers, contrast, what the developer must
  do and what is not supported, every example a doctest; README, CHANGELOG and
  the archive README brought in line. Deps: GUP-619–625 📋.

## Track N5: Performance at Scale

Track N5 from the [November 2026 review](../STRATEGIC_REVIEW_2026-11.md).
GUP-566, GUP-577 and GUP-567 were measured regressions found in N2. The track
proper (written 2026-11-04) measures ingest first (GUP-627), then adds columnar
input (GUP-628, opus; GUP-629; Arrow in GUP-630), chooses a level-of-detail
design by measurement (GUP-631, opus, which lists the stories that build it) and
ends in a 100M-row demonstration with README wording by rule (GUP-632). A 1B-row
run needs out-of-core paging and is not written. Each story commits its
benchmark; the orchestrator, not the worker, records `PERF_BUDGETS.md`. GUP-492
(windowed size-ordered draws, a GPU sort or index buffer) is 💡 and needs a
scribe pass before it starts.

- [GUP-566](GUP-566_Cheaper_Owned_Category_Keys.md) ✅ — A keyed first-resolve
  benchmark, then builder keys encoded from a borrowed label (no second
  allocation) and a `foldhash` dictionary, each kept only if it helps: owned-key
  overhead from 52 to at most 30 ns per row. Deps: GUP-537 ✅.
- [GUP-577](GUP-577_Cap_Evicts_Unowned_Incremental_Caches.md) ✅ — Rule 5's
  least-recently-used queue holds incremental crate caches too (ties go to
  caches), with a seeded path-dependency case and a control, so the cap can
  always be met. Edits `scripts/`, `maskfile.md` and `CLAUDE.md`; runs beside
  any story that does not edit `maskfile.md`. Deps: GUP-428 ✅.
- [GUP-567](GUP-567_Cached_Edge_Extremes_For_Zoom.md) ✅ — Sorted per-chunk
  values cached for the edge-inset solve, so a programmatic zoom answers by
  binary search with identical insets: `zoom_culled.cpu` from 1.24 back under
  0.90 ms. Deps: GUP-528 ✅.
- [GUP-627](GUP-627_Scale_Benchmark_Ingest_And_Memory.md) ✅ — A headless
  `scale_bench` example timing each ingest phase (closures, f64 columns, chunk
  conversion and upload, first resolve, first frame, zoom) with peak RSS, run at
  1M–100M rows, so N5 is designed from numbers. Deps: GUP-457 ✅, GUP-482 ✅.
- [GUP-628](GUP-628_Columnar_Layers.md) 📋 — `Columns`/`Column` (`Arc`'d f64 and
  f32), `Plot::add_columns::<M>` over `()` rows, `x_column`/`y_column` and
  `encode_column`: identical pictures to the rows path, and an f32 fast path
  chosen by a measured rule. Opus. Deps: GUP-627 📋.
- [GUP-629](GUP-629_Key_Columns.md) 📋 — `Column::Keys(KeyColumn)` (codes plus
  labels) through Band, Point and `Categorical` scales, remapped once per label
  in the row path's first-seen order, with images identical to key closures.
  Deps: GUP-628 📋.
- [GUP-630](GUP-630_Arrow_Record_Batches.md) 📋 — Feature `arrow`
  (`arrow-array`, `arrow-schema`): `Columns::from_arrow(&RecordBatch)` for
  floats, integers, dates, timestamps, strings, dictionaries and booleans, nulls
  kept, in the feature matrix and docs.rs. Deps: GUP-628 📋, GUP-629 📋, GUP-602
  ✅.
- [GUP-631](GUP-631_Level_Of_Detail_Spike.md) 📋 — Spike: per-frame density, a
  decimation pyramid ported from `gup-culling-lod`, and binned tiles, measured
  at 100M rows for GPU time, build time, memory and fidelity, chosen by a stated
  rule, with RFC-001 findings and the follow-up stories. Opus. Deps: GUP-627 📋,
  GUP-628 📋.
- [GUP-632](GUP-632_Hundred_Million_Row_Demonstration.md) 📋 — A
  `hundred_million` example from `Columns` with zoom, pan and keys, measured at
  10M, 30M and 100M rows, proposed budget rows, and README wording by a stated
  rule ("interactive" only under 16.7 ms; no "billion"). Deps: GUP-628 📋,
  GUP-629 📋, GUP-631 📋 and its follow-ups (soft).
- [GUP-492](GUP-492_Windowed_Size_Order_Draws.md) 💡 — bounded draw calls for a
  windowed layer sized by a column whose sizes do not follow data order
  (per-chunk sort runs draw about one instance per draw today). Deps: GUP-482
  ✅.

## Polish after 0.1

Fixes found while building the gallery, the tutorials and release 0.1, each
checked against `main` on 2026-11-04. GUP-584, GUP-585 and GUP-611 (listed under
track N2) belong to this group too. The run order for this group, track N4 and
track N5 is at the end of this section.

- [GUP-612](GUP-612_Line_And_Area_X_Axes_End_At_The_Data.md) ✅ — The x axis of
  `gup::line` and `gup::area` ends at the first and last rows (a third per-axis
  fit policy beside GUP-561's ticks), so the gallery's stacked area loses its
  pinned domain. Deps: GUP-561 ✅, GUP-547 ✅.
- [GUP-613](GUP-613_Chart_Types_In_The_Prelude.md) ✅ — The seven builder chart
  types in the prelude and `Horizontal`/`Vertical`/`Orientation` at the root, so
  no example imports from `gup::builder::`. Deps: GUP-594 ✅, GUP-595 ✅.
- [GUP-614](GUP-614_Builder_Text_Layers_And_Pick_Row.md) ✅ —
  `.text(rows, |t| ..)` and an inherent `pick_row(impl Into<Point>)` on all
  seven builders (one macro arm), and tutorial 7's labels without `plot_mut()`.
  Deps: GUP-542 ✅, GUP-563 ✅, GUP-536 ✅.
- [GUP-615](GUP-615_Value_Labels_Read_The_Axis_Format.md) ✅ — Bar value labels
  follow the value axis's effective tick format, including one set only through
  `ScaleRef::tick_format`, and a documented one-unit rule that the gallery's
  grouped bars follow. Deps: GUP-560 ✅, GUP-544 ✅.
- [GUP-616](GUP-616_Legends_Match_What_Is_Drawn.md) ✅ — Vertical stacks (bars,
  areas, histograms) list their legend top-down, and swatches of translucent
  marks are composited at the marks' opacity, so each matches what is drawn; the
  description follows. Deps: GUP-547 ✅, GUP-592 ✅.
- [GUP-651](GUP-651_Host_Pass_Srgb_Blending.md) ✅ — A host pass with an sRGB
  colour view blends translucent marks in linear light; document and check the
  non-sRGB contract. Deps: GUP-616.
- [GUP-617](GUP-617_Tight_Box_Width_Test.md) ✅ — Measure why the box plot's
  width test needed 3 px and tighten it to 1 px by a stated rule (wrong
  expectation, wrong measurement, or a geometry bug to escalate), with a 3 px
  negative control. Deps: GUP-554 ✅.
- [GUP-618](GUP-618_Harness_Catches_Invisible_Labels.md) ✅ — `text_present`
  fails a label drawn in its background's colour (today it passes, because the
  sampled background skips text-coloured pixels), with a threshold set from
  every real region and a seeded failing chart. Deps: GUP-565 ✅, GUP-397 ✅.

Run order for this group, track N4 and track N5 (2026-11-04). At most two
build-heavy stories at once; two stories in one step share no hot file
(`plot.rs`, `selection.rs`, `render.rs`, `encoding.rs`, `builder/mod.rs`,
`crates/gup-core/src/lib.rs`, `src/lib.rs`, `maskfile.md`, the WASM scenes, this
file). Lane A holds the `plot.rs` and `selection.rs` stories, one at a time;
lane B runs beside it. Opus stories (GUP-628, GUP-585, GUP-631) are best started
early in the week.

| Step | Lane A                      | Lane B                        |
| ---- | --------------------------- | ----------------------------- |
| 1    | GUP-612                     | GUP-618                       |
| 2    | GUP-615                     | GUP-617                       |
| 3    | GUP-616                     | GUP-613                       |
| 4    | GUP-619                     | GUP-614                       |
| 5    | GUP-620                     | GUP-584 (`render.rs`)         |
| 6    | GUP-621                     | GUP-627                       |
| 7    | GUP-628 (opus)              | GUP-623 (WASM scenes)         |
| 8    | GUP-624 (WASM scenes)       | GUP-622                       |
| 9    | GUP-585 (opus, `render.rs`) | GUP-625 (`lib.rs`)            |
| 10   | GUP-629 (`encoding.rs`)     | GUP-611                       |
| 11   | GUP-631 (opus spike)        | GUP-630 (`maskfile.md`)       |
| 12   | GUP-631's follow-ups        | GUP-626 (`src/lib.rs`)        |
| 13   | GUP-632                     | GUP-492 (after a scribe pass) |

GUP-625 touches `lib.rs` and must not run beside GUP-628 or GUP-619. GUP-611
(💡) needs its candidate-list setter named before it starts. Close GUP-376 when
GUP-622 lands if the owner agrees.

## Open Backlog

Stories still worth doing that no active initiative owns, with the track that
will pick each up.

- [GUP-413](GUP-413_Per_Checkout_Clippy_Artifacts.md) 💡 — Clippy artifacts are
  still shared between checkouts (GUP-411 made them re-lint on a switch, ~90 s
  instead of 26 s for `mask all-check`). Decide with measurements whether a
  per-checkout member `codegen-units` is worth its disk, and if so add it to
  `scripts/cargo_env.sh`. Deps: GUP-411 ✅. Related to GUP-427 (disk growth),
  not superseded by it. No track; keep.
