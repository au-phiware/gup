// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for Animation Event System (GUP-142)
//!
//! Validates event registration, dispatch, timeline coordination,
//! and various event types.

use gup::{AnimationEventType, AnimationTimelineWithEvents};
use std::sync::{Arc, Mutex};

#[test]
fn test_event_registration() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = Arc::clone(&fired);

    timeline.on_time(
        5.0,
        Box::new(move |_tl, _time| {
            *fired_clone.lock().unwrap() = true;
        }),
    );

    // Event should not have fired yet
    assert!(!*fired.lock().unwrap());

    // Update to before event time
    timeline.timeline.play();
    timeline.update(3.0);
    assert!(!*fired.lock().unwrap());

    // Update to cross event time
    timeline.update(3.0);
    assert!(*fired.lock().unwrap());
}

#[test]
fn test_repeating_event() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fire_count = Arc::new(Mutex::new(0));
    let fire_count_clone = Arc::clone(&fire_count);

    timeline.on_time_repeating(
        3.0,
        Box::new(move |_tl, _time| {
            *fire_count_clone.lock().unwrap() += 1;
        }),
    );

    timeline.timeline.play();
    timeline.timeline.enable_loop(true);

    // First crossing
    timeline.update(4.0);
    assert_eq!(*fire_count.lock().unwrap(), 1);

    // Cross again after looping
    timeline.update(10.0); // Should loop and cross 3.0 again
    assert_eq!(*fire_count.lock().unwrap(), 2);
}

#[test]
fn test_completion_event() {
    let mut timeline = AnimationTimelineWithEvents::new(5.0);
    let completed = Arc::new(Mutex::new(false));
    let completed_clone = Arc::clone(&completed);

    timeline.on_complete(Box::new(move |_tl, _time| {
        *completed_clone.lock().unwrap() = true;
    }));

    timeline.timeline.play();

    // Update past duration
    timeline.update(6.0);
    assert!(*completed.lock().unwrap());
}

#[test]
fn test_progress_event() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fired_25 = Arc::new(Mutex::new(false));
    let fired_50 = Arc::new(Mutex::new(false));
    let fired_75 = Arc::new(Mutex::new(false));

    let fired_25_clone = Arc::clone(&fired_25);
    let fired_50_clone = Arc::clone(&fired_50);
    let fired_75_clone = Arc::clone(&fired_75);

    timeline.on_progress(
        0.25,
        Box::new(move |_tl, _time| {
            *fired_25_clone.lock().unwrap() = true;
        }),
    );
    timeline.on_progress(
        0.5,
        Box::new(move |_tl, _time| {
            *fired_50_clone.lock().unwrap() = true;
        }),
    );
    timeline.on_progress(
        0.75,
        Box::new(move |_tl, _time| {
            *fired_75_clone.lock().unwrap() = true;
        }),
    );

    timeline.timeline.play();

    // Update to 30% (should fire 25%)
    timeline.update(3.0);
    assert!(*fired_25.lock().unwrap());
    assert!(!*fired_50.lock().unwrap());

    // Update to 60% (should fire 50%)
    timeline.update(3.0);
    assert!(*fired_50.lock().unwrap());
    assert!(!*fired_75.lock().unwrap());

    // Update to 80% (should fire 75%)
    timeline.update(2.0);
    assert!(*fired_75.lock().unwrap());
}

#[test]
fn test_marker_event() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = Arc::clone(&fired);

    timeline.add_marker("mid_point".to_string(), 5.0);
    timeline.on_marker(
        "mid_point".to_string(),
        Box::new(move |_tl, _time| {
            *fired_clone.lock().unwrap() = true;
        }),
    );

    timeline.timeline.play();
    timeline.update(3.0);
    assert!(!*fired.lock().unwrap());

    timeline.update(3.0);
    assert!(*fired.lock().unwrap());
}

#[test]
fn test_event_order() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let order = Arc::new(Mutex::new(Vec::new()));

    let order1 = Arc::clone(&order);
    let order2 = Arc::clone(&order);
    let order3 = Arc::clone(&order);

    timeline.on_time(
        2.0,
        Box::new(move |_tl, _time| {
            order1.lock().unwrap().push(1);
        }),
    );
    timeline.on_time(
        5.0,
        Box::new(move |_tl, _time| {
            order2.lock().unwrap().push(2);
        }),
    );
    timeline.on_time(
        3.0,
        Box::new(move |_tl, _time| {
            order3.lock().unwrap().push(3);
        }),
    );

    timeline.timeline.play();
    timeline.update(6.0); // Cross all events

    let result = order.lock().unwrap();
    assert_eq!(*result, vec![1, 3, 2]); // Should fire in time order
}

#[test]
fn test_rapid_time_change() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fire_count = Arc::new(Mutex::new(0));
    let fire_count_clone = Arc::clone(&fire_count);

    timeline.on_time(
        3.0,
        Box::new(move |_tl, _time| {
            *fire_count_clone.lock().unwrap() += 1;
        }),
    );

    timeline.timeline.play();

    // Large jump crossing event time
    timeline.update(5.0);
    assert_eq!(*fire_count.lock().unwrap(), 1);

    // Jump back and forth shouldn't fire again (once event)
    timeline.seek(1.0);
    timeline.update(5.0);
    assert_eq!(*fire_count.lock().unwrap(), 1);
}

#[test]
fn test_seek_behavior() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fired = Arc::new(Mutex::new(false));
    let fired_clone = Arc::clone(&fired);

    timeline.on_time(
        5.0,
        Box::new(move |_tl, _time| {
            *fired_clone.lock().unwrap() = true;
        }),
    );

    timeline.timeline.play();

    // Seek directly to after event time
    timeline.seek(7.0);
    timeline.update(0.1);

    // Event should not fire when seeking past it
    assert!(!*fired.lock().unwrap());

    // Seek back and play through
    timeline.seek(3.0);
    timeline.update(3.0);
    assert!(*fired.lock().unwrap());
}

#[test]
fn test_remove_events() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fire_count = Arc::new(Mutex::new(0));
    let fire_count_clone = Arc::clone(&fire_count);

    timeline.on_time(
        3.0,
        Box::new(move |_tl, _time| {
            *fire_count_clone.lock().unwrap() += 1;
        }),
    );

    timeline.timeline.play();
    timeline.update(2.0);

    // Remove all time-based events
    timeline.remove_events(|event_type| matches!(event_type, AnimationEventType::Once(_)));

    // Continue to cross event time
    timeline.update(2.0);

    // Event should not fire because it was removed
    assert_eq!(*fire_count.lock().unwrap(), 0);
}

#[test]
fn test_clear_events() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fire_count = Arc::new(Mutex::new(0));
    let fire_count_clone = Arc::clone(&fire_count);

    timeline.on_time(
        3.0,
        Box::new(move |_tl, _time| {
            *fire_count_clone.lock().unwrap() += 1;
        }),
    );

    timeline.clear_events();
    timeline.timeline.play();
    timeline.update(5.0);

    assert_eq!(*fire_count.lock().unwrap(), 0);
}

#[test]
fn test_hierarchical_timelines() {
    let mut parent = AnimationTimelineWithEvents::new(10.0);
    let mut child1 = AnimationTimelineWithEvents::new(5.0);
    let mut child2 = AnimationTimelineWithEvents::new(8.0);

    let parent_fired = Arc::new(Mutex::new(false));
    let child1_fired = Arc::new(Mutex::new(false));
    let child2_fired = Arc::new(Mutex::new(false));

    let parent_fired_clone = Arc::clone(&parent_fired);
    let child1_fired_clone = Arc::clone(&child1_fired);
    let child2_fired_clone = Arc::clone(&child2_fired);

    parent.on_time(
        5.0,
        Box::new(move |_tl, _time| {
            *parent_fired_clone.lock().unwrap() = true;
        }),
    );
    child1.on_time(
        2.0,
        Box::new(move |_tl, _time| {
            *child1_fired_clone.lock().unwrap() = true;
        }),
    );
    child2.on_time(
        4.0,
        Box::new(move |_tl, _time| {
            *child2_fired_clone.lock().unwrap() = true;
        }),
    );

    parent.add_child(child1);
    parent.add_child(child2);

    parent.play();
    parent.update(6.0);

    assert!(*parent_fired.lock().unwrap());
    assert!(*child1_fired.lock().unwrap());
    assert!(*child2_fired.lock().unwrap());
}

#[test]
fn test_pause_resume_groups() {
    let mut parent = AnimationTimelineWithEvents::new(10.0);
    let child = AnimationTimelineWithEvents::new(5.0);

    parent.add_child(child);

    parent.play();
    parent.update(2.0);
    assert_eq!(parent.current_time(), 2.0);

    parent.pause();
    parent.update(1.0); // Should not advance
    assert_eq!(parent.current_time(), 2.0);

    parent.play();
    parent.update(1.0);
    assert_eq!(parent.current_time(), 3.0);
}

#[test]
fn test_chain_animations() {
    let mut first = AnimationTimelineWithEvents::new(5.0);
    let mut second = AnimationTimelineWithEvents::new(5.0);

    let second_started = Arc::new(Mutex::new(false));
    let second_started_clone = Arc::clone(&second_started);

    // Capture second timeline to start it on completion
    first.on_complete(Box::new(move |_tl, _time| {
        *second_started_clone.lock().unwrap() = true;
    }));

    first.timeline.play();
    first.update(6.0); // Complete first animation

    // Check that completion event fired
    assert!(*second_started.lock().unwrap());

    // In a real scenario, we'd start the second timeline here
    second.timeline.play();
    second.update(2.0);
    assert_eq!(second.current_time(), 2.0);
}

#[test]
fn test_backward_playback() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fire_count = Arc::new(Mutex::new(0));
    let fire_count_clone = Arc::clone(&fire_count);

    timeline.on_time_repeating(
        5.0,
        Box::new(move |_tl, _time| {
            *fire_count_clone.lock().unwrap() += 1;
        }),
    );

    timeline.timeline.play();
    timeline.update(7.0); // Forward to 7.0, crossing 5.0
    assert_eq!(*fire_count.lock().unwrap(), 1);

    // Reverse playback
    timeline.set_playback_rate(-1.0);
    timeline.update(3.0); // Go backward, crossing 5.0 again
    assert_eq!(*fire_count.lock().unwrap(), 2);
}

#[test]
fn test_no_duplicate_events_same_frame() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fire_count = Arc::new(Mutex::new(0));
    let fire_count_clone = Arc::clone(&fire_count);

    timeline.on_time_repeating(
        5.0,
        Box::new(move |_tl, _time| {
            *fire_count_clone.lock().unwrap() += 1;
        }),
    );

    timeline.timeline.play();

    // Even with a huge time jump that could theoretically cross multiple times,
    // each event should only fire once per frame
    timeline.update(6.0);
    assert_eq!(*fire_count.lock().unwrap(), 1);
}

#[test]
fn test_event_receives_timeline_context() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let captured_time = Arc::new(Mutex::new(0.0));
    let captured_time_clone = Arc::clone(&captured_time);

    timeline.on_time(
        5.0,
        Box::new(move |tl, time| {
            *captured_time_clone.lock().unwrap() = tl.current_time;
            assert!(time >= 5.0);
        }),
    );

    timeline.timeline.play();
    timeline.update(6.0);

    let time = *captured_time.lock().unwrap();
    assert!(time >= 5.0 && time <= 6.0);
}

#[test]
fn test_normalized_time() {
    let timeline = AnimationTimelineWithEvents::new(10.0);
    assert_eq!(timeline.normalized_time(), 0.0);

    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    timeline.timeline.current_time = 5.0;
    assert_eq!(timeline.normalized_time(), 0.5);

    timeline.timeline.current_time = 10.0;
    assert_eq!(timeline.normalized_time(), 1.0);
}

#[test]
fn test_playback_state_access() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    assert!(matches!(
        timeline.state(),
        gup::AnimationPlaybackState::Stopped
    ));

    timeline.play();
    assert!(matches!(
        timeline.state(),
        gup::AnimationPlaybackState::Playing
    ));

    timeline.pause();
    assert!(matches!(
        timeline.state(),
        gup::AnimationPlaybackState::Paused
    ));

    timeline.stop();
    assert!(matches!(
        timeline.state(),
        gup::AnimationPlaybackState::Stopped
    ));
}

#[test]
fn test_loop_with_events() {
    let mut timeline = AnimationTimelineWithEvents::new(5.0);
    let fire_count = Arc::new(Mutex::new(0));
    let fire_count_clone = Arc::clone(&fire_count);

    timeline.on_time_repeating(
        2.0,
        Box::new(move |_tl, _time| {
            *fire_count_clone.lock().unwrap() += 1;
        }),
    );

    timeline.timeline.play();
    timeline.enable_loop(true);

    // First loop
    timeline.update(3.0); // Cross 2.0
    assert_eq!(*fire_count.lock().unwrap(), 1);

    // Second loop
    timeline.update(5.0); // Loop back and cross 2.0 again
    assert_eq!(*fire_count.lock().unwrap(), 2);
}

#[test]
fn test_multiple_markers() {
    let mut timeline = AnimationTimelineWithEvents::new(10.0);

    timeline.add_marker("start".to_string(), 1.0);
    timeline.add_marker("middle".to_string(), 5.0);
    timeline.add_marker("end".to_string(), 9.0);

    let markers_hit = Arc::new(Mutex::new(Vec::new()));

    let markers_hit1 = Arc::clone(&markers_hit);
    let markers_hit2 = Arc::clone(&markers_hit);
    let markers_hit3 = Arc::clone(&markers_hit);

    timeline.on_marker(
        "start".to_string(),
        Box::new(move |_tl, _time| {
            markers_hit1.lock().unwrap().push("start");
        }),
    );
    timeline.on_marker(
        "middle".to_string(),
        Box::new(move |_tl, _time| {
            markers_hit2.lock().unwrap().push("middle");
        }),
    );
    timeline.on_marker(
        "end".to_string(),
        Box::new(move |_tl, _time| {
            markers_hit3.lock().unwrap().push("end");
        }),
    );

    timeline.timeline.play();
    timeline.update(10.0);

    let result = markers_hit.lock().unwrap();
    assert_eq!(result.len(), 3);
    assert!(result.contains(&"start"));
    assert!(result.contains(&"middle"));
    assert!(result.contains(&"end"));
}
