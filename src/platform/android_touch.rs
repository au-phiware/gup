// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Android `MotionEvent` → [`TouchEvent`] translation logic.
//!
//! This module is **not** target-gated to `android` so that the pure
//! translation functions can be unit-tested on any host platform.  The
//! `android-shim` feature flag is still required.
//!
//! The companion [`super::android`] module (which *is* Android-only)
//! re-exports these types and adds the `ANativeWindow` surface helpers.

use crate::mark_selection::{TouchEvent, TouchPhase};

// ---------------------------------------------------------------------------
// C-ABI–compatible touch descriptor
// ---------------------------------------------------------------------------

/// Describes a single pointer from an Android `MotionEvent` as received
/// across the JNI / NDK bridge.
///
/// The Kotlin wrapper extracts one [`RawAndroidTouch`] per active pointer
/// and passes it into Rust.
///
/// # Field semantics
///
/// | Field          | Android source                              |
/// |----------------|---------------------------------------------|
/// | `pointer_id`   | `MotionEvent.getPointerId(index)`           |
/// | `x`            | `MotionEvent.getX(index)` (display pixels)  |
/// | `y`            | `MotionEvent.getY(index)` (display pixels)  |
/// | `action`       | Masked action — see [`action_to_phase`]     |
/// | `density`      | `DisplayMetrics.density`                    |
/// | `event_time`   | `MotionEvent.getEventTime()` (milliseconds) |
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawAndroidTouch {
    /// Stable pointer ID across move events (from `getPointerId`).
    pub pointer_id: u64,
    /// X position in *display pixels* (not density-independent pixels).
    pub x: f32,
    /// Y position in *display pixels*.
    pub y: f32,
    /// Masked `MotionEvent` action code:
    ///   0 = `ACTION_DOWN` / `ACTION_POINTER_DOWN`,
    ///   1 = `ACTION_UP` / `ACTION_POINTER_UP`,
    ///   2 = `ACTION_MOVE`,
    ///   3 = `ACTION_CANCEL`.
    pub action: u8,
    /// `DisplayMetrics.density` of the hosting display.
    pub density: f32,
    /// Event timestamp in milliseconds (from `MotionEvent.getEventTime()`).
    pub event_time_ms: f64,
}

impl RawAndroidTouch {
    /// Convenience constructor for tests and FFI bridges.
    pub fn new(
        pointer_id: u64,
        x: f32,
        y: f32,
        action: u8,
        density: f32,
        event_time_ms: f64,
    ) -> Self {
        Self {
            pointer_id,
            x,
            y,
            action,
            density,
            event_time_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// Translation
// ---------------------------------------------------------------------------

/// Map a masked Android `MotionEvent` action code to a [`TouchPhase`].
///
/// | Code | Android constant              | Gup phase           |
/// |------|-------------------------------|----------------------|
/// | 0    | `ACTION_DOWN` / `POINTER_DOWN`| `TouchPhase::Started`|
/// | 1    | `ACTION_UP` / `POINTER_UP`    | `TouchPhase::Ended`  |
/// | 2    | `ACTION_MOVE`                 | `TouchPhase::Moved`  |
/// | 3    | `ACTION_CANCEL`               | `TouchPhase::Cancelled` |
/// | _    | (unknown)                     | `TouchPhase::Cancelled` |
fn action_to_phase(action: u8) -> TouchPhase {
    match action {
        0 => TouchPhase::Started,   // ACTION_DOWN / ACTION_POINTER_DOWN
        1 => TouchPhase::Ended,     // ACTION_UP / ACTION_POINTER_UP
        2 => TouchPhase::Moved,     // ACTION_MOVE
        3 => TouchPhase::Cancelled, // ACTION_CANCEL
        _ => TouchPhase::Cancelled, // defensive
    }
}

/// Translate a batch of Android `MotionEvent` pointers into Gup
/// [`TouchEvent`]s.
///
/// Positions in Android are already in *display pixels* (not dp).  To
/// convert to Gup's logical coordinate space we divide by `density`
/// (the inverse of the iOS convention which multiplies by scale).  This
/// ensures Gup always works in density-independent coordinates.
///
/// If `view_bounds` is `Some((vw, vh))` (size in *display pixels*),
/// positions are clamped to `[0, vw / density)` × `[0, vh / density)` so
/// that touches at the very edge of the view do not exceed the logical
/// surface size.
///
/// Timestamps are converted from milliseconds to seconds to match the
/// [`TouchEvent::timestamp`] convention used by the iOS bridge.
///
/// # Arguments
///
/// * `touches`     – Slice of raw pointer contacts from the Android
///                   `MotionEvent`.
/// * `view_bounds` – Optional view size in *display pixels*
///                   (width, height).
pub fn translate_motion_event(
    touches: &[RawAndroidTouch],
    view_bounds: Option<(f32, f32)>,
) -> Vec<TouchEvent> {
    touches
        .iter()
        .map(|t| {
            let phase = action_to_phase(t.action);

            // Android reports in display pixels; convert to logical (dp)
            // coordinates by dividing by density.
            let density = if t.density > 0.0 { t.density } else { 1.0 };
            let mut lx = t.x / density;
            let mut ly = t.y / density;

            if let Some((vw, vh)) = view_bounds {
                let max_x = vw / density;
                let max_y = vh / density;
                lx = lx.clamp(0.0, max_x);
                ly = ly.clamp(0.0, max_y);
            }

            // Convert milliseconds → seconds.
            let timestamp = t.event_time_ms / 1000.0;

            TouchEvent {
                id: t.pointer_id,
                position: [lx, ly],
                phase,
                timestamp,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a single touch.
    fn touch(id: u64, x: f32, y: f32, action: u8, density: f32, ts_ms: f64) -> RawAndroidTouch {
        RawAndroidTouch::new(id, x, y, action, density, ts_ms)
    }

    // -- Single tap -----------------------------------------------------------

    #[test]
    fn single_tap_translates_correctly() {
        let touches = [
            touch(0, 200.0, 400.0, 0, 2.0, 1000.0), // ACTION_DOWN
            touch(0, 200.0, 400.0, 1, 2.0, 1050.0), // ACTION_UP
        ];
        let events = translate_motion_event(&touches, None);

        assert_eq!(events.len(), 2);

        // DOWN → Started, coordinates divided by density
        assert_eq!(events[0].id, 0);
        assert!((events[0].position[0] - 100.0).abs() < f32::EPSILON);
        assert!((events[0].position[1] - 200.0).abs() < f32::EPSILON);
        assert!(matches!(events[0].phase, TouchPhase::Started));
        assert!((events[0].timestamp - 1.0).abs() < f64::EPSILON);

        // UP → Ended
        assert_eq!(events[1].id, 0);
        assert!(matches!(events[1].phase, TouchPhase::Ended));
        assert!((events[1].timestamp - 1.05).abs() < f64::EPSILON);
    }

    // -- Two-finger pinch (multi-touch) ---------------------------------------

    #[test]
    fn two_finger_pinch_preserves_ids() {
        let touches = [
            touch(0, 150.0, 300.0, 0, 3.0, 0.0),   // finger 0 DOWN
            touch(1, 450.0, 300.0, 0, 3.0, 0.0),   // finger 1 DOWN
            touch(0, 120.0, 300.0, 2, 3.0, 20.0),  // finger 0 MOVE
            touch(1, 480.0, 300.0, 2, 3.0, 20.0),  // finger 1 MOVE
            touch(0, 120.0, 300.0, 1, 3.0, 100.0), // finger 0 UP
            touch(1, 480.0, 300.0, 1, 3.0, 100.0), // finger 1 UP
        ];
        let events = translate_motion_event(&touches, None);

        assert_eq!(events.len(), 6);
        // All finger-0 events share id 0
        assert!(events.iter().step_by(2).all(|e| e.id == 0));
        // All finger-1 events share id 1
        assert!(events.iter().skip(1).step_by(2).all(|e| e.id == 1));
    }

    // -- Five simultaneous pointers -------------------------------------------

    #[test]
    fn five_simultaneous_pointers() {
        let touches: Vec<_> = (0..5)
            .map(|i| touch(i as u64, (i as f32) * 100.0, 50.0, 0, 2.0, 0.0))
            .collect();
        let events = translate_motion_event(&touches, None);

        assert_eq!(events.len(), 5);
        for (i, ev) in events.iter().enumerate() {
            assert_eq!(ev.id, i as u64);
            assert!(matches!(ev.phase, TouchPhase::Started));
        }
    }

    // -- Rapid phase transitions ----------------------------------------------

    #[test]
    fn rapid_phase_transitions() {
        let touches = [
            touch(5, 20.0, 40.0, 0, 2.0, 0.0), // DOWN
            touch(5, 22.0, 42.0, 2, 2.0, 1.0), // MOVE
            touch(5, 24.0, 44.0, 2, 2.0, 2.0), // MOVE
            touch(5, 24.0, 44.0, 1, 2.0, 3.0), // UP
        ];
        let events = translate_motion_event(&touches, None);

        assert_eq!(events.len(), 4);
        assert!(matches!(events[0].phase, TouchPhase::Started));
        assert!(matches!(events[1].phase, TouchPhase::Moved));
        assert!(matches!(events[2].phase, TouchPhase::Moved));
        assert!(matches!(events[3].phase, TouchPhase::Ended));
    }

    // -- ACTION_CANCEL --------------------------------------------------------

    #[test]
    fn cancel_maps_correctly() {
        let touches = [
            touch(3, 100.0, 100.0, 0, 2.0, 0.0),   // DOWN
            touch(3, 100.0, 100.0, 3, 2.0, 100.0), // CANCEL
        ];
        let events = translate_motion_event(&touches, None);
        assert!(matches!(events[1].phase, TouchPhase::Cancelled));
    }

    // -- Unknown action (defensive) -------------------------------------------

    #[test]
    fn unknown_action_maps_to_cancelled() {
        let touches = [touch(1, 0.0, 0.0, 99, 1.0, 0.0)];
        let events = translate_motion_event(&touches, None);
        assert!(matches!(events[0].phase, TouchPhase::Cancelled));
    }

    // -- Density 1.0 (mdpi) --------------------------------------------------

    #[test]
    fn density_one_passes_through() {
        let touches = [touch(1, 75.5, 120.3, 0, 1.0, 0.0)];
        let events = translate_motion_event(&touches, None);
        assert!((events[0].position[0] - 75.5).abs() < f32::EPSILON);
        assert!((events[0].position[1] - 120.3).abs() < f32::EPSILON);
    }

    // -- Density 3.0 (xxhdpi) ------------------------------------------------

    #[test]
    fn density_three_divides_correctly() {
        let touches = [touch(1, 300.0, 600.0, 0, 3.0, 0.0)];
        let events = translate_motion_event(&touches, None);
        assert!((events[0].position[0] - 100.0).abs() < f32::EPSILON);
        assert!((events[0].position[1] - 200.0).abs() < f32::EPSILON);
    }

    // -- Zero/negative density (defensive) ------------------------------------

    #[test]
    fn zero_density_falls_back_to_one() {
        let touches = [touch(1, 150.0, 250.0, 0, 0.0, 0.0)];
        let events = translate_motion_event(&touches, None);
        // density clamped to 1.0, so coordinates pass through
        assert!((events[0].position[0] - 150.0).abs() < f32::EPSILON);
        assert!((events[0].position[1] - 250.0).abs() < f32::EPSILON);
    }

    // -- View bounds clamping -------------------------------------------------

    #[test]
    fn view_bounds_clamp_position() {
        // View is 400×800 display pixels, density 2.0
        // → logical range [0..200] × [0..400]
        let touches = [
            touch(1, 500.0, 1000.0, 0, 2.0, 0.0), // outside right+bottom
            touch(2, -20.0, -10.0, 0, 2.0, 0.0),  // outside left+top
        ];
        let events = translate_motion_event(&touches, Some((400.0, 800.0)));

        // Clamped to max logical
        assert!((events[0].position[0] - 200.0).abs() < f32::EPSILON);
        assert!((events[0].position[1] - 400.0).abs() < f32::EPSILON);
        // Clamped to 0
        assert!((events[1].position[0]).abs() < f32::EPSILON);
        assert!((events[1].position[1]).abs() < f32::EPSILON);
    }

    // -- Empty input ----------------------------------------------------------

    #[test]
    fn empty_input_returns_empty() {
        let events = translate_motion_event(&[], None);
        assert!(events.is_empty());
    }

    // -- Timestamp conversion (ms → s) ----------------------------------------

    #[test]
    fn timestamps_converted_to_seconds() {
        let touches = [
            touch(1, 0.0, 0.0, 0, 1.0, 1_234_567.0),
            touch(1, 0.0, 0.0, 1, 1.0, 1_234_890.0),
        ];
        let events = translate_motion_event(&touches, None);
        assert!((events[0].timestamp - 1234.567).abs() < f64::EPSILON);
        assert!((events[1].timestamp - 1234.890).abs() < f64::EPSILON);
    }
}
