// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive hover reveal system for truncated/clipped text.
//!
//! When text is truncated with ellipsis or hidden by clipping strategies, this
//! module provides hover-based reveal of the full original text content via
//! tooltips.
//!
//! # Architecture
//!
//! The system has three main components:
//!
//! 1. **[`ClippedTextRegistry`]** — Tracks clipped text regions and their
//!    original (un-truncated) content. Text is registered during layout when
//!    [`ClippingStrategyConfig::enable_hover_reveal`] is `true`.
//!
//! 2. **[`HoverRevealState`]** — Manages the current tooltip display state
//!    including smooth opacity transitions (fade-in/fade-out).
//!
//! 3. **[`TooltipConfig`]** — Configures tooltip appearance (colors, padding,
//!    font size, transition timing).
//!
//! # Usage
//!
//! ```rust,ignore
//! use gup::text::hover_reveal::{ClippedTextRegistry, HoverRevealState, TooltipConfig};
//!
//! let mut registry = ClippedTextRegistry::new();
//! let mut hover_state = HoverRevealState::new(TooltipConfig::default());
//!
//! // During layout: register clipped text
//! registry.register(bounds, "Original long text...");
//!
//! // Each frame: update with mouse position
//! hover_state.update(&registry, mouse_x, mouse_y, delta_time);
//!
//! // During rendering: get active tooltip for drawing
//! if let Some(tooltip) = hover_state.active_tooltip() {
//!     // Render tooltip text at tooltip.position with tooltip.opacity
//! }
//! ```

use super::TextBounds;
use crate::shader_function::Vec2;

/// Entry in the clipped text registry tracking a single truncated text element.
#[derive(Debug, Clone)]
pub struct ClippedTextEntry {
    /// Bounding rectangle of the rendered (truncated) text.
    pub rendered_bounds: TextBounds,
    /// The original, un-truncated text content.
    pub original_text: String,
}

/// Registry of clipped/truncated text elements for hover reveal.
///
/// Call [`clear`](Self::clear) at the start of each frame (before layout),
/// then [`register`](Self::register) for every text element that was clipped.
/// During the event/update phase, call [`hit_test`](Self::hit_test) to find
/// which entry (if any) the cursor is over.
#[derive(Debug, Clone, Default)]
pub struct ClippedTextRegistry {
    entries: Vec<ClippedTextEntry>,
}

impl ClippedTextRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Remove all entries. Call at the start of each frame before re-laying
    /// out text.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Register a clipped text element.
    ///
    /// * `rendered_bounds` — the bounding box of the *displayed* (truncated)
    ///   text.
    /// * `original_text` — the full, un-truncated text string.
    pub fn register(&mut self, rendered_bounds: TextBounds, original_text: &str) {
        self.entries.push(ClippedTextEntry {
            rendered_bounds,
            original_text: original_text.to_string(),
        });
    }

    /// Test whether a screen-space point hits any registered clipped text.
    ///
    /// Returns the first matching entry (front-to-back, most recently
    /// registered wins for overlapping entries).
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&ClippedTextEntry> {
        // Iterate in reverse so the most recently registered (top-most) entry
        // takes priority when entries overlap.
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.rendered_bounds.contains_point(x, y))
    }

    /// Return the number of registered entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all registered entries.
    pub fn entries(&self) -> &[ClippedTextEntry] {
        &self.entries
    }
}

/// Configuration for tooltip appearance and behaviour.
#[derive(Debug, Clone)]
pub struct TooltipConfig {
    /// Horizontal padding inside the tooltip box (pixels).
    pub padding_x: f32,
    /// Vertical padding inside the tooltip box (pixels).
    pub padding_y: f32,
    /// Background colour (RGBA, 0.0–1.0).
    pub background_color: [f32; 4],
    /// Text colour (RGBA, 0.0–1.0).
    pub text_color: [f32; 4],
    /// Border colour (RGBA, 0.0–1.0).
    pub border_color: [f32; 4],
    /// Border width in pixels.
    pub border_width: f32,
    /// Font size for tooltip text.
    pub font_size: f32,
    /// Vertical offset from the hovered text (pixels). Positive = below.
    pub offset_y: f32,
    /// Duration of the fade-in transition (seconds).
    pub fade_in_duration: f32,
    /// Duration of the fade-out transition (seconds).
    pub fade_out_duration: f32,
    /// Delay before the tooltip appears (seconds). Prevents flicker on fast
    /// mouse movement.
    pub show_delay: f32,
    /// Maximum tooltip width before wrapping (pixels). 0 = no limit.
    pub max_width: f32,
}

impl Default for TooltipConfig {
    fn default() -> Self {
        Self {
            padding_x: 6.0,
            padding_y: 4.0,
            background_color: [0.15, 0.15, 0.15, 0.92],
            text_color: [1.0, 1.0, 1.0, 1.0],
            border_color: [0.4, 0.4, 0.4, 1.0],
            border_width: 1.0,
            font_size: 14.0,
            offset_y: 4.0,
            fade_in_duration: 0.15,
            fade_out_duration: 0.1,
            show_delay: 0.3,
            max_width: 300.0,
        }
    }
}

/// Represents an active tooltip ready for rendering.
#[derive(Debug, Clone)]
pub struct ActiveTooltip {
    /// The full original text to display.
    pub text: String,
    /// Screen-space position for the tooltip (top-left corner).
    pub position: Vec2,
    /// Current opacity (0.0–1.0) for smooth transitions.
    pub opacity: f32,
    /// Bounding box of the source (hovered) text element.
    pub source_bounds: TextBounds,
}

/// Internal phase of the tooltip visibility state machine.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TooltipPhase {
    /// No tooltip active.
    Hidden,
    /// Cursor is over clipped text but the show-delay hasn't elapsed yet.
    Waiting,
    /// Tooltip is fading in.
    FadingIn,
    /// Tooltip is fully visible.
    Visible,
    /// Tooltip is fading out (cursor left the text).
    FadingOut,
}

/// Manages hover reveal state including smooth transitions.
///
/// Maintains a state machine that tracks whether a tooltip should be shown,
/// handles show-delay, fade-in, and fade-out.
#[derive(Debug, Clone)]
pub struct HoverRevealState {
    config: TooltipConfig,
    phase: TooltipPhase,
    /// Elapsed time in the current phase (seconds).
    phase_time: f32,
    /// Current opacity (0.0–1.0).
    opacity: f32,
    /// Text and bounds of the currently targeted clipped entry.
    current_text: Option<String>,
    current_source_bounds: Option<TextBounds>,
    /// Screen position where the tooltip should appear.
    tooltip_position: Vec2,
}

impl HoverRevealState {
    /// Create a new hover reveal state with the given configuration.
    pub fn new(config: TooltipConfig) -> Self {
        Self {
            config,
            phase: TooltipPhase::Hidden,
            phase_time: 0.0,
            opacity: 0.0,
            current_text: None,
            current_source_bounds: None,
            tooltip_position: Vec2 { x: 0.0, y: 0.0 },
        }
    }

    /// Get a reference to the tooltip configuration.
    pub fn config(&self) -> &TooltipConfig {
        &self.config
    }

    /// Get a mutable reference to the tooltip configuration.
    pub fn config_mut(&mut self) -> &mut TooltipConfig {
        &mut self.config
    }

    /// Update the hover state for the current frame.
    ///
    /// * `registry` — the clipped text registry to query.
    /// * `mouse_x`, `mouse_y` — current cursor position in screen space.
    /// * `dt` — time elapsed since last frame (seconds).
    pub fn update(&mut self, registry: &ClippedTextRegistry, mouse_x: f32, mouse_y: f32, dt: f32) {
        let hit = registry.hit_test(mouse_x, mouse_y);

        // Loop to allow zero-duration transitions to chain in a single frame
        // (e.g. Hidden → Waiting → FadingIn → Visible when all durations are 0).
        // The `settled` flag prevents infinite loops: once a phase processes
        // without transitioning, we stop.
        loop {
            let prev_phase = self.phase;

            match (&self.phase, &hit) {
                // ── Hidden + no hit = stay hidden (no-op) ──
                (TooltipPhase::Hidden, None) => {}

                // ── Hidden → Waiting (cursor entered clipped text) ──
                (TooltipPhase::Hidden, Some(entry)) => {
                    self.begin_target(entry, mouse_x);
                    self.phase = TooltipPhase::Waiting;
                    self.phase_time = 0.0;
                }

                // ── Waiting: accumulate delay ──
                (TooltipPhase::Waiting, Some(entry)) => {
                    // If the cursor moved to a different entry, restart the delay.
                    if self.target_changed(entry) {
                        self.begin_target(entry, mouse_x);
                        self.phase_time = 0.0;
                    } else {
                        self.phase_time += dt;
                        if self.phase_time >= self.config.show_delay {
                            self.phase = TooltipPhase::FadingIn;
                            self.phase_time = 0.0;
                        }
                    }
                }

                // ── Waiting → Hidden (cursor left before delay elapsed) ──
                (TooltipPhase::Waiting, None) => {
                    self.phase = TooltipPhase::Hidden;
                    self.phase_time = 0.0;
                    self.current_text = None;
                    self.current_source_bounds = None;
                }

                // ── FadingIn: increase opacity ──
                (TooltipPhase::FadingIn, Some(entry)) => {
                    if self.target_changed(entry) {
                        // Switched target while fading in — restart
                        self.begin_target(entry, mouse_x);
                        self.phase_time = 0.0;
                        self.opacity = 0.0;
                    } else {
                        self.phase_time += dt;
                        if self.config.fade_in_duration > 0.0 {
                            self.opacity =
                                (self.phase_time / self.config.fade_in_duration).clamp(0.0, 1.0);
                        } else {
                            self.opacity = 1.0;
                        }
                        if self.opacity >= 1.0 {
                            self.phase = TooltipPhase::Visible;
                            self.opacity = 1.0;
                        }
                    }
                }

                // ── FadingIn → FadingOut (cursor left during fade-in) ──
                (TooltipPhase::FadingIn, None) => {
                    self.phase = TooltipPhase::FadingOut;
                    self.phase_time = 0.0;
                }

                // ── Visible: tooltip fully shown ──
                (TooltipPhase::Visible, Some(entry)) => {
                    if self.target_changed(entry) {
                        // Seamlessly switch to new target (stay visible)
                        self.begin_target(entry, mouse_x);
                    }
                }

                // ── Visible → FadingOut (cursor left) ──
                (TooltipPhase::Visible, None) => {
                    self.phase = TooltipPhase::FadingOut;
                    self.phase_time = 0.0;
                }

                // ── FadingOut: decrease opacity ──
                (TooltipPhase::FadingOut, None) => {
                    self.phase_time += dt;
                    if self.config.fade_out_duration > 0.0 {
                        self.opacity =
                            1.0 - (self.phase_time / self.config.fade_out_duration).clamp(0.0, 1.0);
                    } else {
                        self.opacity = 0.0;
                    }
                    if self.opacity <= 0.0 {
                        self.phase = TooltipPhase::Hidden;
                        self.opacity = 0.0;
                        self.current_text = None;
                        self.current_source_bounds = None;
                    }
                }

                // ── FadingOut → FadingIn (cursor re-entered before fully hidden) ──
                (TooltipPhase::FadingOut, Some(entry)) => {
                    self.begin_target(entry, mouse_x);
                    self.phase = TooltipPhase::FadingIn;
                    self.phase_time = self.opacity * self.config.fade_in_duration;
                }
            }

            // If phase didn't change, we've settled — stop iterating.
            if self.phase == prev_phase {
                break;
            }
        }
    }

    /// Return the currently active tooltip (if any) for rendering.
    ///
    /// Returns `None` when the tooltip is hidden or has zero opacity.
    pub fn active_tooltip(&self) -> Option<ActiveTooltip> {
        if self.opacity <= 0.0 || self.phase == TooltipPhase::Hidden {
            return None;
        }

        let text = self.current_text.as_ref()?;
        let source_bounds = self.current_source_bounds?;

        Some(ActiveTooltip {
            text: text.clone(),
            position: self.tooltip_position,
            opacity: self.opacity,
            source_bounds,
        })
    }

    /// Check whether a tooltip is currently being displayed (any phase except
    /// Hidden).
    pub fn is_active(&self) -> bool {
        self.phase != TooltipPhase::Hidden
    }

    /// Return the current opacity (0.0–1.0).
    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    /// Force-hide the tooltip immediately (no fade-out).
    pub fn hide(&mut self) {
        self.phase = TooltipPhase::Hidden;
        self.phase_time = 0.0;
        self.opacity = 0.0;
        self.current_text = None;
        self.current_source_bounds = None;
    }

    // ── helpers ──────────────────────────────────────────────────────────

    /// Set the current target entry and compute the tooltip position.
    fn begin_target(&mut self, entry: &ClippedTextEntry, mouse_x: f32) {
        self.current_text = Some(entry.original_text.clone());
        self.current_source_bounds = Some(entry.rendered_bounds);
        // Position tooltip centred horizontally on the mouse, below the source.
        self.tooltip_position = Vec2 {
            x: mouse_x,
            y: entry.rendered_bounds.bottom + self.config.offset_y,
        };
    }

    /// Check whether the given entry is a different target than the current one.
    fn target_changed(&self, entry: &ClippedTextEntry) -> bool {
        match &self.current_text {
            Some(current) => *current != entry.original_text,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bounds(left: f32, top: f32, right: f32, bottom: f32) -> TextBounds {
        TextBounds::new(left, top, right, bottom)
    }

    // ── ClippedTextRegistry tests ────────────────────────────────────────

    #[test]
    fn registry_starts_empty() {
        let reg = ClippedTextRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn registry_register_and_len() {
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "Hello World");
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
    }

    #[test]
    fn registry_clear() {
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "text");
        reg.clear();
        assert!(reg.is_empty());
    }

    #[test]
    fn registry_hit_test_inside() {
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(10.0, 10.0, 60.0, 30.0), "Full label text");
        let hit = reg.hit_test(30.0, 20.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().original_text, "Full label text");
    }

    #[test]
    fn registry_hit_test_outside() {
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(10.0, 10.0, 60.0, 30.0), "text");
        assert!(reg.hit_test(5.0, 20.0).is_none());
        assert!(reg.hit_test(30.0, 5.0).is_none());
        assert!(reg.hit_test(65.0, 20.0).is_none());
    }

    #[test]
    fn registry_hit_test_overlapping_returns_last() {
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 50.0, 20.0), "First");
        reg.register(make_bounds(10.0, 0.0, 60.0, 20.0), "Second");
        // Point (20, 10) is inside both; the second (most recent) should win.
        let hit = reg.hit_test(20.0, 10.0);
        assert_eq!(hit.unwrap().original_text, "Second");
    }

    #[test]
    fn registry_entries_accessor() {
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 10.0, 10.0), "A");
        reg.register(make_bounds(20.0, 0.0, 30.0, 10.0), "B");
        assert_eq!(reg.entries().len(), 2);
        assert_eq!(reg.entries()[0].original_text, "A");
        assert_eq!(reg.entries()[1].original_text, "B");
    }

    // ── HoverRevealState tests ──────────────────────────────────────────

    fn fast_config() -> TooltipConfig {
        TooltipConfig {
            show_delay: 0.0,
            fade_in_duration: 0.0,
            fade_out_duration: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn hover_state_initially_hidden() {
        let state = HoverRevealState::new(TooltipConfig::default());
        assert!(!state.is_active());
        assert!(state.active_tooltip().is_none());
        assert_eq!(state.opacity(), 0.0);
    }

    #[test]
    fn hover_instant_show_no_delay() {
        let mut state = HoverRevealState::new(fast_config());
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "Full text");

        // Move cursor over the text
        state.update(&reg, 50.0, 10.0, 0.016);
        assert!(state.is_active());
        let tt = state.active_tooltip().unwrap();
        assert_eq!(tt.text, "Full text");
        assert_eq!(tt.opacity, 1.0);
    }

    #[test]
    fn hover_hide_when_cursor_leaves() {
        let mut state = HoverRevealState::new(fast_config());
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "text");

        state.update(&reg, 50.0, 10.0, 0.016);
        assert!(state.is_active());

        // Move cursor outside
        state.update(&reg, 200.0, 10.0, 0.016);
        assert!(!state.is_active());
        assert!(state.active_tooltip().is_none());
    }

    #[test]
    fn hover_show_delay() {
        let config = TooltipConfig {
            show_delay: 0.5,
            fade_in_duration: 0.0,
            fade_out_duration: 0.0,
            ..Default::default()
        };
        let mut state = HoverRevealState::new(config);
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "Delayed");

        // First update: enters Waiting phase
        state.update(&reg, 50.0, 10.0, 0.016);
        assert!(state.active_tooltip().is_none()); // Still waiting

        // Accumulate time but not enough
        state.update(&reg, 50.0, 10.0, 0.2);
        assert!(state.active_tooltip().is_none());

        // Enough time has passed (total ~0.516s > 0.5s delay)
        state.update(&reg, 50.0, 10.0, 0.3);
        assert!(state.active_tooltip().is_some());
    }

    #[test]
    fn hover_fade_in() {
        let config = TooltipConfig {
            show_delay: 0.0,
            fade_in_duration: 1.0,
            fade_out_duration: 0.0,
            ..Default::default()
        };
        let mut state = HoverRevealState::new(config);
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "Fading");

        // Enter → immediately FadingIn (no delay)
        state.update(&reg, 50.0, 10.0, 0.016);
        // Should have partial opacity
        let tt = state.active_tooltip().unwrap();
        assert!(tt.opacity > 0.0);
        assert!(tt.opacity < 1.0);

        // Advance 0.5s — should be ~50% opacity
        state.update(&reg, 50.0, 10.0, 0.5);
        let tt = state.active_tooltip().unwrap();
        assert!((tt.opacity - 0.516).abs() < 0.05);
    }

    #[test]
    fn hover_fade_out() {
        let config = TooltipConfig {
            show_delay: 0.0,
            fade_in_duration: 0.0,
            fade_out_duration: 1.0,
            ..Default::default()
        };
        let mut state = HoverRevealState::new(config);
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "FadeOut");

        // Show tooltip
        state.update(&reg, 50.0, 10.0, 0.016);
        assert_eq!(state.opacity(), 1.0);

        // Cursor leaves → start fade out
        state.update(&reg, 200.0, 10.0, 0.3);
        assert!(state.is_active());
        assert!(state.opacity() > 0.0);
        assert!(state.opacity() < 1.0);

        // Finish fade out
        state.update(&reg, 200.0, 10.0, 1.0);
        assert!(!state.is_active());
        assert_eq!(state.opacity(), 0.0);
    }

    #[test]
    fn hover_force_hide() {
        let mut state = HoverRevealState::new(fast_config());
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "text");

        state.update(&reg, 50.0, 10.0, 0.016);
        assert!(state.is_active());

        state.hide();
        assert!(!state.is_active());
        assert_eq!(state.opacity(), 0.0);
    }

    #[test]
    fn hover_switch_target_while_visible() {
        let mut state = HoverRevealState::new(fast_config());
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 50.0, 20.0), "First");
        reg.register(make_bounds(60.0, 0.0, 110.0, 20.0), "Second");

        // Hover first
        state.update(&reg, 25.0, 10.0, 0.016);
        assert_eq!(state.active_tooltip().unwrap().text, "First");

        // Move to second
        state.update(&reg, 85.0, 10.0, 0.016);
        assert_eq!(state.active_tooltip().unwrap().text, "Second");
    }

    #[test]
    fn hover_empty_registry_stays_hidden() {
        let mut state = HoverRevealState::new(fast_config());
        let reg = ClippedTextRegistry::new();

        state.update(&reg, 50.0, 10.0, 0.016);
        assert!(!state.is_active());
    }

    #[test]
    fn tooltip_position_below_source() {
        let config = TooltipConfig {
            show_delay: 0.0,
            fade_in_duration: 0.0,
            offset_y: 8.0,
            ..Default::default()
        };
        let mut state = HoverRevealState::new(config);
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(10.0, 50.0, 90.0, 70.0), "text");

        state.update(&reg, 50.0, 60.0, 0.016);
        let tt = state.active_tooltip().unwrap();
        // y should be source bottom (70) + offset (8) = 78
        assert!((tt.position.y - 78.0).abs() < 0.01);
    }

    #[test]
    fn hover_re_enter_during_fade_out() {
        let config = TooltipConfig {
            show_delay: 0.0,
            fade_in_duration: 0.5,
            fade_out_duration: 0.5,
            ..Default::default()
        };
        let mut state = HoverRevealState::new(config);
        let mut reg = ClippedTextRegistry::new();
        reg.register(make_bounds(0.0, 0.0, 100.0, 20.0), "text");

        // Show fully
        state.update(&reg, 50.0, 10.0, 0.016);
        state.update(&reg, 50.0, 10.0, 1.0); // Fully visible
        assert_eq!(state.opacity(), 1.0);

        // Start fading out
        state.update(&reg, 200.0, 10.0, 0.25); // 50% through fade out
        let fade_out_opacity = state.opacity();
        assert!(fade_out_opacity > 0.0 && fade_out_opacity < 1.0);

        // Re-enter: should resume from current opacity, not restart from 0
        state.update(&reg, 50.0, 10.0, 0.016);
        assert!(state.opacity() >= fade_out_opacity);
    }
}
