// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Debug overlay for the adaptive renderer.
//!
//! The overlay shows the current LOD tier, visible point count after culling,
//! and total points in the selected tier. When disabled it introduces zero
//! CPU or GPU overhead.

/// Per-frame debug information collected by the adaptive renderer.
///
/// This struct is populated only when the debug overlay is enabled.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DebugOverlayInfo {
    /// Current LOD tier index (0-based).
    pub tier_index: usize,
    /// Total number of LOD tiers in the pyramid.
    pub tier_count: usize,
    /// Number of visible points after frustum culling.
    pub visible_points: u32,
    /// Total points in the selected LOD tier (before culling).
    pub total_points_in_tier: u32,
    /// Whether a blend transition is in progress.
    pub blending: bool,
    /// Current blend alpha (1.0 when settled).
    pub blend_alpha: f32,
}

impl std::fmt::Display for DebugOverlayInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LOD {}/{} | visible: {} / {}",
            self.tier_index + 1,
            self.tier_count,
            self.visible_points,
            self.total_points_in_tier,
        )?;
        if self.blending {
            write!(f, " | blending: {:.0}%", self.blend_alpha * 100.0)?;
        }
        Ok(())
    }
}

/// Debug overlay controller.
///
/// When enabled, the adaptive renderer collects [`DebugOverlayInfo`] each
/// frame. When disabled (the default), no work is done and no GPU resources
/// are allocated.
#[derive(Debug, Clone, Default)]
pub struct DebugOverlay {
    enabled: bool,
    /// Most recent frame info (only valid when `enabled`).
    latest_info: DebugOverlayInfo,
}

impl DebugOverlay {
    /// Create a new debug overlay (disabled by default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable the overlay.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.latest_info = DebugOverlayInfo::default();
        }
    }

    /// Whether the overlay is enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Update the overlay info for the current frame.
    ///
    /// This is a no-op when the overlay is disabled, ensuring zero overhead.
    #[inline]
    pub fn update(&mut self, info: DebugOverlayInfo) {
        if self.enabled {
            self.latest_info = info;
        }
    }

    /// Get the latest overlay info.
    ///
    /// Returns `None` if the overlay is disabled.
    pub fn info(&self) -> Option<&DebugOverlayInfo> {
        if self.enabled {
            Some(&self.latest_info)
        } else {
            None
        }
    }

    /// Format the overlay as a display string.
    ///
    /// Returns `None` if the overlay is disabled.
    pub fn display_string(&self) -> Option<String> {
        if self.enabled {
            Some(self.latest_info.to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default() {
        let overlay = DebugOverlay::new();
        assert!(!overlay.is_enabled());
        assert!(overlay.info().is_none());
        assert!(overlay.display_string().is_none());
    }

    #[test]
    fn enable_and_update() {
        let mut overlay = DebugOverlay::new();
        overlay.set_enabled(true);
        assert!(overlay.is_enabled());

        overlay.update(DebugOverlayInfo {
            tier_index: 2,
            tier_count: 5,
            visible_points: 1000,
            total_points_in_tier: 5000,
            blending: false,
            blend_alpha: 1.0,
        });

        let info = overlay.info().unwrap();
        assert_eq!(info.tier_index, 2);
        assert_eq!(info.visible_points, 1000);
    }

    #[test]
    fn update_is_noop_when_disabled() {
        let mut overlay = DebugOverlay::new();
        overlay.update(DebugOverlayInfo {
            tier_index: 3,
            tier_count: 6,
            visible_points: 42,
            total_points_in_tier: 100,
            blending: false,
            blend_alpha: 1.0,
        });
        assert!(overlay.info().is_none());
    }

    #[test]
    fn display_format() {
        let info = DebugOverlayInfo {
            tier_index: 2,
            tier_count: 6,
            visible_points: 1234,
            total_points_in_tier: 5000,
            blending: false,
            blend_alpha: 1.0,
        };
        assert_eq!(info.to_string(), "LOD 3/6 | visible: 1234 / 5000");
    }

    #[test]
    fn display_format_blending() {
        let info = DebugOverlayInfo {
            tier_index: 1,
            tier_count: 4,
            visible_points: 500,
            total_points_in_tier: 2000,
            blending: true,
            blend_alpha: 0.5,
        };
        assert_eq!(
            info.to_string(),
            "LOD 2/4 | visible: 500 / 2000 | blending: 50%"
        );
    }

    #[test]
    fn disable_clears_info() {
        let mut overlay = DebugOverlay::new();
        overlay.set_enabled(true);
        overlay.update(DebugOverlayInfo {
            tier_index: 2,
            tier_count: 5,
            visible_points: 1000,
            total_points_in_tier: 5000,
            blending: false,
            blend_alpha: 1.0,
        });
        overlay.set_enabled(false);
        assert!(overlay.info().is_none());
    }
}
