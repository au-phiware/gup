// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Advanced mark rendering features for sophisticated GPU rendering.
//!
//! This module provides advanced rendering capabilities for marks including:
//!
//! - **Multi-pass rendering**: Support for marks that require multiple draw calls
//!   with different pipelines (e.g., base + outline, base + shadow)
//! - **Blend-aware marks**: Mark-specific blend mode preferences and custom blend states
//! - **Dynamic attribute mapping**: Runtime attribute updates without pipeline recreation
//! - **Render state management**: Viewport/scissor management and state isolation
//!
//! # Architecture
//!
//! Multi-pass rendering uses multiple draw calls within a single render pass,
//! following the project's "single render pass per frame" pattern. Each pass
//! can use a different pipeline (e.g., different blend state or shader variant).
//!
//! # Examples
//!
//! ```rust,ignore
//! use gup::mark::advanced_rendering::*;
//!
//! // Define a multi-pass mark with base + outline
//! let config = MultiPassConfig::new()
//!     .add_pass(RenderPassConfig {
//!         label: "base".into(),
//!         blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
//!         ..Default::default()
//!     })
//!     .add_pass(RenderPassConfig {
//!         label: "outline".into(),
//!         blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
//!         polygon_mode: wgpu::PolygonMode::Line,
//!         ..Default::default()
//!     });
//! ```

use crate::error::{GupError, GupResult};
use crate::mixable::BlendMode;
use std::collections::{HashMap, HashSet};
use wgpu::RenderPass;

// ---------------------------------------------------------------------------
// Multi-Pass Mark Rendering
// ---------------------------------------------------------------------------

/// Configuration for a single draw pass within a multi-pass mark.
///
/// Each pass describes how a draw call should be configured including
/// blend state, polygon mode, and shader entry point overrides.
#[derive(Debug, Clone)]
pub struct RenderPassConfig {
    /// Human-readable label for debugging
    pub label: String,
    /// Blend state for this pass (None = replace/opaque)
    pub blend_state: Option<wgpu::BlendState>,
    /// Polygon rasterization mode (fill, line, point)
    pub polygon_mode: wgpu::PolygonMode,
    /// Override vertex shader entry point (None = "vs_main")
    pub vertex_entry_point: Option<String>,
    /// Override fragment shader entry point (None = "fs_main")
    pub fragment_entry_point: Option<String>,
    /// Whether depth write is enabled for this pass
    pub depth_write_enabled: bool,
    /// Stencil reference value for this pass
    pub stencil_reference: Option<u32>,
}

impl Default for RenderPassConfig {
    fn default() -> Self {
        Self {
            label: String::new(),
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
            polygon_mode: wgpu::PolygonMode::Fill,
            vertex_entry_point: None,
            fragment_entry_point: None,
            depth_write_enabled: false,
            stencil_reference: None,
        }
    }
}

/// Configuration for multi-pass mark rendering.
///
/// Stores an ordered list of [`RenderPassConfig`]s that define how each
/// draw call should be configured within a single GPU render pass.
#[derive(Debug, Clone, Default)]
pub struct MultiPassConfig {
    passes: Vec<RenderPassConfig>,
}

impl MultiPassConfig {
    /// Create an empty multi-pass configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a render pass configuration (builder pattern).
    pub fn add_pass(mut self, config: RenderPassConfig) -> Self {
        self.passes.push(config);
        self
    }

    /// Push a render pass configuration.
    pub fn push_pass(&mut self, config: RenderPassConfig) {
        self.passes.push(config);
    }

    /// Get the number of passes.
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Get an iterator over the pass configurations.
    pub fn passes(&self) -> &[RenderPassConfig] {
        &self.passes
    }

    /// Get a specific pass by index.
    pub fn get_pass(&self, index: usize) -> Option<&RenderPassConfig> {
        self.passes.get(index)
    }

    /// Whether this is a multi-pass configuration (more than one pass).
    pub fn is_multi_pass(&self) -> bool {
        self.passes.len() > 1
    }
}

/// Multi-pass renderer that executes draw calls within a render pass.
///
/// This renderer manages multiple draw calls with different pipeline states
/// while staying within a single GPU render pass (following the project's
/// single render pass per frame pattern).
pub struct MultiPassRenderer {
    /// Cached pipelines keyed by (mark_type_name, pass_index)
    pipeline_cache: HashMap<(String, usize), wgpu::RenderPipeline>,
}

impl MultiPassRenderer {
    /// Create a new multi-pass renderer.
    pub fn new() -> Self {
        Self {
            pipeline_cache: HashMap::new(),
        }
    }

    /// Render a mark with multiple passes within the given render pass.
    ///
    /// Each pass uses its own pipeline configuration. Pipelines are cached
    /// for subsequent frames. The method issues multiple draw calls but stays
    /// within the single provided render pass.
    pub fn render_multi_pass<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        config: &MultiPassConfig,
        pipelines: &'a [wgpu::RenderPipeline],
        bind_group: &'a wgpu::BindGroup,
        vertex_buffer: &'a wgpu::Buffer,
        index_buffer: Option<&'a wgpu::Buffer>,
        vertex_count: u32,
        index_count: Option<u32>,
        instance_count: u32,
    ) -> GupResult<()> {
        if pipelines.len() != config.pass_count() {
            return Err(GupError::render_error(format!(
                "Pipeline count ({}) doesn't match pass count ({})",
                pipelines.len(),
                config.pass_count()
            )));
        }

        for (i, (pass_config, pipeline)) in config.passes().iter().zip(pipelines.iter()).enumerate()
        {
            // Set pipeline for this pass
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));

            // Set stencil reference if configured
            if let Some(stencil_ref) = pass_config.stencil_reference {
                render_pass.set_stencil_reference(stencil_ref);
            }

            // Execute draw call
            if let (Some(idx_buf), Some(idx_count)) = (index_buffer, index_count) {
                render_pass.set_index_buffer(idx_buf.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..idx_count, 0, 0..instance_count);
            } else {
                render_pass.draw(0..vertex_count, 0..instance_count);
            }

            log::trace!(
                "Multi-pass render: pass {i} '{}' - {} instances",
                pass_config.label,
                instance_count
            );
        }

        Ok(())
    }

    /// Get the number of cached pipelines.
    pub fn cached_pipeline_count(&self) -> usize {
        self.pipeline_cache.len()
    }

    /// Clear the pipeline cache.
    pub fn clear_cache(&mut self) {
        self.pipeline_cache.clear();
    }
}

impl Default for MultiPassRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Blend-Aware Marks
// ---------------------------------------------------------------------------

/// Preferred blend mode and custom blend state for a mark type.
///
/// This uses an enum-based approach (per project conventions) instead of
/// a trait object, providing compile-time safety and pattern matching.
#[derive(Debug, Clone)]
pub struct MarkBlendConfig {
    /// The preferred blend mode for this mark
    pub preferred_mode: BlendMode,
    /// Whether the mark supports overriding its blend mode at runtime
    pub supports_override: bool,
    /// Optional custom wgpu blend state (takes precedence over preferred_mode)
    pub custom_blend_state: Option<wgpu::BlendState>,
}

impl Default for MarkBlendConfig {
    fn default() -> Self {
        Self {
            preferred_mode: BlendMode::AlphaBlending,
            supports_override: true,
            custom_blend_state: None,
        }
    }
}

impl MarkBlendConfig {
    /// Create a blend config with alpha blending (common default).
    pub fn alpha_blending() -> Self {
        Self::default()
    }

    /// Create a blend config with additive blending.
    pub fn additive() -> Self {
        Self {
            preferred_mode: BlendMode::Additive,
            supports_override: true,
            custom_blend_state: None,
        }
    }

    /// Create a blend config with a custom wgpu blend state.
    pub fn custom(state: wgpu::BlendState) -> Self {
        Self {
            preferred_mode: BlendMode::None,
            supports_override: false,
            custom_blend_state: Some(state),
        }
    }

    /// Resolve the effective wgpu blend state, considering overrides.
    ///
    /// If a `context_override` is provided and the mark supports overrides,
    /// the context blend mode takes precedence. Otherwise, the mark's own
    /// preference or custom state is used.
    pub fn resolve_blend_state(
        &self,
        context_override: Option<BlendMode>,
    ) -> Option<wgpu::BlendState> {
        // Custom blend state always wins if the mark doesn't support overrides
        if let Some(custom) = &self.custom_blend_state
            && !self.supports_override {
                return Some(*custom);
            }

        // Apply context override if available and supported
        let effective_mode = if self.supports_override {
            context_override.unwrap_or(self.preferred_mode)
        } else {
            self.preferred_mode
        };

        blend_mode_to_wgpu(effective_mode)
    }
}

/// Convert a [`BlendMode`] to a wgpu [`BlendState`].
pub fn blend_mode_to_wgpu(mode: BlendMode) -> Option<wgpu::BlendState> {
    match mode {
        BlendMode::None => None,
        BlendMode::AlphaBlending => Some(wgpu::BlendState::ALPHA_BLENDING),
        BlendMode::Additive => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        BlendMode::Multiply => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        }),
    }
}

// ---------------------------------------------------------------------------
// Dynamic Attribute Mapping
// ---------------------------------------------------------------------------

/// A dynamic attribute value that can be static, computed, or conditional.
///
/// Uses an enum (per project conventions) instead of trait objects for
/// type safety and pattern matching.
#[derive(Debug, Clone)]
pub enum DynamicAttributeValue {
    /// Fixed value that never changes
    Static([f32; 4]),
    /// Value that varies per data index (lookup table)
    PerInstance(Vec<[f32; 4]>),
    /// Flag indicating the attribute uses a shader function (GPU-side)
    ShaderDriven {
        function_name: String,
        wgsl_code: String,
    },
}

impl DynamicAttributeValue {
    /// Create a static attribute value from a scalar.
    pub fn from_scalar(v: f32) -> Self {
        Self::Static([v, 0.0, 0.0, 0.0])
    }

    /// Create a static attribute value from an RGBA color.
    pub fn from_color(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::Static([r, g, b, a])
    }

    /// Create a static attribute value from a 2D position.
    pub fn from_vec2(x: f32, y: f32) -> Self {
        Self::Static([x, y, 0.0, 0.0])
    }

    /// Create a per-instance attribute with a lookup table.
    pub fn from_instances(values: Vec<[f32; 4]>) -> Self {
        Self::PerInstance(values)
    }

    /// Create a shader-driven attribute.
    pub fn shader_driven(function_name: &str, wgsl_code: &str) -> Self {
        Self::ShaderDriven {
            function_name: function_name.to_string(),
            wgsl_code: wgsl_code.to_string(),
        }
    }

    /// Get the static value if this is a static attribute.
    pub fn as_static(&self) -> Option<&[f32; 4]> {
        match self {
            Self::Static(v) => Some(v),
            _ => None,
        }
    }

    /// Get the per-instance values if this is a per-instance attribute.
    pub fn as_per_instance(&self) -> Option<&[[f32; 4]]> {
        match self {
            Self::PerInstance(v) => Some(v),
            _ => None,
        }
    }

    /// Whether this attribute requires a pipeline rebuild.
    pub fn requires_pipeline_rebuild(&self) -> bool {
        matches!(self, Self::ShaderDriven { .. })
    }
}

/// Manages dynamic attribute mappings for a mark, allowing runtime updates.
///
/// Attributes can be updated at runtime without recreating the GPU pipeline
/// (for static and per-instance values). Only shader-driven attribute changes
/// require pipeline recreation.
#[derive(Debug, Clone, Default)]
pub struct DynamicAttributeMap {
    /// Current attribute bindings keyed by attribute name
    mappings: HashMap<String, DynamicAttributeValue>,
    /// Set of attributes that have been modified since last upload
    dirty_attributes: HashSet<String>,
    /// Generation counter for change tracking
    generation: u64,
}

impl DynamicAttributeMap {
    /// Create an empty attribute map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an attribute value.
    ///
    /// Returns `true` if this change requires a pipeline rebuild (shader change).
    pub fn set(&mut self, name: &str, value: DynamicAttributeValue) -> bool {
        let needs_rebuild = value.requires_pipeline_rebuild();
        self.dirty_attributes.insert(name.to_string());
        self.mappings.insert(name.to_string(), value);
        self.generation += 1;
        needs_rebuild
    }

    /// Get an attribute value by name.
    pub fn get(&self, name: &str) -> Option<&DynamicAttributeValue> {
        self.mappings.get(name)
    }

    /// Remove an attribute binding.
    pub fn remove(&mut self, name: &str) -> Option<DynamicAttributeValue> {
        self.dirty_attributes.insert(name.to_string());
        self.generation += 1;
        self.mappings.remove(name)
    }

    /// Get all dirty (modified) attribute names.
    pub fn dirty_attributes(&self) -> &HashSet<String> {
        &self.dirty_attributes
    }

    /// Clear the dirty flag for all attributes (after upload).
    pub fn clear_dirty(&mut self) {
        self.dirty_attributes.clear();
    }

    /// Whether any attributes have been modified since last upload.
    pub fn is_dirty(&self) -> bool {
        !self.dirty_attributes.is_empty()
    }

    /// Get the current generation counter for change tracking.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Get the number of bound attributes.
    pub fn attribute_count(&self) -> usize {
        self.mappings.len()
    }

    /// Get all attribute names.
    pub fn attribute_names(&self) -> Vec<&str> {
        self.mappings.keys().map(|k| k.as_str()).collect()
    }

    /// Collect all static attribute values as a flat buffer suitable for GPU upload.
    ///
    /// Returns `(data, attribute_count)` where data is packed [f32; 4] per attribute.
    pub fn collect_static_values(&self) -> Vec<[f32; 4]> {
        let mut values = Vec::new();
        // Sort by name for deterministic ordering
        let mut names: Vec<_> = self.mappings.keys().collect();
        names.sort();
        for name in names {
            if let Some(DynamicAttributeValue::Static(v)) = self.mappings.get(name.as_str()) {
                values.push(*v);
            }
        }
        values
    }
}

// ---------------------------------------------------------------------------
// Render State Management
// ---------------------------------------------------------------------------

/// Viewport configuration for mark rendering.
///
/// Provides fine-grained control over the rendering viewport and scissor
/// rect, enabling state isolation between mark types in compositions.
#[derive(Debug, Clone, Copy)]
pub struct MarkViewport {
    /// X origin in pixels
    pub x: f32,
    /// Y origin in pixels
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels
    pub height: f32,
    /// Minimum depth (0.0 for 2D)
    pub min_depth: f32,
    /// Maximum depth (1.0 for 2D)
    pub max_depth: f32,
}

impl Default for MarkViewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }
}

impl MarkViewport {
    /// Create a viewport from pixel dimensions.
    pub fn from_dimensions(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// Create a viewport for a sub-region (e.g., for composition layouts).
    pub fn sub_region(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            ..Default::default()
        }
    }
}

/// Scissor rectangle for clipping mark rendering.
#[derive(Debug, Clone, Copy)]
pub struct ScissorRect {
    /// X origin in pixels
    pub x: u32,
    /// Y origin in pixels
    pub y: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

impl ScissorRect {
    /// Create a scissor rect from pixel coordinates.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Render state snapshot for a mark type, enabling state isolation.
///
/// Before rendering a mark type, the current state is captured. After
/// rendering, the state is restored, preventing mark types from
/// interfering with each other in compositions.
#[derive(Debug, Clone)]
pub struct RenderStateSnapshot {
    /// Viewport at capture time
    pub viewport: Option<MarkViewport>,
    /// Scissor rect at capture time
    pub scissor: Option<ScissorRect>,
    /// Blend mode at capture time
    pub blend_mode: BlendMode,
    /// Stencil reference at capture time
    pub stencil_reference: Option<u32>,
}

impl RenderStateSnapshot {
    /// Capture the current render state.
    pub fn capture(blend_mode: BlendMode) -> Self {
        Self {
            viewport: None,
            scissor: None,
            blend_mode,
            stencil_reference: None,
        }
    }

    /// Capture with viewport information.
    pub fn capture_with_viewport(blend_mode: BlendMode, viewport: MarkViewport) -> Self {
        Self {
            viewport: Some(viewport),
            scissor: None,
            blend_mode,
            stencil_reference: None,
        }
    }
}

/// Manages render state transitions between mark types in compositions.
///
/// Provides state batching to minimize GPU state changes when rendering
/// multiple mark types in sequence.
pub struct RenderStateManager {
    /// Stack of saved states for nested compositions
    state_stack: Vec<RenderStateSnapshot>,
    /// Current viewport
    current_viewport: Option<MarkViewport>,
    /// Current scissor rect
    current_scissor: Option<ScissorRect>,
    /// Statistics: number of state transitions performed
    transition_count: u64,
}

impl RenderStateManager {
    /// Create a new render state manager.
    pub fn new() -> Self {
        Self {
            state_stack: Vec::new(),
            current_viewport: None,
            current_scissor: None,
            transition_count: 0,
        }
    }

    /// Push the current state onto the stack (for nested compositions).
    pub fn push_state(&mut self, blend_mode: BlendMode) {
        let snapshot = match self.current_viewport {
            Some(vp) => RenderStateSnapshot::capture_with_viewport(blend_mode, vp),
            None => RenderStateSnapshot::capture(blend_mode),
        };
        self.state_stack.push(snapshot);
    }

    /// Pop and return the previous state.
    pub fn pop_state(&mut self) -> Option<RenderStateSnapshot> {
        let snapshot = self.state_stack.pop();
        if let Some(ref s) = snapshot {
            self.current_viewport = s.viewport;
            self.current_scissor = s.scissor;
            self.transition_count += 1;
        }
        snapshot
    }

    /// Set the viewport for subsequent rendering.
    pub fn set_viewport(&mut self, viewport: MarkViewport) {
        self.current_viewport = Some(viewport);
        self.transition_count += 1;
    }

    /// Set the scissor rect for subsequent rendering.
    pub fn set_scissor(&mut self, scissor: ScissorRect) {
        self.current_scissor = Some(scissor);
        self.transition_count += 1;
    }

    /// Apply the current state to a render pass.
    pub fn apply_to_render_pass(&self, render_pass: &mut RenderPass<'_>) {
        if let Some(vp) = &self.current_viewport {
            render_pass.set_viewport(vp.x, vp.y, vp.width, vp.height, vp.min_depth, vp.max_depth);
        }
        if let Some(sc) = &self.current_scissor {
            render_pass.set_scissor_rect(sc.x, sc.y, sc.width, sc.height);
        }
    }

    /// Clear viewport and scissor overrides (reset to full framebuffer).
    pub fn reset_state(&mut self) {
        self.current_viewport = None;
        self.current_scissor = None;
        self.transition_count += 1;
    }

    /// Get the total number of state transitions performed.
    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    /// Get the current state stack depth.
    pub fn stack_depth(&self) -> usize {
        self.state_stack.len()
    }

    /// Get the current viewport.
    pub fn current_viewport(&self) -> Option<&MarkViewport> {
        self.current_viewport.as_ref()
    }

    /// Get the current scissor rect.
    pub fn current_scissor(&self) -> Option<&ScissorRect> {
        self.current_scissor.as_ref()
    }
}

impl Default for RenderStateManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- MultiPassConfig tests ---

    #[test]
    fn test_multi_pass_config_empty() {
        let config = MultiPassConfig::new();
        assert_eq!(config.pass_count(), 0);
        assert!(!config.is_multi_pass());
    }

    #[test]
    fn test_multi_pass_config_single_pass() {
        let config = MultiPassConfig::new().add_pass(RenderPassConfig {
            label: "base".into(),
            ..Default::default()
        });
        assert_eq!(config.pass_count(), 1);
        assert!(!config.is_multi_pass());
    }

    #[test]
    fn test_multi_pass_config_two_passes() {
        let config = MultiPassConfig::new()
            .add_pass(RenderPassConfig {
                label: "fill".into(),
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            })
            .add_pass(RenderPassConfig {
                label: "outline".into(),
                polygon_mode: wgpu::PolygonMode::Line,
                ..Default::default()
            });
        assert_eq!(config.pass_count(), 2);
        assert!(config.is_multi_pass());
        assert_eq!(config.get_pass(0).unwrap().label, "fill");
        assert_eq!(config.get_pass(1).unwrap().label, "outline");
    }

    #[test]
    fn test_render_pass_config_defaults() {
        let config = RenderPassConfig::default();
        assert!(config.blend_state.is_some());
        assert_eq!(config.polygon_mode, wgpu::PolygonMode::Fill);
        assert!(config.vertex_entry_point.is_none());
        assert!(config.fragment_entry_point.is_none());
        assert!(!config.depth_write_enabled);
        assert!(config.stencil_reference.is_none());
    }

    // --- MarkBlendConfig tests ---

    #[test]
    fn test_blend_config_default() {
        let config = MarkBlendConfig::default();
        assert_eq!(config.preferred_mode, BlendMode::AlphaBlending);
        assert!(config.supports_override);
        assert!(config.custom_blend_state.is_none());
    }

    #[test]
    fn test_blend_config_alpha() {
        let config = MarkBlendConfig::alpha_blending();
        assert_eq!(config.preferred_mode, BlendMode::AlphaBlending);
        let resolved = config.resolve_blend_state(None);
        assert!(resolved.is_some());
    }

    #[test]
    fn test_blend_config_additive() {
        let config = MarkBlendConfig::additive();
        assert_eq!(config.preferred_mode, BlendMode::Additive);
        let resolved = config.resolve_blend_state(None);
        assert!(resolved.is_some());
    }

    #[test]
    fn test_blend_config_override() {
        let config = MarkBlendConfig::alpha_blending();
        // Override with additive
        let resolved = config.resolve_blend_state(Some(BlendMode::Additive));
        assert!(resolved.is_some());
    }

    #[test]
    fn test_blend_config_no_override() {
        let config = MarkBlendConfig::custom(wgpu::BlendState::ALPHA_BLENDING);
        assert!(!config.supports_override);
        // Attempting override should still use custom state
        let resolved = config.resolve_blend_state(Some(BlendMode::Additive));
        assert!(resolved.is_some());
    }

    #[test]
    fn test_blend_mode_to_wgpu_none() {
        assert!(blend_mode_to_wgpu(BlendMode::None).is_none());
    }

    #[test]
    fn test_blend_mode_to_wgpu_alpha() {
        assert!(blend_mode_to_wgpu(BlendMode::AlphaBlending).is_some());
    }

    #[test]
    fn test_blend_mode_to_wgpu_additive() {
        assert!(blend_mode_to_wgpu(BlendMode::Additive).is_some());
    }

    #[test]
    fn test_blend_mode_to_wgpu_multiply() {
        assert!(blend_mode_to_wgpu(BlendMode::Multiply).is_some());
    }

    // --- DynamicAttributeValue tests ---

    #[test]
    fn test_dynamic_attribute_static_scalar() {
        let attr = DynamicAttributeValue::from_scalar(5.0);
        assert!(attr.as_static().is_some());
        assert_eq!(attr.as_static().unwrap()[0], 5.0);
        assert!(!attr.requires_pipeline_rebuild());
    }

    #[test]
    fn test_dynamic_attribute_static_color() {
        let attr = DynamicAttributeValue::from_color(1.0, 0.5, 0.0, 1.0);
        let v = attr.as_static().unwrap();
        assert_eq!(v, &[1.0, 0.5, 0.0, 1.0]);
    }

    #[test]
    fn test_dynamic_attribute_static_vec2() {
        let attr = DynamicAttributeValue::from_vec2(10.0, 20.0);
        let v = attr.as_static().unwrap();
        assert_eq!(v[0], 10.0);
        assert_eq!(v[1], 20.0);
    }

    #[test]
    fn test_dynamic_attribute_per_instance() {
        let values = vec![[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]];
        let attr = DynamicAttributeValue::from_instances(values.clone());
        assert!(attr.as_per_instance().is_some());
        assert_eq!(attr.as_per_instance().unwrap().len(), 2);
        assert!(!attr.requires_pipeline_rebuild());
    }

    #[test]
    fn test_dynamic_attribute_shader_driven() {
        let attr =
            DynamicAttributeValue::shader_driven("my_func", "fn my_func() -> f32 { return 1.0; }");
        assert!(attr.as_static().is_none());
        assert!(attr.as_per_instance().is_none());
        assert!(attr.requires_pipeline_rebuild());
    }

    // --- DynamicAttributeMap tests ---

    #[test]
    fn test_attribute_map_empty() {
        let map = DynamicAttributeMap::new();
        assert_eq!(map.attribute_count(), 0);
        assert!(!map.is_dirty());
        assert_eq!(map.generation(), 0);
    }

    #[test]
    fn test_attribute_map_set_and_get() {
        let mut map = DynamicAttributeMap::new();
        map.set(
            "color",
            DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
        );
        assert_eq!(map.attribute_count(), 1);
        assert!(map.is_dirty());
        assert_eq!(map.generation(), 1);

        let val = map.get("color").unwrap();
        assert_eq!(val.as_static().unwrap(), &[1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_attribute_map_remove() {
        let mut map = DynamicAttributeMap::new();
        map.set("radius", DynamicAttributeValue::from_scalar(5.0));
        assert_eq!(map.attribute_count(), 1);

        let removed = map.remove("radius");
        assert!(removed.is_some());
        assert_eq!(map.attribute_count(), 0);
        assert_eq!(map.generation(), 2); // set + remove
    }

    #[test]
    fn test_attribute_map_dirty_tracking() {
        let mut map = DynamicAttributeMap::new();
        map.set("x", DynamicAttributeValue::from_scalar(1.0));
        map.set("y", DynamicAttributeValue::from_scalar(2.0));
        assert!(map.is_dirty());
        assert_eq!(map.dirty_attributes().len(), 2);

        map.clear_dirty();
        assert!(!map.is_dirty());
        assert_eq!(map.dirty_attributes().len(), 0);
    }

    #[test]
    fn test_attribute_map_collect_static_values() {
        let mut map = DynamicAttributeMap::new();
        map.set("alpha", DynamicAttributeValue::from_scalar(0.5));
        map.set(
            "color",
            DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
        );
        // Add a non-static attribute that should be skipped
        map.set(
            "data",
            DynamicAttributeValue::from_instances(vec![[0.0; 4]]),
        );

        let values = map.collect_static_values();
        // Only "alpha" and "color" are static (sorted by name)
        assert_eq!(values.len(), 2);
        assert_eq!(values[0][0], 0.5); // "alpha" comes first alphabetically
        assert_eq!(values[1], [1.0, 0.0, 0.0, 1.0]); // "color"
    }

    #[test]
    fn test_attribute_map_names() {
        let mut map = DynamicAttributeMap::new();
        map.set("position", DynamicAttributeValue::from_vec2(0.0, 0.0));
        map.set(
            "color",
            DynamicAttributeValue::from_color(1.0, 1.0, 1.0, 1.0),
        );

        let mut names = map.attribute_names();
        names.sort();
        assert_eq!(names, vec!["color", "position"]);
    }

    #[test]
    fn test_attribute_map_shader_driven_requires_rebuild() {
        let mut map = DynamicAttributeMap::new();
        let needs_rebuild = map.set(
            "position",
            DynamicAttributeValue::shader_driven("transform", "fn transform() {}"),
        );
        assert!(needs_rebuild);

        let no_rebuild = map.set(
            "color",
            DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
        );
        assert!(!no_rebuild);
    }

    // --- MarkViewport tests ---

    #[test]
    fn test_viewport_default() {
        let vp = MarkViewport::default();
        assert_eq!(vp.x, 0.0);
        assert_eq!(vp.y, 0.0);
        assert_eq!(vp.width, 800.0);
        assert_eq!(vp.height, 600.0);
    }

    #[test]
    fn test_viewport_from_dimensions() {
        let vp = MarkViewport::from_dimensions(1920.0, 1080.0);
        assert_eq!(vp.width, 1920.0);
        assert_eq!(vp.height, 1080.0);
        assert_eq!(vp.x, 0.0);
    }

    #[test]
    fn test_viewport_sub_region() {
        let vp = MarkViewport::sub_region(100.0, 50.0, 400.0, 300.0);
        assert_eq!(vp.x, 100.0);
        assert_eq!(vp.y, 50.0);
        assert_eq!(vp.width, 400.0);
        assert_eq!(vp.height, 300.0);
    }

    // --- ScissorRect tests ---

    #[test]
    fn test_scissor_rect() {
        let rect = ScissorRect::new(10, 20, 100, 200);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 200);
    }

    // --- RenderStateSnapshot tests ---

    #[test]
    fn test_state_snapshot_capture() {
        let snapshot = RenderStateSnapshot::capture(BlendMode::AlphaBlending);
        assert!(snapshot.viewport.is_none());
        assert!(snapshot.scissor.is_none());
        assert_eq!(snapshot.blend_mode, BlendMode::AlphaBlending);
    }

    #[test]
    fn test_state_snapshot_with_viewport() {
        let vp = MarkViewport::from_dimensions(1024.0, 768.0);
        let snapshot = RenderStateSnapshot::capture_with_viewport(BlendMode::Additive, vp);
        assert!(snapshot.viewport.is_some());
        assert_eq!(snapshot.blend_mode, BlendMode::Additive);
    }

    // --- RenderStateManager tests ---

    #[test]
    fn test_state_manager_initial() {
        let mgr = RenderStateManager::new();
        assert_eq!(mgr.stack_depth(), 0);
        assert_eq!(mgr.transition_count(), 0);
        assert!(mgr.current_viewport().is_none());
        assert!(mgr.current_scissor().is_none());
    }

    #[test]
    fn test_state_manager_push_pop() {
        let mut mgr = RenderStateManager::new();

        mgr.set_viewport(MarkViewport::from_dimensions(800.0, 600.0));
        mgr.push_state(BlendMode::AlphaBlending);
        assert_eq!(mgr.stack_depth(), 1);

        mgr.set_viewport(MarkViewport::from_dimensions(400.0, 300.0));
        assert_eq!(mgr.current_viewport().unwrap().width, 400.0);

        let restored = mgr.pop_state().unwrap();
        assert_eq!(restored.blend_mode, BlendMode::AlphaBlending);
        assert_eq!(mgr.current_viewport().unwrap().width, 800.0);
        assert_eq!(mgr.stack_depth(), 0);
    }

    #[test]
    fn test_state_manager_nested_push_pop() {
        let mut mgr = RenderStateManager::new();

        // Level 1
        mgr.push_state(BlendMode::None);
        mgr.set_viewport(MarkViewport::from_dimensions(800.0, 600.0));

        // Level 2
        mgr.push_state(BlendMode::AlphaBlending);
        mgr.set_viewport(MarkViewport::from_dimensions(400.0, 300.0));

        assert_eq!(mgr.stack_depth(), 2);

        // Pop level 2
        let s2 = mgr.pop_state().unwrap();
        assert_eq!(s2.blend_mode, BlendMode::AlphaBlending);
        assert_eq!(mgr.current_viewport().unwrap().width, 800.0);

        // Pop level 1
        let s1 = mgr.pop_state().unwrap();
        assert_eq!(s1.blend_mode, BlendMode::None);
        assert_eq!(mgr.stack_depth(), 0);
    }

    #[test]
    fn test_state_manager_reset() {
        let mut mgr = RenderStateManager::new();
        mgr.set_viewport(MarkViewport::from_dimensions(800.0, 600.0));
        mgr.set_scissor(ScissorRect::new(0, 0, 400, 300));
        assert!(mgr.current_viewport().is_some());
        assert!(mgr.current_scissor().is_some());

        mgr.reset_state();
        assert!(mgr.current_viewport().is_none());
        assert!(mgr.current_scissor().is_none());
    }

    #[test]
    fn test_state_manager_transition_count() {
        let mut mgr = RenderStateManager::new();
        assert_eq!(mgr.transition_count(), 0);

        mgr.set_viewport(MarkViewport::default());
        assert_eq!(mgr.transition_count(), 1);

        mgr.set_scissor(ScissorRect::new(0, 0, 100, 100));
        assert_eq!(mgr.transition_count(), 2);

        mgr.reset_state();
        assert_eq!(mgr.transition_count(), 3);
    }

    #[test]
    fn test_state_manager_pop_empty_stack() {
        let mut mgr = RenderStateManager::new();
        assert!(mgr.pop_state().is_none());
    }

    // --- MultiPassRenderer tests ---

    #[test]
    fn test_multi_pass_renderer_new() {
        let renderer = MultiPassRenderer::new();
        assert_eq!(renderer.cached_pipeline_count(), 0);
    }

    #[test]
    fn test_multi_pass_renderer_clear_cache() {
        let mut renderer = MultiPassRenderer::new();
        renderer.clear_cache();
        assert_eq!(renderer.cached_pipeline_count(), 0);
    }
}
