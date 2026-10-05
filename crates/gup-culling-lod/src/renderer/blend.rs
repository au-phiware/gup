// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! LOD blend state for smooth tier transitions.

/// Tracks a cross-fade transition between two LOD tiers.
///
/// When the selected tier changes, `LodBlendState` linearly interpolates
/// `alpha` from 0.0 to 1.0 over `blend_frames` calls to [`tick`](Self::tick).
/// The outgoing tier is drawn at `1.0 - alpha` and the incoming tier at
/// `alpha`.
///
/// Setting `blend_frames = 0` disables blending — `alpha` immediately jumps
/// to 1.0.
#[derive(Debug, Clone)]
pub struct LodBlendState {
    /// The LOD tier being faded out (previous tier).
    from_tier: usize,
    /// The LOD tier being faded in (current target tier).
    to_tier: usize,
    /// Current blend progress: 0.0 = fully `from_tier`, 1.0 = fully `to_tier`.
    progress: f32,
    /// Number of frames over which to blend. 0 = instant switch.
    blend_frames: u32,
    /// Whether a transition is currently in progress.
    transitioning: bool,
}

impl LodBlendState {
    /// Create a new blend state starting at `initial_tier` with no transition.
    pub fn new(initial_tier: usize, blend_frames: u32) -> Self {
        Self {
            from_tier: initial_tier,
            to_tier: initial_tier,
            progress: 1.0,
            blend_frames,
            transitioning: false,
        }
    }

    /// Begin a transition to a new tier.
    ///
    /// If `blend_frames` is 0, the transition completes immediately.
    /// If the target tier is the same as the current `to_tier` (or `from_tier`
    /// when already settled), this is a no-op.
    pub fn transition_to(&mut self, new_tier: usize) {
        let current_settled = if self.transitioning {
            self.to_tier
        } else {
            self.from_tier
        };

        if new_tier == current_settled {
            return;
        }

        self.from_tier = current_settled;
        self.to_tier = new_tier;

        if self.blend_frames == 0 {
            self.progress = 1.0;
            self.transitioning = false;
        } else {
            self.progress = 0.0;
            self.transitioning = true;
        }
    }

    /// Advance the blend by one frame.
    ///
    /// Returns the current alpha (0.0 → 1.0). Once alpha reaches 1.0 the
    /// transition is complete and further calls to `tick` return 1.0 without
    /// change.
    pub fn tick(&mut self) -> f32 {
        if !self.transitioning {
            return 1.0;
        }

        let step = if self.blend_frames > 0 {
            1.0 / self.blend_frames as f32
        } else {
            1.0
        };

        self.progress = (self.progress + step).min(1.0);

        if (self.progress - 1.0).abs() < f32::EPSILON {
            self.progress = 1.0;
            self.transitioning = false;
            self.from_tier = self.to_tier;
        }

        self.progress
    }

    /// Current blend alpha (0.0 = fully outgoing, 1.0 = fully incoming).
    #[inline]
    pub fn alpha(&self) -> f32 {
        self.progress
    }

    /// Whether a transition is currently in progress.
    #[inline]
    pub fn is_transitioning(&self) -> bool {
        self.transitioning
    }

    /// The tier being faded out.
    #[inline]
    pub fn from_tier(&self) -> usize {
        self.from_tier
    }

    /// The tier being faded in (the target tier).
    #[inline]
    pub fn to_tier(&self) -> usize {
        self.to_tier
    }

    /// The currently active tier (the target, regardless of transition state).
    #[inline]
    pub fn active_tier(&self) -> usize {
        self.to_tier
    }

    /// The blend frame count.
    #[inline]
    pub fn blend_frames(&self) -> u32 {
        self.blend_frames
    }

    /// Set the blend frame count.
    ///
    /// This affects future transitions, not any currently in-progress
    /// transition.
    pub fn set_blend_frames(&mut self, frames: u32) {
        self.blend_frames = frames;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_settled() {
        let state = LodBlendState::new(2, 8);
        assert_eq!(state.alpha(), 1.0);
        assert!(!state.is_transitioning());
        assert_eq!(state.from_tier(), 2);
        assert_eq!(state.to_tier(), 2);
        assert_eq!(state.active_tier(), 2);
    }

    #[test]
    fn transition_progresses_correctly() {
        let mut state = LodBlendState::new(0, 4);
        state.transition_to(1);

        assert!(state.is_transitioning());
        assert_eq!(state.from_tier(), 0);
        assert_eq!(state.to_tier(), 1);
        assert!((state.alpha() - 0.0).abs() < f32::EPSILON);

        // Tick 1: 0.25
        let a1 = state.tick();
        assert!((a1 - 0.25).abs() < 1e-6, "Expected 0.25, got {a1}");

        // Tick 2: 0.5
        let a2 = state.tick();
        assert!((a2 - 0.5).abs() < 1e-6, "Expected 0.5, got {a2}");

        // Tick 3: 0.75
        let a3 = state.tick();
        assert!((a3 - 0.75).abs() < 1e-6, "Expected 0.75, got {a3}");

        // Tick 4: 1.0 — transition complete
        let a4 = state.tick();
        assert!((a4 - 1.0).abs() < 1e-6, "Expected 1.0, got {a4}");
        assert!(!state.is_transitioning());
        assert_eq!(state.from_tier(), 1);
        assert_eq!(state.to_tier(), 1);
    }

    #[test]
    fn alpha_clamps_at_one() {
        let mut state = LodBlendState::new(0, 2);
        state.transition_to(3);

        state.tick(); // 0.5
        state.tick(); // 1.0

        // Further ticks should stay at 1.0
        let a = state.tick();
        assert!((a - 1.0).abs() < f32::EPSILON);
        assert!(!state.is_transitioning());
    }

    #[test]
    fn zero_blend_frames_instant_switch() {
        let mut state = LodBlendState::new(0, 0);
        state.transition_to(3);

        assert!(!state.is_transitioning());
        assert!((state.alpha() - 1.0).abs() < f32::EPSILON);
        assert_eq!(state.active_tier(), 3);
    }

    #[test]
    fn transition_to_same_tier_is_noop() {
        let mut state = LodBlendState::new(2, 8);
        state.transition_to(2);

        assert!(!state.is_transitioning());
        assert!((state.alpha() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn eight_frame_blend_default() {
        let mut state = LodBlendState::new(0, 8);
        state.transition_to(2);

        let mut alphas = Vec::new();
        for _ in 0..8 {
            alphas.push(state.tick());
        }

        // Verify monotonic increase
        for window in alphas.windows(2) {
            assert!(
                window[1] >= window[0],
                "Alpha should be monotonically increasing"
            );
        }

        // Final alpha should be 1.0
        assert!(
            (alphas[7] - 1.0).abs() < 1e-6,
            "Expected 1.0 after 8 frames, got {}",
            alphas[7]
        );
        assert!(!state.is_transitioning());
    }

    #[test]
    fn mid_transition_retarget() {
        let mut state = LodBlendState::new(0, 4);
        state.transition_to(1);

        state.tick(); // 0.25
        state.tick(); // 0.5

        // Change target mid-transition — should start new transition from current to_tier
        state.transition_to(3);
        assert!(state.is_transitioning());
        assert_eq!(state.from_tier(), 1);
        assert_eq!(state.to_tier(), 3);
        assert!((state.alpha() - 0.0).abs() < f32::EPSILON);
    }
}
