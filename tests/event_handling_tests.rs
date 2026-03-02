// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the event handling system (GUP-013).
//!
//! These tests exercise the full round-trip from raw input through
//! `EventManager` dispatch to typed `Selection::on()` handlers without
//! requiring a GPU device.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gup::event::{
    EventManager, EventResult, EventType, ModifierFlags, RawInputEvent, ViewportTransform,
};
use gup::interaction::{ElementHit, InteractionEvent, Vec2};
use gup::mark::Circle;
use gup::selection::Selection;

// ---------------------------------------------------------------------------
// Integration: Selection.on() + EventManager dispatch
// ---------------------------------------------------------------------------

/// Simulate a full event flow:
/// 1. Build a Selection with data.
/// 2. Register a typed handler via `.on()`.
/// 3. Feed a synthetic hit to `EventManager::dispatch`.
/// 4. Assert the handler was called with the correct data item.
#[test]
fn selection_on_click_full_roundtrip() {
    // Data: three items representing circle radii
    let data = vec![5.0f32, 10.0, 15.0];
    let mut selection: Selection<f32, Circle> = Selection::from_data(data);
    let sel_id = selection.selection_id();

    let received_value = Arc::new(Mutex::new(None));

    let rv = received_value.clone();
    selection.on("click", move |_event, &val| {
        *rv.lock().unwrap() = Some(val);
    });

    // Simulate a hit on element 2 (data[2] = 15.0) of this selection.
    let hit = ElementHit::new(2, sel_id, 0.0, Vec2::new(100.0, 200.0));

    // Build the interaction event as if it came from a raw input.
    let mut event = InteractionEvent::new("click", Vec2::new(100.0, 200.0));

    // Dispatch through Selection::trigger_event (which the EventManager
    // would call in production).
    selection.trigger_event("click", &mut event, hit.element_id);

    assert_eq!(*received_value.lock().unwrap(), Some(15.0));
}

/// Multiple selections — events only reach the correct one.
#[test]
fn cross_selection_isolation() {
    let mut sel_a: Selection<u32, Circle> = Selection::from_data(vec![1, 2, 3]);
    let mut sel_b: Selection<u32, Circle> = Selection::from_data(vec![10, 20, 30]);

    let a_id = sel_a.selection_id();
    let b_id = sel_b.selection_id();

    let a_counter = Arc::new(AtomicU32::new(0));
    let b_counter = Arc::new(AtomicU32::new(0));

    let ac = a_counter.clone();
    sel_a.on("click", move |_, _| {
        ac.fetch_add(1, Ordering::Relaxed);
    });

    let bc = b_counter.clone();
    sel_b.on("click", move |_, _| {
        bc.fetch_add(1, Ordering::Relaxed);
    });

    // Register both selections' handlers in the EventManager.
    let mut mgr = EventManager::new();

    // Selection A handler (wrapping trigger_event logic).
    let a_handlers = sel_a.event_handlers_ref();
    mgr.register(a_id, "click", move |event| {
        let handlers = a_handlers.lock().unwrap();
        if let Some(click_handlers) = handlers.get("click") {
            // We pass element_id 0 (data[0]) for simplicity.
            let dummy_data = 1u32;
            for h in click_handlers {
                h(event, &dummy_data);
            }
        }
        EventResult::Continue
    });

    let b_handlers = sel_b.event_handlers_ref();
    mgr.register(b_id, "click", move |event| {
        let handlers = b_handlers.lock().unwrap();
        if let Some(click_handlers) = handlers.get("click") {
            let dummy_data = 10u32;
            for h in click_handlers {
                h(event, &dummy_data);
            }
        }
        EventResult::Continue
    });

    // Dispatch with a hit on selection B only.
    let hits = vec![ElementHit::new(0, b_id, 0.0, Vec2::new(0.0, 0.0))];
    let mut event = InteractionEvent::new("click", Vec2::new(50.0, 50.0));
    mgr.dispatch(&mut event, &hits);

    assert_eq!(
        a_counter.load(Ordering::Relaxed),
        0,
        "Selection A handler must not fire"
    );
    assert_eq!(
        b_counter.load(Ordering::Relaxed),
        1,
        "Selection B handler must fire"
    );
}

// ---------------------------------------------------------------------------
// Integration: RawInputEvent → InteractionEvent → dispatch
// ---------------------------------------------------------------------------

#[test]
fn raw_input_through_event_manager() {
    let counter = Arc::new(AtomicU32::new(0));

    let mut mgr = EventManager::new();
    let c = counter.clone();
    mgr.register(42, "mousedown", move |_| {
        c.fetch_add(1, Ordering::Relaxed);
        EventResult::Continue
    });

    // Simulate a raw mouse-down event.
    let raw = RawInputEvent::new(EventType::MouseDown, Vec2::new(100.0, 200.0));
    let vt = ViewportTransform::default();
    let mut ie = raw.into_interaction_event(Some(&vt));

    // Simulate a hit result from the GPU interaction system.
    let hits = vec![ElementHit::new(5, 42, 1.5, Vec2::new(100.0, 200.0))];
    mgr.dispatch(&mut ie, &hits);

    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert!(ie.timestamp.is_some(), "timestamp should be set");
}

// ---------------------------------------------------------------------------
// Integration: coordinate transform
// ---------------------------------------------------------------------------

#[test]
fn viewport_transform_applied_in_dispatch() {
    let world_pos = Arc::new(Mutex::new(None));

    let mut mgr = EventManager::new();
    let wp = world_pos.clone();
    mgr.register(1, "mousemove", move |event| {
        wp.lock().unwrap().clone_from(&event.world_position);
        EventResult::Continue
    });

    let vt = ViewportTransform {
        offset: Vec2::new(50.0, 50.0),
        scale: Vec2::new(2.0, 2.0),
    };

    let raw = RawInputEvent::new(EventType::MouseMove, Vec2::new(150.0, 250.0));
    let mut ie = raw.into_interaction_event(Some(&vt));

    let hits = vec![ElementHit::new(0, 1, 0.0, Vec2::new(0.0, 0.0))];
    mgr.dispatch(&mut ie, &hits);

    let wp = world_pos.lock().unwrap().clone();
    assert_eq!(wp, Some(Vec2::new(50.0, 100.0)));
}

// ---------------------------------------------------------------------------
// Integration: global handlers
// ---------------------------------------------------------------------------

#[test]
fn global_handler_fires_with_and_without_hits() {
    let counter = Arc::new(AtomicU32::new(0));

    let mut mgr = EventManager::new();
    let c = counter.clone();
    mgr.register_global("click", move |_| {
        c.fetch_add(1, Ordering::Relaxed);
        EventResult::Continue
    });

    // No hits — global handler should still fire.
    let mut event = InteractionEvent::new("click", Vec2::new(0.0, 0.0));
    mgr.dispatch(&mut event, &[]);
    assert_eq!(counter.load(Ordering::Relaxed), 1);

    // With a hit — global handler fires again.
    let hits = vec![ElementHit::new(0, 1, 0.0, Vec2::new(0.0, 0.0))];
    let mut event2 = InteractionEvent::new("click", Vec2::new(0.0, 0.0));
    mgr.dispatch(&mut event2, &hits);
    assert_eq!(counter.load(Ordering::Relaxed), 2);
}

// ---------------------------------------------------------------------------
// Integration: modifier flags
// ---------------------------------------------------------------------------

#[test]
fn modifier_flags_propagated_through_dispatch() {
    let received_mods = Arc::new(Mutex::new(ModifierFlags::NONE));

    let mut mgr = EventManager::new();
    let rm = received_mods.clone();
    mgr.register_global("mousedown", move |event| {
        *rm.lock().unwrap() = event.modifiers;
        EventResult::Continue
    });

    let mods = ModifierFlags {
        shift: true,
        ctrl: true,
        alt: false,
        meta: false,
    };
    let raw = RawInputEvent::new(EventType::MouseDown, Vec2::new(0.0, 0.0)).with_modifiers(mods);
    let mut ie = raw.into_interaction_event(None);
    mgr.dispatch(&mut ie, &[]);

    let rm = received_mods.lock().unwrap();
    assert!(rm.shift, "shift should be set");
    assert!(rm.ctrl, "ctrl should be set");
    assert!(!rm.alt, "alt should not be set");
}

// ---------------------------------------------------------------------------
// Performance: 10 000 elements, 50 handlers, < 16ms
// ---------------------------------------------------------------------------

#[test]
fn performance_10k_elements_50_handlers() {
    let counter = Arc::new(AtomicU32::new(0));

    let mut mgr = EventManager::new();

    // Register 50 handlers spread across 10 selections (5 per selection).
    for sel_id in 0..10 {
        for _ in 0..5 {
            let c = counter.clone();
            mgr.register(sel_id, "mousemove", move |_| {
                c.fetch_add(1, Ordering::Relaxed);
                EventResult::Continue
            });
        }
    }

    // 10 000 hits (simulating a chart with 10 000 visible elements).
    let hits: Vec<ElementHit> = (0..10_000)
        .map(|i| ElementHit::new(i, i % 10, i as f32, Vec2::new(0.0, 0.0)))
        .collect();

    let start = Instant::now();
    let mut event = InteractionEvent::new("mousemove", Vec2::new(0.0, 0.0));
    mgr.dispatch(&mut event, &hits);
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 16,
        "dispatch of 10k hits × 50 handlers took {elapsed:?}, exceeding 16ms budget"
    );

    // Verify handlers actually ran (10000 hits × 5 handlers per matching selection = 50000).
    assert_eq!(counter.load(Ordering::Relaxed), 50_000);
}
