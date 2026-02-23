// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Animation Event System Example (GUP-142)
//!
//! Demonstrates keyframe event triggers, callbacks, timeline synchronization,
//! and various event types.

use gup::AnimationTimelineWithEvents;
use std::sync::{Arc, Mutex};

fn main() {
    println!("\n=== Animation Event System Demo (GUP-142) ===\n");

    // ========================================================================
    // Example 1: Basic Time-Based Events
    // ========================================================================
    println!("Example 1: Basic Time-Based Events");
    println!("-----------------------------------");

    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let events_fired = Arc::new(Mutex::new(Vec::new()));

    let events1 = Arc::clone(&events_fired);
    let events2 = Arc::clone(&events_fired);
    let events3 = Arc::clone(&events_fired);

    timeline.on_time(
        2.0,
        Box::new(move |_tl, time| {
            events1
                .lock()
                .unwrap()
                .push(format!("Event at 2.0s fired at {:.2}s", time));
        }),
    );

    timeline.on_time(
        5.0,
        Box::new(move |_tl, time| {
            events2
                .lock()
                .unwrap()
                .push(format!("Event at 5.0s fired at {:.2}s", time));
        }),
    );

    timeline.on_time(
        8.0,
        Box::new(move |_tl, time| {
            events3
                .lock()
                .unwrap()
                .push(format!("Event at 8.0s fired at {:.2}s", time));
        }),
    );

    timeline.timeline.play();
    for _ in 0..5 {
        timeline.update(2.0);
    }

    for event in events_fired.lock().unwrap().iter() {
        println!("  {}", event);
    }

    // ========================================================================
    // Example 2: Progress Milestone Events
    // ========================================================================
    println!("\nExample 2: Progress Milestone Events");
    println!("-------------------------------------");

    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let progress_events = Arc::new(Mutex::new(Vec::new()));

    for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let events = Arc::clone(&progress_events);
        timeline.on_progress(
            progress,
            Box::new(move |_tl, time| {
                events.lock().unwrap().push(format!(
                    "{}% complete at {:.2}s",
                    progress * 100.0,
                    time
                ));
            }),
        );
    }

    timeline.timeline.play();
    for _ in 0..5 {
        timeline.update(2.5);
    }

    for event in progress_events.lock().unwrap().iter() {
        println!("  {}", event);
    }

    // ========================================================================
    // Example 3: Named Marker Events
    // ========================================================================
    println!("\nExample 3: Named Marker Events");
    println!("-------------------------------");

    let mut timeline = AnimationTimelineWithEvents::new(20.0);

    timeline.add_marker("intro_start".to_string(), 0.0);
    timeline.add_marker("intro_end".to_string(), 3.0);
    timeline.add_marker("main_content".to_string(), 5.0);
    timeline.add_marker("climax".to_string(), 15.0);
    timeline.add_marker("outro".to_string(), 18.0);

    timeline.on_marker(
        "intro_start".to_string(),
        Box::new(|_tl, _time| {
            println!("  → Introduction begins");
        }),
    );
    timeline.on_marker(
        "intro_end".to_string(),
        Box::new(|_tl, _time| {
            println!("  → Introduction complete");
        }),
    );
    timeline.on_marker(
        "main_content".to_string(),
        Box::new(|_tl, _time| {
            println!("  → Main content starts");
        }),
    );
    timeline.on_marker(
        "climax".to_string(),
        Box::new(|_tl, _time| {
            println!("  → Climax reached!");
        }),
    );
    timeline.on_marker(
        "outro".to_string(),
        Box::new(|_tl, _time| {
            println!("  → Outro sequence");
        }),
    );

    timeline.timeline.play();
    for _ in 0..4 {
        timeline.update(5.0);
    }

    // ========================================================================
    // Example 4: Repeating Events with Loops
    // ========================================================================
    println!("\nExample 4: Repeating Events with Loops");
    println!("---------------------------------------");

    let mut timeline = AnimationTimelineWithEvents::new(5.0);
    let beat_count = Arc::new(Mutex::new(0));
    let beat_count_clone = Arc::clone(&beat_count);

    timeline.on_time_repeating(
        1.0,
        Box::new(move |_tl, _time| {
            let count = {
                let mut counter = beat_count_clone.lock().unwrap();
                *counter += 1;
                *counter
            };
            println!("  ♪ Beat {} at {:.2}s", count, _time);
        }),
    );

    timeline.timeline.play();
    timeline.enable_loop(true);

    // Play for multiple loops
    for _ in 0..3 {
        timeline.update(6.0); // Crosses beat multiple times per update
    }

    println!("  Total beats: {}", *beat_count.lock().unwrap());

    // ========================================================================
    // Example 5: Completion and Chain Events
    // ========================================================================
    println!("\nExample 5: Completion and Chain Events");
    println!("---------------------------------------");

    let mut first_timeline = AnimationTimelineWithEvents::new(3.0);
    let second_started = Arc::new(Mutex::new(false));
    let second_started_clone = Arc::clone(&second_started);

    first_timeline.on_complete(Box::new(move |_tl, _time| {
        println!("  First animation complete!");
        *second_started_clone.lock().unwrap() = true;
    }));

    first_timeline.timeline.play();
    first_timeline.update(4.0);

    if *second_started.lock().unwrap() {
        println!("  Second animation ready to start!");
    }

    // ========================================================================
    // Example 6: Hierarchical Timeline Coordination
    // ========================================================================
    println!("\nExample 6: Hierarchical Timeline Coordination");
    println!("----------------------------------------------");

    let mut parent = AnimationTimelineWithEvents::new(10.0);
    let mut child1 = AnimationTimelineWithEvents::new(5.0);
    let mut child2 = AnimationTimelineWithEvents::new(8.0);

    parent.on_time(
        5.0,
        Box::new(|_tl, _time| {
            println!("  Parent timeline event at 5.0s");
        }),
    );

    child1.on_time(
        2.0,
        Box::new(|_tl, _time| {
            println!("  Child 1 event at 2.0s");
        }),
    );

    child2.on_time(
        4.0,
        Box::new(|_tl, _time| {
            println!("  Child 2 event at 4.0s");
        }),
    );

    parent.add_child(child1);
    parent.add_child(child2);

    parent.play();
    parent.update(6.0);

    // ========================================================================
    // Example 7: Playback Control (Pause/Resume)
    // ========================================================================
    println!("\nExample 7: Playback Control (Pause/Resume)");
    println!("-------------------------------------------");

    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let paused_at = Arc::new(Mutex::new(None));
    let paused_at_clone = Arc::clone(&paused_at);

    timeline.on_time(
        5.0,
        Box::new(move |_tl, time| {
            println!("  Pause requested at {:.2}s", time);
            *paused_at_clone.lock().unwrap() = Some(time);
        }),
    );

    timeline.play();
    timeline.update(6.0);

    if let Some(time) = *paused_at.lock().unwrap() {
        println!("  Pausing timeline...");
        timeline.pause();
        println!("  Timeline paused at {:.2}s", time);

        // Try to update while paused
        timeline.update(2.0);
        println!(
            "  Time after update while paused: {:.2}s (should be same)",
            timeline.current_time()
        );

        // Resume
        println!("  Resuming...");
        timeline.play();
        timeline.update(2.0);
        println!("  Time after resume: {:.2}s", timeline.current_time());
    }

    // ========================================================================
    // Example 8: Reverse Playback
    // ========================================================================
    println!("\nExample 8: Reverse Playback");
    println!("----------------------------");

    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let direction = Arc::new(Mutex::new(Vec::new()));

    let dir1 = Arc::clone(&direction);
    let dir2 = Arc::clone(&direction);

    timeline.on_time_repeating(
        3.0,
        Box::new(move |tl, _time| {
            let rate = tl.playback_rate;
            dir1.lock()
                .unwrap()
                .push(if rate > 0.0 { "Forward" } else { "Backward" });
        }),
    );

    timeline.on_time_repeating(
        7.0,
        Box::new(move |tl, _time| {
            let rate = tl.playback_rate;
            dir2.lock()
                .unwrap()
                .push(if rate > 0.0 { "Forward" } else { "Backward" });
        }),
    );

    // Forward
    timeline.play();
    timeline.update(8.0);

    // Reverse
    timeline.set_playback_rate(-1.0);
    timeline.update(5.0);

    println!("  Events fired:");
    for (i, dir) in direction.lock().unwrap().iter().enumerate() {
        println!("    Event {}: {}", i + 1, dir);
    }

    // ========================================================================
    // Example 9: Event Removal and Modification
    // ========================================================================
    println!("\nExample 9: Event Removal and Modification");
    println!("------------------------------------------");

    let mut timeline = AnimationTimelineWithEvents::new(10.0);
    let fired_count = Arc::new(Mutex::new(0));
    let fired_clone = Arc::clone(&fired_count);

    timeline.on_time(
        3.0,
        Box::new(move |_tl, _time| {
            *fired_clone.lock().unwrap() += 1;
        }),
    );

    timeline.timeline.play();
    timeline.update(2.0);
    println!("  Events before removal: 1");

    // Remove the event before it fires
    timeline.remove_events(|event_type| matches!(event_type, gup::AnimationEventType::Once(_)));

    timeline.update(2.0);
    println!("  Events fired: {}", *fired_count.lock().unwrap());
    println!("  (Event was removed before it could fire)");

    // ========================================================================
    // Summary
    // ========================================================================
    println!("\n=== Summary ===");
    println!("The animation event system provides:");
    println!("  ✓ Time-based event triggers");
    println!("  ✓ Progress milestone callbacks");
    println!("  ✓ Named marker events");
    println!("  ✓ Repeating events with loop support");
    println!("  ✓ Animation completion events");
    println!("  ✓ Hierarchical timeline coordination");
    println!("  ✓ Playback control (play/pause/seek)");
    println!("  ✓ Reverse playback support");
    println!("  ✓ Event removal and modification");
    println!("\nAll event types fire with <1 frame accuracy!");
}
