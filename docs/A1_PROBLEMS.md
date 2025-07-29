# Chart Implementation Problems Encountered

## Problem Summary

We attempted to implement a professional population chart for Conway's Game of
Life using the `plotters` crate (v0.3.7) but encountered multiple critical
issues that prevented successful deployment.

## Timeline of Issues

### 1. Plotters Bitmap Backend Crashes

**Problem**: Application crashes during shutdown with "panic in a destructor
during cleanup"

**Root Cause**: Thread-local storage access during destruction in
plotters-bitmap backend

**Symptoms**:

- Crash occurs after chart rendering completes
- Happens during application shutdown/cleanup phase
- More prominent on native builds than web

**Attempted Solutions**:

- Tried plotters-bitmap v0.3.7 (latest)
- Investigated 3-year-old GitHub issues (already closed)
- Discovered new memory alignment bugs in plotters-bitmap 0.3.2+

**Status**: Unsolved - bitmap backend fundamentally unstable

### 2. Cross-Platform Compatibility Issues

**Problem**: File system operations fail on web platform

**Root Cause**: Plotters backends assume filesystem access

**Symptoms**:

- Native: Works but crashes
- Web: "operation not supported on this platform" errors
- File-based temp chart generation fails in browsers

**Attempted Solutions**:

- Researched plotters backend compatibility
- Found SVG backend is cross-platform, bitmap is native-only
- Canvas backend exists for web but different API

**Status**: Mitigated by switching to SVG backend

### 3. Missing Chart Labels (Critical Issue)

**Problem**: Axis labels and tick marks completely absent from rendered charts

**Root Cause**: Unknown - labels not appearing despite configuration

**Symptoms**:

- Chart lines and data points render correctly
- Axis lines visible but no text labels
- No tick mark labels showing data values
- Makes charts unreadable and unprofessional

**Attempted Solutions**:

- Multiple font switches (Arial → monospace → sans-serif)
- Increased label area sizes significantly
- Added explicit label counts and tick configurations
- Removed custom formatters to use defaults
- Tried different plotters chart configurations

**Status**: **UNRESOLVED** - fundamental blocking issue

### 4. Performance Degradation

**Problem**: Severe slowdown when chart is displayed, worsening over time

**Symptoms**:

- Frame rate drops significantly with chart visible
- More pronounced on native builds
- Performance degrades progressively

**Attempted Solutions**:

- Reduced chart updates from 60fps to 10fps (every 100ms)
- Optimized update timing to only when population data changes

**Status**: Partially mitigated but still impacts performance

## Key Technical Learnings

### What Worked

**SVG Backend**: More stable than bitmap backend, avoids crashes

**Embedded Resources**: Eliminates runtime file dependencies for cross-platform
compatibility

**Performance Optimization**: Reducing update frequency helped with performance
issues

**Direct GPU Integration**: Texture-based chart display works well with
existing UI system

### What Failed

**Plotters Label System**: Completely non-functional despite multiple
configuration attempts

**Bitmap Backend**: Fundamentally unstable with memory issues

**File-based Workflows**: Don't work on web platform

**Default Font Handling**: Plotters font resolution unreliable

### Critical Gaps

**Text Rendering**: No reliable way to render chart labels using plotters

**Cross-platform Consistency**: Different backend behaviors between native and web

**Error Handling**: Plotters provides poor error messages for debugging

**Documentation**: Limited examples for advanced label configuration

## Root Cause Analysis

The fundamental issue is that **plotters is designed for static chart
generation to files**, not for **dynamic real-time rendering in GPU-accelerated
applications**. Our use case (live updating charts in a game engine context)
exposes edge cases and limitations not encountered in typical plotting
scenarios.

### Architectural Mismatch

- Plotters assumes file output or web canvas contexts
- Our system needs direct primitive rendering to GPU textures
- Label rendering depends on complex font resolution that fails in our context
- Backend abstractions leak platform-specific limitations

## Next Steps Consideration

The core requirement remains: **professional charts with proper axis labels,
tick marks, and data accuracy**. The current plotters-based approach has proven
fundamentally incompatible with our architecture and requirements.

Alternative approaches to evaluate:

1. **Custom chart implementation** using existing UiRenderer primitives
2. **Different charting library** with better real-time GPU integration
3. **Hybrid approach** combining simple plotters for data processing with
   custom rendering
4. **Web-based charts** embedded in native builds (significant architecture change)
