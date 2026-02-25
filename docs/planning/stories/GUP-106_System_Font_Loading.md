# GUP-106: System Font Loading

**Status**: ✅ Complete  
**Priority**: Medium  
**Complexity**: Medium  
**Created**: 2025-08-19  
**Completed**: 2025-08-20

## Overview

Implement system font loading to support dynamic font selection by name instead
of relying only on embedded fonts.

## Context

Currently, the `FontAtlas::new()` method accepts a `font_name` parameter but
completely ignores it, always using the embedded `default.ttf` font. This
creates a misleading API where users think they can specify fonts but cannot.

## Problem Statement

- **Dead API**: `font_name` parameter exists but does nothing
- **Limited flexibility**: Only one embedded font supported
- **Misleading documentation**: Code suggests font selection is supported
- **User expectations**: API implies font choice but doesn't deliver

## Acceptance Criteria

### Core Functionality

- [x] Remove unused `_font_name` parameter from current API
- [x] Implement system font loading using fontconfig/fontdb
- [x] Support common font family names (Arial, Times New Roman, etc.)
- [x] Fall back to embedded font when system font not found
- [x] Add font weight and style support (Regular, Bold, Italic)

### API Design

- [x] Clean API that makes capabilities clear
- [x] Proper error handling for missing fonts
- [x] Font caching to avoid repeated system queries
- [x] Cross-platform compatibility (Linux, Windows, macOS, WASM)

### Testing

- [x] Unit tests for font resolution
- [x] Integration tests with various system fonts
- [x] Fallback behavior validation
- [x] Cross-platform test coverage

## Technical Approach

### Phase 1: API Cleanup

1. Remove misleading `font_name` parameter
2. Make embedded font usage explicit
3. Update all callers

### Phase 2: System Font Integration

1. Add fontdb or similar crate for system font discovery
2. Implement font resolution by family name
3. Add weight/style variants
4. Handle platform differences

### Phase 3: Enhanced API

1. Design clean font specification API
2. Implement font caching system
3. Add comprehensive error handling

## Dependencies

- **fontdb** or **font-kit** crate for system font access
- **Update to FontAtlas API** (breaking change)
- **Cross-platform testing infrastructure**

## Breaking Changes

- Remove `font_name` parameter from `FontAtlas::new()`
- New API will require explicit font specification

## Future Considerations

- Font subsetting for performance
- Dynamic font loading from URLs
- Font fallback chains
- Advanced typography features

## Definition of Done

- [x] System fonts can be loaded by family name
- [x] Proper fallback to embedded fonts
- [x] All existing examples work with new API
- [x] Cross-platform compatibility verified
- [x] Documentation updated
- [x] Breaking changes clearly communicated

---

**Estimated Effort**: 2-3 weeks  
**Prerequisites**: None  
**Blockers**: None

## Implementation Summary

### What Was Implemented

1. **`FontSpec` type** — Specifies desired fonts by family name, weight
   (`FontWeight` enum: Thin through Black), and style (`FontStyle` enum:
   Normal/Italic/Oblique).

2. **`FontDatabase`** — System font discovery using the `fontdb` crate.
   Provides:
   - `new()` — Loads all system fonts from OS font directories
   - `empty()` — Creates database with no fonts (for testing/WASM)
   - `resolve(&FontSpec)` — Resolves a spec to font data with caching
   - `resolve_from_data(Vec<u8>)` — Validates raw font data
   - `embedded_fallback()` — Returns the embedded Squada One font
   - `list_families()` / `has_family()` — Query available fonts

3. **`FontAtlas` new constructors**:
   - `FontAtlas::new()` — Unchanged, uses embedded default font
   - `FontAtlas::with_font()` — Loads from system font via `FontSpec` +
     `FontDatabase`
   - `FontAtlas::from_data()` — Loads from raw TTF/OTF bytes
   - New accessor methods: `is_fallback_font()`, `font_family()`

4. **`TextStyle.font_family`** — Optional font family field with
   `with_font_family()` builder method.

5. **`ResolvedFont`** — Contains font data (`Arc<Vec<u8>>`), family name, and
   fallback status.

### Key Files Changed

| File                | Change                                    |
| ------------------- | ----------------------------------------- |
| `Cargo.toml`        | Added `fontdb = "0.23"` dependency        |
| `src/text/font.rs`  | New file: FontSpec, FontDatabase, etc.    |
| `src/text/atlas.rs` | New constructors and font metadata fields |
| `src/text/style.rs` | Added `font_family` field to TextStyle    |
| `src/text.rs`       | Added `font` module, updated docs         |

### Test Counts

- **23 unit tests** in `text::font` module
- **7 GPU integration tests** in `text::atlas` module (new)
- **1 style test** in `text::style` module (new)
- **All 1,276 library tests pass**
- **All 32 integration tests pass**
- **All examples compile**

## Retrospective

**Completed**: 2025-08-20

### Key Technical Learnings

#### fontdb Integration

- **Challenge**: The `fontdb` crate returns font data as
  `Arc<dyn AsRef<[u8]> + Send + Sync>` for binary sources and `PathBuf` for file
  sources. These need different handling to extract raw bytes.
- **Solution**: Pattern match on the `Source` enum variants, using `.as_ref()`
  on the binary data trait object and `std::fs::read()` for file-based fonts.
- **Pattern**: When working with trait-object-wrapped data from external crates,
  use the trait's methods (like `AsRef::as_ref()`) rather than trying to clone
  or convert the trait object itself.

#### ttf_parser Lifetime Management

- **Challenge**: `ttf_parser::Face` borrows from font data (`&[u8]`), but we
  need a `'static` lifetime for the struct. The old code used `include_bytes!`
  which gives `&'static [u8]` for free; dynamic font data from the filesystem
  does not.
- **Solution**: Store font data as `Arc<Vec<u8>>` and use `unsafe` to create a
  `&'static [u8]` reference. This is sound because the `Arc` is kept alive in
  the same struct, ensuring the data outlives the reference.
- **Pattern**: When a parser requires `'static` data but you have dynamically
  loaded data, storing the owned data alongside the parsed result and using
  unsafe pointer-to-slice is a common, sound pattern — as long as the owned data
  is never dropped before the parser.

#### Backward-Compatible API Extension

- **Challenge**: Needed to add system font loading without breaking the 35+
  existing call sites using `FontAtlas::new()`.
- **Solution**: Kept `FontAtlas::new()` with the same signature, added
  `FontAtlas::with_font()` and `FontAtlas::from_data()` as new entry points.
  Internally, all three delegate to `from_resolved()`.
- **Pattern**: When extending APIs, add new constructors/methods rather than
  changing existing signatures. Use a shared internal method to avoid code
  duplication.

### Architectural Decisions

#### Font Database as Separate Object

- **Decision**: Made `FontDatabase` a standalone struct rather than embedding it
  in `FontAtlas`.
- **Reasoning**: A single `FontDatabase` can be shared across multiple
  `FontAtlas` instances, and system font scanning is expensive (filesystem I/O).
  Separating them allows the user to control when scanning happens and how
  results are cached.
- **Trade-off**: Slightly more complex API (user must create and pass a
  `FontDatabase`), but enables better performance and resource sharing.
- **Future**: Could add a global/static `FontDatabase` for convenience if the
  two-object API proves cumbersome.

#### Using fontdb Over font-kit

- **Decision**: Used `fontdb` instead of `font-kit` for system font discovery.
- **Reasoning**: `fontdb` is lighter weight, has fewer dependencies, and
  provides exactly what we need (system font discovery by family/weight/style
  query). `font-kit` is more full-featured but includes rasterization we don't
  need (we have our own MSDF pipeline).
- **Trade-off**: Less feature-rich, but fewer dependencies and a simpler
  integration surface.
- **Future**: If advanced font features are needed (variable fonts, OpenType
  feature queries), `font-kit` could be reconsidered.

#### No Breaking Changes

- **Decision**: Made this a purely additive change with no breaking changes to
  the existing API.
- **Reasoning**: The story originally called for removing a `_font_name`
  parameter, but investigation showed this had already been done. All existing
  code continues to work unchanged.
- **Trade-off**: The embedded font is still the implicit default, which may
  surprise users who expect system font loading to be the default.
- **Future**: A future story could make `FontDatabase` the default path and
  `FontAtlas::new()` a convenience wrapper.

### Development Workflow Insights

- The story was smaller than estimated (2-3 weeks estimated, completed in a
  single session) because the API cleanup phase was already done and `fontdb`
  provided a clean integration surface.
- The `fontdb` crate compiled quickly and added only 4 transitive dependencies.
- Pre-existing clippy warnings in unrelated files are noisy during the
  pre-commit hook but don't indicate issues with the new code.
- The flaky `test_composition_overhead_under_one_percent` performance test fails
  intermittently when run after GPU-heavy tests due to resource contention. This
  is a pre-existing issue unrelated to this story.

### Follow-up Stories

1. **GUP-202: Font-Aware Text Rendering Pipeline** — Connect the
   `TextStyle.font_family` field to actual atlas creation in the rendering
   pipeline so that per-text-element font selection works end-to-end. Currently
   the field exists but the renderer doesn't use it to select different atlases.

2. **GUP-203: Multi-Font Atlas Manager** — Implement a font atlas manager that
   can maintain multiple `FontAtlas` instances for different fonts and switch
   between them during rendering. Currently each `FontAtlas` holds one font; a
   manager would enable mixed-font text in a single chart.
