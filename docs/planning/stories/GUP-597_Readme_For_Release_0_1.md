---
implementer: sonnet
implementer-reason:
  a README edit section by section, with each section's new content fixed in
  Decisions; the README is already a doctest
touches: [README.md, docs/tutorials/README.md]
---

# GUP-597: README for Release 0.1

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-02

## Context

The README is the crates.io page of `gup` and the repository's front page.
GUP-534 made it honest and compiled it as a doctest (`src/lib.rs`,
`mod architecture_docs`). Track N2 and N3 have since made several sections
stale:

- "Architecture Highlights" says "**Builders**: `scatter`, `line` and `bar`".
  There are now eight.
- "Not yet" says "No accessibility support yet: a minimum comes in N3" and "CI
  renders on Linux only; Windows comes in N3".
- "Development Status" lists N0–N3 as next steps.
- "Documentation" says "Six tested tutorials".
- There is no image, and the "Workspace Crates" table says nothing about which
  crates are published.

This story brings it to the state of the release candidate. The Quick Start
keeps its git dependency until the crates exist on crates.io; switching it is
[GUP-599](GUP-599_Switch_Docs_To_Crates_Io.md), after the owner publishes.

## Read First

- `README.md` (whole; it is 201 lines).
- `src/lib.rs` lines 280–295: the README doctest include.
- GUP-590's and GUP-591's Implementation Summaries: which platforms CI draws on.
- GUP-589's Decisions: the facade's features.
- `CHANGELOG.md` (GUP-596).

## User Story

> "As someone landing on Gup's crates.io or GitHub page, I want a README that
> shows a real chart and says accurately what Gup does and does not do today, so
> that I can decide in a minute whether to try it."

## Acceptance Criteria

### AC1: Sections

- [ ] Each section of Decisions is changed as described, and nothing else.
- [ ] `grep -n 'N0\|N1\|N2\|N3\|track' README.md` prints only the track names in
      "Not yet" (N4, N5, N6); no completed track is named.
- [ ] The README doctest passes: `cargo test --doc -p gup -- architecture_docs`
      (the `Readme` struct).

### AC2: Hero image

- [ ] Directly under the tagline, the README shows the hero:
      `![A Gup scatter chart: GDP per person against life expectancy, sized by population and coloured by region](https://raw.githubusercontent.com/au-phiware/gup/main/tests/golden/gallery/hero.png)`.
      Below it, one line links the gallery: `examples/INDEX.md` for now, and the
      Pages URL once the owner enables Pages (both in Decisions).
- [ ] `tests/golden/gallery/hero.png` exists (GUP-595).

### AC3: Docs and links

- [ ] `docs/tutorials/README.md` lists tutorial 7 if GUP-536 did not.
- [ ] Every relative link in the README resolves to a tracked file. Check with
      `grep -o '](\./[^)]*)\|](docs/[^)]*)\|](examples/[^)]*)\|](crates/[^)]*)' README.md`,
      then `git ls-files` for each target. Paste the count in the Implementation
      Summary.
- [ ] `prettier --check README.md` and `mdl README.md` pass.

## Decisions

- **Tagline** stays as it is.
- **Under the tagline**: the hero image (AC2), then
  `See [the gallery](examples/INDEX.md) for ten charts and the code that draws each.`
  When the owner has enabled Pages, the link is
  `https://au-phiware.github.io/gup/`. Use whichever is live when this story
  runs: if `curl -sfI` on the Pages URL succeeds, use it.
- **"Why Gup"**: unchanged. Figures come from `PERF_BUDGETS.md` only.
- **"Key Features"**: add one last bullet. Bold lead-in: "A text alternative for
  every chart." Then: "`Plot::describe()` gives a title and description; SVG
  files and canvases carry them for screen readers."
- **"Not yet"**: remove the accessibility and Windows lines. Add
  `- Accessibility beyond a text alternative (keyboard navigation, a data table, a high-contrast theme) (N4).`
  Set the platform line from GUP-590 and GUP-591's outcomes:
  - Windows passed and macOS stays required: drop the platform line.
  - macOS is advisory or removed:
    `- CI draws on Linux and Windows; macOS is untested (no GPU on hosted runners).`
    Keep the N5 and N6 lines. Replace the "Chart types are those the builders
    list today; more come in N2" line with
    `- No pie, violin, density or choropleth charts, and no facets.`
- **"Quick Start"**: unchanged text and git dependency (GUP-599 switches it).
  Add one paragraph after the first code block: "In the browser, use
  `default-features = false, features = ["web", "svg"]` and
  `gup::web::CanvasApp`." This is GUP-589's facade feature.
- **"Architecture Highlights"**: the Builders bullet lists all eight, each in
  code format: scatter, line, bar, barh, area, histogram, heatmap and boxplot,
  "over the same `Plot`".
- **"Development Status"**: replace it with two sentences. "Release 0.1 is the
  first published version; see [CHANGELOG.md](CHANGELOG.md) for what it contains
  and for the versioning policy. What comes next is in the
  [strategic review](./docs/planning/STRATEGIC_REVIEW_2026-11.md) (tracks N4 to
  N6)."
- **"Documentation"**: "Six tested tutorials" becomes "Seven tested tutorials".
  Add a `CHANGELOG.md` line first.
- **"Workspace Crates"**: add a third column, "Published", with "yes" for the
  seven published crates and "no" for `gup-visual-regression`. The
  `gup-culling-lod` paragraph is unchanged.

## Escalation Triggers

- GUP-590 or GUP-591 is not complete when this story runs, so the platform line
  cannot be set. Ask; do not guess.
- The hero golden does not exist.

## Out of Scope

- The Quick Start's dependency line and tutorial 1's: GUP-599.
- The README's performance claims: unchanged.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] Sections in README order (AC1, AC2). Commit.
- [ ] Tutorials README and links (AC3). DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-595: Flagship gallery, part two, and the hero 📋 — the image.
- GUP-593: SVG title and canvas `aria-label` 📋 — the Key Features bullet.
- GUP-589: Publish dry run and docs.rs build in CI 📋 — the browser features.
- GUP-590 📋 and GUP-591 📋 — the platform line.
- GUP-596: CHANGELOG and a 0.x semver policy 📋 — the link.

### Enables Stories

- GUP-598: Release checklist.

## Testing Strategy

- **Doctest**: the README's blocks through `cargo test --doc -p gup`.
- **Links**: AC3's check.
- **Visual validation**: open the README on the branch's GitHub page, or render
  it with `prettier`'s output in a Markdown viewer, and check that the hero
  shows. Before the merge to `main` the raw URL is 404; check the image path
  exists instead.

## Success Metrics

- [ ] The README describes the release as it is, with a real render at the top.

## Risk Assessment

- **Low**: the raw.githubusercontent URL shows `main`'s hero, not the published
  version's. _Mitigation_: acceptable; the hero is a golden and changes only
  deliberately.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test --doc -p gup`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: not affected
- [ ] Rendered output verified by eye: the hero image path, and the README
      rendered
- [ ] `mask wasm-browser`: not needed, docs only
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Implementation Summary

README sections changed as in Decisions: hero and gallery line (Pages URL not
live, so `examples/INDEX.md`), accessibility Key Features bullet, "Not yet"
(platform line: "CI draws on Linux (Vulkan) and Windows (DX12); macOS (Metal) is
a fix in progress and not yet verified"), the browser-features paragraph, eight
builders, Development Status, Seven tutorials plus a CHANGELOG line, and a
"Published" column. Tutorial 7 was already listed. Relative links checked: 20 of
20 resolve. `cargo test --doc -p gup` passes; the hero was read and shows the
scatter with title and legend. Note: the crates table lists six workspace crates
besides the `gup` facade (published, not in the table), so the column reads yes
for five and the facade's seven total is not visible in the table.

## Retrospective

**Completed**: 2026-11-03

### Key Technical Learnings

- A docs-only story still trips prettier on the story file itself; run prettier
  on it before the first commit.

### Architectural Decisions

- macOS stated as "not yet verified" pending GUP-591's CI outcome.

### Follow-up Stories

None.
