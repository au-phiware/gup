# GUP-285: Tauri Event-Driven Streaming Updates

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-07-18

## Context

GUP-264 delivered a Tauri integration example that uses explicit `invoke()`
calls to fetch data from the Rust backend. For real-time visualisation use cases
(sensor dashboards, financial tickers, log monitoring), the frontend should
receive data updates automatically as the backend produces them, without
polling.

Tauri 2.x provides an event system (`app.emit(...)` on the backend,
`listen(...)` on the frontend) that enables push-based data delivery over the
same IPC bridge.

## User Story

> "As a desktop-application developer, I want my Gup chart to update in
> real-time as my Rust backend streams new data, without the frontend explicitly
> polling for updates."

## Acceptance Criteria

- [ ] The gup-tauri example demonstrates a streaming mode where the Rust backend
      emits `scatter-data-update` events on a timer.
- [ ] The frontend listens for the event and re-renders the chart automatically.
- [ ] Frame rate is ≥ 30 FPS with 30 data points updated every 100 ms.
- [ ] The streaming mode can be started and stopped from the UI.

## Dependencies

### Prerequisite Stories

- GUP-264 ✅ (Tauri Integration)

## Testing Strategy

- Manual validation: observe the chart updating in real-time.
- Measure frame rate via browser performance tools.

## Risk Assessment

- **Low**: Tauri's event system is well-documented and straightforward.

## Definition of Done

- [ ] Streaming mode works in the gup-tauri example.
- [ ] Documentation updated in `docs/TAURI_INTEGRATION.md`.
- [ ] All tests pass.
