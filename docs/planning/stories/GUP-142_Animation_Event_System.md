# GUP-142: Animation Event System

**Status**: ✅ Complete (2025-02-22)

## Story Overview

**Title**: Keyframe Event Triggers and Callbacks  
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping  
**Priority**: Medium  
**Story Points**: 8

## Context

GUP-138 implemented AnimationTimeline and keyframe animations. Complex
visualizations need to trigger events at specific animation times (e.g., update
data, play sounds, synchronize animations).

## User Story

**As a** data visualization developer  
**I want** to trigger events at animation keyframes  
**So that** I can synchronize multiple animations and respond to animation
progress

## Acceptance Criteria

### AC1: Event Registration

- [x] Register callbacks at specific times
- [x] Register callbacks at keyframe markers
- [x] Support one-time and repeating events
- [x] Remove and modify registered events

### AC2: Event Dispatch

- [x] Detect when animation crosses event time
- [x] Fire events in order during single update
- [x] Handle rapid time changes (seeks, loops)
- [x] Prevent duplicate events on same frame

### AC3: Timeline Coordination

- [x] Synchronize multiple timelines
- [x] Pause/resume groups of animations
- [x] Chain animations (play next on complete)
- [x] Timeline hierarchies (parent-child)

### AC4: Event Types

- [x] Completion events (animation finished)
- [x] Progress events (percentage milestones)
- [x] Keyframe events (specific frame reached)
- [x] Custom marker events

## Technical Requirements

- CPU-side event system (not GPU)
- Efficient time-based lookup (sorted event list)
- Thread-safe for async scenarios
- Support for closure callbacks

## Dependencies

- **Requires**: GUP-138 (Advanced Temporal Animation System) - Complete
- **May require**: Async runtime integration
- **Enables**: Complex synchronized animations

## Testing Strategy

- Test event firing accuracy
- Test event order when multiple fire
- Test loop and reverse scenarios
- Test multi-timeline synchronization

## Success Metrics

- Events fire within 1 frame of target time
- Zero missed events in stress tests
- Minimal overhead when no events registered

## Definition of Done

- [x] Event registration and dispatch working
- [x] Timeline synchronization implemented
- [x] All event types supported
- [x] Performance tested with many events
- [x] Examples demonstrating use cases
- [x] All tests pass

## Implementation Summary

### Delivered Components

1. **AnimationTimelineWithEvents** (AC1-AC4)
   - Event registration API with time-based, repeating, completion, progress,
     and marker events
   - `on_time()`, `on_time_repeating()`, `on_complete()`, `on_progress()`,
     `on_marker()` methods
   - Event removal with `remove_events()` and `clear_events()`
   - Support for closure callbacks with timeline context

2. **Event Dispatch System** (AC2)
   - Accurate time crossing detection with loop handling
   - Events fire in chronological order within single update
   - Proper handling of rapid time changes, seeks, and loops
   - Duplicate prevention with `fired_this_frame` flag

3. **Timeline Coordination** (AC3)
   - Hierarchical timeline support with `add_child()`
   - Synchronized play/pause/stop operations across parent and children
   - Completion callbacks enable animation chaining
   - Timeline groups for complex multi-track animations

4. **Named Marker System** (AC1, AC4)
   - `add_marker()` for creating named time points
   - Custom event triggers at marker times
   - Flexible marker-based event coordination

5. **Loop Detection** (AC2)
   - Unwrapped time calculation to detect when timeline loops
   - Proper event firing when crossing start/end boundary
   - Support for repeating events across multiple loops

### Key Files Modified/Created

- `src/shader_function.rs`: +350 lines implementing event system
- `src/prelude.rs`: Exported event types
- `tests/animation_event_system_tests.rs`: 20 comprehensive integration tests
- `examples/animation_events.rs`: Full demonstration of all event types

### Test Coverage

- **20 integration tests**: All passing, covering:
  - Basic event registration and firing
  - Repeating events with loops
  - Completion and progress events
  - Marker-based events
  - Event ordering and time crossing
  - Rapid time changes and seeks
  - Hierarchical timelines
  - Event removal and modification
  - Backward playback
  - Duplicate prevention
  - Pause/resume groups
  - Animation chaining

### Notable Design Decisions

1. **CPU-Side Event System**: Events managed on CPU, separate from GPU animation
   evaluation - clean separation of concerns

2. **Loop Detection via Unwrapped Time**: Calculate unwrapped time before modulo
   operation to detect when timeline loops

3. **Closure-Based Callbacks**: Use `Box<dyn FnMut>` for flexible event handlers
   with captured state

4. **Event Ordering**: Sort fired events by time to ensure chronological
   execution

5. **Hierarchical Coordination**: Parent timelines cascade play/pause/stop to
   children for synchronized control

---

_Identified during GUP-138 implementation as requirement for complex animation
scenarios._

## Retrospective

**Completed**: 2025-02-22

### Key Technical Learnings

#### Loop Detection in Animation Timelines

- **Challenge**: Detecting when a timeline loops back to the start after
  reaching its duration
- **Solution**: Calculate "unwrapped" time before modulo operation to detect
  when time exceeds duration
- **Pattern**: Store `unwrapped_new_time = old_time + delta * rate`, then check
  `unwrapped_new_time > duration`
- **Result**: Accurate event firing across loop boundaries without missing
  repeating events

#### Time Crossing Detection Algorithm

- **Challenge**: Determining if an event time was crossed between two updates
- **Solution**: Separate logic for forward playback, backward playback, and
  loops
- **Implementation**:
  ```rust
  if looped {
      // Check if target is in tail (old_time..duration) or head (0..new_time)
      (old_time < target_time && target_time <= duration)
          || (0.0 <= target_time && target_time <= new_time)
  } else if old_time < new_time {
      // Forward: crossed if we moved past the target
      old_time < target_time && new_time >= target_time
  } else if old_time > new_time {
      // Backward: crossed if we moved back past the target
      old_time > target_time && new_time <= target_time
  }
  ```
- **Future**: This algorithm could be generalized for other time-based systems

#### Closure-Based Event Callbacks

- **Decision**: Use `Box<dyn FnMut(&AnimationTimeline, f32) + Send + Sync>` for
  callbacks
- **Reasoning**: Provides access to timeline state and event time while
  supporting captured variables
- **Trade-off**: Requires heap allocation for each callback, but necessary for
  flexibility
- **Pattern**: Common pattern in Rust for flexible callback systems with state
  capture

#### Event Ordering and Sorting

- **Decision**: Sort events by time before firing them in a single update
- **Reasoning**: Ensures events fire in chronological order even when multiple
  cross in one frame
- **Implementation**: Collect indices of crossed events, sort by `event_time()`,
  then fire in order
- **Result**: Predictable event ordering independent of registration order

### Architectural Decisions

#### Separation of Timeline and Event Systems

- **Decision**: AnimationTimeline remains GPU-focused,
  AnimationTimelineWithEvents adds CPU events
- **Reasoning**: Clean separation - timeline manages playback state, events add
  behavioral hooks
- **Alternative Considered**: Adding events directly to AnimationTimeline -
  rejected for single responsibility principle
- **Future**: Other systems could wrap AnimationTimeline similarly (e.g.,
  AnimationTimelineWithRecording)

#### Hierarchical Timeline Coordination

- **Decision**: Support parent-child timeline relationships with cascading
  controls
- **Reasoning**: Complex animations often need synchronized groups (e.g.,
  character + UI animations)
- **Implementation**: Parent stores `Vec<Box<AnimationTimelineWithEvents>>` of
  children
- **Pattern**: Composite pattern - parent operations (play/pause/stop) cascade
  to children

#### Named Markers Instead of Keyframe Indices

- **Decision**: Provide named markers (`add_marker("name", time)`) instead of
  only keyframe indices
- **Reasoning**: More flexible and readable - names are self-documenting
- **Usage**: `timeline.on_marker("climax".to_string(), callback)` vs
  `timeline.on_keyframe(7, callback)`
- **Trade-off**: Slightly more memory for HashMap, but significantly better
  developer experience

#### Repeating vs One-Time Events

- **Decision**: Separate `on_time()` (fires once) and `on_time_repeating()`
  methods
- **Reasoning**: Makes intention explicit and prevents accidental repeated
  firing
- **Implementation**: Track `last_fire_time` for one-time events to prevent
  re-firing
- **Pattern**: Clear API design that prevents common mistakes

### Development Workflow Insights

- **Test-First Approach**: Wrote 20 comprehensive tests before implementation,
  catching edge cases early
- **Loop Detection Bug**: Initial implementation missed loops because `old_time`
  and `new_time` were equal after modulo
- **Debugging Strategy**: Created simple calculation tests outside the system to
  verify loop math
- **Example-Driven Validation**: The comprehensive example helped validate that
  the API is intuitive and complete

### Performance Insights

- **Event Lookup**: Linear scan of events is acceptable for typical use cases
  (10-100 events)
- **Future Optimization**: Could use binary search tree if needed for 1000+
  events per timeline
- **Memory Footprint**: Each event ~48 bytes (vtable pointer + closure data),
  minimal for typical usage
- **Zero Overhead**: When no events registered, no performance impact on
  timeline update

### Integration with Existing System

The event system integrates cleanly with existing Gup components:

- **AnimationTimeline**: Used as foundation, not modified - composition over
  modification
- **Type Safety**: Callback signatures enforced at compile time
- **Prelude**: All event types exported for easy access
- **Testing**: Follows established patterns for integration tests

### API Consistency

The event API follows established Gup patterns:

- Fluent builder API: methods return `&mut Self` for chaining
- Explicit naming: `on_time_repeating` vs `on_time` makes behavior clear
- Closure callbacks: Consistent with Rust ecosystem patterns
- Hierarchical control: Parent-child pattern used elsewhere in Gup

### Follow-up Stories

During implementation, no significant gaps were identified. The event system is
complete and ready for use. Potential future enhancements:

1. **Event Prioritization** (Low Priority)
   - Allow assigning priority to events that fire at the same time
   - Use case: Ensure certain state changes happen before others

2. **Event History/Replay** (Low Priority)
   - Record fired events for debugging or replay
   - Use case: Analyzing animation behavior or creating replay systems

3. **Async Event Handlers** (Medium Priority)
   - Support `async` callbacks that can perform network requests or file I/O
   - Requires integration with tokio or async-std
   - Use case: Loading data during animation playback

These are not critical for Phase 1 and can be added as needed based on user
feedback.
