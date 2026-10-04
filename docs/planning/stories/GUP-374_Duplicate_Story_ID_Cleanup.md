# GUP-374: Duplicate Story ID Cleanup

## Story Overview

**Initiative**: Project Maintenance **Status**: 📋 Planned **Created**:
2025-07-22 **Revised**: 2026-10-04

## Context

This story originally described a single duplicate: GUP-315 was used by both
"Graph Node Label Rendering" and "3D Axis and Grid". A full audit of
`docs/planning/stories/` (triggered while drafting this revision) found that the
problem is systemic, not isolated: **34 story IDs are currently reused by two or
more files**, totalling 41 files that need a new, unique ID. One of these
collisions has already been worked around ad-hoc: INDEX.md references `GUP-285B`
for `GUP-285_Fix_WASM_Integration_Test_Compilation.md`, a non-standard suffix
that doesn't exist anywhere else in the project and isn't a real story ID.

The collisions appear to come from two sources: (1) parallel work that drew a
new story number from an out-of-date view of the index, and (2) at least one
orphaned file (`GUP-065_Procedural_Macro_Performance_Optimization.md`) that was
never added to INDEX.md at all, so its number silently diverged from the indexed
`GUP-065_Documentation_Macro_First_API.md`. Left unaddressed, duplicate IDs make
cross-references ambiguous (which GUP-288 does a dependency line mean?), break
tooling that keys off the ID, and will keep recurring as long as nothing
prevents a second file from claiming an already-used number.

This story covers the **inventory and renumbering** of every existing duplicate.
A companion story, [GUP-383](GUP-383_Duplicate_Story_ID_Guard.md), adds an
automated guard so the problem cannot silently recur. The two are split because
the renumbering is a large, mechanical, one-time cleanup across ~41 files, while
the guard is a small, independent piece of tooling that can be built and enabled
in CI immediately — it does not need to wait for the renumbering to be finished
(it will simply report many failures until the renumbering lands).

## User Story

> "As a project maintainer, I want every story ID in `docs/planning/stories/` to
> be unique so that dependency references, retrospectives, and commit messages
> that cite a GUP number are unambiguous."

## Duplicate ID Inventory (as of 2026-10-04)

Found via:

```bash
ls docs/planning/stories | grep -oE '^GUP-[0-9]+_' | sort | uniq -d
grep -oE '^- \[GUP-[0-9A-Z]+\]' docs/planning/stories/INDEX.md | sort | uniq -d
```

**New IDs are not pre-allocated by this story.** An earlier revision proposed a
concrete numbering starting at GUP-384, but GUP-384 through GUP-387 were
allocated to other stories before this renumbering was picked up (and more IDs
have been allocated since, for the October 2026 strategic-review wave-1
stories), which would have made the hard-coded table wrong the moment any of
those stories landed. Instead: **at implementation time**, re-run
`ls docs/planning/stories | grep -oP 'GUP-\d+' | sort -t- -k2 -n | tail -1` to
find the then-current max ID, and allocate new IDs sequentially from `max + 1`
for every row below marked "renumber" in table order (top to bottom). The
table's job is to record which file in each collision group keeps its ID and
which must be renumbered — not which specific number each gets.

"Keep" marks the file that retains its current ID. The rule applied: if the
group has exactly one `✅ Complete` entry, keep it (completed stories are the
most entangled with commit history, retrospectives, and shipped code —
renumbering them is riskier and lower-value than renumbering a still-`📋`
story). If a group has more than one, or none, `✅ Complete` entry, keep
whichever file appears first in INDEX.md document order and renumber the rest.

| Old ID  | File                                                    | Status          | Keep / New ID                        |
| ------- | ------------------------------------------------------- | --------------- | ------------------------------------ |
| GUP-013 | `GUP-013_Event_Handling_System.md`                      | ✅              | **Keep**                             |
| GUP-013 | `GUP-013_GPU_Shader_Position_Precision_Fix.md`          | ✅              | renumber                             |
| GUP-014 | `GUP-014_Interaction_Performance_Optimization.md`       | ✅              | **Keep**                             |
| GUP-014 | `GUP-014_Performance_Validation.md`                     | ✅              | renumber                             |
| GUP-015 | `GUP-015_GPU_Debugging_Tools.md`                        | ✅              | **Keep**                             |
| GUP-015 | `GUP-015_Real_Time_Data_Streaming.md`                   | ✅              | renumber                             |
| GUP-053 | `GUP-053_Advanced_Shader_Function_Library.md`           | ✅              | **Keep**                             |
| GUP-053 | `GUP-053_Shader_Pipeline_Performance_Optimization.md`   | ✅              | renumber                             |
| GUP-054 | `GUP-054_Existing_Solutions_Analysis.md`                | ✅              | **Keep**                             |
| GUP-054 | `GUP-054_Shader_Function_Performance_Optimization.md`   | ✅              | renumber                             |
| GUP-054 | `GUP-054_Shader_Function_Type_Safety_Enhancement.md`    | ✅              | renumber                             |
| GUP-065 | `GUP-065_Documentation_Macro_First_API.md`              | ✅              | **Keep**                             |
| GUP-065 | `GUP-065_Procedural_Macro_Performance_Optimization.md`¹ | ✅              | renumber                             |
| GUP-076 | `GUP-076_GPU_Occlusion_Culling.md`                      | ✅              | **Keep**                             |
| GUP-076 | `GUP-076_Spatial_Index_Bind_Group_Layout_Fix.md`        | ✅              | renumber                             |
| GUP-077 | `GUP-077_Compute_Shader_Instance_Filtering.md`          | ✅              | **Keep**                             |
| GUP-077 | `GUP-077_Performance_Benchmarking_Suite.md`             | ✅              | renumber                             |
| GUP-086 | `GUP-086_Observable_Plot_Migration_Guide.md`            | ✅              | **Keep**                             |
| GUP-086 | `GUP-086_Web_Profiling_Dashboard.md`                    | ✅              | renumber                             |
| GUP-096 | `GUP-096_Grid_Performance_Benchmarking.md`              | ✅              | **Keep** (real story)                |
| GUP-096 | `GUP-096_performance_report.md`²                        | n/a             | not a story — rename, don't renumber |
| GUP-140 | `GUP-140_Selection_API_Parallel_Output.md`              | ✅              | **Keep**                             |
| GUP-140 | `GUP-140_Storage_Buffer_Keyframes.md`                   | ✅              | renumber                             |
| GUP-147 | `GUP-147_Box_Plot_Visualization.md`                     | ✅              | **Keep**                             |
| GUP-147 | `GUP-147_GPU_Memory_Bandwidth_Profiling.md`             | ✅              | renumber                             |
| GUP-148 | `GUP-148_Fix_Statistics_Shader_Bug.md`                  | ✅              | **Keep**                             |
| GUP-148 | `GUP-148_Profiling_Data_Export_Visualization.md`        | ✅              | renumber                             |
| GUP-149 | `GUP-149_Automatic_Device_Loss_Detection.md`            | ✅              | **Keep**                             |
| GUP-149 | `GUP-149_Box_Plot_GPU_Rendering.md`                     | ✅              | renumber                             |
| GUP-150 | `GUP-150_Recovery_Metrics_and_Analytics.md`             | ✅              | **Keep**                             |
| GUP-150 | `GUP-150_Statistical_Mark_Builder_API.md`               | ✅              | renumber                             |
| GUP-151 | `GUP-151_Multi_Category_Box_Plots.md`                   | ✅              | **Keep**                             |
| GUP-151 | `GUP-151_Surface_Configuration_Caching.md`              | ✅              | renumber                             |
| GUP-233 | `GUP-233_Fix_Flaky_Registry_Scalability_Test.md`        | ✅              | **Keep**                             |
| GUP-233 | `GUP-233_Winit_Touch_Event_Integration.md`              | ✅              | renumber                             |
| GUP-234 | `GUP-234_Adaptive_Build_Coverage_Budget.md`             | ✅              | **Keep**                             |
| GUP-234 | `GUP-234_Touch_Lasso_Selection.md`                      | ✅              | renumber                             |
| GUP-272 | `GUP-272_WCAG_2_1_AA_Compliance_Validation.md`          | ✅              | **Keep**                             |
| GUP-272 | `GUP-272_iOS_Chart_Rendering_Integration.md`            | 📋              | renumber                             |
| GUP-273 | `GUP-273_Geographic_Projection_Shader_System.md`        | ✅              | **Keep**                             |
| GUP-273 | `GUP-273_cbindgen_FFI_Integration.md`                   | 📋              | renumber                             |
| GUP-274 | `GUP-274_Map_Mark_Rendering.md`                         | ✅              | **Keep**                             |
| GUP-274 | `GUP-274_iOS_Real_Device_Testing.md`                    | 📋              | renumber                             |
| GUP-277 | `GUP-277_GPU_Render_Loop_Transition_Integration.md`     | ✅              | **Keep**                             |
| GUP-277 | `GUP-277_Zoom_Pan_Interactions.md`                      | ✅              | renumber                             |
| GUP-278 | `GUP-278_Staggered_Transition_Delays.md`                | ✅              | **Keep**                             |
| GUP-278 | `GUP-278_Brush_Mark_Rectangular_Selection.md`           | ✅              | renumber                             |
| GUP-283 | `GUP-283_Event_Coalescing.md`                           | ✅              | **Keep**                             |
| GUP-283 | `GUP-283_Fix_WASM_Build_StreamingBuffer.md`             | ✅              | renumber                             |
| GUP-284 | `GUP-284_Unified_Vec2_Type.md`                          | ✅              | **Keep**                             |
| GUP-284 | `GUP-284_Unify_Chart_Builder_Data_Layer.md`             | ✅              | renumber                             |
| GUP-285 | `GUP-285_BrushMark_GPU_Overlay_Rendering.md`            | ✅              | **Keep**                             |
| GUP-285 | `GUP-285_High_Resolution_GeoJSON_Streaming.md`          | 📋              | renumber                             |
| GUP-285 | `GUP-285_Legend_Rendering_System.md`                    | 💡              | renumber                             |
| GUP-285 | `GUP-285_Tauri_Streaming_Updates.md`                    | 📋              | renumber                             |
| GUP-285 | `GUP-285_Fix_WASM_Integration_Test_Compilation.md`³     | 📋              | renumber                             |
| GUP-286 | `GUP-286_GPU_Accelerated_Brush_Region_Query.md`         | ✅              | **Keep**                             |
| GUP-286 | `GUP-286_Line_Chart_Data_Mark_Rendering.md`             | ✅              | renumber                             |
| GUP-286 | `GUP-286_Per_Bar_Instance_Buffer_Fill.md`               | 💡              | renumber                             |
| GUP-286 | `GUP-286_Spherical_Polygon_Simplification.md`           | 📋              | renumber                             |
| GUP-287 | `GUP-287_LinkedSelection_Wrapper_Type.md`               | ✅              | **Keep**                             |
| GUP-287 | `GUP-287_GPU_Side_Choropleth_Recolouring.md`            | ✅              | renumber                             |
| GUP-287 | `GUP-287_Dynamic_Data_Refresh.md`                       | 📋              | renumber                             |
| GUP-288 | `GUP-288_GPU_Selection_Mask_Buffer.md`                  | ✅              | **Keep**                             |
| GUP-288 | `GUP-288_Choropleth_Tooltip_Hover_Interaction.md`       | ✅              | renumber                             |
| GUP-288 | `GUP-288_Area_Chart_Data_Mark_Rendering.md`⁴            | ✅ (superseded) | renumber, or delete                  |
| GUP-289 | `GUP-289_LinkedSelection_GPU_Integration.md`            | ✅              | **Keep**                             |
| GUP-289 | `GUP-289_Bar_Chart_Builder_Prepare_Render_Bound.md`     | ✅              | renumber                             |
| GUP-312 | `GUP-312_GPU_Compute_Treemap.md`                        | ✅              | **Keep**                             |
| GUP-312 | `GUP-312_Full_GPU_Quadtree_Construction.md`             | 📋              | renumber                             |
| GUP-313 | `GUP-313_Adaptive_Barnes_Hut_Theta_Tuning.md`           | ✅              | **Keep**                             |
| GUP-313 | `GUP-313_Interactive_Treemap_Drill_Down.md`             | 📋              | renumber                             |
| GUP-314 | `GUP-314_Windowed_Treemap_Rendering.md`                 | ✅              | **Keep**                             |
| GUP-314 | `GUP-314_Shared_Device_Layout_Engine.md`                | 📋              | renumber                             |
| GUP-315 | `GUP-315_3D_Axis_and_Grid.md`                           | ✅              | **Keep**                             |
| GUP-315 | `GUP-315_Graph_Node_Label_Rendering.md`                 | 📋              | renumber                             |

¹ Not referenced in INDEX.md at all — an orphan. Renumbering must also add a
missing INDEX.md entry for it (see Technical Tasks).

² Not a story file (no `# GUP-NNN:` header, no Status/AC/Dependencies sections)
— it's a benchmark report that happens to share a filename prefix with the real
GUP-096 story. Rename it (e.g. to `grid-performance-benchmarking-report.md`) for
clarity, but it does not need a new GUP ID and is out of scope for the guard
script described in GUP-383 (the guard only inspects files with a `# GUP-<N>:`
first-line header).

³ Currently aliased in INDEX.md as `GUP-285B` — a one-off, non-standard
workaround. Resolving this via proper renumbering removes the need for the
alias; GUP-285B should not be used as a precedent for future collisions (that is
exactly what GUP-383 exists to prevent).

⁴ Closed 2026-10-04 as superseded by GUP-379/GUP-364 (see
`GUP-288_Area_Chart_Data_Mark_Rendering.md`). Since it was never implemented, it
has no retrospective or commit history tied to its ID; **deleting** the file and
its INDEX.md entry (replacing it with a one-line "superseded, see
GUP-379/GUP-364" mention under the GUP-379 entry) is simpler than renumbering it
and loses nothing. Renumbering it (allocating a new ID at implementation time,
per the inventory note above) remains a valid fallback if the maintainer prefers
to keep a historical record of every planned story.

## Acceptance Criteria

### AC1: All story IDs are unique

- [ ] `ls docs/planning/stories | grep -oE '^GUP-[0-9]+_' | sort | uniq -d`
      produces no output (except intentionally non-story files explicitly
      excluded, e.g. `GUP-096_performance_report.md` after it is renamed to not
      start with a GUP ID).
- [ ] `grep -oE '^- \[GUP-[0-9A-Z]+\]' docs/planning/stories/INDEX.md | sort |     uniq -d`
      produces no output.
- [ ] The `GUP-285B` alias no longer appears anywhere in the repository.

### AC2: Renamed files are internally consistent

- [ ] Each renamed file's `#` heading and any self-referential text (e.g. "As
      part of GUP-NNN...") use the new ID.
- [ ] Each renamed file is renamed on disk to match its new ID (filename prefix
      `GUP-<new-id>_`).
- [ ] `docs/planning/stories/GUP-065_Procedural_Macro_Performance_Optimization.md`
      (renamed) gains a proper INDEX.md entry, since it previously had none.

### AC3: Cross-references are updated

- [ ] Every `Deps:` / "Prerequisite Stories" / "Enables Stories" line in
      INDEX.md and in story files that referenced an old ID now references the
      new ID.
- [ ] Every "Follow-up Stories" section in a retrospective that referenced an
      old ID is updated, OR — where the retrospective is historical prose
      describing what happened at the time — an inline note is added clarifying
      the renumbering (retrospectives should not be silently rewritten to imply
      the new ID existed at the time; add a parenthetical, e.g. "GUP-285
      (renumbered to GUP-NNN during GUP-374 cleanup)" using the ID actually
      allocated at implementation time).
- [ ] Git commit messages that reference an old ID are **not** rewritten (git
      history is immutable). Where a renamed story's own document would benefit
      from disambiguation against historical commit messages, add an "Formerly
      referenced as" line near the top of the renamed file, e.g.:

  ```markdown
  **Formerly numbered**: GUP-285 (renumbered during GUP-374 duplicate-ID
  cleanup; see commit history prior to this rename for references under the old
  number)
  ```

### AC4: No broken links

- [ ] Every `[GUP-NNN](...)` markdown link in INDEX.md resolves to an existing
      file.
- [ ] `mask all-check` (which includes markdown lint) passes.

## Technical Tasks

- [ ] Re-verify the duplicate inventory above is still accurate and re-run
      `ls docs/planning/stories | grep -oP 'GUP-\d+' | sort -t- -k2 -n | tail -1`
      to find the current max ID before allocating any new numbers (the max has
      advanced past GUP-382 since this story was written, and will keep moving
      as other stories are created).
- [ ] Decide the disposition of GUP-288 (Area Chart Data Mark Rendering):
      delete, or renumber. Record the decision in this story before starting.
- [ ] For each row marked with a new ID: rename the file, update its `#`
      heading, update its own cross-references to other stories if any shifted.
- [ ] Add a "Formerly numbered: GUP-NNN" line to each renamed file (per AC3).
- [ ] Add the missing INDEX.md entry for the renamed
      `GUP-065_Procedural_Macro_Performance_Optimization.md`.
- [ ] Rename `GUP-096_performance_report.md` to a non-GUP-prefixed filename and
      fix any links to it (check `GUP-096_Grid_Performance_Benchmarking.md` and
      any benchmark scripts under `scripts/` for references).
- [ ] Remove the `GUP-285B` alias from INDEX.md once
      `GUP-285_Fix_WASM_Integration_Test_Compilation.md` is renumbered; update
      the link to point at the new filename/ID.
- [ ] Grep the entire `docs/planning/stories/` tree and `src/` doc-comments for
      every old ID (`grep -rn "GUP-013\b"` etc.) and update non-INDEX
      cross-references (other stories' "Dependencies" / "Enables" sections,
      retrospective "Follow-up Stories" lists).
- [ ] Update INDEX.md: move each renamed entry's bullet to reflect its new ID
      and filename; keep it under the same initiative heading as before, since
      the initiative did not change.
- [ ] Run `ls docs/planning/stories | grep -oE '^GUP-[0-9]+_' | sort | uniq -d`
      and the INDEX.md duplicate-ID grep to confirm zero output.
- [ ] Run the guard script from GUP-383 (once available) against the result as a
      final check.

## Dependencies

### Prerequisite Stories

None — this is a documentation-only cleanup with no code dependencies.

### Enables Stories

- GUP-383 📋 (Duplicate Story ID Guard) — the guard's CI check will pass cleanly
  once this renumbering lands; until then it will correctly fail, which is
  expected and documents the remaining cleanup work.

## Testing Strategy

- **Scripted verification**: run the two duplicate-detection commands above
  (filename-prefix and INDEX.md bracket) and confirm empty output.
- **Link check**: confirm every `[GUP-NNN](FILE.md)` link in INDEX.md resolves
  to a file that exists (a simple script, or reuse of the pattern in
  `scripts/check_gallery_links.sh`, can do this).
- **Cross-reference grep**: `grep -rn "GUP-<old-id>" docs/ src/` for each
  renumbered ID should return zero hits outside of retrospective prose and the
  new "Formerly numbered" disambiguation lines.
- **Manual spot-check**: open 3-4 renamed files and confirm their Dependencies
  sections, and the files that depend on them, agree on the new ID.

## Success Metrics

- [ ] Zero duplicate story IDs across `docs/planning/stories/`.
- [ ] Zero broken links in INDEX.md.
- [ ] The `GUP-285B` ad-hoc alias is gone.
- [ ] `mask all-check` passes.

## Risk Assessment

- **Medium**: This is a large, mechanical change across ~41 files plus INDEX.md
  and any cross-references in other story files. The main risk is missing a
  cross-reference, especially in retrospective "Follow-up Stories" prose that
  mentions an old ID in free text rather than a structured `Deps:` line.
  _Mitigation_: grep exhaustively for each old ID (not just in INDEX.md) before
  considering a given rename complete; do the renumbering in small batches (e.g.
  one initiative's worth of groups at a time) and run the duplicate check after
  each batch.
- **Low**: Renumbering a `✅ Complete` story could be confused with reopening
  it. _Mitigation_: the "Formerly numbered" line and unchanged Status field make
  clear the story's completion status is untouched — only its ID moved.
- **Low**: Deciding which of two completed stories "keeps" an ID is somewhat
  arbitrary when both are equally entangled in history. _Mitigation_: the
  documented tie-break rule (first in INDEX.md order) is applied consistently
  and is easy to re-derive if challenged later.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1` (no source code changes
      expected, but run for safety since doc-comments are grepped for references
      too).
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document.
