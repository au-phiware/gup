# GUP-043: Visual Blend Mode Demonstration

## Story

**As a** library developer  
**I want** a visual demonstration of blend modes working in real windows  
**So that** I can validate the visual correctness of blend operations and
provide compelling examples

## Background

GUP-027 implemented comprehensive GPU blend state integration with excellent
console output demonstrations. However, the visual windowed examples encountered
technical challenges with winit API compatibility and `Arc<GupContext>`
borrowing conflicts. A visual demonstration would provide:

1. **Visual validation** of blend mode correctness
2. **Compelling demonstrations** for users and documentation
3. **Debug capabilities** for blend mode behavior
4. **Marketing material** showing library capabilities

## Acceptance Criteria

### Core Visual Example

- [ ] Create windowed application showing all 4 blend modes side-by-side
- [ ] Display: None, AlphaBlending, Additive, Multiply modes with clear labels
- [ ] Use simple colored rectangles overlapping to show blend effects
- [ ] Run at stable 60fps with smooth rendering

### Interactive Features

- [ ] Keyboard controls to switch between blend modes
- [ ] Real-time alpha adjustment with slider or keyboard
- [ ] Toggle global alpha effects on/off
- [ ] Display current blend mode and performance stats

### Cross-Platform Compatibility

- [ ] Works on Linux, macOS, Windows
- [ ] WebAssembly version deployable to web
- [ ] Consistent visual output across platforms
- [ ] Proper handling of different surface formats

### Technical Requirements

- [ ] Resolve winit API compatibility issues
- [ ] Fix `Arc<GupContext>` borrowing conflicts
- [ ] Clean separation between windowing and rendering
- [ ] Example runs with `cargo run --example visual_blend_demo`

## Implementation Notes

### Key Technical Challenges

1. **Winit API Compatibility**: Use current winit 0.30 APIs properly
2. **Arc Sharing**: Implement proper context sharing without borrow conflicts
3. **Window Lifecycle**: Handle window events and resize properly
4. **Cross-Platform**: Ensure consistent behavior across platforms

### Suggested Architecture

```rust
struct BlendDemoApp {
    context: `Arc<GupContext>`,
    current_mode: BlendMode,
    global_alpha: f32,
    demo_renderer: BlendDemoRenderer,
}

impl BlendDemoApp {
    fn handle_input(&mut self, input: &InputEvent);
    fn update(&mut self, dt: f32);
    fn render(&mut self) -> Result<()>;
}
```

### Visual Layout

```text
+------------------+------------------+
| None Mode        | Alpha Blending   |
| [Red] [Blue]     | [Red] [Blue]     |
|   Separate       |   Transparent    |
+------------------+------------------+
| Additive         | Multiply         |
| [Red] [Blue]     | [Red] [Blue]     |
|   Bright         |   Dark           |
+------------------+------------------+
| Controls: [Space] to cycle modes    |
| Alpha: [←] [→] Current: 0.75        |
+-------------------------------------+
```

## Dependencies

- **Depends on**: GUP-027 (GPU Blend State Integration) - Complete
- **Blocks**: Documentation and marketing materials

## Definition of Done

- [ ] Visual example compiles and runs on all supported platforms
- [ ] All 4 blend modes visually demonstrate correct behavior
- [ ] Interactive controls work smoothly
- [ ] Performance maintains 60fps with blend mode switching
- [ ] Code is well-documented with clear architecture
- [ ] Example is included in project documentation

## Estimated Effort

**2-3 days** - Medium complexity due to windowing system integration challenges

## Success Metrics

- Visual validation of blend mode correctness
- Stable 60fps performance
- Cross-platform compatibility
- User-friendly interactive demonstration

## Notes

This story directly addresses the user feedback "Don't make the example
headless, I want to see it" from GUP-027. The visual demonstration will serve as
both validation and marketing material for the blend state system.
