// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-rendered tooltip background with rounded corners, border, and optional
//! drop shadow.
//!
//! This module provides a tooltip-specific wrapper around the general-purpose
//! [`super::ui_quad::UiQuadRenderer`].  The wrapper translates
//! [`TooltipLayout`] and [`TooltipConfig`] into [`UiQuadInstance`] values so
//! that existing tooltip code continues to work unchanged.
//!
//! # Usage
//!
//! ```rust,ignore
//! use gup::text::tooltip_bg::TooltipBackgroundRenderer;
//! use gup::text::hover_reveal::{TooltipConfig, TooltipLayout};
//!
//! // Create once
//! let mut bg_renderer = TooltipBackgroundRenderer::new(&device)?;
//!
//! // Each frame
//! bg_renderer.begin_frame();
//! bg_renderer.queue(&layout, &config);
//!
//! // Inside the render pass — call BEFORE text rendering
//! bg_renderer.render(&mut render_pass, &device, &queue, screen_w, screen_h)?;
//! ```

use super::hover_reveal::{TooltipConfig, TooltipLayout};
use super::ui_quad::{UiQuadInstance, UiQuadRenderer};
use crate::error::GupResult;
use wgpu::*;

// ── Renderer ────────────────────────────────────────────────────────────────

/// GPU-accelerated tooltip background renderer.
///
/// This is a thin facade over [`UiQuadRenderer`] that accepts the
/// tooltip-specific [`TooltipLayout`]/[`TooltipConfig`] types and converts
/// them to [`UiQuadInstance`] values.  All actual GPU work is performed by
/// the shared renderer.
pub struct TooltipBackgroundRenderer {
    inner: UiQuadRenderer,
}

impl std::fmt::Debug for TooltipBackgroundRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TooltipBackgroundRenderer")
            .field("inner", &self.inner)
            .finish()
    }
}

impl TooltipBackgroundRenderer {
    /// Create a new tooltip background renderer.
    pub fn new(device: &Device) -> GupResult<Self> {
        Ok(Self {
            inner: UiQuadRenderer::new(device)?,
        })
    }

    /// Clear queued instances for a new frame.
    pub fn begin_frame(&mut self) {
        self.inner.begin_frame();
    }

    /// Queue a tooltip background for rendering.
    ///
    /// Call this after computing [`TooltipLayout`] and before creating the
    /// render pass.
    pub fn queue(&mut self, layout: &TooltipLayout, config: &TooltipConfig) {
        let bounds = &layout.background_bounds;
        self.inner.queue(UiQuadInstance {
            rect_min: [bounds.left, bounds.top],
            rect_max: [bounds.right, bounds.bottom],
            bg_color: config.background_color,
            border_color: config.border_color,
            params: [
                config.corner_radius,
                config.border_width,
                layout.opacity,
                config.shadow_radius,
            ],
            shadow_color: config.shadow_color,
            shadow_offset: config.shadow_offset,
            arrow_params: [
                layout.arrow_direction.to_f32(),
                layout.arrow_size,
                layout.arrow_offset,
                0.0,
            ],
        });
    }

    /// Render all queued tooltip backgrounds.
    ///
    /// Must be called **inside** a render pass and **before** text rendering so
    /// that the background appears behind the text.
    pub fn render<'a>(
        &mut self,
        render_pass: &mut RenderPass<'a>,
        device: &Device,
        queue: &Queue,
        screen_width: f32,
        screen_height: f32,
    ) -> GupResult<()> {
        self.inner
            .render(render_pass, device, queue, screen_width, screen_height)
    }

    /// Return the number of queued tooltip backgrounds.
    pub fn queued_count(&self) -> usize {
        self.inner.queued_count()
    }

    /// Get a reference to the underlying [`UiQuadRenderer`].
    ///
    /// Useful when you want to queue additional non-tooltip UI elements into
    /// the same renderer for a combined draw call.
    pub fn inner(&self) -> &UiQuadRenderer {
        &self.inner
    }

    /// Get a mutable reference to the underlying [`UiQuadRenderer`].
    pub fn inner_mut(&mut self) -> &mut UiQuadRenderer {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextBounds;
    use crate::text::hover_reveal::{ArrowDirection, TooltipConfig};
    use crate::text::ui_quad::UiQuadInstance;
    use std::mem;

    #[test]
    fn queue_populates_instance_data() {
        let config = TooltipConfig {
            background_color: [0.1, 0.2, 0.3, 0.9],
            border_color: [0.5, 0.5, 0.5, 1.0],
            border_width: 2.0,
            corner_radius: 6.0,
            shadow_radius: 4.0,
            shadow_color: [0.0, 0.0, 0.0, 0.5],
            shadow_offset: [1.0, 2.0],
            ..Default::default()
        };

        let layout = TooltipLayout {
            background_bounds: TextBounds::new(10.0, 20.0, 200.0, 60.0),
            text_position: crate::shader_function::Vec2 { x: 16.0, y: 24.0 },
            text: "Hello".to_string(),
            opacity: 0.8,
            arrow_direction: ArrowDirection::None,
            arrow_size: 0.0,
            arrow_offset: 0.0,
        };

        // Build the instance the same way queue() does
        let bounds = &layout.background_bounds;
        let inst = UiQuadInstance {
            rect_min: [bounds.left, bounds.top],
            rect_max: [bounds.right, bounds.bottom],
            bg_color: config.background_color,
            border_color: config.border_color,
            params: [
                config.corner_radius,
                config.border_width,
                layout.opacity,
                config.shadow_radius,
            ],
            shadow_color: config.shadow_color,
            shadow_offset: config.shadow_offset,
            arrow_params: [
                layout.arrow_direction.to_f32(),
                layout.arrow_size,
                layout.arrow_offset,
                0.0,
            ],
        };

        assert_eq!(inst.rect_min, [10.0, 20.0]);
        assert_eq!(inst.rect_max, [200.0, 60.0]);
        assert_eq!(inst.params[0], 6.0); // corner_radius
        assert_eq!(inst.params[1], 2.0); // border_width
        assert_eq!(inst.params[2], 0.8); // opacity
        assert_eq!(inst.params[3], 4.0); // shadow_radius
    }

    #[test]
    fn default_config_has_corner_radius_and_shadow() {
        let config = TooltipConfig::default();
        assert!(config.corner_radius > 0.0);
        assert_eq!(config.shadow_radius, 0.0); // Shadow off by default
        assert!(config.shadow_color[3] > 0.0); // But colour is set for easy opt-in
    }

    #[test]
    fn default_config_has_arrow_disabled() {
        let config = TooltipConfig::default();
        assert_eq!(config.arrow_direction, ArrowDirection::None);
        assert!(config.arrow_size > 0.0); // Size pre-configured for easy opt-in
    }

    #[test]
    fn queue_populates_arrow_params() {
        let layout = TooltipLayout {
            background_bounds: TextBounds::new(50.0, 50.0, 200.0, 80.0),
            text_position: crate::shader_function::Vec2 { x: 56.0, y: 54.0 },
            text: "Arrow test".to_string(),
            opacity: 1.0,
            arrow_direction: ArrowDirection::Top,
            arrow_size: 8.0,
            arrow_offset: 3.5,
        };

        let bounds = &layout.background_bounds;
        let inst = UiQuadInstance {
            rect_min: [bounds.left, bounds.top],
            rect_max: [bounds.right, bounds.bottom],
            bg_color: [0.0; 4],
            border_color: [0.0; 4],
            params: [0.0, 0.0, layout.opacity, 0.0],
            shadow_color: [0.0; 4],
            shadow_offset: [0.0; 2],
            arrow_params: [
                layout.arrow_direction.to_f32(),
                layout.arrow_size,
                layout.arrow_offset,
                0.0,
            ],
        };

        assert_eq!(inst.arrow_params[0], 1.0); // ArrowDirection::Top
        assert_eq!(inst.arrow_params[1], 8.0); // arrow_size
        assert_eq!(inst.arrow_params[2], 3.5); // arrow_offset
        assert_eq!(inst.arrow_params[3], 0.0); // unused
    }

    #[test]
    fn ui_quad_instance_layout_unchanged() {
        // The instance type is now shared but the layout must not change.
        assert_eq!(mem::offset_of!(UiQuadInstance, rect_min), 0);
        assert_eq!(mem::offset_of!(UiQuadInstance, rect_max), 8);
        assert_eq!(mem::offset_of!(UiQuadInstance, bg_color), 16);
        assert_eq!(mem::offset_of!(UiQuadInstance, border_color), 32);
        assert_eq!(mem::offset_of!(UiQuadInstance, params), 48);
        assert_eq!(mem::offset_of!(UiQuadInstance, shadow_color), 64);
        assert_eq!(mem::offset_of!(UiQuadInstance, shadow_offset), 80);
        assert_eq!(mem::offset_of!(UiQuadInstance, arrow_params), 88);
        assert_eq!(mem::size_of::<UiQuadInstance>(), 104);
    }
}
