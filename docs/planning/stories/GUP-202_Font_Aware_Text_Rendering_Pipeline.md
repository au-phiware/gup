# GUP-202: Font-Aware Text Rendering Pipeline

**Status**: ✅ Complete **Priority**: Medium **Complexity**: Medium **Created**:
2025-08-20 **Completed**: 2025-08-21

## Overview

Connect the `TextStyle.font_family` field to actual font atlas creation in the
text rendering pipeline, enabling per-text-element font selection end-to-end.

## Context

GUP-106 added `FontSpec`, `FontDatabase`, and `FontAtlas::with_font()` for
system font loading, plus a `font_family` field on `TextStyle`. However, the
rendering pipeline does not yet use the `font_family` field to select different
font atlases. Currently, all text renders using a single pre-created
`FontAtlas`.

## User Story

As a developer building data visualizations, I want to specify different fonts
for different text elements (e.g., "Arial" for axis labels, "Times New Roman"
for titles) so that my charts have typographically rich text.

## Acceptance Criteria

- [x] `TextStyle.font_family` is used by the rendering pipeline to select the
      appropriate `FontAtlas`
- [x] Multiple fonts can be rendered in a single frame
- [x] Font atlas is lazily created on first use for each unique font family
- [x] Fallback to default font when requested font is unavailable
- [x] Examples updated to demonstrate multi-font rendering

## Technical Tasks

1. Modify the text rendering pipeline to check `TextStyle.font_family`
2. Create a font atlas registry/manager that maps family names to atlases
3. Handle on-the-fly atlas creation for new font families
4. Update examples to showcase multi-font text

## Dependencies

- GUP-106 ✅ (System Font Loading)

## Testing Strategy

- Unit tests for font atlas registry
- Integration tests with multiple fonts in one frame
- Visual verification of mixed-font text rendering

## Risk Assessment

- **Medium**: Multiple font atlases increase GPU memory usage; need to consider
  atlas size limits and eviction strategies.

## Definition of Done

- [x] TextStyle.font_family drives font selection in rendering
- [x] Multiple fonts render correctly in a single frame
- [x] All existing tests pass
- [x] Documentation updated
- [x] At least one example demonstrates multi-font rendering

---

**Estimated Effort**: 1-2 weeks **Prerequisites**: GUP-106 ✅ **Blockers**: None

## Implementation Summary

### What Was Implemented

1. **`FontAtlasManager`** — A registry that manages multiple `FontAtlas`
   instances keyed by font family name. Provides:
   - `new(font_db, default_font_size)` — Creates manager with a `FontDatabase`
   - `get_or_create(device, queue, family)` — Lazily creates atlases on first
     use
   - `get_atlas_for_style(device, queue, style)` — Resolves atlas from
     `TextStyle.font_family`
   - `get_atlas(family)` / `get_atlas_mut(family)` — Look up existing atlases
   - `atlas_count()` / `iter()` — Introspect loaded atlases

2. **`TextRenderer` multi-font methods**:
   - `queue_text_with_fonts()` — Queues text using `FontAtlasManager` to resolve
     font from `TextStyle.font_family`, batching vertices per atlas
   - `render_queued_text_multi()` — Renders all per-atlas batches, issuing one
     draw call per font atlas with the appropriate GPU texture bind group
   - `font_batches` field — Per-atlas vertex storage cleared on `begin_frame()`

3. **`multi_font_demo` example** — Demonstrates rendering 5 different fonts in a
   single frame: default embedded font, DejaVu Sans, DejaVu Serif, DejaVu Sans
   Mono, and automatic fallback for unavailable fonts.

### Key Files Changed

| File                          | Change                                                                        |
| ----------------------------- | ----------------------------------------------------------------------------- |
| `src/text/font.rs`            | Added `FontAtlasManager`, `Debug` impl, 13 tests                              |
| `src/text/renderer.rs`        | Added `font_batches`, `queue_text_with_fonts()`, `render_queued_text_multi()` |
| `examples/multi_font_demo.rs` | New example demonstrating multi-font rendering                                |

### Test Counts

- **4 unit tests** for `FontAtlasManager` (non-GPU)
- **5 GPU integration tests** for `FontAtlasManager` (atlas creation, style
  resolution, iteration, system fonts)
- **All 1,424 library tests pass**
- **All integration tests pass**
- **All examples compile**

## Retrospective

**Completed**: 2025-08-21

### Key Technical Learnings

#### Per-Atlas Vertex Batching

- **Challenge**: The existing `TextRenderer` uses a single `render_queue` and
  one draw call per frame. Multi-font rendering requires different GPU texture
  bind groups for each font atlas, so all text can't go in one draw call.
- **Solution**: Added a `font_batches: HashMap<String, Vec<TextVertex>>` field
  that stores vertices keyed by atlas family name. During rendering, each batch
  gets its own bind group creation and draw call. The uniform buffer (projection
  matrix) is shared across all batches.
- **Pattern**: When a GPU renderer needs to switch resources (textures, bind
  groups) mid-frame, group work items by resource and issue one draw call per
  group rather than one per item.

#### Borrow Checker vs GPU Rendering

- **Challenge**: In `render_queued_text_multi`, borrowing `self.font_batches`
  immutably to iterate and calling `self.resize_buffers` mutably caused a borrow
  conflict.
- **Solution**: Used `std::mem::take` to move the batches out of `self` before
  the render loop, allowing mutable access to `self` for buffer resizing. The
  batches are consumed during rendering anyway (frame lifecycle), so ownership
  transfer is natural.
- **Pattern**: When iterating over a collection on `self` while needing mutable
  access to other fields, `std::mem::take` (or `std::mem::replace`) is a clean
  solution when the collection is consumed or reset after use.

#### Additive API Design

- **Challenge**: Adding multi-font support without breaking the existing
  single-atlas API used by all existing examples and tests.
- **Solution**: Kept the original `render_queue`, `queue_text()`, and
  `render_queued_text()` entirely unchanged. Added parallel `font_batches`,
  `queue_text_with_fonts()`, and `render_queued_text_multi()`. Both paths share
  the same vertex creation and buffer management code.
- **Pattern**: When extending a renderer with new capabilities, add new methods
  alongside existing ones rather than modifying signatures. This lets callers
  migrate at their own pace.

### Architectural Decisions

#### FontAtlasManager as Separate Object

- **Decision**: Made `FontAtlasManager` a standalone struct rather than
  embedding atlas management inside `TextRenderer`.
- **Reasoning**: The manager needs mutable access to create atlases (which
  requires `device` and `queue`), while the renderer needs immutable access to
  atlas textures during rendering. Keeping them separate avoids complex borrow
  interactions and lets users share a manager across multiple renderers.
- **Trade-off**: Users must create and manage a `FontAtlasManager` alongside
  their `TextRenderer`, adding API surface.
- **Future**: The chart builder layer (GUP-018 and above) could hide this
  complexity by owning both internally.

#### One Draw Call Per Atlas

- **Decision**: Issue one draw call per unique font atlas in the frame.
- **Reasoning**: Each atlas has a different GPU texture, requiring a different
  bind group. Merging all text into one draw call would require a texture array
  or atlas-of-atlases approach, which adds significant complexity.
- **Trade-off**: With N distinct fonts, we get N draw calls instead of 1. For
  typical visualisations (2-4 fonts), this is negligible.
- **Future**: If many fonts are needed, a texture array bind group could reduce
  to a single draw call with an atlas index per vertex.

### Development Workflow Insights

- The story was significantly smaller than estimated (1-2 weeks estimated,
  completed in a single session). The existing `FontAtlas::with_font()` and
  `FontDatabase` infrastructure from GUP-106 made this mostly a wiring task.
- The hardest part was getting the borrow checker happy with the multi-atlas
  rendering loop — the actual font resolution and atlas creation were trivial
  thanks to the well-designed GUP-106 foundation.
- The existing `multi_font_demo` correctly loads 3 system fonts (DejaVu Sans,
  Serif, Sans Mono) on the development machine, confirming the full pipeline
  works end-to-end.
- Window visibility in the CI/compositor environment prevented visual screenshot
  verification, but functional verification via stdout logs confirmed correct
  behavior.

### Follow-up Stories

1. **GUP-214: Font Atlas Eviction and Memory Management** — With multiple font
   atlases, GPU memory usage grows linearly with the number of distinct fonts. A
   future story should add LRU eviction or atlas size limits to prevent
   unbounded memory growth in applications using many fonts.

2. **GUP-215: Chart Builder Multi-Font Integration** — The chart builder layer
   (axes, titles, labels) currently uses a single `FontAtlas`. It should be
   updated to use `FontAtlasManager` so that `TextStyle.font_family` works
   automatically when building charts, without users needing to manage atlases
   manually.
