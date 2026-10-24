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
