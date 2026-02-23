// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core accessibility system for Gup visualizations.
//!
//! This module provides comprehensive accessibility features including:
//! - Screen reader support via ARIA integration
//! - Keyboard navigation for all interactive elements
//! - High contrast and colorblind-friendly rendering modes
//! - Data sonification and narration
//!
//! # Design Philosophy
//!
//! Accessibility is built into the core architecture rather than retrofitted.
//! All visualizations are accessible by default, with opt-out rather than opt-in.
//!
//! # Examples
//!
//! ```rust,ignore
//! use gup::accessibility::{AccessibilitySystem, ContrastMode};
//!
//! let mut accessibility = AccessibilitySystem::new();
//! accessibility.set_contrast_mode(ContrastMode::HighContrast);
//! accessibility.enable_keyboard_navigation(true);
//! ```

pub mod aria;
pub mod focus;
pub mod focus_elements;
pub mod focus_ring;
pub mod high_contrast;
pub mod keyboard;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod mark_positions;
pub mod pattern_renderer;
pub mod platform;
pub mod position_sync;
pub mod sonification;
pub mod web_overlay;

use std::collections::HashMap;

pub use aria::*;
pub use focus::*;
pub use focus_elements::*;
pub use focus_ring::*;
pub use high_contrast::*;
pub use mark_positions::*;
pub use pattern_renderer::*;
pub use platform::*;
pub use position_sync::*;
pub use sonification::*;
pub use web_overlay::*;

/// Central accessibility system coordinating all accessibility features.
pub struct AccessibilitySystem {
    /// Screen reader integration
    pub aria_tree: AriaTree,

    /// Keyboard navigation
    pub focus_manager: FocusManager,

    /// Alternative rendering
    pub high_contrast_renderer: HighContrastRenderer,

    /// Data sonification
    pub sonification_engine: SonificationEngine,

    /// Platform-specific accessibility bridge
    platform_bridge: Box<dyn PlatformAccessibility>,

    /// Global accessibility settings
    settings: AccessibilitySettings,

    /// Whether accessibility features are enabled
    enabled: bool,
}

/// Global accessibility configuration.
#[derive(Debug, Clone)]
pub struct AccessibilitySettings {
    /// Enable screen reader support
    pub screen_reader_enabled: bool,

    /// Enable keyboard navigation
    pub keyboard_navigation_enabled: bool,

    /// Contrast mode for visual accessibility
    pub contrast_mode: ContrastMode,

    /// Enable data sonification
    pub sonification_enabled: bool,

    /// Custom accessibility overrides
    pub custom_overrides: HashMap<String, String>,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            screen_reader_enabled: true,
            keyboard_navigation_enabled: true,
            contrast_mode: ContrastMode::Standard,
            sonification_enabled: false,
            custom_overrides: HashMap::new(),
        }
    }
}

impl AccessibilitySystem {
    /// Create a new accessibility system with default settings.
    pub fn new() -> Self {
        let mut platform_bridge = platform::create_platform_accessibility();
        let _ = platform_bridge.initialize();

        Self {
            aria_tree: AriaTree::new(),
            focus_manager: FocusManager::new(),
            high_contrast_renderer: HighContrastRenderer::new(ContrastMode::Standard),
            sonification_engine: SonificationEngine::new(),
            platform_bridge,
            settings: AccessibilitySettings::default(),
            enabled: true,
        }
    }

    /// Create a new accessibility system with custom settings.
    pub fn with_settings(settings: AccessibilitySettings) -> Self {
        let mut platform_bridge = platform::create_platform_accessibility();
        let _ = platform_bridge.initialize();

        Self {
            aria_tree: AriaTree::new(),
            focus_manager: FocusManager::new(),
            high_contrast_renderer: HighContrastRenderer::new(settings.contrast_mode.clone()),
            sonification_engine: SonificationEngine::new(),
            platform_bridge,
            settings,
            enabled: true,
        }
    }

    /// Enable or disable all accessibility features.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if accessibility features are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the contrast mode for visual accessibility.
    pub fn set_contrast_mode(&mut self, mode: ContrastMode) {
        self.settings.contrast_mode = mode.clone();
        self.high_contrast_renderer = HighContrastRenderer::new(mode);
    }

    /// Get the current contrast mode.
    pub fn contrast_mode(&self) -> &ContrastMode {
        &self.settings.contrast_mode
    }

    /// Enable or disable keyboard navigation.
    pub fn set_keyboard_navigation_enabled(&mut self, enabled: bool) {
        self.settings.keyboard_navigation_enabled = enabled;
    }

    /// Check if keyboard navigation is enabled.
    pub fn is_keyboard_navigation_enabled(&self) -> bool {
        self.settings.keyboard_navigation_enabled
    }

    /// Enable or disable screen reader support.
    pub fn set_screen_reader_enabled(&mut self, enabled: bool) {
        self.settings.screen_reader_enabled = enabled;
    }

    /// Check if screen reader support is enabled.
    pub fn is_screen_reader_enabled(&self) -> bool {
        self.settings.screen_reader_enabled
    }

    /// Enable or disable data sonification.
    pub fn set_sonification_enabled(&mut self, enabled: bool) {
        self.settings.sonification_enabled = enabled;
    }

    /// Check if sonification is enabled.
    pub fn is_sonification_enabled(&self) -> bool {
        self.settings.sonification_enabled
    }

    /// Get pending ARIA updates for screen reader.
    pub fn get_pending_aria_updates(&mut self) -> Vec<AriaUpdate> {
        if !self.enabled || !self.settings.screen_reader_enabled {
            return Vec::new();
        }

        let updates = self.aria_tree.drain_update_queue();

        // Send updates to platform bridge
        if !updates.is_empty()
            && let Err(e) = self.platform_bridge.update_accessibility_tree(&updates)
        {
            log::warn!("Failed to update platform accessibility tree: {}", e);
        }

        updates
    }

    /// Get a description of the currently focused element.
    pub fn describe_current_focus(&self) -> Option<String> {
        if !self.enabled || !self.settings.keyboard_navigation_enabled {
            return None;
        }
        self.focus_manager.describe_current_focus()
    }

    /// Update accessibility settings.
    pub fn update_settings(&mut self, settings: AccessibilitySettings) {
        self.settings = settings.clone();
        self.high_contrast_renderer = HighContrastRenderer::new(settings.contrast_mode);
    }

    /// Get current accessibility settings.
    pub fn settings(&self) -> &AccessibilitySettings {
        &self.settings
    }

    /// Announce a message to the screen reader.
    pub fn announce(
        &mut self,
        message: &str,
        priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError> {
        if !self.enabled || !self.settings.screen_reader_enabled {
            return Ok(());
        }

        self.platform_bridge.announce(message, priority)
    }

    /// Set the currently focused element.
    pub fn set_platform_focus(&mut self, element_id: &str) -> Result<(), AccessibilityError> {
        if !self.enabled || !self.settings.keyboard_navigation_enabled {
            return Ok(());
        }

        self.platform_bridge.set_focus(element_id)
    }

    /// Get the platform name for debugging.
    pub fn platform_name(&self) -> &str {
        self.platform_bridge.platform_name()
    }

    /// Check if platform accessibility is available.
    pub fn is_platform_available(&self) -> bool {
        self.platform_bridge.is_available()
    }
}

impl Default for AccessibilitySystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessibility_system_creation() {
        let system = AccessibilitySystem::new();
        assert!(system.is_enabled());
        assert!(system.is_keyboard_navigation_enabled());
        assert!(system.is_screen_reader_enabled());
        assert!(!system.is_sonification_enabled());
    }

    #[test]
    fn test_accessibility_settings() {
        let mut system = AccessibilitySystem::new();

        system.set_contrast_mode(ContrastMode::HighContrast);
        assert!(matches!(system.contrast_mode(), ContrastMode::HighContrast));

        system.set_keyboard_navigation_enabled(false);
        assert!(!system.is_keyboard_navigation_enabled());

        system.set_screen_reader_enabled(false);
        assert!(!system.is_screen_reader_enabled());

        system.set_sonification_enabled(true);
        assert!(system.is_sonification_enabled());
    }

    #[test]
    fn test_accessibility_disabled() {
        let mut system = AccessibilitySystem::new();
        system.set_enabled(false);

        assert!(!system.is_enabled());
        assert_eq!(system.get_pending_aria_updates().len(), 0);
        assert_eq!(system.describe_current_focus(), None);
    }

    #[test]
    fn test_custom_settings() {
        let settings = AccessibilitySettings {
            screen_reader_enabled: false,
            keyboard_navigation_enabled: true,
            contrast_mode: ContrastMode::Colorblind,
            sonification_enabled: true,
            custom_overrides: HashMap::new(),
        };

        let system = AccessibilitySystem::with_settings(settings);
        assert!(!system.is_screen_reader_enabled());
        assert!(system.is_keyboard_navigation_enabled());
        assert!(system.is_sonification_enabled());
        assert!(matches!(system.contrast_mode(), ContrastMode::Colorblind));
    }
}
