---
implementer: sonnet
implementer-reason:
  a large mechanical move with exact lists and counting checks; Sonnet is less
  likely than Haiku to mangle 1,600 lines
touches:
  [
    docs/planning/stories/INDEX.md,
    docs/planning/stories/,
    docs/planning/INDEX.md,
    .github/agents/story-worker.md,
    .github/agents/story-scribe.md,
  ]
---

# GUP-503: Archive the Story Index History

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: ✅ Complete
(2026-10-24) **Created**: 2026-10-24

## Context

`docs/planning/stories/INDEX.md` is 1,693 lines and 105 KB: 426 completed, 31
parked and 36 open entries over 30 sections, almost all history. Every scribe
and worker greps it, and the agent docs warn them not to read it whole. The
review recommends archiving the completed and parked legacy stories so agents
read a small index (STRATEGIC_REVIEW_2026-11 §2, recommended change 8), and
closing the legacy open stories that target code deleted in GUP-486 and GUP-488
(§3, "Legacy open stories ... Close: obsolete").

This story moves the history verbatim to `INDEX_ARCHIVE.md`, leaves `INDEX.md`
with the current initiative and an "Open Backlog" of the nine stories still
worth doing, and closes 27 stories with a new status, 🚫 Closed.

Every story edits its own status line in `INDEX.md`, so this story runs with no
other story in flight.

## Read First

- `docs/planning/stories/INDEX.md` lines 1–10 and the "Strategic Review 2026-11"
  section (find with `grep -n '^## ' docs/planning/stories/INDEX.md`).
- The entries named in Decisions, found with `grep -n '<file name>'`; do not
  read the file whole.
- `.github/agents/story-worker.md` lines 22–40 (Phase 0) and 254–265 (Phase 4);
  `.github/agents/story-scribe.md` lines 28–55 (Step 1) and 287–315 (Step 4).

## User Story

> "As a scribe or worker, I want the story index to hold only current work, so
> that reading it costs a few thousand tokens instead of thirty thousand."

## Acceptance Criteria

### AC1: the split

- [x] Before any edit, record
      `grep -c '^- \[GUP-' docs/planning/stories/INDEX.md` (N₀) in the
      Implementation Summary.
- [x] `docs/planning/stories/INDEX_ARCHIVE.md` holds every section of the old
      `INDEX.md` except "Strategic Review 2026-11", in the same order, with the
      same headings and intro paragraphs, minus the nine Open Backlog entries.
      Its title is `# Stories Archive` and its first paragraph says it is the
      history before 2026-11, to be grepped, not read whole, and that new
      entries never go here.
- [x] `docs/planning/stories/INDEX.md` holds, in order: `# Stories`; the status
      legend with `🚫 Closed` added; one line linking `INDEX_ARCHIVE.md`; the
      "Strategic Review 2026-11" section unchanged; and `## Open Backlog` with
      the nine entries in Decisions, moved verbatim and each followed by its
      track note.
- [x] `grep -c '^- \[GUP-' INDEX.md INDEX_ARCHIVE.md` sums to N₀, and
      `wc -l < docs/planning/stories/INDEX.md` is at most 200.
- [x] Every entry link in both files resolves:

      ```bash
      cd docs/planning/stories && cat INDEX.md INDEX_ARCHIVE.md |
        grep -oP '^- \[GUP-[^]]+\]\(\K[^)]+' |
        while read -r l; do [ -e "$l" ] || echo "BROKEN $l"; done
      ```

      prints nothing.

### AC2: 27 stories closed

- [x] Each entry in the closure list (Decisions) has its status emoji replaced
      by `🚫` and `Closed 2026-MM-DD (<reason>).` inserted after its em dash,
      using the reason given for its group and today's `date -I`.
- [x] Each of those story files' `**Status**:` value becomes
      `🚫 Closed (YYYY-MM-DD): <reason>`. No other text in them changes.
- [x] `grep -c '^- \[GUP-[^]]*\]([^)]*) \(📋\|💡\)' INDEX_ARCHIVE.md` prints 0
      (every open entry left in the archive was closed or moved).

### AC3: agent docs and links

- [x] `.github/agents/story-worker.md` Phase 0 item 3: `INDEX.md` is small and
      may be read; `INDEX_ARCHIVE.md` and RFC-001 are never read whole (grep
      them). Phase 4's INDEX step is unchanged.
- [x] `.github/agents/story-scribe.md` Step 1 item 3: read `INDEX.md` whole;
      grep `INDEX_ARCHIVE.md` for history and existing coverage. Step 1 item 5
      names both files. Step 4: new entries go under the active initiative's
      section in `INDEX.md` (or "Open Backlog" if standalone), never in the
      archive.
- [x] `docs/planning/INDEX.md` lists `stories/INDEX_ARCHIVE.md` next to
      `stories/INDEX.md`.
- [x] `mask all-check` passes.

## Decisions

- **Status 🚫 Closed**: not done and not coming back (obsolete or superseded),
  distinct from ⏸ Parked (may return). Legend line:
  `> Status: ✅ Complete · 🚧 In Progress · 📋 Planned · 💡 New · ⏸ Parked · 🚫 Closed`.
- **Open Backlog** (move each entry verbatim from its current section; append
  the note as a new sentence at the end of the entry):
  - GUP-392 Correct Visual Defaults — "Track: N0, verify against `gup-core`
    (Inter, Okabe-Ito, step-derived tick precision, muted theme) and close."
  - GUP-397 Dogfood Checks on Shared Visual Assertions — "Track: N0."
  - GUP-413 Per-Checkout Clippy Artifacts — "No track; keep."
  - GUP-428 Prune Superseded Shared Units — "Parked until the next toolchain or
    `Cargo.lock` bump, or a disk alert."
  - GUP-405 gup-text OpenType Shaping — "Track: N2 (text quality)."
  - GUP-416 gup-text Font Fallback — "Track: N2 (text quality)."
  - GUP-439 Line Translucent Joins — "Track: N2, with line curves and area
    outlines (same polyline geometry)."
  - GUP-492 Windowed Size-Order Draws — "Track: N5 (GPU sort or index buffer)."
  - GUP-376 GupApp Event Callbacks — "Track: N4 (keyboard)."
- **Closure list** (by story file; reasons by group):
  - "obsolete: targets code deleted in GUP-486/488":
    `GUP-285_High_Resolution_GeoJSON_Streaming.md`,
    `GUP-286_Spherical_Polygon_Simplification.md`,
    `GUP-402_Accessibility_Nested_Runtime_Panic.md`,
    `GUP-350_Live_Profiling_WebSocket_Server.md`,
    `GUP-305_Release_Mode_Performance_Validation.md`,
    `GUP-358_GPU_Binning_Performance_Benchmarks.md`,
    `GUP-359_Streaming_GPU_Binning.md`, `GUP-308_Fixed_Capacity_Cell_Layout.md`,
    `GUP-309_Viewport_Aware_Eviction.md`,
    `GUP-375_COPY_SRC_Surface_Fallback.md`,
    `GUP-386_Qualified_Type_Paths_In_Shader_Macros.md`,
    `GUP-285_Fix_WASM_Integration_Test_Compilation.md` (the GUP-285B entry).
  - "obsolete: targets code deleted in GUP-486/488; may seed N5":
    `GUP-312_Full_GPU_Quadtree_Construction.md`,
    `GUP-313_Interactive_Treemap_Drill_Down.md`,
    `GUP-314_Shared_Device_Layout_Engine.md`,
    `GUP-315_Graph_Node_Label_Rendering.md`,
    `GUP-370_GPU_Timestamp_Query_Profiling.md`,
    `GUP-371_Squarified_GPU_Treemap_Hybrid.md`,
    `GUP-372_TreemapResult_Direct_Bind.md`,
    `GUP-373_Billboard_Text_Labels_3D_Axes.md`.
  - "obsolete: the mobile and Tauri integrations were deleted (GUP-497)":
    `GUP-274_iOS_Real_Device_Testing.md`,
    `GUP-354_Android_Real_Device_Testing.md`,
    `GUP-285_Tauri_Streaming_Updates.md`.
  - "obsolete: the tutorials are rewritten in track N1":
    `GUP-378_Tutorial_Examples_2_and_3.md`.
  - "superseded: all 8 remaining `#[ignore]`s carry reasons (2026-10-24); the
    reason lint is GUP-501": `GUP-404_Triage_Every_Ignored_Test.md`.
  - "superseded by GUP-504 (a guard with a frozen legacy baseline; legacy
    duplicates are not renumbered)": `GUP-374_Duplicate_Story_ID_Cleanup.md`,
    `GUP-383_Duplicate_Story_ID_Guard.md`.
- **Verbatim**: apart from the 27 closures and the nine moves, the archive's
  text is byte-identical to the old sections (check with `diff` against
  `git show main:docs/planning/stories/INDEX.md` after removing the moved
  lines).
- **The file name** `INDEX_ARCHIVE.md` sits next to `INDEX.md`, so every
  relative link keeps working.

## Escalation Triggers

- Another story's branch has changed `INDEX.md` since this story started
  (`git log main -- docs/planning/stories/INDEX.md` shows a new commit): stop
  and report; the orchestrator rebases.
- An entry in the closure list is ✅ or ⏸ rather than 📋 or 💡, or a story file
  in it does not exist.
- After the split, an open (📋 or 💡) entry remains in the archive that neither
  list names.

## Out of Scope

- Renumbering legacy duplicate IDs (GUP-374 is closed by this story).
- The story ID guard script: GUP-504.
- Splitting RFC-001 into design and findings (review change 8, second half): not
  planned in N0.
- Editing completed stories' files.

## Technical Tasks

- [x] Start commit within 30 minutes; record N₀.
- [x] Create `INDEX_ARCHIVE.md` from the old sections; trim `INDEX.md`; move the
      nine entries; commit.
- [x] Apply the 27 closures to the archive entries and the story files; commit.
- [x] Edit the agent docs and `docs/planning/INDEX.md`.
- [x] Run the AC checks and `mask all-check`.

## Dependencies

### Prerequisite Stories

- GUP-496: Checkpoint Script 📋 — soft, for the DoD's checkpoint line.

### Enables Stories

- GUP-504: Story ID Guard — checks both index files.

## Testing Strategy

- **Counts**: entry total N₀ preserved; `INDEX.md` ≤ 200 lines; 0 open entries
  in the archive.
- **Links**: the AC1 loop.
- **Format**: `mask all-check` (prettier and mdl on both files).

## Success Metrics

- [x] `wc -c docs/planning/stories/INDEX.md` under 20 KB (105 KB before).
- [x] 27 stories closed; 9 in Open Backlog; N0 section unchanged.

## Risk Assessment

- **Medium**: a hand move drops or duplicates an entry. _Mitigation_: the count
  and link checks, and the verbatim `diff`.
- **Low**: prettier rewraps moved entries. _Mitigation_: rewrapping is allowed;
  the count and link checks still hold.

## Definition of Done

- [x] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-check` passes
- [x] Lint and format clean: `mask all-fix`
- [x] `mask wasm-browser`: not needed (planning docs only); say so
- [x] WASM size: "unchanged: no library code change"
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- N₀ (`grep -c '^- \[GUP-' INDEX.md` before any edit) was **505**, not the 493
  the story's Context implies: GUP-496 to GUP-502 completed and GUP-511 and
  GUP-517 were added since it was written. After the split `INDEX.md` has 20
  entries (the 11 N0 entries, unchanged, plus the nine Open Backlog) and
  `INDEX_ARCHIVE.md` 485: 505 in total. Because GUP-511 and GUP-517 are N0
  entries they stay in the N0 section, so the story's "N0 section unchanged"
  holds.
- `INDEX.md` is 132 lines, about 8 KB (from 1,772 lines, 109 KB). No entry link
  is broken. The archive has 0 open (📋/💡) entries; 27 entries carry 🚫.
- The split was done by a script (`/tmp`, not committed) that parses entries,
  asserts all 27 closures and nine moves were found, and reports leftover open
  entries (none). Prettier re-wrapped some archive lines and the legend; a `>`
  line joins the archive's intro and legend into one blockquote (MD028).
- The 27 story files' `**Status**:` values became
  `🚫 Closed (2026-10-24): <reason>`; nothing else in them changed apart from
  prettier's re-wrap of that line.
- Agent docs: `story-worker.md` Phase 0 item 3 and the Phase "Check before
  writing" item; `story-scribe.md` Step 1 items 3 and 5, Step 4;
  `docs/planning/INDEX.md` lists the archive.
- `mask wasm-browser`: not needed (planning docs only). WASM size: unchanged, no
  library code change.

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Parsing a hand-written index

- **Challenge**: Entries are multi-line bullets with irregular shapes: an em
  dash at a line end (GUP-402), a link wrapped onto the next line (GUP-105), and
  prettier re-wraps after the move, so a status or "Closed" marker can sit
  across lines.
- **Solution**: Treat an entry as its first line plus following indented lines;
  assert every closure and move was found; verify with counts (`🚫` at entry
  start: 27) rather than by `grep -c 'Closed 2026'` on lines, which undercounts
  wrapped entries.
- **Pattern**: Script the move with assertions, then check by entry count, not
  by line count.

### Architectural Decisions

#### Archive keeps the 2026-10 section

- **Decision**: "Strategic Review 2026-10" went to the archive with the rest.
- **Reasoning**: The story says every section except 2026-11 moves.
- **Trade-off**: INDEX.md no longer shows the 2026-10 initiative's stories.
- **Future**: GUP-504's guard reads both files.

### Development Workflow Insights

- Worktree-isolated agents cannot run `git` through `with_cargo_env.sh` or with
  a `cd`; a plain `git commit` runs the hook, which sets its own environment.
- Counts in the story had drifted (493 vs 505) because other stories landed
  first; the AC checks hold by the sum, not by the literal number.

### Follow-up Stories

None.
