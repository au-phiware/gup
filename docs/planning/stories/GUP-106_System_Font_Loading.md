# GUP-106: System Font Loading

**Status**: 🚧 In Progress  
**Priority**: Medium  
**Complexity**: Medium  
**Created**: 2025-08-19

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

- [ ] Remove unused `_font_name` parameter from current API
- [ ] Implement system font loading using fontconfig/fontdb
- [ ] Support common font family names (Arial, Times New Roman, etc.)
- [ ] Fall back to embedded font when system font not found
- [ ] Add font weight and style support (Regular, Bold, Italic)

### API Design

- [ ] Clean API that makes capabilities clear
- [ ] Proper error handling for missing fonts
- [ ] Font caching to avoid repeated system queries
- [ ] Cross-platform compatibility (Linux, Windows, macOS, WASM)

### Testing

- [ ] Unit tests for font resolution
- [ ] Integration tests with various system fonts
- [ ] Fallback behavior validation
- [ ] Cross-platform test coverage

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

- [ ] System fonts can be loaded by family name
- [ ] Proper fallback to embedded fonts
- [ ] All existing examples work with new API
- [ ] Cross-platform compatibility verified
- [ ] Documentation updated
- [ ] Breaking changes clearly communicated

---

**Estimated Effort**: 2-3 weeks  
**Prerequisites**: None  
**Blockers**: None
