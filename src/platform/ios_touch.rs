// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! UITouch → [`TouchEvent`] translation logic.
//!
//! This module is **not** target-gated to `ios` so that the pure translation
//! functions can be unit-tested on any host platform.  The `ios-shim` feature
//! flag is still required.
//!
//! The companion [`super::ios`] module (which *is* iOS-only) re-exports these
//! types and adds the Metal surface / orientation helpers.

use crate::mark_selection::{TouchEvent, TouchPhase};

// ---------------------------------------------------------------------------
// C-ABI–compatible touch descriptor
// ---------------------------------------------------------------------------

/// Describes a single UITouch contact as received across the C ABI.
///
/// The Swift wrapper converts each `UITouch` into this struct before calling
/// into Rust.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawIosTouch {
    /// Unique `hash` of the `UITouch` object (stable across phases).
    pub touch_id: u64,
    /// X position in the view's coordinate space (points).
    pub x: f32,
    /// Y position in the view's coordinate space (points).
    pub y: f32,
    /// Touch phase: 0 = Began, 1 = Moved, 2 = Ended, 3 = Cancelled.
    pub phase: u8,
    /// `UIScreen.scale` of the screen hosting the view.
    pub scale_factor: f32,
    /// Timestamp of the touch event (seconds, `ProcessInfo.systemUptime`
    /// epoch).
    pub timestamp: f64,
}

impl RawIosTouch {
    /// Convenience constructor for tests and FFI bridges.
    pub fn new(
        touch_id: u64,
        x: f32,
        y: f32,
        phase: u8,
        scale_factor: f32,
        timestamp: f64,
    ) -> Self {
        Self {
            touch_id,
            x,
            y,
            phase,
            scale_factor,
            timestamp,
        }
    }
}

// ---------------------------------------------------------------------------
// Translation
// ---------------------------------------------------------------------------

/// Map the C-level phase byte to a [`TouchPhase`].
fn phase_from_u8(raw: u8) -> TouchPhase {
    match raw {
        0 => TouchPhase::Started,
        1 => TouchPhase::Moved,
        2 => TouchPhase::Ended,
        3 => TouchPhase::Cancelled,
        _ => TouchPhase::Cancelled, // defensive
    }
}

/// Translate a batch of iOS `UITouch` contacts into Gup [`TouchEvent`]s.
///
/// Positions are converted from UIKit *points* to physical *pixels* by
/// multiplying by each touch's `scale_factor`, matching the coordinate space
/// that `GupContext` and its surface use.
///
/// If `view_bounds` is `Some((vw, vh))` (size in *points*), positions are
/// clamped to `[0, vw * scale)` × `[0, vh * scale)` so that touches at the
/// very edge of the view do not exceed the drawable size.
///
/// # Arguments
///
/// * `touches`     – Slice of raw touch contacts from UIKit.
/// * `view_bounds` – Optional view size in *points* (width, height).
pub fn translate_uitouch(
    touches: &[RawIosTouch],
    view_bounds: Option<(f32, f32)>,
) -> Vec<TouchEvent> {
    touches
        .iter()
        .map(|t| {
            let phase = phase_from_u8(t.phase);

            let mut px = t.x * t.scale_factor;
            let mut py = t.y * t.scale_factor;

            if let Some((vw, vh)) = view_bounds {
                let max_x = vw * t.scale_factor;
                let max_y = vh * t.scale_factor;
                px = px.clamp(0.0, max_x);
                py = py.clamp(0.0, max_y);
            }

            TouchEvent {
                id: t.touch_id,
                position: [px, py],
                phase,
                timestamp: t.timestamp,
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
    fn touch(id: u64, x: f32, y: f32, phase: u8, scale: f32, ts: f64) -> RawIosTouch {
        RawIosTouch::new(id, x, y, phase, scale, ts)
    }

    // -- Single tap -----------------------------------------------------------

    #[test]
    fn single_tap_translates_correctly() {
        let touches = [
            touch(1, 100.0, 200.0, 0, 2.0, 0.0), // Began
            touch(1, 100.0, 200.0, 2, 2.0, 0.05), // Ended
        ];
        let events = translate_uitouch(&touches, None);

        assert_eq!(events.len(), 2);

        // Began → Started
        assert_eq!(events[0].id, 1);
        assert_eq!(events[0].position, [200.0, 400.0]); // points * scale
        assert!(matches!(events[0].phase, TouchPhase::Started));

        // Ended
        assert_eq!(events[1].id, 1);
        assert!(matches!(events[1].phase, TouchPhase::Ended));
        assert!((events[1].timestamp - 0.05).abs() < f64::EPSILON);
    }

    // -- Two-finger pinch -----------------------------------------------------

    #[test]
    fn two_finger_pinch_preserves_ids() {
        let touches = [
            touch(1, 50.0, 100.0, 0, 3.0, 0.0),
            touch(2, 150.0, 100.0, 0, 3.0, 0.0),
            touch(1, 40.0, 100.0, 1, 3.0, 0.02),
            touch(2, 160.0, 100.0, 1, 3.0, 0.02),
            touch(1, 40.0, 100.0, 2, 3.0, 0.10),
            touch(2, 160.0, 100.0, 2, 3.0, 0.10),
        ];
        let events = translate_uitouch(&touches, None);

        assert_eq!(events.len(), 6);
        // All finger-1 events share id 1
        assert!(events.iter().step_by(2).all(|e| e.id == 1));
        // All finger-2 events share id 2
        assert!(events.iter().skip(1).step_by(2).all(|e| e.id == 2));
    }

    // -- Rapid phase transitions ----------------------------------------------

    #[test]
    fn rapid_phase_transitions() {
        let touches = [
            touch(5, 10.0, 20.0, 0, 2.0, 0.0),
            touch(5, 11.0, 21.0, 1, 2.0, 0.001),
            touch(5, 12.0, 22.0, 1, 2.0, 0.002),
            touch(5, 12.0, 22.0, 2, 2.0, 0.003),
        ];
        let events = translate_uitouch(&touches, None);
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0].phase, TouchPhase::Started));
        assert!(matches!(events[1].phase, TouchPhase::Moved));
        assert!(matches!(events[2].phase, TouchPhase::Moved));
        assert!(matches!(events[3].phase, TouchPhase::Ended));
    }

    // -- Cancelled touches ----------------------------------------------------

    #[test]
    fn cancelled_touch_maps_correctly() {
        let touches = [
            touch(3, 50.0, 50.0, 0, 2.0, 0.0),
            touch(3, 50.0, 50.0, 3, 2.0, 0.1), // Cancelled
        ];
        let events = translate_uitouch(&touches, None);
        assert!(matches!(events[1].phase, TouchPhase::Cancelled));
    }

    // -- Unknown phase (defensive) --------------------------------------------

    #[test]
    fn unknown_phase_maps_to_cancelled() {
        let touches = [touch(1, 0.0, 0.0, 99, 1.0, 0.0)];
        let events = translate_uitouch(&touches, None);
        assert!(matches!(events[0].phase, TouchPhase::Cancelled));
    }

    // -- Scale factor 1.0 (non-Retina) ----------------------------------------

    #[test]
    fn scale_factor_one_passes_through() {
        let touches = [touch(1, 75.5, 120.3, 0, 1.0, 0.0)];
        let events = translate_uitouch(&touches, None);
        assert!((events[0].position[0] - 75.5).abs() < f32::EPSILON);
        assert!((events[0].position[1] - 120.3).abs() < f32::EPSILON);
    }

    // -- Scale factor 3.0 (iPhone Plus / Pro Max) -----------------------------

    #[test]
    fn scale_factor_three() {
        let touches = [touch(1, 100.0, 200.0, 0, 3.0, 0.0)];
        let events = translate_uitouch(&touches, None);
        assert!((events[0].position[0] - 300.0).abs() < f32::EPSILON);
        assert!((events[0].position[1] - 600.0).abs() < f32::EPSILON);
    }

    // -- View bounds clamping -------------------------------------------------

    #[test]
    fn view_bounds_clamp_position() {
        // View is 200×400 points, scale 2.0 → pixel range [0..400] × [0..800]
        let touches = [
            touch(1, 250.0, 500.0, 0, 2.0, 0.0),  // outside right+bottom
            touch(2, -10.0, -5.0, 0, 2.0, 0.0),    // outside left+top
        ];
        let events = translate_uitouch(&touches, Some((200.0, 400.0)));

        // Clamped to max
        assert!((events[0].position[0] - 400.0).abs() < f32::EPSILON);
        assert!((events[0].position[1] - 800.0).abs() < f32::EPSILON);
        // Clamped to 0
        assert!((events[1].position[0]).abs() < f32::EPSILON);
        assert!((events[1].position[1]).abs() < f32::EPSILON);
    }

    // -- Empty input ----------------------------------------------------------

    #[test]
    fn empty_input_returns_empty() {
        let events = translate_uitouch(&[], None);
        assert!(events.is_empty());
    }

    // -- Timestamp passthrough ------------------------------------------------

    #[test]
    fn timestamps_are_preserved() {
        let touches = [
            touch(1, 0.0, 0.0, 0, 1.0, 1234.567),
            touch(1, 0.0, 0.0, 2, 1.0, 1234.890),
        ];
        let events = translate_uitouch(&touches, None);
        assert!((events[0].timestamp - 1234.567).abs() < f64::EPSILON);
        assert!((events[1].timestamp - 1234.890).abs() < f64::EPSILON);
    }
}
