// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! High-level event handling system for GPU-accelerated visualizations.
//!
//! This module provides the developer-facing event layer that sits on top of
//! the GPU interaction system (GUP-012). It bridges raw window input events,
//! GPU hit-test results, and typed Rust closures via a familiar
//! `.on(event, handler)` API.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌──────────────────┐
//! │ winit input  │────▶│ EventManager │────▶│ Selection.on()   │
//! │ (raw events) │     │  (routing)   │     │ handlers (typed) │
//! └─────────────┘     └──────────────┘     └──────────────────┘
//!                          │                        │
//!                          ▼                        ▼
//!                   ┌──────────────┐         InteractionEvent
//!                   │ GPU hit test │         with &T data
//!                   │ (GUP-012)   │
//!                   └──────────────┘
//! ```
//!
//! # Event Types
//!
//! [`EventType`] covers mouse events (`Move`, `Down`, `Up`, `Enter`, `Leave`)
//! and touch events (`TouchStart`, `TouchMove`, `TouchEnd`).
//!
//! # Propagation
//!
//! Events are dispatched in hit-depth order (front-most element first).
//! Handlers can return [`EventResult::StopPropagation`] to halt further
//! bubbling, or [`EventResult::Continue`] to allow propagation to continue.

use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use crate::interaction::{ElementHit, InteractionEvent, Vec2};

// ---------------------------------------------------------------------------
// EventType
// ---------------------------------------------------------------------------

/// Classification of input events into mouse and touch categories.
///
/// This enum provides a structured alternative to string-based event names
/// while maintaining interoperability with the `.on("click", handler)` API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    // -- Mouse events --
    /// Mouse cursor moved over the visualization area.
    MouseMove,
    /// Mouse button pressed down.
    MouseDown,
    /// Mouse button released.
    MouseUp,
    /// Mouse cursor entered an element's bounds.
    MouseEnter,
    /// Mouse cursor left an element's bounds.
    MouseLeave,

    // -- Touch events --
    /// A touch point was placed on the screen.
    TouchStart,
    /// A touch point moved on the screen.
    TouchMove,
    /// A touch point was removed from the screen.
    TouchEnd,
}

impl EventType {
    /// Returns the canonical string name for this event type, matching the
    /// keys used with `Selection::on()`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MouseMove => "mousemove",
            Self::MouseDown => "mousedown",
            Self::MouseUp => "mouseup",
            Self::MouseEnter => "mouseenter",
            Self::MouseLeave => "mouseleave",
            Self::TouchStart => "touchstart",
            Self::TouchMove => "touchmove",
            Self::TouchEnd => "touchend",
        }
    }

    /// Parse a string event name into an `EventType`, if it matches a known name.
    pub fn from_str_name(s: &str) -> Option<Self> {
        match s {
            "mousemove" => Some(Self::MouseMove),
            "mousedown" => Some(Self::MouseDown),
            "mouseup" => Some(Self::MouseUp),
            "mouseenter" => Some(Self::MouseEnter),
            "mouseleave" => Some(Self::MouseLeave),
            "touchstart" => Some(Self::TouchStart),
            "touchmove" => Some(Self::TouchMove),
            "touchend" => Some(Self::TouchEnd),
            // Allow common aliases
            "click" => Some(Self::MouseUp),
            "hover" => Some(Self::MouseMove),
            _ => None,
        }
    }

    /// Returns `true` if this is a mouse event.
    pub fn is_mouse(&self) -> bool {
        matches!(
            self,
            Self::MouseMove | Self::MouseDown | Self::MouseUp | Self::MouseEnter | Self::MouseLeave
        )
    }

    /// Returns `true` if this is a touch event.
    pub fn is_touch(&self) -> bool {
        matches!(self, Self::TouchStart | Self::TouchMove | Self::TouchEnd)
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EventResult
// ---------------------------------------------------------------------------

/// Outcome returned by an event handler to control propagation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// Allow the event to continue propagating to the next handler / element.
    Continue,
    /// Stop propagation — no further handlers will be invoked for this event.
    StopPropagation,
}

// ---------------------------------------------------------------------------
// ModifierFlags
// ---------------------------------------------------------------------------

/// Keyboard modifier state at the time an event was dispatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierFlags {
    /// `true` when the Shift key is held.
    pub shift: bool,
    /// `true` when the Ctrl (or Cmd on macOS) key is held.
    pub ctrl: bool,
    /// `true` when the Alt (or Option on macOS) key is held.
    pub alt: bool,
    /// `true` when the Meta / Super / Windows key is held.
    pub meta: bool,
}

impl ModifierFlags {
    /// No modifiers held.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };
}

// ---------------------------------------------------------------------------
// EventManager
// ---------------------------------------------------------------------------

/// A type-erased event handler stored in the [`EventManager`].
///
/// Called with the interaction event and, for selection-scoped handlers, only
/// for hits that match the registered `(selection_id, event_name)` key.
type AnyHandler = Box<dyn Fn(&mut InteractionEvent) -> EventResult + Send + Sync>;

/// Manages event dispatch from raw window input to registered handlers.
///
/// The `EventManager` is the central routing hub that:
///
/// 1. Receives raw input events (from winit or test harnesses).
/// 2. Converts cursor/touch coordinates to visualization space.
/// 3. Invokes hit tests via the GPU interaction system.
/// 4. Dispatches [`InteractionEvent`]s to registered handlers in hit-depth
///    order, respecting [`EventResult::StopPropagation`].
///
/// # Handler Scoping
///
/// Handlers can be **selection-scoped** (keyed by `(SelectionId, event_name)`)
/// or **global** (receive every dispatched event regardless of hit result).
#[derive(Default)]
pub struct EventManager {
    /// Selection-scoped handlers keyed by `(selection_id, event_name)`.
    ///
    /// The inner `Vec` preserves registration order — handlers at lower
    /// indices were registered first and are called first.
    selection_handlers: HashMap<(u32, String), Vec<AnyHandler>>,

    /// Global handlers keyed by event name.
    ///
    /// Global handlers fire for *every* dispatched event, regardless of
    /// whether a hit was detected. They run after all selection-scoped
    /// handlers for a given depth level and are subject to
    /// `StopPropagation` like any other handler.
    global_handlers: HashMap<String, Vec<AnyHandler>>,
}

impl fmt::Debug for EventManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventManager")
            .field(
                "selection_handlers",
                &self.selection_handlers.keys().collect::<Vec<_>>(),
            )
            .field(
                "global_handler_count",
                &self.global_handlers.values().map(Vec::len).sum::<usize>(),
            )
            .finish()
    }
}

impl EventManager {
    /// Create a new, empty event manager.
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Register a handler scoped to a specific selection and event name.
    ///
    /// The handler will only fire when a hit test resolves to an element
    /// belonging to `selection_id` and the event name matches.
    pub fn register<F>(&mut self, selection_id: u32, event_name: &str, handler: F)
    where
        F: Fn(&mut InteractionEvent) -> EventResult + Send + Sync + 'static,
    {
        self.selection_handlers
            .entry((selection_id, event_name.to_string()))
            .or_default()
            .push(Box::new(handler));
    }

    /// Register a global handler that receives every dispatched event.
    pub fn register_global<F>(&mut self, event_name: &str, handler: F)
    where
        F: Fn(&mut InteractionEvent) -> EventResult + Send + Sync + 'static,
    {
        self.global_handlers
            .entry(event_name.to_string())
            .or_default()
            .push(Box::new(handler));
    }

    /// Remove all handlers for a given selection.
    pub fn remove_selection_handlers(&mut self, selection_id: u32) {
        self.selection_handlers
            .retain(|(sid, _), _| *sid != selection_id);
    }

    /// Remove all handlers (selection-scoped and global).
    pub fn clear(&mut self) {
        self.selection_handlers.clear();
        self.global_handlers.clear();
    }

    // -----------------------------------------------------------------------
    // Dispatch
    // -----------------------------------------------------------------------

    /// Dispatch an event against a set of hit results sorted in depth order
    /// (front-most first).
    ///
    /// For each hit, selection-scoped handlers registered under the hit's
    /// `selection_id` are invoked. If any handler returns
    /// [`EventResult::StopPropagation`], dispatch halts immediately —
    /// remaining hits and global handlers are skipped.
    ///
    /// After processing all hits (or if there are no hits), global handlers
    /// for the event name are invoked unless propagation has been stopped.
    ///
    /// # Arguments
    ///
    /// * `event` – The interaction event to dispatch.  Its `hit` field
    ///    will be updated to the current hit as dispatch proceeds through
    ///    the hit list.
    /// * `hits` – Hit results from the GPU interaction system, sorted by
    ///    depth/distance (front-most first).
    ///
    /// # Returns
    ///
    /// The final [`EventResult`] — `StopPropagation` if any handler halted
    /// dispatch, otherwise `Continue`.
    pub fn dispatch(&self, event: &mut InteractionEvent, hits: &[ElementHit]) -> EventResult {
        let event_name = event.interaction_type.clone();

        // 1. Dispatch to selection-scoped handlers in hit-depth order.
        for hit in hits {
            event.hit = Some(hit.clone());

            let key = (hit.selection_id, event_name.clone());
            if let Some(handlers) = self.selection_handlers.get(&key) {
                for handler in handlers {
                    if event.is_propagation_stopped() {
                        return EventResult::StopPropagation;
                    }
                    let result = handler(event);
                    if result == EventResult::StopPropagation {
                        event.stop_propagation();
                        return EventResult::StopPropagation;
                    }
                }
            }

            // Check propagation after processing a hit's handlers.
            if event.is_propagation_stopped() {
                return EventResult::StopPropagation;
            }
        }

        // 2. Dispatch to global handlers.
        if let Some(handlers) = self.global_handlers.get(&event_name) {
            for handler in handlers {
                if event.is_propagation_stopped() {
                    return EventResult::StopPropagation;
                }
                let result = handler(event);
                if result == EventResult::StopPropagation {
                    event.stop_propagation();
                    return EventResult::StopPropagation;
                }
            }
        }

        EventResult::Continue
    }

    /// Return the total number of registered selection-scoped handlers.
    pub fn selection_handler_count(&self) -> usize {
        self.selection_handlers.values().map(Vec::len).sum()
    }

    /// Return the total number of registered global handlers.
    pub fn global_handler_count(&self) -> usize {
        self.global_handlers.values().map(Vec::len).sum()
    }

    /// Return `true` if no handlers are registered.
    pub fn is_empty(&self) -> bool {
        self.selection_handlers.is_empty() && self.global_handlers.is_empty()
    }
}

// ---------------------------------------------------------------------------
// RawInputEvent — abstraction over windowing system events
// ---------------------------------------------------------------------------

/// A simplified representation of a raw window input event, suitable for
/// feeding into the [`EventManager`] dispatch pipeline.
///
/// This type provides a windowing-system-agnostic interface. In production
/// it is constructed from `winit::event::WindowEvent`; in tests it can be
/// built directly.
#[derive(Debug, Clone)]
pub struct RawInputEvent {
    /// The high-level event classification.
    pub event_type: EventType,
    /// Cursor or primary touch position in physical pixel coordinates.
    pub position: Vec2,
    /// Keyboard modifier state.
    pub modifiers: ModifierFlags,
    /// Monotonic timestamp of the event.
    pub timestamp: Instant,
}

impl RawInputEvent {
    /// Create a new raw input event.
    pub fn new(event_type: EventType, position: Vec2) -> Self {
        Self {
            event_type,
            position,
            modifiers: ModifierFlags::NONE,
            timestamp: Instant::now(),
        }
    }

    /// Builder: attach modifier flags.
    pub fn with_modifiers(mut self, modifiers: ModifierFlags) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Builder: override the timestamp.
    pub fn with_timestamp(mut self, timestamp: Instant) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Convert this raw event into an [`InteractionEvent`] suitable for
    /// dispatch, applying a viewport transform to produce visualization-space
    /// coordinates.
    pub fn into_interaction_event(
        self,
        viewport_transform: Option<&ViewportTransform>,
    ) -> InteractionEvent {
        let world_position = viewport_transform.map(|vt| vt.screen_to_world(self.position));

        let mut event = InteractionEvent::new(self.event_type.as_str(), self.position);
        if let Some(wp) = world_position {
            event = event.with_world_position(wp);
        }
        event.timestamp = Some(self.timestamp);
        event.modifiers = self.modifiers;
        event
    }
}

// ---------------------------------------------------------------------------
// ViewportTransform
// ---------------------------------------------------------------------------

/// Maps between screen (physical pixel) coordinates and visualization-space
/// (world) coordinates.
///
/// The transform accounts for viewport offset (position of the chart area
/// within the window) and scale (zoom level).
#[derive(Debug, Clone)]
pub struct ViewportTransform {
    /// Offset of the visualization origin from the top-left of the window,
    /// in physical pixels.
    pub offset: Vec2,
    /// Scale factor (pixels per world unit). Defaults to `(1.0, 1.0)`.
    pub scale: Vec2,
}

impl Default for ViewportTransform {
    fn default() -> Self {
        Self {
            offset: Vec2::new(0.0, 0.0),
            scale: Vec2::new(1.0, 1.0),
        }
    }
}

impl ViewportTransform {
    /// Convert screen coordinates to world (visualization-space) coordinates.
    pub fn screen_to_world(&self, screen: Vec2) -> Vec2 {
        Vec2::new(
            (screen.x - self.offset.x) / self.scale.x,
            (screen.y - self.offset.y) / self.scale.y,
        )
    }

    /// Convert world coordinates to screen coordinates.
    pub fn world_to_screen(&self, world: Vec2) -> Vec2 {
        Vec2::new(
            world.x * self.scale.x + self.offset.x,
            world.y * self.scale.y + self.offset.y,
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- EventType ---------------------------------------------------------

    #[test]
    fn event_type_string_roundtrip() {
        let types = [
            EventType::MouseMove,
            EventType::MouseDown,
            EventType::MouseUp,
            EventType::MouseEnter,
            EventType::MouseLeave,
            EventType::TouchStart,
            EventType::TouchMove,
            EventType::TouchEnd,
        ];
        for ty in &types {
            let s = ty.as_str();
            let parsed = EventType::from_str_name(s).unwrap();
            assert_eq!(*ty, parsed, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn event_type_aliases() {
        assert_eq!(EventType::from_str_name("click"), Some(EventType::MouseUp));
        assert_eq!(
            EventType::from_str_name("hover"),
            Some(EventType::MouseMove)
        );
        assert_eq!(EventType::from_str_name("unknown"), None);
    }

    #[test]
    fn event_type_categories() {
        assert!(EventType::MouseDown.is_mouse());
        assert!(!EventType::MouseDown.is_touch());
        assert!(EventType::TouchStart.is_touch());
        assert!(!EventType::TouchStart.is_mouse());
    }

    // -- EventResult -------------------------------------------------------

    #[test]
    fn event_result_debug() {
        assert_eq!(format!("{:?}", EventResult::Continue), "Continue");
        assert_eq!(
            format!("{:?}", EventResult::StopPropagation),
            "StopPropagation"
        );
    }

    // -- ModifierFlags -----------------------------------------------------

    #[test]
    fn modifier_flags_default() {
        let m = ModifierFlags::default();
        assert!(!m.shift && !m.ctrl && !m.alt && !m.meta);
        assert_eq!(m, ModifierFlags::NONE);
    }

    // -- ViewportTransform -------------------------------------------------

    #[test]
    fn viewport_identity_transform() {
        let vt = ViewportTransform::default();
        let screen = Vec2::new(100.0, 200.0);
        assert_eq!(vt.screen_to_world(screen), screen);
        assert_eq!(vt.world_to_screen(screen), screen);
    }

    #[test]
    fn viewport_with_offset_and_scale() {
        let vt = ViewportTransform {
            offset: Vec2::new(50.0, 50.0),
            scale: Vec2::new(2.0, 2.0),
        };
        let screen = Vec2::new(150.0, 250.0);
        let world = vt.screen_to_world(screen);
        assert_eq!(world, Vec2::new(50.0, 100.0));
        assert_eq!(vt.world_to_screen(world), screen);
    }

    // -- EventManager: registration ----------------------------------------

    #[test]
    fn register_and_count_handlers() {
        let mut mgr = EventManager::new();
        assert!(mgr.is_empty());

        mgr.register(1, "click", |_| EventResult::Continue);
        mgr.register(1, "click", |_| EventResult::Continue);
        mgr.register(2, "hover", |_| EventResult::Continue);
        assert_eq!(mgr.selection_handler_count(), 3);

        mgr.register_global("click", |_| EventResult::Continue);
        assert_eq!(mgr.global_handler_count(), 1);
        assert!(!mgr.is_empty());
    }

    #[test]
    fn remove_selection_handlers() {
        let mut mgr = EventManager::new();
        mgr.register(1, "click", |_| EventResult::Continue);
        mgr.register(1, "hover", |_| EventResult::Continue);
        mgr.register(2, "click", |_| EventResult::Continue);
        assert_eq!(mgr.selection_handler_count(), 3);

        mgr.remove_selection_handlers(1);
        assert_eq!(mgr.selection_handler_count(), 1);
    }

    #[test]
    fn clear_removes_everything() {
        let mut mgr = EventManager::new();
        mgr.register(1, "click", |_| EventResult::Continue);
        mgr.register_global("click", |_| EventResult::Continue);
        mgr.clear();
        assert!(mgr.is_empty());
    }

    // -- EventManager: dispatch --------------------------------------------

    #[test]
    fn dispatch_invokes_matching_selection_handlers() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let counter = std::sync::Arc::new(AtomicU32::new(0));

        let mut mgr = EventManager::new();
        let c = counter.clone();
        mgr.register(1, "click", move |_| {
            c.fetch_add(1, Ordering::Relaxed);
            EventResult::Continue
        });

        // Different selection — should NOT fire
        let c2 = counter.clone();
        mgr.register(2, "click", move |_| {
            c2.fetch_add(100, Ordering::Relaxed);
            EventResult::Continue
        });

        let hits = vec![ElementHit::new(0, 1, 0.0, Vec2::new(0.0, 0.0))];
        let mut event = InteractionEvent::new("click", Vec2::new(50.0, 50.0));
        mgr.dispatch(&mut event, &hits);

        // Only the selection-1 handler should have fired.
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn dispatch_respects_stop_propagation() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let counter = std::sync::Arc::new(AtomicU32::new(0));

        let mut mgr = EventManager::new();
        // First hit: selection 1 — stops propagation
        let c = counter.clone();
        mgr.register(1, "click", move |_| {
            c.fetch_add(1, Ordering::Relaxed);
            EventResult::StopPropagation
        });

        // Second hit: selection 2 — should NOT fire
        let c2 = counter.clone();
        mgr.register(2, "click", move |_| {
            c2.fetch_add(100, Ordering::Relaxed);
            EventResult::Continue
        });

        let hits = vec![
            ElementHit::new(0, 1, 0.0, Vec2::new(0.0, 0.0)),
            ElementHit::new(1, 2, 1.0, Vec2::new(0.0, 0.0)),
        ];
        let mut event = InteractionEvent::new("click", Vec2::new(50.0, 50.0));
        let result = mgr.dispatch(&mut event, &hits);

        assert_eq!(result, EventResult::StopPropagation);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn dispatch_selection_a_does_not_fire_for_selection_b() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let fired_a = std::sync::Arc::new(AtomicBool::new(false));
        let fired_b = std::sync::Arc::new(AtomicBool::new(false));

        let mut mgr = EventManager::new();
        let fa = fired_a.clone();
        mgr.register(10, "click", move |_| {
            fa.store(true, Ordering::Relaxed);
            EventResult::Continue
        });
        let fb = fired_b.clone();
        mgr.register(20, "click", move |_| {
            fb.store(true, Ordering::Relaxed);
            EventResult::Continue
        });

        // Hit resolves to selection 20 only.
        let hits = vec![ElementHit::new(0, 20, 0.0, Vec2::new(0.0, 0.0))];
        let mut event = InteractionEvent::new("click", Vec2::new(50.0, 50.0));
        mgr.dispatch(&mut event, &hits);

        assert!(
            !fired_a.load(Ordering::Relaxed),
            "handler A should not fire"
        );
        assert!(fired_b.load(Ordering::Relaxed), "handler B should fire");
    }

    #[test]
    fn global_handlers_receive_all_events() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let counter = std::sync::Arc::new(AtomicU32::new(0));

        let mut mgr = EventManager::new();
        let c = counter.clone();
        mgr.register_global("click", move |_| {
            c.fetch_add(1, Ordering::Relaxed);
            EventResult::Continue
        });

        // Dispatch with no hits — global handler should still fire.
        let mut event = InteractionEvent::new("click", Vec2::new(50.0, 50.0));
        mgr.dispatch(&mut event, &[]);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Dispatch with a hit — global handler should fire again.
        let hits = vec![ElementHit::new(0, 1, 0.0, Vec2::new(0.0, 0.0))];
        let mut event2 = InteractionEvent::new("click", Vec2::new(50.0, 50.0));
        mgr.dispatch(&mut event2, &hits);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn global_handlers_skipped_on_stop_propagation() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let global_fired = std::sync::Arc::new(AtomicBool::new(false));

        let mut mgr = EventManager::new();
        mgr.register(1, "click", |_| EventResult::StopPropagation);

        let gf = global_fired.clone();
        mgr.register_global("click", move |_| {
            gf.store(true, Ordering::Relaxed);
            EventResult::Continue
        });

        let hits = vec![ElementHit::new(0, 1, 0.0, Vec2::new(0.0, 0.0))];
        let mut event = InteractionEvent::new("click", Vec2::new(0.0, 0.0));
        mgr.dispatch(&mut event, &hits);

        assert!(
            !global_fired.load(Ordering::Relaxed),
            "global handler should be skipped when propagation stopped"
        );
    }

    #[test]
    fn multiple_handlers_same_event_invoked_in_order() {
        use std::sync::{Arc, Mutex};
        let order = Arc::new(Mutex::new(Vec::new()));

        let mut mgr = EventManager::new();
        let o1 = order.clone();
        mgr.register(1, "click", move |_| {
            o1.lock().unwrap().push(1);
            EventResult::Continue
        });
        let o2 = order.clone();
        mgr.register(1, "click", move |_| {
            o2.lock().unwrap().push(2);
            EventResult::Continue
        });
        let o3 = order.clone();
        mgr.register(1, "click", move |_| {
            o3.lock().unwrap().push(3);
            EventResult::Continue
        });

        let hits = vec![ElementHit::new(0, 1, 0.0, Vec2::new(0.0, 0.0))];
        let mut event = InteractionEvent::new("click", Vec2::new(0.0, 0.0));
        mgr.dispatch(&mut event, &hits);

        assert_eq!(*order.lock().unwrap(), vec![1, 2, 3]);
    }

    // -- RawInputEvent -----------------------------------------------------

    #[test]
    fn raw_input_to_interaction_event_without_transform() {
        let raw = RawInputEvent::new(EventType::MouseDown, Vec2::new(100.0, 200.0));
        let ie = raw.into_interaction_event(None);
        assert_eq!(ie.interaction_type, "mousedown");
        assert_eq!(ie.screen_position, Vec2::new(100.0, 200.0));
        assert!(ie.world_position.is_none());
    }

    #[test]
    fn raw_input_to_interaction_event_with_transform() {
        let vt = ViewportTransform {
            offset: Vec2::new(10.0, 20.0),
            scale: Vec2::new(2.0, 2.0),
        };
        let raw = RawInputEvent::new(EventType::MouseDown, Vec2::new(110.0, 220.0));
        let ie = raw.into_interaction_event(Some(&vt));
        assert_eq!(ie.world_position, Some(Vec2::new(50.0, 100.0)));
    }

    #[test]
    fn raw_input_modifiers_propagated() {
        let mods = ModifierFlags {
            shift: true,
            ctrl: false,
            alt: true,
            meta: false,
        };
        let raw = RawInputEvent::new(EventType::MouseUp, Vec2::new(0.0, 0.0)).with_modifiers(mods);
        let ie = raw.into_interaction_event(None);
        assert!(ie.modifiers.shift);
        assert!(ie.modifiers.alt);
        assert!(!ie.modifiers.ctrl);
    }

    // -- Performance: dispatch is allocation-free on hot path ---------------

    #[test]
    fn dispatch_performance_baseline() {
        // Verify that dispatch of 50 handlers across 100 hits completes
        // well under 16ms (the frame budget).
        use std::sync::atomic::{AtomicU32, Ordering};
        let counter = std::sync::Arc::new(AtomicU32::new(0));

        let mut mgr = EventManager::new();
        for sel_id in 0..10 {
            for _ in 0..5 {
                let c = counter.clone();
                mgr.register(sel_id, "mousemove", move |_| {
                    c.fetch_add(1, Ordering::Relaxed);
                    EventResult::Continue
                });
            }
        }
        assert_eq!(mgr.selection_handler_count(), 50);

        // 100 hits spread across 10 selections.
        let hits: Vec<ElementHit> = (0..100)
            .map(|i| ElementHit::new(i, i % 10, i as f32, Vec2::new(0.0, 0.0)))
            .collect();

        let start = Instant::now();
        let mut event = InteractionEvent::new("mousemove", Vec2::new(0.0, 0.0));
        mgr.dispatch(&mut event, &hits);
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 16,
            "dispatch took {elapsed:?}, exceeding 16ms budget"
        );
    }
}
