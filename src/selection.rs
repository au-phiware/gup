// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Selection module for managing data selections and marks.
//!
//! This module provides the Selection type that enables GPU-accelerated interactive
//! visualizations with event handling, spatial queries, and GPU rendering.
//!
//! # Rendering
//!
//! The Selection can render its bound data to a GPU render pass using the mark's
//! hand-optimized shaders. Call [`Selection::prepare_render`] to upload data and
//! set up GPU resources, then [`Selection::render`] in the render pass.
//!
//! ```rust,ignore
//! // Prepare GPU resources with a data-to-instance mapping
//! selection.prepare_render(&device, &queue, |attrs| CircleInstance::from(attrs), None, None)?;
//!
//! // Later, in a render pass:
//! selection.render(&mut render_pass)?;
//! ```

use crate::buffer::{BufferPool, BufferType};
use crate::interaction::{InteractionElement, InteractionEvent, Renderable};
use crate::mark::{MarkInfo, MarkInfoImpl};
use crate::pipeline_cache::PipelineCache;
use crate::shader_function::{ComposableShaderFunction, ShaderType, ShaderUniform};
use crate::{GupResult, RenderContext};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;
use wgpu::{Device, Queue, RenderPass};

// ---------------------------------------------------------------------------
// Viewport uniform type
// ---------------------------------------------------------------------------

/// GPU-ready viewport dimensions for pixel-space SDF calculations.
///
/// Passed to mark shaders as a uniform buffer so that pixel-based values
/// (stroke widths, radii) can be converted to clip-space units. This
/// enables visually consistent rendering regardless of window size.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewportUniforms {
    /// Viewport width in pixels.
    pub width: f32,
    /// Viewport height in pixels.
    pub height: f32,
}

// ---------------------------------------------------------------------------
// Attribute binding types
// ---------------------------------------------------------------------------

/// A type-erased attribute value for use in declarative attribute bindings.
///
/// `AttrValue` represents the set of GPU-compatible scalar and vector types
/// that can be bound to mark attributes. It is produced by closures stored
/// via [`Selection::attr`] and consumed by [`MarkInstanceBuilder`]
/// implementations when constructing GPU instance data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttrValue {
    /// A single floating-point value (e.g., radius, stroke width).
    Float(f32),
    /// A 2-component vector (e.g., position, size).
    Vec2([f32; 2]),
    /// A 4-component vector (e.g., RGBA colour).
    Vec4([f32; 4]),
}

/// Trait for types that can be converted into an [`AttrValue`].
///
/// Implementing this trait for a type allows it to be used as the return value
/// of attribute binding closures passed to [`Selection::attr`]. Only types
/// that are valid GPU attribute values should implement this trait, which
/// provides compile-time safety — attempting to bind an unsupported type
/// (e.g., `String`) will produce a compiler error.
pub trait IntoAttrValue: Send + Sync + 'static {
    /// Convert this value into an [`AttrValue`].
    fn into_attr_value(self) -> AttrValue;
}

impl IntoAttrValue for f32 {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Float(self)
    }
}

impl IntoAttrValue for [f32; 2] {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Vec2(self)
    }
}

impl IntoAttrValue for [f32; 4] {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Vec4(self)
    }
}

impl IntoAttrValue for crate::shader_function::Vec2 {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Vec2([self.x, self.y])
    }
}

impl IntoAttrValue for crate::shader_function::Vec4 {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Vec4([self.x, self.y, self.z, self.w])
    }
}

/// A named attribute binding that extracts a value from data item `T`.
struct AttributeBinding<T> {
    name: String,
    extractor: Box<dyn Fn(&T) -> AttrValue + Send + Sync>,
}

// ---------------------------------------------------------------------------
// GPU shader function binding types
// ---------------------------------------------------------------------------

/// Type-erased metadata for a GPU shader function binding.
///
/// Stores all the information needed to generate WGSL code and create uniform
/// buffers for a shader function that transforms an attribute on the GPU.
struct ShaderFnInfo {
    /// The name of the shader function in WGSL (e.g., `"linear_scale"`).
    function_name: String,
    /// WGSL code defining the shader function.
    wgsl_code: String,
    /// WGSL struct definition for the uniform type, or empty if no uniforms.
    uniform_struct_def: String,
    /// WGSL type name for the uniform (e.g., `"LinearScaleUniforms"`).
    uniform_type_name: String,
    /// Serialised uniform bytes (`bytemuck::bytes_of`), or empty if no uniforms.
    uniform_bytes: Vec<u8>,
    /// WGSL type name for the shader function input (stored for diagnostics).
    #[allow(dead_code)]
    input_wgsl_type: &'static str,
    /// WGSL type name for the shader function output.
    output_wgsl_type: &'static str,
}

/// A named attribute binding that extracts raw data from `T` on the CPU and
/// transforms it on the GPU via a [`ComposableShaderFunction`].
///
/// The `extractor` pulls a lightweight raw value from each data item; the
/// heavy transformation is deferred to the GPU vertex shader.
struct ShaderAttributeBinding<T> {
    /// Attribute name (must match a field recognised by the mark's vertex shader).
    name: String,
    /// CPU-side extractor that provides the shader function's raw input value.
    extractor: Box<dyn Fn(&T) -> AttrValue + Send + Sync>,
    /// Type-erased shader function metadata for WGSL generation.
    shader_fn: ShaderFnInfo,
}

/// Helper: create a [`ShaderFnInfo`] from a concrete [`ComposableShaderFunction`].
fn shader_fn_info_from<S: ComposableShaderFunction>(shader_fn: &S) -> ShaderFnInfo {
    let uniform_bytes = match shader_fn.create_uniforms() {
        Some(u) => bytemuck::bytes_of(&u).to_vec(),
        None => Vec::new(),
    };
    ShaderFnInfo {
        function_name: S::function_name().to_string(),
        wgsl_code: shader_fn.generate_wgsl(),
        uniform_struct_def: <S::Uniforms as ShaderUniform>::wgsl_struct_definition(),
        uniform_type_name: <S::Uniforms as ShaderUniform>::wgsl_type_name().to_string(),
        uniform_bytes,
        input_wgsl_type: <S::Input as ShaderType>::wgsl_type_name(),
        output_wgsl_type: <S::Output as ShaderType>::wgsl_type_name(),
    }
}

/// Trait for extracting multiple attribute values from a single closure.
///
/// This trait enables [`Selection::attr_parallel`] to accept closures that
/// return tuples of attribute values. Each tuple element is mapped to a
/// corresponding attribute name.
pub trait IntoAttrValues<T, const N: usize>: Send + Sync + 'static {
    /// Extract `N` attribute values from a data item.
    fn extract(&self, data: &T) -> [AttrValue; N];
}

impl<T, V1, V2, F> IntoAttrValues<T, 2> for F
where
    F: Fn(&T) -> (V1, V2) + Send + Sync + 'static,
    V1: IntoAttrValue,
    V2: IntoAttrValue,
{
    fn extract(&self, data: &T) -> [AttrValue; 2] {
        let (v1, v2) = self(data);
        [v1.into_attr_value(), v2.into_attr_value()]
    }
}

impl<T, V1, V2, V3, F> IntoAttrValues<T, 3> for F
where
    F: Fn(&T) -> (V1, V2, V3) + Send + Sync + 'static,
    V1: IntoAttrValue,
    V2: IntoAttrValue,
    V3: IntoAttrValue,
{
    fn extract(&self, data: &T) -> [AttrValue; 3] {
        let (v1, v2, v3) = self(data);
        [
            v1.into_attr_value(),
            v2.into_attr_value(),
            v3.into_attr_value(),
        ]
    }
}

/// Trait for mark types that can build GPU instances from named attribute
/// bindings.
///
/// Implement this trait for a mark's associated `Instance` type to enable
/// [`Selection::prepare_render_bound`], which constructs instances
/// automatically from attribute closures instead of requiring a manual mapper.
///
/// # Default values
///
/// [`default_instance`](MarkInstanceBuilder::default_instance) provides
/// sensible defaults for every field so that users only need to bind the
/// attributes they care about.
pub trait MarkInstanceBuilder: Mark {
    /// The GPU-ready instance type produced by this builder.
    type Instance: bytemuck::Pod + bytemuck::Zeroable;

    /// Build an instance by overlaying the given attribute values on top of
    /// the default instance.
    fn build_instance(attrs: &[(&str, AttrValue)]) -> Self::Instance;

    /// Return a default instance with sensible placeholder values.
    fn default_instance() -> Self::Instance;
}

/// Mark types should implement the mark::Mark trait
pub use crate::mark::Mark;

/// Global counter for generating unique selection IDs
static NEXT_SELECTION_ID: AtomicU32 = AtomicU32::new(0);

/// Event handler function type
///
/// Event handlers receive a mutable reference to the event to support propagation control.
pub type EventHandlerFn<T> = Box<dyn Fn(&mut InteractionEvent, &T) + Send + Sync>;

/// Trait for data types that can provide interaction geometry.
///
/// Implement this trait to enable your data types to work with the interaction system.
pub trait InteractionData: Send + Sync {
    /// Extract position for this data item
    fn position(&self) -> [f32; 2];

    /// Extract size for this data item.
    ///
    /// For circles, size should be [radius, 0].
    /// For rectangles, size should be [width, height].
    /// Default implementation provides a radius of 10.0 for circles.
    fn size(&self) -> [f32; 2] {
        [10.0, 0.0] // Default: circle with radius 10.0
    }
}

/// Capitalize the first letter of a string.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Configuration for ARIA live-region announcements when selection data changes.
///
/// Controls how screen readers are notified when data in a [`Selection`]
/// changes via [`set_data`](Selection::set_data) or attribute updates.
///
/// # Examples
///
/// ```rust,ignore
/// use gup::selection::AriaUpdateConfig;
/// use gup::accessibility::AriaLive;
///
/// let config = AriaUpdateConfig {
///     urgency: AriaLive::Assertive,
///     ..Default::default()
/// };
/// selection.set_aria_update_config(config);
/// ```
#[derive(Debug, Clone)]
pub struct AriaUpdateConfig {
    /// The urgency level for live-region announcements.
    ///
    /// - [`AriaLive::Polite`] — announced when the screen reader is idle
    ///   (default).
    /// - [`AriaLive::Assertive`] — announced immediately, interrupting the
    ///   current speech.
    /// - [`AriaLive::Off`] — no announcements.
    pub urgency: crate::accessibility::aria::AriaLive,

    /// Whether live-region announcements are enabled.  When `false`, data
    /// changes still update the ARIA tree but no screen reader announcement is
    /// queued.
    pub announce_changes: bool,
}

impl Default for AriaUpdateConfig {
    fn default() -> Self {
        Self {
            urgency: crate::accessibility::aria::AriaLive::Polite,
            announce_changes: true,
        }
    }
}

/// Trait for marks that provide accessible descriptions.
///
/// Implement this trait to enable automatic ARIA tree generation for a mark
/// type.  The methods produce human-readable text that screen readers use to
/// describe individual data points and overall data patterns.
///
/// Default implementations are provided so that only `describe_point()` is
/// required for a minimal implementation.
pub trait AccessibleMark: Mark {
    /// Generate an accessible description for a single data point.
    ///
    /// `index` is the zero-based position within the dataset.
    /// `total` is the total number of data points.
    /// `attrs` contains the bound attribute values for this point.
    fn describe_point(index: usize, total: usize, attrs: &[(&str, AttrValue)]) -> String;

    /// Human-readable name for the mark type (e.g. "circle", "line").
    fn describe_mark_type() -> &'static str {
        "mark"
    }

    /// Optionally describe a high-level pattern detected in the data.
    fn describe_pattern(_all_attrs: &[Vec<(&str, AttrValue)>]) -> Option<String> {
        None
    }
}

/// Selection type for managing data-driven visualizations with GPU acceleration and interaction.
///
/// This type provides:
/// - GPU-accelerated rendering of large datasets
/// - Interactive event handling (click, hover, drag)
/// - Spatial queries for hit testing
/// - Shader function composition for visual attributes
///
/// # GPU Rendering
///
/// The Selection manages a complete GPU rendering pipeline for its bound mark type.
/// Call [`prepare_render`](Selection::prepare_render) to upload data and then
/// [`render`](Selection::render) inside a render pass.
///
/// Pipelines are created once per mark type and cached. Instance buffers resize
/// automatically when the data set grows. Bind groups are rebuilt only when the
/// underlying storage buffer changes.
pub struct Selection<T, M: Mark> {
    /// Unique identifier for this selection
    selection_id: u32,
    /// Data items in this selection
    data: Vec<T>,
    /// Render context for GPU operations (optional — needed only for interaction)
    context: Option<Arc<RenderContext>>,
    /// Event handlers keyed by event type
    event_handlers: Arc<Mutex<HashMap<String, Vec<EventHandlerFn<T>>>>>,
    /// Mark type phantom
    _mark: PhantomData<M>,
    /// GPU render state, lazily initialised via prepare_render()
    render_state: Option<SelectionRenderState>,
    /// Named attribute bindings stored via [`attr`](Self::attr) /
    /// [`attr_parallel`](Self::attr_parallel).
    attr_bindings: Vec<AttributeBinding<T>>,
    /// GPU shader function bindings stored via [`attr_shader`](Self::attr_shader).
    shader_attr_bindings: Vec<ShaderAttributeBinding<T>>,
    /// Whether automatic ARIA registration is enabled (default: `true`).
    auto_aria: bool,
    /// Root ARIA node ID registered for this selection (if any).
    aria_root_node: Option<crate::accessibility::aria::NodeId>,
    /// Whether the ARIA tree needs to be regenerated due to data or attribute
    /// changes.  Set by [`set_data`] and [`attr`]/[`attr_parallel`]/[`attr_shader`];
    /// cleared by [`sync_aria_from_context`] or [`register_aria`].
    aria_dirty: bool,
    /// Data count at the time of the last ARIA registration (used for change
    /// summaries).
    aria_previous_data_count: Option<usize>,
    /// Configuration for ARIA live-region announcements on data changes.
    aria_update_config: AriaUpdateConfig,
}

impl<T, M: Mark> std::fmt::Debug for Selection<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selection")
            .field("selection_id", &self.selection_id)
            .field("data_count", &self.data.len())
            .field("render_ready", &self.render_state.is_some())
            .finish()
    }
}

impl<T, M: Mark> Selection<T, M> {
    /// Create a new selection from data and render context.
    ///
    /// The render context is used for interaction features (hit testing).
    /// For rendering-only use cases, prefer [`from_data`](Self::from_data).
    pub fn new(data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self> {
        let selection_id = NEXT_SELECTION_ID.fetch_add(1, Ordering::Relaxed);
        Ok(Self {
            selection_id,
            data,
            context: Some(context),
            event_handlers: Arc::new(Mutex::new(HashMap::new())),
            _mark: PhantomData,
            render_state: None,
            attr_bindings: Vec::new(),
            shader_attr_bindings: Vec::new(),
            auto_aria: true,
            aria_root_node: None,
            aria_dirty: false,
            aria_previous_data_count: None,
            aria_update_config: AriaUpdateConfig::default(),
        })
    }

    /// Create a render-only selection from data (no interaction context).
    ///
    /// Use this when you only need GPU rendering via
    /// [`prepare_render`](Self::prepare_render) / [`render`](Self::render)
    /// and do not need the interaction system.
    pub fn from_data(data: Vec<T>) -> Self {
        let selection_id = NEXT_SELECTION_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            selection_id,
            data,
            context: None,
            event_handlers: Arc::new(Mutex::new(HashMap::new())),
            _mark: PhantomData,
            render_state: None,
            attr_bindings: Vec::new(),
            shader_attr_bindings: Vec::new(),
            auto_aria: true,
            aria_root_node: None,
            aria_dirty: false,
            aria_previous_data_count: None,
            aria_update_config: AriaUpdateConfig::default(),
        }
    }

    /// Get the unique ID of this selection
    pub fn selection_id(&self) -> u32 {
        self.selection_id
    }

    /// Register an event handler for a specific event type.
    ///
    /// # Arguments
    ///
    /// * `event_type` - The type of event to handle (e.g., "click", "hover", "drag")
    /// * `handler` - Closure that receives the event and the data item
    ///
    /// # Examples
    ///
    /// ```ignore
    /// selection.on("click", |event, data| {
    ///     println!("Clicked on data at position: {:?}", event.screen_position);
    /// });
    /// ```
    pub fn on<F>(&mut self, event_type: &str, handler: F) -> &mut Self
    where
        F: Fn(&mut InteractionEvent, &T) + Send + Sync + 'static,
    {
        {
            let mut handlers = self.event_handlers.lock().unwrap();
            handlers
                .entry(event_type.to_string())
                .or_default()
                .push(Box::new(handler));
        }
        self
    }

    /// Trigger event handlers for a specific event on a data item.
    ///
    /// This is called internally when the interaction system detects an event on an element.
    /// Supports event propagation control via stop_propagation() and stop_immediate_propagation().
    pub fn trigger_event(&self, event_type: &str, event: &mut InteractionEvent, element_id: u32) {
        if let Some(data_item) = self.data.get(element_id as usize) {
            let handlers = self.event_handlers.lock().unwrap();
            if let Some(event_handlers) = handlers.get(event_type) {
                for handler in event_handlers {
                    // Stop executing handlers if immediate propagation was stopped
                    if event.is_immediate_propagation_stopped() {
                        break;
                    }
                    handler(event, data_item);
                }
            }
        }
    }

    /// Bind a named attribute to a closure that extracts a value from each data item.
    ///
    /// The closure `binding` is called once per data item when
    /// [`prepare_render_bound`](Self::prepare_render_bound) is invoked, and
    /// the returned value is fed into the mark's
    /// [`MarkInstanceBuilder`] to construct GPU instance data.
    ///
    /// # Type Safety
    ///
    /// The return type `V` must implement [`IntoAttrValue`], which is only
    /// implemented for GPU-compatible types (`f32`, `[f32; 2]`, `[f32; 4]`,
    /// [`Vec2`](crate::shader_function::Vec2),
    /// [`Vec4`](crate::shader_function::Vec4)).
    /// Attempting to bind an unsupported type produces a compile-time error.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// selection
    ///     .attr("center", |d: &MyData| [d.x, d.y])
    ///     .attr("radius", |d: &MyData| d.size * 0.1)
    ///     .attr("fill_color", |d: &MyData| [d.r, d.g, d.b, 1.0]);
    /// ```
    pub fn attr<V, F>(&mut self, name: &str, binding: F) -> &mut Self
    where
        F: Fn(&T) -> V + Send + Sync + 'static,
        V: IntoAttrValue,
    {
        self.attr_bindings.push(AttributeBinding {
            name: name.to_string(),
            extractor: Box::new(move |t| binding(t).into_attr_value()),
        });
        // Invalidate GPU state — new bindings require re-upload.
        self.render_state = None;
        // New bindings change data-point descriptions.
        self.aria_dirty = true;
        self
    }

    /// Bind a named attribute to a GPU shader function.
    ///
    /// The `extractor` closure extracts a raw input value from each data item
    /// on the CPU.  Instead of transforming the value on the CPU (as
    /// [`attr`](Self::attr) does), the raw value is uploaded to the GPU and
    /// `shader_fn` is executed in the vertex shader.
    ///
    /// This provides three benefits:
    /// 1. **Performance**: GPU parallelism for complex transformations on large
    ///    datasets.
    /// 2. **Re-mapping**: Changing shader function parameters (e.g. scale
    ///    domain) only requires a uniform buffer update — no data re-upload.
    /// 3. **Composition**: Shader functions integrate with the
    ///    [`ComposableShaderFunction`] composition system.
    ///
    /// # Type Safety
    ///
    /// The shader function's output WGSL type must match the mark attribute's
    /// expected type.  This is validated at `prepare_render_shader_bound` time.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let scale = LinearScale::new(0.0, 100.0, -1.0, 1.0);
    /// selection
    ///     .attr("center", |d: &MyData| [d.x, d.y])       // CPU binding
    ///     .attr_shader("radius", |d: &MyData| d.size, scale); // GPU binding
    /// ```
    pub fn attr_shader<V, F, S>(&mut self, name: &str, extractor: F, shader_fn: S) -> &mut Self
    where
        F: Fn(&T) -> V + Send + Sync + 'static,
        V: IntoAttrValue,
        S: ComposableShaderFunction + 'static,
        S::Uniforms: ShaderUniform,
    {
        let info = shader_fn_info_from(&shader_fn);
        self.shader_attr_bindings.push(ShaderAttributeBinding {
            name: name.to_string(),
            extractor: Box::new(move |t| extractor(t).into_attr_value()),
            shader_fn: info,
        });
        // Invalidate GPU state.
        self.render_state = None;
        // New bindings change data-point descriptions.
        self.aria_dirty = true;
        self
    }

    /// Returns `true` if any GPU shader function bindings have been set.
    pub fn has_shader_bindings(&self) -> bool {
        !self.shader_attr_bindings.is_empty()
    }

    /// Get the names of GPU shader-function-bound attributes.
    pub fn shader_bound_attributes(&self) -> Vec<&str> {
        self.shader_attr_bindings
            .iter()
            .map(|b| b.name.as_str())
            .collect()
    }

    /// Update the shader function parameters for a named GPU-bound attribute
    /// **without** re-uploading instance data.
    ///
    /// This is the key performance feature for interactive re-mapping: when the
    /// user adjusts a scale domain/range or a colour palette, only the small
    /// uniform buffer (typically 16–64 bytes) is written to the GPU.  The
    /// instance storage buffer — which may hold megabytes of raw data for large
    /// datasets — is left untouched.
    ///
    /// # Arguments
    ///
    /// * `name`  — Attribute name that was previously bound via
    ///   [`attr_shader`](Self::attr_shader).
    /// * `new_shader_fn` — A new shader function instance with updated
    ///   parameters.  It must be the *same function type* (same WGSL code,
    ///   same input/output types) — only the uniform values may differ.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `name` does not match any GPU-bound attribute.
    /// - The render state has not been initialised (call
    ///   [`prepare_render_bound`](Self::prepare_render_bound) at least once
    ///   first).
    /// - The new shader function's output type differs from the original
    ///   binding.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Initial binding.
    /// let scale = LinearScale::new(0.0, 100.0, 0.01, 0.2);
    /// selection.attr_shader("radius", |d: &MyData| d.value, scale);
    /// selection.prepare_render_bound(&device, &queue, None)?;
    ///
    /// // Later: change the scale range without touching instance data.
    /// let new_scale = LinearScale::new(0.0, 100.0, 0.05, 0.5);
    /// selection.update_shader_uniforms("radius", new_scale, &queue)?;
    /// ```
    pub fn update_shader_uniforms<S>(
        &mut self,
        name: &str,
        new_shader_fn: S,
        queue: &Queue,
    ) -> GupResult<()>
    where
        S: ComposableShaderFunction + 'static,
        S::Uniforms: ShaderUniform,
    {
        // 1. Find the binding index for `name`.
        let binding_index = self
            .shader_attr_bindings
            .iter()
            .position(|b| b.name == name)
            .ok_or_else(|| {
                crate::error::GupError::validation_error(format!(
                    "No GPU shader binding found for attribute '{name}'"
                ))
            })?;

        // 2. Validate the new shader function matches the original.
        let existing = &self.shader_attr_bindings[binding_index].shader_fn;
        let new_info = shader_fn_info_from(&new_shader_fn);

        if new_info.output_wgsl_type != existing.output_wgsl_type {
            return Err(crate::error::GupError::validation_error(format!(
                "Shader function output type mismatch for attribute '{name}': \
                 existing '{}', new '{}'",
                existing.output_wgsl_type, new_info.output_wgsl_type,
            )));
        }

        // 3. Ensure render state is initialised.
        let state = self.render_state.as_ref().ok_or_else(|| {
            crate::error::GupError::validation_error(
                "Cannot update shader uniforms before prepare_render_bound() has been called"
                    .to_string(),
            )
        })?;

        // 4. Write the new uniform bytes to the GPU buffer.
        if let Some(buf) = state.uniform_buffers.get(binding_index)
            && !new_info.uniform_bytes.is_empty()
        {
            queue.write_buffer(buf, 0, &new_info.uniform_bytes);
        }

        // 5. Update the stored binding so that future prepare_render_shader_bound
        //    calls (e.g., after data changes) use the latest parameters.
        self.shader_attr_bindings[binding_index]
            .shader_fn
            .uniform_bytes = new_info.uniform_bytes;

        Ok(())
    }

    /// Bind multiple attributes from a single closure that returns a tuple.
    ///
    /// This is more efficient than separate [`attr`](Self::attr) calls when
    /// multiple attributes are computed from the same data — the closure is
    /// called only once per data item.
    ///
    /// # Arguments
    ///
    /// * `parallel_function` — A closure returning a tuple of values.
    ///   Tuples of 2 and 3 elements are supported; each element must
    ///   implement [`IntoAttrValue`].
    /// * `attribute_names` — An array of attribute names corresponding
    ///   positionally to the tuple elements.
    ///
    /// # Type Safety
    ///
    /// The compiler verifies that each tuple element is a valid GPU attribute
    /// type. Binding a closure that returns `(String, bool)` will fail to
    /// compile.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Bind position and colour in one pass over the data.
    /// selection.attr_parallel(
    ///     |d: &MyData| ([d.x, d.y], [d.r, d.g, d.b, 1.0]),
    ///     ["center", "fill_color"],
    /// );
    /// ```
    ///
    /// ```rust,ignore
    /// // Three-way parallel binding: position, colour, and radius.
    /// selection.attr_parallel(
    ///     |d: &MyData| ([d.x, d.y], [d.r, d.g, d.b, 1.0], d.size),
    ///     ["center", "fill_color", "radius"],
    /// );
    /// ```
    pub fn attr_parallel<P, const N: usize>(
        &mut self,
        parallel_function: P,
        attribute_names: [&str; N],
    ) -> &mut Self
    where
        P: IntoAttrValues<T, N>,
    {
        // Wrap the parallel function in an Arc so it can be shared across
        // the N per-attribute closures.
        let shared = Arc::new(parallel_function);
        for (idx, name) in attribute_names.iter().enumerate() {
            let f = Arc::clone(&shared);
            self.attr_bindings.push(AttributeBinding {
                name: name.to_string(),
                extractor: Box::new(move |t| f.extract(t)[idx]),
            });
        }
        // Invalidate GPU state.
        self.render_state = None;
        // New bindings change data-point descriptions.
        self.aria_dirty = true;
        self
    }

    /// Get the data in this selection.
    pub fn data(&self) -> &[T] {
        &self.data
    }

    /// Get the render context (if available).
    pub fn context(&self) -> Option<&Arc<RenderContext>> {
        self.context.as_ref()
    }

    /// Get the number of items in this selection.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if this selection is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Replace the data in this selection.
    ///
    /// This invalidates the GPU render state; call
    /// [`prepare_render`](Self::prepare_render) again before the next
    /// [`render`](Self::render) call.
    pub fn set_data(&mut self, data: Vec<T>) {
        self.data = data;
        // Invalidate render state so next prepare_render re-uploads.
        self.render_state = None;
        // Mark ARIA tree as needing a refresh.
        self.aria_dirty = true;
    }

    /// Prepare GPU resources for rendering this selection.
    ///
    /// The `mapper` closure converts each data item `T` into a GPU-ready
    /// instance struct `I` that matches the mark's WGSL storage buffer layout.
    /// For example, `CircleInstance::from` for `Circle` marks or
    /// `RectangleInstance::from` for `Rectangle` marks.
    ///
    /// On the first call this creates the render pipeline, vertex buffer,
    /// instance storage buffer and bind group.  On subsequent calls with
    /// changed data it re-uploads instances and rebuilds the bind group only
    /// if the buffer was reallocated.
    ///
    /// Pass a [`PipelineCache`] to share render pipelines across Selections
    /// of the same mark type.  When `cache` is `None` a new pipeline is
    /// created for every Selection.
    ///
    /// Pass a [`BufferPool`] to allocate instance buffers from a shared pool
    /// instead of creating one-off GPU buffers.  This reduces allocation
    /// overhead in high-churn scenarios (e.g. animated transitions).
    ///
    /// # Errors
    ///
    /// Returns an error if pipeline or buffer creation fails.
    pub fn prepare_render<I>(
        &mut self,
        device: &Device,
        queue: &Queue,
        mapper: impl Fn(&T) -> I,
        cache: Option<&mut PipelineCache>,
        pool: Option<&mut BufferPool>,
    ) -> GupResult<()>
    where
        I: bytemuck::Pod + bytemuck::Zeroable,
    {
        // Convert data items to GPU instances.
        let instances: Vec<I> = self.data.iter().map(&mapper).collect();
        let instance_bytes: &[u8] = bytemuck::cast_slice(&instances);
        let instance_count = instances.len() as u32;

        self.upload_instances(device, queue, instance_bytes, instance_count, cache, pool)
    }

    /// Prepare GPU resources using the attribute bindings stored via
    /// [`attr`](Self::attr) / [`attr_parallel`](Self::attr_parallel).
    ///
    /// This method replaces the manual `mapper` closure required by
    /// [`prepare_render`](Self::prepare_render). Instead, it evaluates the
    /// stored attribute bindings for each data item and uses the mark's
    /// [`MarkInstanceBuilder`] to construct GPU instance data.
    ///
    /// Pass a [`PipelineCache`] to share render pipelines across Selections
    /// of the same mark type.
    ///
    /// Pass a [`BufferPool`] to allocate instance buffers from a shared pool
    /// instead of creating one-off GPU buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if no attribute bindings have been set, or if GPU
    /// resource creation fails.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// selection
    ///     .attr("center", |d: &MyData| [d.x, d.y])
    ///     .attr("radius", |d: &MyData| d.size * 0.1)
    ///     .attr("fill_color", |d: &MyData| [d.r, d.g, d.b, 1.0])
    ///     .prepare_render_bound(&device, &queue, None, None)?;
    /// ```
    pub fn prepare_render_bound(
        &mut self,
        device: &Device,
        queue: &Queue,
        cache: Option<&mut PipelineCache>,
        pool: Option<&mut BufferPool>,
    ) -> GupResult<()>
    where
        M: MarkInstanceBuilder,
    {
        // If there are GPU shader bindings, delegate to the shader-aware path.
        if !self.shader_attr_bindings.is_empty() {
            return self.prepare_render_shader_bound(device, queue);
        }

        if self.attr_bindings.is_empty() {
            return Err(crate::error::GupError::validation_error(
                "No attribute bindings set — call attr() before prepare_render_bound()".to_string(),
            ));
        }

        let bindings = &self.attr_bindings;
        let instances: Vec<M::Instance> = self
            .data
            .iter()
            .map(|t| {
                let attr_values: Vec<(&str, AttrValue)> = bindings
                    .iter()
                    .map(|b| (b.name.as_str(), (b.extractor)(t)))
                    .collect();
                M::build_instance(&attr_values)
            })
            .collect();

        let instance_bytes: &[u8] = bytemuck::cast_slice(&instances);
        let instance_count = instances.len() as u32;

        self.upload_instances(device, queue, instance_bytes, instance_count, cache, pool)
    }

    /// Prepare GPU resources with shader function bindings.
    ///
    /// This method handles the case where some (or all) attribute bindings use
    /// GPU shader functions.  CPU-bound attributes are evaluated as normal;
    /// GPU-bound attributes upload raw input values and the transformation is
    /// deferred to a generated vertex shader.
    ///
    /// # Pipeline
    ///
    /// A custom render pipeline is created with:
    /// - A modified vertex shader that includes shader function code and
    ///   uniform bindings for each GPU-bound attribute.
    /// - The mark's original fragment shader (unchanged).
    /// - A bind group layout with the instance storage buffer at binding 0
    ///   and one uniform buffer per GPU-bound attribute at bindings 1…N.
    ///
    /// # Errors
    ///
    /// Returns an error if no bindings are set, or if GPU resource creation
    /// fails.
    fn prepare_render_shader_bound(&mut self, device: &Device, queue: &Queue) -> GupResult<()>
    where
        M: MarkInstanceBuilder,
    {
        if self.attr_bindings.is_empty() && self.shader_attr_bindings.is_empty() {
            return Err(crate::error::GupError::validation_error(
                "No attribute bindings set".to_string(),
            ));
        }

        // Type safety: validate shader function output matches attribute type.
        for sb in &self.shader_attr_bindings {
            if !M::is_attribute_compatible(&sb.name, sb.shader_fn.output_wgsl_type) {
                let expected = M::get_attribute_type(&sb.name)
                    .unwrap_or("(unknown)")
                    .to_string();
                return Err(crate::error::GupError::validation_error(format!(
                    "Shader function output type '{}' is not compatible with attribute '{}' \
                     (expected '{}')",
                    sb.shader_fn.output_wgsl_type, sb.name, expected,
                )));
            }
        }

        // 1. Build instance data: CPU bindings produce final values; GPU
        //    bindings produce raw input values (the shader function will
        //    transform them on the GPU).
        let cpu_bindings = &self.attr_bindings;
        let gpu_bindings = &self.shader_attr_bindings;

        let instances: Vec<M::Instance> = self
            .data
            .iter()
            .map(|t| {
                let mut attr_values: Vec<(&str, AttrValue)> = cpu_bindings
                    .iter()
                    .map(|b| (b.name.as_str(), (b.extractor)(t)))
                    .collect();
                // GPU-bound attrs: extract the *raw* value (the shader will transform it).
                // Widen the value to match the instance field type if necessary
                // (e.g., a FunctionChain may input f32 but the field is vec4).
                for sb in gpu_bindings {
                    let raw = (sb.extractor)(t);
                    attr_values.push((
                        sb.name.as_str(),
                        widen_attr_value(raw, sb.shader_fn.output_wgsl_type),
                    ));
                }
                M::build_instance(&attr_values)
            })
            .collect();

        let instance_bytes: &[u8] = bytemuck::cast_slice(&instances);
        let instance_count = instances.len() as u32;

        // 2. Generate modified vertex shader with shader function injection.
        let base_vertex_wgsl = M::VERTEX_SHADER
            .ok_or_else(|| {
                crate::error::GupError::validation_error(
                    "Shader function bindings require a mark with a hand-written vertex shader"
                        .to_string(),
                )
            })?
            .to_string();

        let shader_bindings_info: Vec<(&str, &ShaderFnInfo)> = gpu_bindings
            .iter()
            .map(|sb| (sb.name.as_str(), &sb.shader_fn))
            .collect();

        let modified_vertex_wgsl =
            generate_shader_bound_vertex_wgsl(&base_vertex_wgsl, &shader_bindings_info);

        let fragment_wgsl = M::FRAGMENT_SHADER
            .ok_or_else(|| {
                crate::error::GupError::validation_error(
                    "Shader function bindings require a mark with a hand-written fragment shader"
                        .to_string(),
                )
            })?
            .to_string();

        // 3. Create (or update) render state with the generated pipeline.
        if let Some(ref mut state) = self.render_state {
            // Re-use existing pipeline (shader functions don't change between
            // frames — only uniform values might).  Re-upload instances.
            if instance_bytes.len() > state.instance_buffer_capacity {
                let (instance_buffer, bind_group) =
                    Self::create_shader_bound_buffers_and_bind_group(
                        device,
                        &state.pipeline,
                        instance_bytes,
                        gpu_bindings,
                    );
                state.instance_buffer = instance_buffer;
                state.bind_group = bind_group;
                state.instance_buffer_capacity = instance_bytes.len();
            } else {
                queue.write_buffer(&state.instance_buffer, 0, instance_bytes);
                // Also re-upload uniform data (shader function params may have
                // changed).
                Self::update_uniform_buffers(queue, &state.uniform_buffers, gpu_bindings);
            }
            state.instance_count = instance_count;
        } else {
            let state = SelectionRenderState::new_with_shader_fns::<M, T>(
                device,
                instance_bytes,
                instance_count,
                &modified_vertex_wgsl,
                &fragment_wgsl,
                gpu_bindings,
            )?;
            self.render_state = Some(state);
        }

        Ok(())
    }

    /// Update uniform buffers for shader function bindings (re-upload params).
    fn update_uniform_buffers(
        queue: &Queue,
        uniform_buffers: &[wgpu::Buffer],
        gpu_bindings: &[ShaderAttributeBinding<T>],
    ) {
        for (i, sb) in gpu_bindings.iter().enumerate() {
            if let Some(buf) = uniform_buffers.get(i)
                && !sb.shader_fn.uniform_bytes.is_empty()
            {
                queue.write_buffer(buf, 0, &sb.shader_fn.uniform_bytes);
            }
        }
    }

    /// Create instance buffer, uniform buffers, and bind group for shader
    /// function bindings.
    fn create_shader_bound_buffers_and_bind_group(
        device: &Device,
        pipeline: &wgpu::RenderPipeline,
        instance_bytes: &[u8],
        gpu_bindings: &[ShaderAttributeBinding<T>],
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("selection_shader_instance_buffer"),
            contents: if instance_bytes.is_empty() {
                &[0u8; 16]
            } else {
                instance_bytes
            },
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Create uniform buffers for each shader function.
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: instance_buffer.as_entire_binding(),
        }];

        let uniform_buffers: Vec<wgpu::Buffer> = gpu_bindings
            .iter()
            .enumerate()
            .map(|(i, sb)| {
                let contents = if sb.shader_fn.uniform_bytes.is_empty() {
                    &[0u8; 16][..]
                } else {
                    &sb.shader_fn.uniform_bytes
                };
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("shader_fn_uniform_{i}")),
                    contents,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                })
            })
            .collect();

        for (i, buf) in uniform_buffers.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: (i + 1) as u32,
                resource: buf.as_entire_binding(),
            });
        }

        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("selection_shader_bind_group"),
            layout: &bind_group_layout,
            entries: &entries,
        });

        // Note: uniform_buffers are dropped here but the bind_group holds refs.
        // For update support we store them in SelectionRenderState.
        // For now this is only used on first creation; updates go through
        // update_uniform_buffers with the stored buffers.
        (instance_buffer, bind_group)
    }

    /// Get the names of currently bound attributes.
    pub fn bound_attributes(&self) -> Vec<&str> {
        self.attr_bindings.iter().map(|b| b.name.as_str()).collect()
    }

    /// Returns `true` if any attribute bindings (CPU or GPU) have been set.
    pub fn has_attr_bindings(&self) -> bool {
        !self.attr_bindings.is_empty() || !self.shader_attr_bindings.is_empty()
    }

    /// Internal helper: upload pre-computed instance bytes to the GPU.
    fn upload_instances(
        &mut self,
        device: &Device,
        queue: &Queue,
        instance_bytes: &[u8],
        instance_count: u32,
        cache: Option<&mut PipelineCache>,
        mut pool: Option<&mut BufferPool>,
    ) -> GupResult<()> {
        if let Some(ref mut state) = self.render_state {
            // Re-use existing pipeline and vertex buffers.
            // If the instance buffer is too small, reallocate.
            if instance_bytes.len() > state.instance_buffer_capacity {
                // Return the old buffer to the pool before allocating a new one.
                if let Some((bt, sc)) = state.pool_meta.take() {
                    let old_buffer = std::mem::replace(
                        &mut state.instance_buffer,
                        // Temporary placeholder — will be overwritten below.
                        device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("selection_instance_placeholder"),
                            size: 16,
                            usage: wgpu::BufferUsages::STORAGE,
                            mapped_at_creation: false,
                        }),
                    );
                    if let Some(ref mut p) = pool {
                        p.deallocate_raw(old_buffer, bt, sc);
                    }
                    // else: no pool provided at reallocation — drop the old buffer.
                }

                let (instance_buffer, bind_group, pool_meta) =
                    SelectionRenderState::create_instance_buffer_and_bind_group(
                        device,
                        &state.pipeline,
                        instance_bytes,
                        state.viewport_buffer.as_ref(),
                        queue,
                        pool.as_deref_mut(),
                    );
                state.instance_buffer = instance_buffer;
                state.bind_group = bind_group;
                state.instance_buffer_capacity = instance_bytes.len();
                state.pool_meta = pool_meta;
            } else {
                // Buffer large enough: just re-upload.
                queue.write_buffer(&state.instance_buffer, 0, instance_bytes);
            }
            state.instance_count = instance_count;
        } else {
            // First-time setup: create everything.
            let state = SelectionRenderState::new::<M>(
                device,
                queue,
                instance_bytes,
                instance_count,
                cache,
                pool,
            )?;
            self.render_state = Some(state);
        }

        Ok(())
    }

    /// Render the selection to an active render pass.
    ///
    /// [`prepare_render`](Self::prepare_render) **must** be called at least
    /// once before the first `render` call.  The render pass must have been
    /// created in the same frame (single render pass rule).
    ///
    /// Issues instanced draw calls using the mark's hand-optimised shaders,
    /// with instance data streamed via a storage buffer at `@group(0)
    /// @binding(0)`.
    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> GupResult<()> {
        let state = self.render_state.as_ref().ok_or_else(|| {
            crate::error::GupError::render_error(
                "Selection render state not initialised — call prepare_render() first".to_string(),
            )
        })?;

        if state.instance_count == 0 {
            return Ok(());
        }

        render_pass.set_pipeline(&state.pipeline);
        render_pass.set_bind_group(0, &state.bind_group, &[]);
        render_pass.set_vertex_buffer(0, state.vertex_buffer.slice(..));

        if let (Some(index_count), Some(index_buffer)) = (state.index_count, &state.index_buffer) {
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..index_count, 0, 0..state.instance_count);
        } else {
            render_pass.draw(0..state.vertex_count, 0..state.instance_count);
        }

        Ok(())
    }

    /// Returns `true` if the selection has been prepared for rendering.
    pub fn is_render_ready(&self) -> bool {
        self.render_state.is_some()
    }

    /// Release the instance buffer back to a [`BufferPool`] for reuse.
    ///
    /// Call this before dropping a Selection whose instance buffer was
    /// allocated from a pool (i.e., a pool was passed to
    /// [`prepare_render`](Self::prepare_render)).  If the instance buffer
    /// was *not* pool-allocated this method is a no-op.
    ///
    /// After calling this the Selection is no longer render-ready — you
    /// must call `prepare_render` again to re-create the render state.
    pub fn release_to_pool(&mut self, pool: &mut BufferPool) {
        if let Some(mut state) = self.render_state.take() {
            if let Some((bt, sc)) = state.pool_meta.take() {
                pool.deallocate_raw(state.instance_buffer, bt, sc);
            }
        }
    }

    /// Set the viewport dimensions for pixel-space SDF calculations.
    ///
    /// Marks that use SDF-based rendering (such as BoxPlot) interpret values
    /// like `stroke_width` and `outlier_radius` as pixel measurements. This
    /// method updates the viewport uniform so those pixel values map correctly
    /// to clip-space distances regardless of window size.
    ///
    /// Call this whenever the render target size changes (e.g. on window
    /// resize) and before calling [`render`](Self::render).
    ///
    /// # Panics
    ///
    /// This method is a no-op if [`prepare_render`](Self::prepare_render) has
    /// not been called yet.
    pub fn set_viewport_size(&self, queue: &Queue, width: f32, height: f32) {
        if let Some(ref state) = self.render_state
            && let Some(ref vp_buf) = state.viewport_buffer
        {
            let viewport = ViewportUniforms { width, height };
            queue.write_buffer(vp_buf, 0, bytemuck::bytes_of(&viewport));
        }
    }

    /// Enable or disable automatic ARIA registration on prepare/render.
    ///
    /// When enabled (the default), calling [`prepare_render`](Self::prepare_render)
    /// or [`prepare_render_bound`](Self::prepare_render_bound) will automatically
    /// generate and register an ARIA tree with the provided
    /// [`AriaTree`](crate::accessibility::aria::AriaTree).
    ///
    /// Set this to `false` if you need full manual control over ARIA tree
    /// construction.
    pub fn set_auto_aria(&mut self, enabled: bool) -> &mut Self {
        self.auto_aria = enabled;
        self
    }

    /// Returns `true` if automatic ARIA registration is enabled.
    pub fn auto_aria(&self) -> bool {
        self.auto_aria
    }

    /// Returns the ARIA root node ID registered for this selection, if any.
    pub fn aria_root_node(&self) -> Option<crate::accessibility::aria::NodeId> {
        self.aria_root_node
    }

    /// Returns `true` if the ARIA tree needs to be regenerated due to data or
    /// attribute changes since the last [`sync_aria_from_context`] or
    /// [`register_aria`] call.
    pub fn is_aria_dirty(&self) -> bool {
        self.aria_dirty
    }

    /// Set the configuration for ARIA live-region announcements.
    pub fn set_aria_update_config(&mut self, config: AriaUpdateConfig) -> &mut Self {
        self.aria_update_config = config;
        self
    }

    /// Get the current ARIA update configuration.
    pub fn aria_update_config(&self) -> &AriaUpdateConfig {
        &self.aria_update_config
    }

    /// Generate and register an ARIA tree for this selection.
    ///
    /// Creates a chart-level ARIA node describing the dataset, plus individual
    /// data-point nodes (capped at `max_points` to avoid DOM bloat).
    ///
    /// If the mark type implements [`AccessibleMark`], each data-point node
    /// uses the mark-specific description; otherwise a generic description is
    /// used.
    ///
    /// Any previously registered ARIA tree for this selection is removed first.
    /// If a focused node is inside the old sub-tree, the focus is restored to
    /// the equivalent position in the new sub-tree (or the chart root if the
    /// focused index no longer exists).
    ///
    /// Returns the root [`NodeId`](crate::accessibility::aria::NodeId) of the
    /// newly created sub-tree.
    pub fn register_aria(
        &mut self,
        aria_tree: &mut crate::accessibility::aria::AriaTree,
    ) -> crate::accessibility::aria::NodeId
    where
        M: AccessibleMark,
    {
        use crate::accessibility::aria::{AriaNode, AriaRole};

        // --- Focus preservation ---
        // Determine the index of the currently focused child within this
        // selection's sub-tree so we can restore it after the rebuild.
        let focused_child_index = self.focused_child_index(aria_tree);

        // --- Change summary ---
        let old_count = self.aria_previous_data_count;
        let new_count = self.data.len();

        // Remove any previous registration for this selection.
        if let Some(old_root) = self.aria_root_node.take() {
            aria_tree.remove_subtree(old_root);
        }

        let total = new_count;
        let mark_name = M::describe_mark_type();
        let label = format!(
            "{} chart with {} data point{}",
            capitalize(mark_name),
            total,
            if total == 1 { "" } else { "s" }
        );

        // Evaluate attribute bindings for each point (needed for descriptions).
        let all_attrs: Vec<Vec<(&str, AttrValue)>> = self
            .data
            .iter()
            .map(|t| {
                self.attr_bindings
                    .iter()
                    .map(|b| (b.name.as_str(), (b.extractor)(t)))
                    .collect()
            })
            .collect();

        // Build pattern description if available.
        let description = M::describe_pattern(&all_attrs);

        let chart_id = aria_tree.create_chart_node(label, description);

        // Add individual data-point nodes (cap at 100 to avoid bloat).
        let max_points = 100;
        let point_count = total.min(max_points);
        for (i, attrs) in all_attrs.iter().enumerate().take(point_count) {
            let point_label = M::describe_point(i, total, attrs);
            let node = AriaNode::new(AriaRole::DataPoint, point_label);
            aria_tree.add_child(chart_id, node);
        }

        if total > max_points {
            let note = AriaNode::new(
                AriaRole::DataPoint,
                format!("… and {} more data points", total - max_points),
            );
            aria_tree.add_child(chart_id, note);
        }

        self.aria_root_node = Some(chart_id);
        self.aria_dirty = false;
        self.aria_previous_data_count = Some(new_count);

        // --- Restore focus ---
        if let Some(idx) = focused_child_index
            && let Some(chart_node) = aria_tree.get_node(chart_id)
        {
            let children = &chart_node.children;
            // If the previously focused index still exists, refocus it;
            // otherwise fall back to the chart root.
            let new_focus = children.get(idx).copied().unwrap_or(chart_id);
            aria_tree.set_focus(Some(new_focus));
        }

        // --- Live region announcement ---
        if self.aria_update_config.announce_changes
            && let Some(summary) = Self::summarise_change(old_count, new_count, mark_name)
        {
            aria_tree.update_live_region_with_urgency(
                &format!("selection-{}", self.selection_id),
                &summary,
                self.aria_update_config.urgency,
            );
        }

        chart_id
    }

    /// Remove the ARIA sub-tree previously registered for this selection.
    ///
    /// This is called automatically when the selection is dropped or when
    /// a new ARIA tree replaces the old one.
    pub fn deregister_aria(&mut self, aria_tree: &mut crate::accessibility::aria::AriaTree) {
        if let Some(root) = self.aria_root_node.take() {
            aria_tree.remove_subtree(root);
        }
    }

    /// Automatically register or update the ARIA tree from the
    /// [`RenderContext`]'s accessibility system.
    ///
    /// This is the recommended way to keep the ARIA tree in sync with data.
    /// If `auto_aria` is enabled (the default), and the selection's
    /// [`RenderContext`] has an [`AccessibilitySystem`] attached, this method
    /// generates or refreshes the ARIA tree.
    ///
    /// On the first call, the ARIA tree is always generated.  On subsequent
    /// calls the tree is only regenerated when a data or attribute change has
    /// been detected (i.e. [`is_aria_dirty`](Self::is_aria_dirty) is `true`),
    /// avoiding unnecessary work during steady-state rendering.
    ///
    /// Call this after [`prepare_render`](Self::prepare_render) or
    /// [`prepare_render_bound`](Self::prepare_render_bound) to ensure the
    /// ARIA tree reflects the current data.
    ///
    /// Returns `true` if the ARIA tree was (re-)registered.
    pub fn sync_aria_from_context(&mut self) -> bool
    where
        M: AccessibleMark,
    {
        if !self.auto_aria {
            return false;
        }

        // Skip if tree already exists and nothing has changed.
        if self.aria_root_node.is_some() && !self.aria_dirty {
            return false;
        }

        // Clone the Arc to release the immutable borrow on self.
        let acc = self
            .context
            .as_ref()
            .and_then(|ctx| ctx.accessibility().cloned());

        let Some(acc) = acc else {
            return false;
        };

        if let Ok(mut system) = acc.lock() {
            self.register_aria(&mut system.aria_tree);
            true
        } else {
            false
        }
    }

    /// Register the data points in this selection as focusable elements.
    ///
    /// This is a convenience wrapper around
    /// [`SelectionFocusBridge::sync_focus_elements`](crate::accessibility::SelectionFocusBridge::sync_focus_elements).
    /// The `descriptor_fn` maps each data item to a [`FocusPointDescriptor`]
    /// that specifies the screen position, label, and optional value.
    ///
    /// Returns the number of elements registered (may be capped by max_elements).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use gup::accessibility::selection_focus::{SelectionFocusBridge, FocusPointDescriptor};
    /// use gup::accessibility::FocusManager;
    ///
    /// let mut bridge = SelectionFocusBridge::new(Default::default());
    /// let mut fm = FocusManager::new();
    ///
    /// selection.register_focus_elements(&mut bridge, &mut fm, |item, idx| {
    ///     FocusPointDescriptor {
    ///         position: item.position(),
    ///         label: format!("Point {}", idx),
    ///         value: None,
    ///     }
    /// });
    /// ```
    pub fn register_focus_elements<F>(
        &self,
        bridge: &mut crate::accessibility::SelectionFocusBridge,
        focus_manager: &mut crate::accessibility::FocusManager,
        descriptor_fn: F,
    ) -> usize
    where
        F: Fn(&T, usize) -> crate::accessibility::FocusPointDescriptor,
    {
        bridge.sync_focus_elements(&self.data, focus_manager, descriptor_fn)
    }

    // ------------------------------------------------------------------
    // Private helpers for reactive ARIA updates
    // ------------------------------------------------------------------

    /// Determine the index of the currently focused child within this
    /// selection's ARIA sub-tree.  Returns `None` if there is no focus or the
    /// focus is outside this sub-tree.
    fn focused_child_index(
        &self,
        aria_tree: &crate::accessibility::aria::AriaTree,
    ) -> Option<usize> {
        let root = self.aria_root_node?;
        let focus = aria_tree.get_focus()?;
        let chart = aria_tree.get_node(root)?;
        chart.children.iter().position(|c| *c == focus)
    }

    /// Generate a human-readable summary of a data change.
    ///
    /// Returns `None` when this is the initial registration (no previous data).
    fn summarise_change(
        old_count: Option<usize>,
        new_count: usize,
        mark_name: &str,
    ) -> Option<String> {
        let old = old_count?;
        if old == new_count {
            // Data count unchanged — could be attribute-only update.
            Some(format!("{} chart data updated", capitalize(mark_name)))
        } else if new_count > old {
            let added = new_count - old;
            Some(format!(
                "{} new data point{} added, {} total",
                added,
                if added == 1 { "" } else { "s" },
                new_count,
            ))
        } else {
            let removed = old - new_count;
            Some(format!(
                "{} data point{} removed, {} total",
                removed,
                if removed == 1 { "" } else { "s" },
                new_count,
            ))
        }
    }
}

impl<T, M: Mark> Drop for Selection<T, M> {
    fn drop(&mut self) {
        // Automatically deregister ARIA tree when selection is dropped.
        if let Some(root) = self.aria_root_node.take()
            && let Some(acc) = self
                .context
                .as_ref()
                .and_then(|ctx| ctx.accessibility().cloned())
            && let Ok(mut system) = acc.lock()
        {
            system.aria_tree.remove_subtree(root);
        }
    }
}

/// Internal GPU state for rendering a Selection's data.
///
/// Holds the render pipeline, vertex/index/instance buffers, and bind group.
/// Created by [`Selection::prepare_render`] and consumed by
/// [`Selection::render`].
struct SelectionRenderState {
    /// Render pipeline created from the mark's shaders (shared via Arc when
    /// a [`PipelineCache`] is used).
    pipeline: Arc<wgpu::RenderPipeline>,
    /// Vertex buffer containing the mark's base geometry (e.g., unit quad).
    vertex_buffer: wgpu::Buffer,
    /// Optional index buffer for indexed rendering.
    index_buffer: Option<wgpu::Buffer>,
    /// Storage buffer holding per-instance data.
    instance_buffer: wgpu::Buffer,
    /// Bind group referencing the instance storage buffer.
    bind_group: wgpu::BindGroup,
    /// Number of vertices in the base geometry.
    vertex_count: u32,
    /// Number of indices (None for non-indexed marks).
    index_count: Option<u32>,
    /// Number of instances to draw.
    instance_count: u32,
    /// Current byte capacity of the instance buffer.
    instance_buffer_capacity: usize,
    /// Viewport dimensions uniform buffer (for marks with custom shaders).
    /// Enables pixel-space SDF calculations in shaders.
    viewport_buffer: Option<wgpu::Buffer>,
    /// Uniform buffers for GPU shader function bindings (empty when no shader
    /// functions are used).
    uniform_buffers: Vec<wgpu::Buffer>,
    /// Pool metadata for the instance buffer, if allocated from a
    /// [`BufferPool`]. Stores `(buffer_type, size_class)` so the buffer can
    /// be returned to the pool on reallocation or release.
    pool_meta: Option<(BufferType, usize)>,
}

impl SelectionRenderState {
    /// Create a complete render state for a mark type.
    fn new<M: Mark>(
        device: &Device,
        queue: &Queue,
        instance_bytes: &[u8],
        instance_count: u32,
        cache: Option<&mut PipelineCache>,
        pool: Option<&mut BufferPool>,
    ) -> GupResult<Self> {
        // --- Pipeline ------------------------------------------------
        let pipeline: Arc<wgpu::RenderPipeline> = match cache {
            Some(c) => c.get_or_create::<M>(device)?,
            None => {
                let mark_info = MarkInfoImpl::<M>::new();
                Arc::new(mark_info.create_render_pipeline(device)?)
            }
        };

        // --- Vertex buffer -------------------------------------------
        let vertices = M::generate_vertices();
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&vertices);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("selection_vertex_buffer"),
            contents: vertex_bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });

        // --- Index buffer (optional) ---------------------------------
        let index_buffer = M::generate_indices().map(|indices| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("selection_index_buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            })
        });

        // --- Viewport buffer (for custom-shader marks) ---------------
        let has_custom = M::VERTEX_SHADER.is_some() && M::FRAGMENT_SHADER.is_some();
        let viewport_buffer = if has_custom {
            let default_viewport = ViewportUniforms {
                width: 800.0,
                height: 600.0,
            };
            Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("selection_viewport_uniform"),
                    contents: bytemuck::bytes_of(&default_viewport),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                }),
            )
        } else {
            None
        };

        // --- Instance buffer + bind group ----------------------------
        let (instance_buffer, bind_group, pool_meta) =
            Self::create_instance_buffer_and_bind_group(
                device,
                &pipeline,
                instance_bytes,
                viewport_buffer.as_ref(),
                queue,
                pool,
            );

        let vertex_count = M::vertex_count() as u32;
        let index_count = M::index_count().map(|c| c as u32);

        Ok(Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            bind_group,
            vertex_count,
            index_count,
            instance_count,
            instance_buffer_capacity: instance_bytes.len(),
            viewport_buffer,
            uniform_buffers: Vec::new(),
            pool_meta,
        })
    }

    /// Create (or recreate) the instance storage buffer and matching bind group.
    ///
    /// When `pool` is provided the buffer is allocated from the
    /// [`BufferPool`]; otherwise a one-off buffer is created via
    /// `device.create_buffer_init`.  The returned `Option<(BufferType,
    /// usize)>` is the pool metadata that must be stored on the render state
    /// so the buffer can later be returned to the pool.
    fn create_instance_buffer_and_bind_group(
        device: &Device,
        pipeline: &wgpu::RenderPipeline,
        instance_bytes: &[u8],
        viewport_buffer: Option<&wgpu::Buffer>,
        queue: &Queue,
        pool: Option<&mut BufferPool>,
    ) -> (wgpu::Buffer, wgpu::BindGroup, Option<(BufferType, usize)>) {
        let effective_bytes = if instance_bytes.is_empty() {
            &[0u8; 16][..]
        } else {
            instance_bytes
        };

        let (instance_buffer, pool_meta) = if let Some(pool) = pool {
            let (buf, size_class) =
                pool.allocate_raw(BufferType::Storage, effective_bytes.len());
            queue.write_buffer(&buf, 0, effective_bytes);
            (buf, Some((BufferType::Storage, size_class)))
        } else {
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("selection_instance_buffer"),
                contents: effective_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });
            (buf, None)
        };

        // Derive bind group layout from the pipeline (guaranteed to match).
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: instance_buffer.as_entire_binding(),
        }];

        if let Some(vp_buf) = viewport_buffer {
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: vp_buf.as_entire_binding(),
            });
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("selection_bind_group"),
            layout: &bind_group_layout,
            entries: &entries,
        });

        (instance_buffer, bind_group, pool_meta)
    }

    /// Create a render state with a generated vertex shader that includes
    /// shader function transformations.
    fn new_with_shader_fns<M: Mark, T>(
        device: &Device,
        instance_bytes: &[u8],
        instance_count: u32,
        vertex_wgsl: &str,
        fragment_wgsl: &str,
        gpu_bindings: &[ShaderAttributeBinding<T>],
    ) -> GupResult<Self> {
        // --- Shader modules ------------------------------------------
        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selection_shader_fn_vertex"),
            source: wgpu::ShaderSource::Wgsl(vertex_wgsl.into()),
        });
        let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selection_shader_fn_fragment"),
            source: wgpu::ShaderSource::Wgsl(fragment_wgsl.into()),
        });

        // --- Bind group layout: binding 0 = instance storage,
        //     bindings 1..N = uniform buffers for shader functions.
        let mut bgl_entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];
        for (i, _sb) in gpu_bindings.iter().enumerate() {
            bgl_entries.push(wgpu::BindGroupLayoutEntry {
                binding: (i + 1) as u32,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("selection_shader_fn_bgl"),
            entries: &bgl_entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("selection_shader_fn_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = Arc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("selection_shader_fn_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_module,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<M::Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: M::vertex_attributes(),
                    }],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            }),
        );

        // --- Vertex buffer -------------------------------------------
        let vertices = M::generate_vertices();
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&vertices);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("selection_shader_fn_vertex_buffer"),
            contents: vertex_bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });

        // --- Index buffer (optional) ---------------------------------
        let index_buffer = M::generate_indices().map(|indices| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("selection_shader_fn_index_buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            })
        });

        // --- Instance buffer -----------------------------------------
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("selection_shader_fn_instance_buffer"),
            contents: if instance_bytes.is_empty() {
                &[0u8; 16]
            } else {
                instance_bytes
            },
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // --- Uniform buffers for shader functions --------------------
        let uniform_buffers: Vec<wgpu::Buffer> = gpu_bindings
            .iter()
            .enumerate()
            .map(|(i, sb)| {
                let contents = if sb.shader_fn.uniform_bytes.is_empty() {
                    &[0u8; 16][..]
                } else {
                    &sb.shader_fn.uniform_bytes
                };
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("selection_shader_fn_uniform_{i}")),
                    contents,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                })
            })
            .collect();

        // --- Bind group ----------------------------------------------
        let mut bg_entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: instance_buffer.as_entire_binding(),
        }];
        for (i, buf) in uniform_buffers.iter().enumerate() {
            bg_entries.push(wgpu::BindGroupEntry {
                binding: (i + 1) as u32,
                resource: buf.as_entire_binding(),
            });
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("selection_shader_fn_bind_group"),
            layout: &bind_group_layout,
            entries: &bg_entries,
        });

        let vertex_count = M::vertex_count() as u32;
        let index_count = M::index_count().map(|c| c as u32);

        Ok(Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            bind_group,
            vertex_count,
            index_count,
            instance_count,
            instance_buffer_capacity: instance_bytes.len(),
            viewport_buffer: None,
            uniform_buffers,
            pool_meta: None,
        })
    }
}

// ---------------------------------------------------------------------------
// WGSL generation for shader function attribute bindings
// ---------------------------------------------------------------------------

/// Widen an [`AttrValue`] to match the target field type.
///
/// When a shader function chain has an input type narrower than the instance
/// field type (e.g., `f32` input stored in a `vec4<f32>` field), the raw value
/// must be widened so that [`MarkInstanceBuilder::build_instance`] can store it.
fn widen_attr_value(value: AttrValue, target_wgsl_type: &str) -> AttrValue {
    match (value, target_wgsl_type) {
        // f32 → vec2: store in first component.
        (AttrValue::Float(v), "vec2<f32>") => AttrValue::Vec2([v, 0.0]),
        // f32 → vec4: store in first component.
        (AttrValue::Float(v), "vec4<f32>") => AttrValue::Vec4([v, 0.0, 0.0, 0.0]),
        // vec2 → vec4: store in first two components.
        (AttrValue::Vec2(v), "vec4<f32>") => AttrValue::Vec4([v[0], v[1], 0.0, 0.0]),
        // No conversion needed.
        (v, _) => v,
    }
}

/// Return a WGSL expression that narrows an instance field to the shader
/// function's expected input type.
///
/// For example, if the field is `vec4<f32>` but the function expects `f32`,
/// this returns `"instance.fill_color.x"` instead of `"instance.fill_color"`.
fn narrow_field_expr(attr: &str, input_type: &str, output_type: &str) -> String {
    let base = format!("instance.{attr}");
    if input_type == output_type {
        return base;
    }
    match (input_type, output_type) {
        ("f32", "vec2<f32>") | ("f32", "vec4<f32>") => format!("{base}.x"),
        ("vec2<f32>", "vec4<f32>") => format!("{base}.xy"),
        _ => base,
    }
}

/// Generate a modified vertex shader that applies shader functions to
/// instance attributes.
///
/// Takes the mark's original vertex shader WGSL and injects:
/// 1. Uniform struct definitions for each shader function.
/// 2. Uniform buffer binding declarations (`@group(0) @binding(N)`).
/// 3. Shader function code.
/// 4. Variable declarations that apply the shader function to the raw
///    instance field after loading (`let _gup_<attr> = fn(instance.<attr>, ..)`).
/// 5. Replacements of `instance.<attr>` with the transformed variable in
///    the rest of `vs_main`.
fn generate_shader_bound_vertex_wgsl(
    base_wgsl: &str,
    bindings: &[(&str, &ShaderFnInfo)],
) -> String {
    if bindings.is_empty() {
        return base_wgsl.to_string();
    }

    let mut result = String::with_capacity(base_wgsl.len() + 2048);

    // Split at `@vertex` to inject declarations before the entry point.
    let (before_vertex, at_vertex) = match base_wgsl.find("@vertex") {
        Some(pos) => (&base_wgsl[..pos], &base_wgsl[pos..]),
        None => {
            // Fallback: return unmodified if we can't find the entry point.
            return base_wgsl.to_string();
        }
    };

    // --- Part 1: everything before @vertex (struct defs, storage buffer) ---
    result.push_str(before_vertex);

    // --- Part 2: inject uniform struct defs, bindings, and function code ---
    result.push_str("// --- Gup shader function bindings ---\n");

    // Collect and deduplicate struct definitions across all bindings.
    // A single `uniform_struct_def` may contain multiple struct definitions
    // (e.g., ChainUniforms includes nested component struct defs).  We split
    // on `struct ` boundaries and deduplicate by struct name to avoid WGSL
    // redefinition errors.
    let mut emitted_structs = std::collections::HashSet::new();
    for (_attr_name, info) in bindings.iter() {
        let struct_def = &info.uniform_struct_def;
        if struct_def.is_empty() || struct_def == "f32" || struct_def == "i32" {
            continue;
        }
        for individual_def in split_wgsl_struct_definitions(struct_def) {
            if let Some(name) = extract_wgsl_struct_name(individual_def)
                && emitted_structs.insert(name.to_string())
            {
                result.push_str(individual_def.trim());
                result.push('\n');
            }
        }
    }

    for (i, (_attr_name, info)) in bindings.iter().enumerate() {
        // Uniform binding declaration.
        result.push_str(&format!(
            "@group(0) @binding({}) var<uniform> _gup_uniforms_{}: {};\n",
            i + 1,
            i,
            info.uniform_type_name,
        ));
    }
    result.push('\n');

    // Shader function code (deduplicated by function name).
    let mut emitted_fns = std::collections::HashSet::new();
    for (_attr_name, info) in bindings {
        if emitted_fns.insert(&info.function_name) {
            result.push_str(info.wgsl_code.trim());
            result.push_str("\n\n");
        }
    }

    // --- Part 3: the @vertex function with transformations ---------------
    // Find the line with `let instance = instances[` and insert
    // transformation statements right after it.
    let instance_load_marker = "let instance = instances[";
    if let Some(marker_pos) = at_vertex.find(instance_load_marker) {
        // Find the end of the `let instance = ...;` line.
        let after_marker = &at_vertex[marker_pos..];
        let semicolon = after_marker.find(';').unwrap_or(after_marker.len() - 1);
        let insert_pos = marker_pos + semicolon + 1;

        // Write everything up to and including the instance load.
        result.push_str(&at_vertex[..insert_pos]);
        result.push('\n');

        // Insert shader function application statements.
        for (i, (attr_name, info)) in bindings.iter().enumerate() {
            let input_expr =
                narrow_field_expr(attr_name, info.input_wgsl_type, info.output_wgsl_type);
            result.push_str(&format!(
                "    let _gup_{attr} = {fn_name}({input_expr}, _gup_uniforms_{i});\n",
                attr = attr_name,
                fn_name = info.function_name,
                i = i,
            ));
        }

        // Write the rest of the function, replacing `instance.<attr>` with
        // `_gup_<attr>` for each shader-bound attribute.
        let remaining = &at_vertex[insert_pos..];
        let mut modified_remaining = remaining.to_string();
        for (attr_name, _info) in bindings {
            let search = format!("instance.{attr_name}");
            let replace = format!("_gup_{attr_name}");
            modified_remaining = modified_remaining.replace(&search, &replace);
        }
        result.push_str(&modified_remaining);
    } else {
        // Fallback: just append the vertex function as-is.
        result.push_str(at_vertex);
    }

    result
}

/// Extracts the struct name from a WGSL struct definition like
/// `"struct Foo {\n    bar: f32,\n}"`.
///
/// Returns `None` if the string doesn't start with `struct `.
fn extract_wgsl_struct_name(def: &str) -> Option<&str> {
    let trimmed = def.trim();
    let rest = trimmed.strip_prefix("struct ")?;
    // The struct name ends at the first whitespace or `{`.
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '{')
        .unwrap_or(rest.len());
    let name = &rest[..end];
    if name.is_empty() { None } else { Some(name) }
}

/// Splits a WGSL string that may contain multiple struct definitions into
/// individual definitions.
///
/// Each element in the returned vec starts with `struct ` and ends after its
/// closing `}`.  Non-struct content between definitions is discarded.
fn split_wgsl_struct_definitions(defs: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut search_from = 0;
    while search_from < defs.len() {
        // Find the next `struct ` keyword.
        let start = match defs[search_from..].find("struct ") {
            Some(pos) => search_from + pos,
            None => break,
        };
        // Find the matching closing brace.  Struct definitions in WGSL are
        // always single-level (no nested braces), so the first `}` ends the
        // definition.
        let end = match defs[start..].find('}') {
            Some(pos) => start + pos + 1, // include the `}`
            None => break,
        };
        result.push(&defs[start..end]);
        search_from = end;
    }
    result
}
///
/// This implementation extracts element data from the selection for hit testing.
/// Data types must implement the `InteractionData` trait to provide position and size information.
impl<T, M: Mark> Renderable for Selection<T, M>
where
    T: InteractionData,
{
    fn get_elements_for_interaction(&self) -> GupResult<Vec<InteractionElement>> {
        let elements: Vec<InteractionElement> = self
            .data
            .iter()
            .map(|data_item| InteractionElement {
                position: data_item.position(),
                size: data_item.size(),
                mark_type: get_mark_type_id::<M>(),
            })
            .collect();

        Ok(elements)
    }

    fn selection_id(&self) -> u32 {
        self.selection_id
    }
}

/// Get a numeric mark type identifier for a mark.
///
/// This maps mark types to the numeric IDs expected by the GPU hit test shader.
/// The IDs are provided by the MarkTypeIdProvider trait, which is automatically
/// implemented using the #[derive(MarkTypeId)] macro.
///
/// For marks that don't implement MarkTypeIdProvider, falls back to type name matching
/// for backward compatibility:
/// - 0 = Circle
/// - 1 = Rectangle
/// - 2 = Line
fn get_mark_type_id<M: Mark>() -> u32 {
    // Try to use MarkTypeIdProvider if available, otherwise fall back to type name matching
    use crate::mark::{Circle, Line, Rectangle};

    let type_id = std::any::TypeId::of::<M>();

    if type_id == std::any::TypeId::of::<Circle>() {
        Circle::MARK_TYPE_ID
    } else if type_id == std::any::TypeId::of::<Rectangle>() {
        Rectangle::MARK_TYPE_ID
    } else if type_id == std::any::TypeId::of::<Line>() {
        Line::MARK_TYPE_ID
    } else {
        // Fallback to type name matching for custom marks
        let type_name = std::any::type_name::<M>();
        if type_name.contains("Circle") {
            0
        } else if type_name.contains("Rectangle") {
            1
        } else if type_name.contains("Line") {
            2
        } else {
            // Default to circle for unknown mark types
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::mark::circle::{CircleAttributes, CircleInstance};
    use crate::mark::rectangle::{RectangleAttributes, RectangleInstance};
    use crate::mark::{BoxPlot, Circle, Line, Rectangle};
    use crate::shader_function::{Vec2, Vec4};

    // --- Unit tests (no GPU) ---

    // -- AttrValue & IntoAttrValue tests --

    #[test]
    fn attr_value_from_f32() {
        let v: AttrValue = 3.14f32.into_attr_value();
        assert_eq!(v, AttrValue::Float(3.14));
    }

    #[test]
    fn attr_value_from_array2() {
        let v: AttrValue = [1.0f32, 2.0].into_attr_value();
        assert_eq!(v, AttrValue::Vec2([1.0, 2.0]));
    }

    #[test]
    fn attr_value_from_array4() {
        let v: AttrValue = [1.0f32, 0.0, 0.0, 1.0].into_attr_value();
        assert_eq!(v, AttrValue::Vec4([1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn attr_value_from_vec2() {
        let v: AttrValue = Vec2 { x: 5.0, y: 6.0 }.into_attr_value();
        assert_eq!(v, AttrValue::Vec2([5.0, 6.0]));
    }

    #[test]
    fn attr_value_from_vec4() {
        let v: AttrValue = Vec4 {
            x: 0.1,
            y: 0.2,
            z: 0.3,
            w: 0.4,
        }
        .into_attr_value();
        assert_eq!(v, AttrValue::Vec4([0.1, 0.2, 0.3, 0.4]));
    }

    // -- attr() binding storage tests --

    #[test]
    fn attr_stores_single_binding() {
        let data = vec![CircleAttributes::default()];
        let mut selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

        assert!(!selection.has_attr_bindings());
        selection.attr("center", |a: &CircleAttributes| [a.center.x, a.center.y]);
        assert!(selection.has_attr_bindings());
        assert_eq!(selection.bound_attributes(), vec!["center"]);
    }

    #[test]
    fn attr_chaining_stores_multiple_bindings() {
        let data = vec![CircleAttributes::default()];
        let mut selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

        selection
            .attr("center", |a: &CircleAttributes| [a.center.x, a.center.y])
            .attr("radius", |a: &CircleAttributes| a.radius)
            .attr("fill_color", |a: &CircleAttributes| {
                [
                    a.fill_color.x,
                    a.fill_color.y,
                    a.fill_color.z,
                    a.fill_color.w,
                ]
            });

        assert_eq!(
            selection.bound_attributes(),
            vec!["center", "radius", "fill_color"]
        );
    }

    #[test]
    fn attr_parallel_stores_two_bindings() {
        let data = vec![CircleAttributes::default()];
        let mut selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

        selection.attr_parallel(
            |a: &CircleAttributes| ([a.center.x, a.center.y], a.radius),
            ["center", "radius"],
        );

        assert_eq!(selection.bound_attributes(), vec!["center", "radius"]);
    }

    #[test]
    fn attr_parallel_stores_three_bindings() {
        let data = vec![CircleAttributes::default()];
        let mut selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

        selection.attr_parallel(
            |a: &CircleAttributes| {
                (
                    [a.center.x, a.center.y],
                    a.radius,
                    [
                        a.fill_color.x,
                        a.fill_color.y,
                        a.fill_color.z,
                        a.fill_color.w,
                    ],
                )
            },
            ["center", "radius", "fill_color"],
        );

        assert_eq!(
            selection.bound_attributes(),
            vec!["center", "radius", "fill_color"]
        );
    }

    #[test]
    fn set_data_preserves_attr_bindings() {
        let mut selection: Selection<CircleAttributes, Circle> =
            Selection::from_data(vec![CircleAttributes::default()]);

        selection.attr("center", |a: &CircleAttributes| [a.center.x, a.center.y]);
        assert!(selection.has_attr_bindings());

        selection.set_data(vec![]);
        // Bindings should be preserved after set_data
        assert!(selection.has_attr_bindings());
        assert_eq!(selection.bound_attributes(), vec!["center"]);
    }

    #[test]
    fn prepare_render_bound_without_bindings_returns_error() {
        // prepare_render_bound requires at least one binding
        let selection: Selection<CircleAttributes, Circle> =
            Selection::from_data(vec![CircleAttributes::default()]);

        // Can't call prepare_render_bound in unit test (needs GPU), but we can
        // verify the error path exists via the method signature. The actual
        // GPU test below exercises this fully.
        assert!(!selection.has_attr_bindings());
    }

    #[test]
    fn from_data_creates_selection() {
        let data = vec![
            CircleAttributes {
                center: Vec2 { x: 0.0, y: 0.0 },
                radius: 0.1,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            },
            CircleAttributes {
                center: Vec2 { x: 0.5, y: 0.5 },
                radius: 0.2,
                fill_color: Vec4 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            },
        ];

        let selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);
        assert_eq!(selection.len(), 2);
        assert!(!selection.is_render_ready());
        assert!(selection.context().is_none());
    }

    #[test]
    fn set_data_invalidates_render_state() {
        let mut selection: Selection<CircleAttributes, Circle> =
            Selection::from_data(vec![CircleAttributes {
                center: Vec2 { x: 0.0, y: 0.0 },
                radius: 0.1,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            }]);

        assert_eq!(selection.len(), 1);

        selection.set_data(vec![]);
        assert_eq!(selection.len(), 0);
        assert!(!selection.is_render_ready());
    }

    #[test]
    fn render_without_prepare_returns_error() {
        let selection: Selection<CircleAttributes, Circle> = Selection::from_data(vec![]);
        // We can't call render() without a RenderPass, but we can verify state.
        assert!(!selection.is_render_ready());
    }

    #[test]
    fn circle_instance_from_attributes() {
        let attrs = CircleAttributes {
            center: Vec2 { x: 0.5, y: -0.3 },
            radius: 0.1,
            fill_color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 0.02,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        let instance = CircleInstance::from(&attrs);
        assert_eq!(instance.center, [0.5, -0.3]);
        assert_eq!(instance.radius, 0.1);
        assert_eq!(instance.fill_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(instance.stroke_width, 0.02);
        assert_eq!(instance.stroke_color, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn rectangle_instance_from_attributes() {
        let attrs = RectangleAttributes {
            center: Vec2 { x: 0.0, y: 0.0 },
            size: Vec2 { x: 0.5, y: 0.3 },
            fill_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 1.0,
            },
            stroke_width: 0.01,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            corner_radius: 0.05,
        };

        let instance = RectangleInstance::from(&attrs);
        assert_eq!(instance.center, [0.0, 0.0]);
        assert_eq!(instance.size, [0.5, 0.3]);
        assert_eq!(instance.fill_color, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(instance.stroke_width, 0.01);
        assert_eq!(instance.stroke_color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(instance.corner_radius, 0.05);
    }

    #[test]
    fn register_focus_elements_bridges_to_focus_manager() {
        use crate::accessibility::FocusManager;
        use crate::accessibility::selection_focus::{FocusPointDescriptor, SelectionFocusBridge};

        let data = vec![
            CircleAttributes {
                center: Vec2 { x: 0.1, y: 0.2 },
                radius: 0.05,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            },
            CircleAttributes {
                center: Vec2 { x: 0.5, y: 0.6 },
                radius: 0.1,
                fill_color: Vec4 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            },
        ];

        let selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);
        let mut bridge = SelectionFocusBridge::new(Default::default());
        let mut fm = FocusManager::new();

        let count = selection.register_focus_elements(&mut bridge, &mut fm, |attr, idx| {
            FocusPointDescriptor {
                position: [attr.center.x, attr.center.y],
                label: format!("Circle {}", idx),
                value: Some(attr.radius as f64),
            }
        });

        assert_eq!(count, 2);
        assert_eq!(bridge.last_sync_count(), 2);

        // Navigate and verify.
        fm.handle_key_input(crate::accessibility::keyboard::KeyEvent::Tab);
        let desc = fm.describe_current_focus().unwrap();
        assert!(desc.contains("Circle 0"));
    }

    // --- ARIA registration tests (no GPU) ---

    #[test]
    fn aria_generate_empty_selection() {
        let sel = Selection::<(), Circle>::from_data(vec![]);
        let mut tree = crate::accessibility::aria::AriaTree::new();
        // We need a mutable selection to call register_aria
        let mut sel = sel;
        let root = sel.register_aria(&mut tree);

        let node = tree.get_node(root).unwrap();
        assert_eq!(node.label, "Circle chart with 0 data points");
        assert!(node.children.is_empty());
    }

    #[test]
    fn aria_generate_with_data_and_bindings() {
        struct Pt {
            x: f32,
            y: f32,
        }

        let mut sel = Selection::<Pt, Circle>::from_data(vec![
            Pt { x: 10.0, y: 20.0 },
            Pt { x: 30.0, y: 40.0 },
        ]);
        sel.attr("center", |p: &Pt| [p.x, p.y]);
        sel.attr("radius", |_: &Pt| 5.0f32);

        let mut tree = crate::accessibility::aria::AriaTree::new();
        let root = sel.register_aria(&mut tree);

        let node = tree.get_node(root).unwrap();
        assert_eq!(node.label, "Circle chart with 2 data points");
        assert_eq!(node.children.len(), 2);

        // Check that child node has point description
        let child = tree.get_node(node.children[0]).unwrap();
        assert!(child.label.contains("Point 1 of 2"));
    }

    #[test]
    fn aria_no_duplicate_registration() {
        let mut sel = Selection::<(), Circle>::from_data(vec![]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        let root1 = sel.register_aria(&mut tree);
        assert!(tree.get_node(root1).is_some());

        // Second registration should replace the first
        let root2 = sel.register_aria(&mut tree);
        assert!(tree.get_node(root1).is_none(), "old root should be removed");
        assert!(tree.get_node(root2).is_some());
    }

    #[test]
    fn aria_deregister_cleans_up() {
        let mut sel = Selection::<(), Circle>::from_data(vec![]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        let root = sel.register_aria(&mut tree);
        assert!(tree.get_node(root).is_some());

        sel.deregister_aria(&mut tree);
        assert!(tree.get_node(root).is_none());
        assert!(sel.aria_root_node().is_none());
    }

    #[test]
    fn aria_opt_out_flag() {
        let mut sel = Selection::<(), Circle>::from_data(vec![]);
        assert!(sel.auto_aria());

        sel.set_auto_aria(false);
        assert!(!sel.auto_aria());
    }

    #[test]
    fn aria_truncates_large_datasets() {
        let data: Vec<u32> = (0..200).collect();
        let mut sel = Selection::<u32, Circle>::from_data(data);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        let root = sel.register_aria(&mut tree);
        let node = tree.get_node(root).unwrap();
        assert_eq!(node.label, "Circle chart with 200 data points");
        // 100 point nodes + 1 truncation note = 101
        assert_eq!(node.children.len(), 101);

        let last = tree.get_node(*node.children.last().unwrap()).unwrap();
        assert!(last.label.contains("100 more data points"));
    }

    #[test]
    fn aria_line_mark_description() {
        use crate::mark::Line;

        struct Seg {
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
        }

        let mut sel = Selection::<Seg, Line>::from_data(vec![Seg {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 5.0,
        }]);
        sel.attr("start", |s: &Seg| [s.x1, s.y1]);
        sel.attr("end", |s: &Seg| [s.x2, s.y2]);

        let mut tree = crate::accessibility::aria::AriaTree::new();
        let root = sel.register_aria(&mut tree);

        let node = tree.get_node(root).unwrap();
        assert_eq!(node.label, "Line chart with 1 data point");
        assert_eq!(node.children.len(), 1);

        let child = tree.get_node(node.children[0]).unwrap();
        assert!(child.label.contains("Line 1 of 1"));
        assert!(child.label.contains("from (0.0, 0.0)"));
        assert!(child.label.contains("to (10.0, 5.0)"));
    }

    #[test]
    fn aria_rectangle_mark_description() {
        let mut sel = Selection::<(), Rectangle>::from_data(vec![()]);
        sel.attr("center", |_: &()| [50.0f32, 100.0]);
        sel.attr("size", |_: &()| [20.0f32, 30.0]);

        let mut tree = crate::accessibility::aria::AriaTree::new();
        let root = sel.register_aria(&mut tree);

        let node = tree.get_node(root).unwrap();
        assert_eq!(node.label, "Rectangle chart with 1 data point");

        let child = tree.get_node(node.children[0]).unwrap();
        assert!(child.label.contains("Rectangle 1 of 1"));
        assert!(child.label.contains("at (50.0, 100.0)"));
        assert!(
            child.label.contains("20.0×30.0"),
            "expected size in label: {}",
            child.label
        );
    }

    // --- GPU integration tests (sync_aria_from_context) ---

    #[test]
    fn gpu_sync_aria_from_context() {
        pollster::block_on(async {
            let render_ctx = match crate::RenderContext::new().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            // Attach accessibility system to the render context.
            let acc = std::sync::Arc::new(std::sync::Mutex::new(
                crate::accessibility::AccessibilitySystem::new(),
            ));
            let mut render_ctx = render_ctx;
            render_ctx.set_accessibility(acc.clone());

            let ctx = std::sync::Arc::new(render_ctx);

            let mut sel =
                Selection::<(), Circle>::new(vec![(), (), ()], ctx).expect("selection creation");
            sel.attr("center", |_: &()| [0.1f32, 0.2]);

            // sync should register ARIA
            assert!(sel.sync_aria_from_context());
            assert!(sel.aria_root_node().is_some());

            // Verify the ARIA tree contents.
            let system = acc.lock().unwrap();
            let root = sel.aria_root_node().unwrap();
            let node = system.aria_tree.get_node(root).unwrap();
            assert_eq!(node.label, "Circle chart with 3 data points");
            assert_eq!(node.children.len(), 3);
        });
    }

    #[test]
    fn gpu_sync_aria_opt_out() {
        pollster::block_on(async {
            let render_ctx = match crate::RenderContext::new().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let acc = std::sync::Arc::new(std::sync::Mutex::new(
                crate::accessibility::AccessibilitySystem::new(),
            ));
            let mut render_ctx = render_ctx;
            render_ctx.set_accessibility(acc.clone());

            let ctx = std::sync::Arc::new(render_ctx);

            let mut sel = Selection::<(), Circle>::new(vec![()], ctx).expect("selection creation");
            sel.set_auto_aria(false);

            // Should not register when opted out.
            assert!(!sel.sync_aria_from_context());
            assert!(sel.aria_root_node().is_none());
        });
    }

    #[test]
    fn gpu_sync_aria_no_accessibility_system() {
        pollster::block_on(async {
            let render_ctx = match crate::RenderContext::new().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            // No accessibility system attached.
            let ctx = std::sync::Arc::new(render_ctx);

            let mut sel = Selection::<(), Circle>::new(vec![()], ctx).expect("selection creation");

            // Should not register when no accessibility system.
            assert!(!sel.sync_aria_from_context());
            assert!(sel.aria_root_node().is_none());
        });
    }

    #[test]
    fn gpu_sync_aria_updates_on_second_call() {
        pollster::block_on(async {
            let render_ctx = match crate::RenderContext::new().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let acc = std::sync::Arc::new(std::sync::Mutex::new(
                crate::accessibility::AccessibilitySystem::new(),
            ));
            let mut render_ctx = render_ctx;
            render_ctx.set_accessibility(acc.clone());

            let ctx = std::sync::Arc::new(render_ctx);

            let mut sel =
                Selection::<(), Circle>::new(vec![(), ()], ctx).expect("selection creation");

            // First sync.
            sel.sync_aria_from_context();
            let root1 = sel.aria_root_node().unwrap();

            {
                let system = acc.lock().unwrap();
                assert_eq!(
                    system.aria_tree.get_node(root1).unwrap().label,
                    "Circle chart with 2 data points"
                );
            }

            // Change data and re-sync — should replace the old tree.
            sel.set_data(vec![(), (), (), ()]);
            sel.sync_aria_from_context();
            let root2 = sel.aria_root_node().unwrap();

            let system = acc.lock().unwrap();
            // Old root should be gone.
            assert!(system.aria_tree.get_node(root1).is_none());
            // New root should be present.
            assert_eq!(
                system.aria_tree.get_node(root2).unwrap().label,
                "Circle chart with 4 data points"
            );
        });
    }

    #[test]
    fn gpu_aria_drop_cleans_up() {
        pollster::block_on(async {
            let render_ctx = match crate::RenderContext::new().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let acc = std::sync::Arc::new(std::sync::Mutex::new(
                crate::accessibility::AccessibilitySystem::new(),
            ));
            let mut render_ctx = render_ctx;
            render_ctx.set_accessibility(acc.clone());

            let ctx = std::sync::Arc::new(render_ctx);

            let root;
            {
                let mut sel =
                    Selection::<(), Circle>::new(vec![(), ()], ctx).expect("selection creation");
                sel.sync_aria_from_context();
                root = sel.aria_root_node().unwrap();

                // Verify the tree is registered.
                let system = acc.lock().unwrap();
                assert!(system.aria_tree.get_node(root).is_some());
                // Drop sel at end of scope.
            }

            // After drop, the ARIA tree should be cleaned up.
            let system = acc.lock().unwrap();
            assert!(
                system.aria_tree.get_node(root).is_none(),
                "ARIA node should be removed after selection drop"
            );
        });
    }

    // --- Reactive ARIA update tests (GUP-126) ---

    #[test]
    fn aria_dirty_on_set_data() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0, 2.0]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.attr("center", |d: &f32| [*d, 0.0]);

        // First registration clears dirty flag.
        sel.register_aria(&mut tree);
        assert!(!sel.is_aria_dirty());

        // set_data should mark aria as dirty.
        sel.set_data(vec![3.0, 4.0, 5.0]);
        assert!(sel.is_aria_dirty());

        // Re-registering should clear the dirty flag again.
        sel.register_aria(&mut tree);
        assert!(!sel.is_aria_dirty());
    }

    #[test]
    fn aria_dirty_on_attr() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.attr("center", |d: &f32| [*d, 0.0]);
        sel.register_aria(&mut tree);
        assert!(!sel.is_aria_dirty());

        // Adding a new attribute should mark dirty.
        sel.attr("radius", |d: &f32| *d);
        assert!(sel.is_aria_dirty());
    }

    #[test]
    fn aria_dirty_on_attr_parallel() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.attr("center", |d: &f32| [*d, 0.0]);
        sel.register_aria(&mut tree);
        assert!(!sel.is_aria_dirty());

        // attr_parallel should also mark dirty.
        sel.attr_parallel(|d: &f32| ([*d, 0.0], *d), ["center", "radius"]);
        assert!(sel.is_aria_dirty());
    }

    #[test]
    fn sync_aria_skips_when_not_dirty() {
        pollster::block_on(async {
            let render_ctx = match crate::RenderContext::new().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let acc = std::sync::Arc::new(std::sync::Mutex::new(
                crate::accessibility::AccessibilitySystem::new(),
            ));
            let mut render_ctx = render_ctx;
            render_ctx.set_accessibility(acc.clone());
            let ctx = std::sync::Arc::new(render_ctx);

            let mut sel =
                Selection::<(), Circle>::new(vec![(), ()], ctx).expect("selection creation");
            sel.attr("center", |_: &()| [0.0f32, 0.0]);

            // First sync always runs (no existing ARIA tree).
            assert!(sel.sync_aria_from_context());
            let root1 = sel.aria_root_node().unwrap();

            // Second sync without changes should be a no-op.
            assert!(!sel.sync_aria_from_context());
            // Root should be unchanged.
            assert_eq!(sel.aria_root_node().unwrap(), root1);
        });
    }

    #[test]
    fn sync_aria_runs_when_dirty() {
        pollster::block_on(async {
            let render_ctx = match crate::RenderContext::new().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let acc = std::sync::Arc::new(std::sync::Mutex::new(
                crate::accessibility::AccessibilitySystem::new(),
            ));
            let mut render_ctx = render_ctx;
            render_ctx.set_accessibility(acc.clone());
            let ctx = std::sync::Arc::new(render_ctx);

            let mut sel =
                Selection::<(), Circle>::new(vec![(), ()], ctx).expect("selection creation");
            sel.attr("center", |_: &()| [0.0f32, 0.0]);

            assert!(sel.sync_aria_from_context());
            let root1 = sel.aria_root_node().unwrap();

            // Mark dirty via set_data, then sync should run again.
            sel.set_data(vec![(), (), (), ()]);
            assert!(sel.sync_aria_from_context());
            let root2 = sel.aria_root_node().unwrap();
            // New root should differ from the old one.
            assert_ne!(root1, root2);

            // Check updated label
            let system = acc.lock().unwrap();
            assert_eq!(
                system.aria_tree.get_node(root2).unwrap().label,
                "Circle chart with 4 data points"
            );
        });
    }

    #[test]
    fn aria_focus_preserved_during_update() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0, 2.0, 3.0]);
        sel.attr("center", |d: &f32| [*d, 0.0]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.register_aria(&mut tree);
        let root = sel.aria_root_node().unwrap();

        // Focus on the second data point child.
        let children = tree.get_node(root).unwrap().children.clone();
        assert_eq!(children.len(), 3);
        tree.set_focus(Some(children[1]));

        // Update data — the child NodeIds will change but focus should
        // land on index 1 in the new tree.
        sel.set_data(vec![10.0, 20.0, 30.0, 40.0]);
        sel.register_aria(&mut tree);

        let new_root = sel.aria_root_node().unwrap();
        let new_children = tree.get_node(new_root).unwrap().children.clone();
        assert_eq!(tree.get_focus(), Some(new_children[1]));
    }

    #[test]
    fn aria_focus_falls_back_to_chart_root_when_index_gone() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0, 2.0, 3.0]);
        sel.attr("center", |d: &f32| [*d, 0.0]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.register_aria(&mut tree);
        let root = sel.aria_root_node().unwrap();

        // Focus on the third (last) child.
        let children = tree.get_node(root).unwrap().children.clone();
        tree.set_focus(Some(children[2]));

        // Shrink data so index 2 no longer exists.
        sel.set_data(vec![10.0, 20.0]);
        sel.register_aria(&mut tree);

        let new_root = sel.aria_root_node().unwrap();
        // Focus should fall back to the chart root.
        assert_eq!(tree.get_focus(), Some(new_root));
    }

    #[test]
    fn aria_live_region_on_data_added() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0, 2.0]);
        sel.attr("center", |d: &f32| [*d, 0.0]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        // Initial registration — no announcement for the very first registration.
        sel.register_aria(&mut tree);
        let updates = tree.drain_update_queue();
        // The queue should have NodeCreated events but no LiveRegion.
        assert!(
            !updates
                .iter()
                .any(|u| matches!(u, crate::accessibility::aria::AriaUpdate::LiveRegion { .. })),
            "No live region announcement on initial registration"
        );

        // Now add data points.
        sel.set_data(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        sel.register_aria(&mut tree);
        let updates = tree.drain_update_queue();

        // Should have a LiveRegion announcement about 3 new points.
        let live = updates
            .iter()
            .find(|u| matches!(u, crate::accessibility::aria::AriaUpdate::LiveRegion { .. }));
        assert!(live.is_some(), "Expected live region announcement");
        if let crate::accessibility::aria::AriaUpdate::LiveRegion { content, .. } = live.unwrap() {
            assert!(
                content.contains("3 new data point"),
                "Expected '3 new data points' but got: {content}"
            );
        }
    }

    #[test]
    fn aria_live_region_on_data_removed() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        sel.attr("center", |d: &f32| [*d, 0.0]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.register_aria(&mut tree);
        tree.drain_update_queue();

        // Remove 2 points.
        sel.set_data(vec![1.0, 2.0, 3.0]);
        sel.register_aria(&mut tree);
        let updates = tree.drain_update_queue();

        let live = updates
            .iter()
            .find(|u| matches!(u, crate::accessibility::aria::AriaUpdate::LiveRegion { .. }));
        assert!(live.is_some(), "Expected live region announcement");
        if let crate::accessibility::aria::AriaUpdate::LiveRegion { content, .. } = live.unwrap() {
            assert!(
                content.contains("2 data point") && content.contains("removed"),
                "Expected removal summary but got: {content}"
            );
        }
    }

    #[test]
    fn aria_live_region_on_same_count_update() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0, 2.0]);
        sel.attr("center", |d: &f32| [*d, 0.0]);
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.register_aria(&mut tree);
        tree.drain_update_queue();

        // Same count, different data.
        sel.set_data(vec![10.0, 20.0]);
        sel.register_aria(&mut tree);
        let updates = tree.drain_update_queue();

        let live = updates
            .iter()
            .find(|u| matches!(u, crate::accessibility::aria::AriaUpdate::LiveRegion { .. }));
        assert!(live.is_some(), "Expected live region announcement");
        if let crate::accessibility::aria::AriaUpdate::LiveRegion { content, .. } = live.unwrap() {
            assert!(
                content.contains("data updated"),
                "Expected 'data updated' but got: {content}"
            );
        }
    }

    #[test]
    fn aria_update_config_urgency() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0]);
        sel.attr("center", |d: &f32| [*d, 0.0]);
        sel.set_aria_update_config(AriaUpdateConfig {
            urgency: crate::accessibility::aria::AriaLive::Assertive,
            announce_changes: true,
        });
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.register_aria(&mut tree);
        tree.drain_update_queue();

        sel.set_data(vec![1.0, 2.0]);
        sel.register_aria(&mut tree);
        let updates = tree.drain_update_queue();

        let live = updates
            .iter()
            .find(|u| matches!(u, crate::accessibility::aria::AriaUpdate::LiveRegion { .. }));
        assert!(live.is_some());
        if let crate::accessibility::aria::AriaUpdate::LiveRegion { urgency, .. } = live.unwrap() {
            assert_eq!(*urgency, crate::accessibility::aria::AriaLive::Assertive);
        }
    }

    #[test]
    fn aria_update_config_announce_off() {
        let mut sel = Selection::<f32, Circle>::from_data(vec![1.0]);
        sel.attr("center", |d: &f32| [*d, 0.0]);
        sel.set_aria_update_config(AriaUpdateConfig {
            urgency: crate::accessibility::aria::AriaLive::Polite,
            announce_changes: false,
        });
        let mut tree = crate::accessibility::aria::AriaTree::new();

        sel.register_aria(&mut tree);
        tree.drain_update_queue();

        sel.set_data(vec![1.0, 2.0]);
        sel.register_aria(&mut tree);
        let updates = tree.drain_update_queue();

        // Should NOT have a LiveRegion announcement.
        assert!(
            !updates
                .iter()
                .any(|u| matches!(u, crate::accessibility::aria::AriaUpdate::LiveRegion { .. })),
            "No live region when announce_changes is false"
        );
    }

    #[test]
    fn aria_update_config_defaults() {
        let config = AriaUpdateConfig::default();
        assert_eq!(config.urgency, crate::accessibility::aria::AriaLive::Polite);
        assert!(config.announce_changes);
    }

    #[test]
    fn summarise_change_additions() {
        let summary = Selection::<(), Circle>::summarise_change(Some(3), 5, "circle");
        assert_eq!(
            summary,
            Some("2 new data points added, 5 total".to_string())
        );
    }

    #[test]
    fn summarise_change_removals() {
        let summary = Selection::<(), Circle>::summarise_change(Some(5), 3, "circle");
        assert_eq!(summary, Some("2 data points removed, 3 total".to_string()));
    }

    #[test]
    fn summarise_change_same_count() {
        let summary = Selection::<(), Circle>::summarise_change(Some(3), 3, "circle");
        assert_eq!(summary, Some("Circle chart data updated".to_string()));
    }

    #[test]
    fn summarise_change_initial() {
        let summary = Selection::<(), Circle>::summarise_change(None, 5, "circle");
        assert!(summary.is_none(), "No summary for initial registration");
    }

    #[test]
    fn summarise_change_single_point() {
        let summary = Selection::<(), Circle>::summarise_change(Some(2), 3, "line");
        assert_eq!(summary, Some("1 new data point added, 3 total".to_string()));
    }

    // --- GPU integration tests ---

    #[test]
    fn gpu_prepare_and_render_circle_selection() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![CircleAttributes {
                center: Vec2 { x: 0.0, y: 0.0 },
                radius: 0.1,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            }];

            let mut selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

            // Prepare GPU resources.
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    None,
                )
                .expect("prepare_render should succeed");

            assert!(selection.is_render_ready());

            // Render to an offscreen frame.
            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_prepare_and_render_rectangle_selection() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                RectangleAttributes {
                    center: Vec2 { x: -0.3, y: 0.0 },
                    size: Vec2 { x: 0.4, y: 0.6 },
                    fill_color: Vec4 {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                        w: 1.0,
                    },
                    stroke_width: 0.01,
                    stroke_color: Vec4 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        w: 1.0,
                    },
                    corner_radius: 0.0,
                },
                RectangleAttributes {
                    center: Vec2 { x: 0.3, y: 0.0 },
                    size: Vec2 { x: 0.4, y: 0.6 },
                    fill_color: Vec4 {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                        w: 1.0,
                    },
                    stroke_width: 0.0,
                    stroke_color: Vec4 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        w: 0.0,
                    },
                    corner_radius: 0.02,
                },
            ];

            let mut selection: Selection<RectangleAttributes, Rectangle> =
                Selection::from_data(data);

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| RectangleInstance::from(a),
                    None,
                    None,
                )
                .expect("prepare_render should succeed");

            assert!(selection.is_render_ready());

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_pipeline_reuse_across_prepare_calls() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![CircleAttributes {
                center: Vec2 { x: 0.0, y: 0.0 },
                radius: 0.1,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            }];

            let mut selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

            // First prepare — creates pipeline, buffers, bind group.
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    None,
                )
                .expect("first prepare_render");

            assert!(selection.is_render_ready());

            // Second prepare with same-sized data — reuses pipeline + buffers.
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    None,
                )
                .expect("second prepare_render (re-upload)");

            assert!(selection.is_render_ready());
        });
    }

    #[test]
    fn gpu_buffer_resize_on_larger_data() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let make_attr = |x: f32| CircleAttributes {
                center: Vec2 { x, y: 0.0 },
                radius: 0.05,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            };

            // Start with 2 items.
            let mut selection: Selection<CircleAttributes, Circle> =
                Selection::from_data(vec![make_attr(-0.5), make_attr(0.5)]);

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    None,
                )
                .expect("first prepare");

            // Grow to 10 items — triggers buffer resize.
            let large_data: Vec<_> = (0..10).map(|i| make_attr(-0.9 + i as f32 * 0.2)).collect();
            selection.set_data(large_data);

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    None,
                )
                .expect("second prepare after resize");

            // Verify the new count is reflected.
            assert_eq!(selection.len(), 10);
        });
    }

    #[test]
    fn gpu_empty_selection_renders_noop() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let mut selection: Selection<CircleAttributes, Circle> =
                Selection::from_data(Vec::new());

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    None,
                )
                .expect("prepare empty selection");

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                // Should be a no-op, not an error.
                selection
                    .render(&mut render_pass)
                    .expect("empty render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_composite_rendering_multiple_selections() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            // Create rectangle and circle selections (like box plot).
            let rect_data = vec![RectangleAttributes {
                center: Vec2 { x: 0.0, y: 0.0 },
                size: Vec2 { x: 0.4, y: 0.6 },
                fill_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
                stroke_width: 0.01,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                corner_radius: 0.0,
            }];

            let circle_data = vec![CircleAttributes {
                center: Vec2 { x: 0.0, y: 0.5 },
                radius: 0.05,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.5,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            }];

            let mut rect_sel: Selection<RectangleAttributes, Rectangle> =
                Selection::from_data(rect_data);
            let mut circle_sel: Selection<CircleAttributes, Circle> =
                Selection::from_data(circle_data);

            rect_sel
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| RectangleInstance::from(a),
                    None,
                    None,
                )
                .expect("rect prepare");
            circle_sel
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    None,
                )
                .expect("circle prepare");

            // Both render in the same render pass (single render pass rule).
            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                rect_sel.render(&mut render_pass).expect("rect render");
                circle_sel.render(&mut render_pass).expect("circle render");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_prepare_and_render_boxplot_selection() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            use crate::BoxPlotAttributes;
            use crate::mark::BoxPlot;
            use crate::mark::boxplot::{BoxPlotInstance, BoxPlotOrientation};

            let data = vec![BoxPlotAttributes {
                position: Vec2 { x: 0.0, y: 0.0 },
                min: -0.5,
                q1: -0.2,
                median: 0.0,
                q3: 0.2,
                max: 0.5,
                outliers: vec![-0.7, 0.8],
                width: 0.3,
                orientation: BoxPlotOrientation::Vertical,
                stroke_width: 3.0,
                outlier_radius: 10.0,
                ..Default::default()
            }];

            let mut selection: Selection<BoxPlotAttributes, BoxPlot> = Selection::from_data(data);

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| BoxPlotInstance::from(a),
                    None,
                    None,
                )
                .expect("boxplot prepare_render");

            assert!(selection.is_render_ready());

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::WHITE));
                selection
                    .render(&mut render_pass)
                    .expect("boxplot render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_render_multiple_boxplots() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            use crate::BoxPlotAttributes;
            use crate::mark::BoxPlot;
            use crate::mark::boxplot::{BoxPlotInstance, BoxPlotOrientation};

            let data: Vec<BoxPlotAttributes> = (0..4)
                .map(|i| {
                    let x = -0.6 + i as f32 * 0.4;
                    BoxPlotAttributes {
                        position: Vec2 { x, y: 0.0 },
                        min: -0.4,
                        q1: -0.1,
                        median: 0.1,
                        q3: 0.3,
                        max: 0.5,
                        outliers: vec![-0.6],
                        width: 0.15,
                        orientation: BoxPlotOrientation::Vertical,
                        stroke_width: 2.0,
                        outlier_radius: 7.0,
                        ..Default::default()
                    }
                })
                .collect();

            let mut selection: Selection<BoxPlotAttributes, BoxPlot> = Selection::from_data(data);
            assert_eq!(selection.len(), 4);

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| BoxPlotInstance::from(a),
                    None,
                    None,
                )
                .expect("prepare_render");

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render 4 box plots");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_render_horizontal_boxplot() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            use crate::BoxPlotAttributes;
            use crate::mark::BoxPlot;
            use crate::mark::boxplot::{BoxPlotInstance, BoxPlotOrientation};

            let data = vec![BoxPlotAttributes {
                position: Vec2 { x: 0.0, y: 0.0 },
                min: -0.5,
                q1: -0.2,
                median: 0.0,
                q3: 0.2,
                max: 0.5,
                outliers: vec![],
                width: 0.3,
                orientation: BoxPlotOrientation::Horizontal,
                stroke_width: 3.0,
                outlier_radius: 10.0,
                ..Default::default()
            }];

            let mut selection: Selection<BoxPlotAttributes, BoxPlot> = Selection::from_data(data);
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| BoxPlotInstance::from(a),
                    None,
                    None,
                )
                .expect("prepare_render horizontal");

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render horizontal boxplot");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_render_notched_boxplot() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            use crate::BoxPlotAttributes;
            use crate::mark::BoxPlot;
            use crate::mark::boxplot::{BoxPlotInstance, BoxPlotOrientation};

            // One notched and one non-notched for comparison
            let data = vec![
                BoxPlotAttributes {
                    position: Vec2 { x: -0.3, y: 0.0 },
                    min: -0.5,
                    q1: -0.2,
                    median: 0.0,
                    q3: 0.2,
                    max: 0.5,
                    outliers: vec![-0.7],
                    width: 0.2,
                    orientation: BoxPlotOrientation::Vertical,
                    stroke_width: 3.0,
                    outlier_radius: 7.0,
                    notched: true,
                    notch_width: 0.5,
                    ..Default::default()
                },
                BoxPlotAttributes {
                    position: Vec2 { x: 0.3, y: 0.0 },
                    min: -0.5,
                    q1: -0.2,
                    median: 0.0,
                    q3: 0.2,
                    max: 0.5,
                    outliers: vec![],
                    width: 0.2,
                    orientation: BoxPlotOrientation::Vertical,
                    stroke_width: 3.0,
                    outlier_radius: 7.0,
                    notched: false,
                    notch_width: 0.5,
                    ..Default::default()
                },
            ];

            // Verify the notch fields are packed correctly
            let inst0 = BoxPlotInstance::from(&data[0]);
            assert_eq!(inst0.notched, 1);
            assert_eq!(inst0.notch_width, 0.5);

            let inst1 = BoxPlotInstance::from(&data[1]);
            assert_eq!(inst1.notched, 0);

            let mut selection: Selection<BoxPlotAttributes, BoxPlot> = Selection::from_data(data);
            assert_eq!(selection.len(), 2);

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| BoxPlotInstance::from(a),
                    None,
                    None,
                )
                .expect("prepare_render notched");

            assert!(selection.is_render_ready());

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::WHITE));
                selection
                    .render(&mut render_pass)
                    .expect("render notched boxplot should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_boxplot_pixel_consistent_strokes() {
        // Render the same box plot at two different viewport sizes and verify
        // that the stroke occupies the same number of pixels.
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            use crate::BoxPlotAttributes;
            use crate::mark::BoxPlot;
            use crate::mark::boxplot::{BoxPlotInstance, BoxPlotOrientation};

            let data = vec![BoxPlotAttributes {
                position: Vec2 { x: 0.0, y: 0.0 },
                min: -0.5,
                q1: -0.2,
                median: 0.0,
                q3: 0.2,
                max: 0.5,
                outliers: vec![],
                width: 0.3,
                orientation: BoxPlotOrientation::Vertical,
                stroke_width: 4.0,
                outlier_radius: 8.0,
                ..Default::default()
            }];

            let mut selection: Selection<BoxPlotAttributes, BoxPlot> =
                Selection::from_data(data.clone());
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| BoxPlotInstance::from(a),
                    None,
                    None,
                )
                .expect("prepare_render");

            // Use 256 and 512 pixel sizes (byte-per-row is 256-byte aligned
            // for BGRA textures: 256*4=1024, 512*4=2048).
            let small = 256u32;
            let large = 512u32;

            // Helper to render and read back non-white pixel count in the
            // middle row for a given viewport/texture size.
            let render_and_count_row = |sel: &Selection<BoxPlotAttributes, BoxPlot>,
                                        size: u32,
                                        label: &str| {
                let tex = context.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size,
                        height: size,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

                let row_bytes = (size * 4) as usize;
                let readback = context.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("{label}_readback")),
                    size: (row_bytes * size as usize) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });

                let mut encoder =
                    context
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some(&format!("{label}_enc")),
                        });

                {
                    let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some(&format!("{label}_rp")),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    sel.render(&mut rp).expect("render");
                }

                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(row_bytes as u32),
                            rows_per_image: Some(size),
                        },
                    },
                    wgpu::Extent3d {
                        width: size,
                        height: size,
                        depth_or_array_layers: 1,
                    },
                );
                context.queue.submit(Some(encoder.finish()));

                let slice = readback.slice(..);
                let (tx, rx) = std::sync::mpsc::channel();
                slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                let _ = context.device.poll(wgpu::PollType::Wait);
                rx.recv().unwrap().unwrap();

                let data = slice.get_mapped_range();
                let mid_row = (size / 2) as usize;
                let start = mid_row * row_bytes;
                let end = start + row_bytes;
                let count = (start..end)
                    .step_by(4)
                    .filter(|&i| data[i] != 255 || data[i + 1] != 255 || data[i + 2] != 255)
                    .count();
                drop(data);
                count
            };

            // Render at small viewport
            selection.set_viewport_size(&context.queue, small as f32, small as f32);
            let count_small = render_and_count_row(&selection, small, "small");

            // Render at large viewport
            selection.set_viewport_size(&context.queue, large as f32, large as f32);
            let count_large = render_and_count_row(&selection, large, "large");

            // Both should have visible non-white pixels
            assert!(
                count_small > 0,
                "{small}px render should have visible box stroke pixels"
            );
            assert!(
                count_large > 0,
                "{large}px render should have visible box stroke pixels"
            );

            // At 2× resolution, the box doubles in pixel width but stroke
            // stays constant at 4 pixels.  The total non-white count in
            // the middle row should roughly double.
            let ratio = count_large as f64 / count_small as f64;
            assert!(
                (1.6..=2.4).contains(&ratio),
                "Non-white pixel ratio should be ~2.0 (was {ratio:.2}). \
                 {small}px: {count_small}, {large}px: {count_large}"
            );
        });
    }

    // --- GPU tests for attribute binding pipeline (GUP-168) ---

    /// Custom data type for testing attribute bindings.
    #[derive(Debug, Clone)]
    struct ScatterPoint {
        x: f32,
        y: f32,
        value: f32,
    }

    #[test]
    fn gpu_prepare_render_bound_circle() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                ScatterPoint {
                    x: -0.5,
                    y: 0.3,
                    value: 0.8,
                },
                ScatterPoint {
                    x: 0.2,
                    y: -0.4,
                    value: 0.3,
                },
                ScatterPoint {
                    x: 0.7,
                    y: 0.1,
                    value: 0.6,
                },
            ];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("radius", |d: &ScatterPoint| d.value * 0.15)
                .attr("fill_color", |d: &ScatterPoint| {
                    [d.value, 0.2, 1.0 - d.value, 1.0]
                });

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound should succeed");

            assert!(selection.is_render_ready());

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_prepare_render_bound_rectangle() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                ScatterPoint {
                    x: -0.3,
                    y: 0.0,
                    value: 0.5,
                },
                ScatterPoint {
                    x: 0.3,
                    y: 0.0,
                    value: 0.7,
                },
            ];

            let mut selection: Selection<ScatterPoint, Rectangle> = Selection::from_data(data);

            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("size", |d: &ScatterPoint| [0.2, d.value * 0.8])
                .attr("fill_color", |d: &ScatterPoint| {
                    [0.0, d.value, 1.0 - d.value, 1.0]
                })
                .attr("corner_radius", |_: &ScatterPoint| 0.02f32);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound should succeed");

            assert!(selection.is_render_ready());

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_prepare_render_bound_with_parallel() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                ScatterPoint {
                    x: -0.5,
                    y: 0.3,
                    value: 0.8,
                },
                ScatterPoint {
                    x: 0.5,
                    y: -0.3,
                    value: 0.2,
                },
            ];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

            // Use attr_parallel to bind position and color in one pass
            selection
                .attr_parallel(
                    |d: &ScatterPoint| ([d.x, d.y], [d.value, 0.0, 1.0 - d.value, 1.0]),
                    ["center", "fill_color"],
                )
                .attr("radius", |d: &ScatterPoint| d.value * 0.1);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound should succeed");

            assert!(selection.is_render_ready());

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_prepare_render_bound_line() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                ScatterPoint {
                    x: -0.8,
                    y: -0.5,
                    value: 0.6,
                },
                ScatterPoint {
                    x: 0.3,
                    y: 0.7,
                    value: 0.4,
                },
                ScatterPoint {
                    x: -0.2,
                    y: 0.1,
                    value: 0.9,
                },
            ];

            let mut selection: Selection<ScatterPoint, Line> = Selection::from_data(data);

            selection
                .attr("start", |d: &ScatterPoint| [d.x, d.y])
                .attr("end", |d: &ScatterPoint| [d.x + 0.3, d.y + 0.2])
                .attr("color", |d: &ScatterPoint| {
                    [d.value, 0.2, 1.0 - d.value, 1.0]
                })
                .attr("width", |d: &ScatterPoint| d.value * 0.05);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound should succeed for Line");

            assert!(selection.is_render_ready());

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_prepare_render_bound_boxplot() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            #[derive(Debug, Clone)]
            struct StatsData {
                category_x: f32,
                low: f32,
                first_q: f32,
                mid: f32,
                third_q: f32,
                high: f32,
            }

            let data = vec![
                StatsData {
                    category_x: -0.5,
                    low: -0.8,
                    first_q: -0.4,
                    mid: -0.1,
                    third_q: 0.2,
                    high: 0.6,
                },
                StatsData {
                    category_x: 0.3,
                    low: -0.5,
                    first_q: -0.2,
                    mid: 0.1,
                    third_q: 0.4,
                    high: 0.8,
                },
            ];

            let mut selection: Selection<StatsData, BoxPlot> = Selection::from_data(data);

            selection
                .attr("position", |d: &StatsData| [d.category_x, 0.0])
                .attr("min", |d: &StatsData| d.low)
                .attr("q1", |d: &StatsData| d.first_q)
                .attr("median", |d: &StatsData| d.mid)
                .attr("q3", |d: &StatsData| d.third_q)
                .attr("max", |d: &StatsData| d.high)
                .attr("width", |_: &StatsData| 0.15f32)
                .attr("box_fill_color", |_: &StatsData| [0.7, 0.7, 1.0, 0.8]);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound should succeed for BoxPlot");

            assert!(selection.is_render_ready());

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_prepare_render_bound_empty_selection() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(Vec::new());

            selection.attr("center", |d: &ScatterPoint| [d.x, d.y]);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare empty selection");

            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");

            {
                let mut render_pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut render_pass)
                    .expect("empty render should succeed");
            }

            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_prepare_render_bound_no_bindings_returns_error() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let mut selection: Selection<ScatterPoint, Circle> =
                Selection::from_data(vec![ScatterPoint {
                    x: 0.0,
                    y: 0.0,
                    value: 1.0,
                }]);

            // No attr() calls — should fail
            let result = selection.prepare_render_bound(&context.device, &context.queue, None, None);
            assert!(result.is_err());
        });
    }

    // --- Unit tests for GPU shader function bindings (GUP-177) ---

    #[test]
    fn attr_shader_stores_binding() {
        use crate::shader_function::LinearScale;

        let data = vec![CircleAttributes::default()];
        let mut selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        selection.attr_shader("radius", |a: &CircleAttributes| a.radius, scale);

        assert!(selection.has_shader_bindings());
        assert_eq!(selection.shader_bound_attributes(), vec!["radius"]);
        // has_attr_bindings returns true for shader bindings too.
        assert!(selection.has_attr_bindings());
    }

    #[test]
    fn attr_shader_invalidates_render_state() {
        use crate::shader_function::LinearScale;

        let data = vec![CircleAttributes::default()];
        let mut selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

        // Initially no render state.
        assert!(!selection.is_render_ready());

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        selection.attr_shader("radius", |a: &CircleAttributes| a.radius, scale);

        // Still not render-ready (need prepare_render_*).
        assert!(!selection.is_render_ready());
    }

    #[test]
    fn shader_fn_info_captures_metadata() {
        use crate::shader_function::LinearScale;

        let scale = LinearScale::new(0.0, 100.0, -1.0, 1.0);
        let info = shader_fn_info_from(&scale);

        assert_eq!(info.function_name, "linear_scale");
        assert_eq!(info.input_wgsl_type, "f32");
        assert_eq!(info.output_wgsl_type, "f32");
        assert!(!info.wgsl_code.is_empty());
        assert!(!info.uniform_bytes.is_empty());
        assert!(info.uniform_type_name.contains("LinearScale"));
    }

    #[test]
    fn shader_fn_info_captures_color_map() {
        use crate::shader_function::ColorMap;

        let cm = ColorMap::new(
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 1.0,
            },
            Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        );
        let info = shader_fn_info_from(&cm);

        assert_eq!(info.function_name, "color_map");
        assert_eq!(info.input_wgsl_type, "f32");
        assert_eq!(info.output_wgsl_type, "vec4<f32>");
    }

    #[test]
    fn generate_shader_bound_vertex_wgsl_no_bindings() {
        let original = "struct S {} @vertex fn vs_main() {}";
        let result = generate_shader_bound_vertex_wgsl(original, &[]);
        assert_eq!(result, original);
    }

    #[test]
    fn generate_shader_bound_vertex_wgsl_injects_code() {
        use crate::shader_function::LinearScale;

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let info = shader_fn_info_from(&scale);

        // Minimal vertex shader text resembling the circle shader.
        let base_wgsl = r#"
struct CircleInstance {
    center: vec2<f32>,
    radius: f32,
}

@group(0) @binding(0) var<storage, read> instances: array<CircleInstance>;

@vertex
fn vs_main() -> VertexOutput {
    let instance = instances[input.instance_index];
    let r = instance.radius;
    return r;
}
"#;

        let bindings: Vec<(&str, &ShaderFnInfo)> = vec![("radius", &info)];
        let result = generate_shader_bound_vertex_wgsl(base_wgsl, &bindings);

        // Should contain uniform binding at binding 1.
        assert!(
            result.contains("@group(0) @binding(1)"),
            "Missing uniform binding: {result}"
        );
        // Should contain the shader function code.
        assert!(
            result.contains("linear_scale"),
            "Missing shader function: {result}"
        );
        // Should contain the transformation variable.
        assert!(
            result.contains("_gup_radius"),
            "Missing transformed variable: {result}"
        );
        // Original uses of instance.radius in the body (after the
        // transformation) should be replaced with _gup_radius.
        // But the transformation line itself reads from instance.radius.
        // Count occurrences: should have exactly 1 (in the transformation line).
        let count = result.matches("instance.radius").count();
        assert_eq!(
            count, 1,
            "Expected 1 instance.radius (in transform line), found {count}: {result}"
        );
    }

    #[test]
    fn mixed_cpu_and_shader_bindings() {
        use crate::shader_function::LinearScale;

        let data = vec![ScatterPoint {
            x: 0.0,
            y: 0.0,
            value: 50.0,
        }];
        let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

        // CPU binding for position.
        selection.attr("center", |d: &ScatterPoint| [d.x, d.y]);
        // GPU shader binding for radius.
        let scale = LinearScale::new(0.0, 100.0, 0.01, 0.2);
        selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

        assert_eq!(selection.bound_attributes(), vec!["center"]);
        assert_eq!(selection.shader_bound_attributes(), vec!["radius"]);
        assert!(selection.has_attr_bindings());
    }

    // --- GPU integration tests for shader function bindings ---

    #[test]
    fn gpu_shader_bound_circle_render() {
        use crate::shader_function::LinearScale;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                ScatterPoint {
                    x: -0.3,
                    y: 0.0,
                    value: 50.0,
                },
                ScatterPoint {
                    x: 0.3,
                    y: 0.0,
                    value: 80.0,
                },
            ];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

            // CPU binding for position + fill.
            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("fill_color", |_: &ScatterPoint| [1.0f32, 0.0, 0.0, 1.0]);

            // GPU binding for radius.
            let scale = LinearScale::new(0.0, 100.0, 0.02, 0.2);
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound with shader fn");

            assert!(selection.is_render_ready());

            // Render to verify pipeline is valid.
            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");
            {
                let mut pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut pass)
                    .expect("render with shader fn bindings");
            }
            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_shader_bound_only_bindings() {
        use crate::shader_function::LinearScale;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![ScatterPoint {
                x: 0.0,
                y: 0.0,
                value: 75.0,
            }];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

            // Only shader bindings (no CPU bindings).
            let radius_scale = LinearScale::new(0.0, 100.0, 0.05, 0.3);
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, radius_scale);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare with only shader bindings");

            assert!(selection.is_render_ready());
        });
    }

    #[test]
    fn gpu_shader_binding_type_mismatch_rejected() {
        use crate::shader_function::ColorMap;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![ScatterPoint {
                x: 0.0,
                y: 0.0,
                value: 0.5,
            }];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

            // ColorMap outputs vec4<f32>, but "radius" expects f32 — type mismatch.
            let color_map = ColorMap::new(
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
                Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            );
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, color_map);

            let result = selection.prepare_render_bound(&context.device, &context.queue, None, None);
            assert!(result.is_err(), "Should reject type-mismatched shader fn");
            let err_msg = format!("{}", result.unwrap_err());
            assert!(
                err_msg.contains("not compatible"),
                "Error should mention incompatibility: {err_msg}"
            );
        });
    }

    #[test]
    fn gpu_shader_binding_performance_100k() {
        use crate::shader_function::LinearScale;
        use std::time::Instant;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU perf test — no adapter available");
                    return;
                }
            };

            const N: usize = 100_000;
            let data: Vec<ScatterPoint> = (0..N)
                .map(|i| {
                    let t = i as f32 / N as f32;
                    ScatterPoint {
                        x: -1.0 + 2.0 * t,
                        y: (t * std::f32::consts::TAU).sin() * 0.8,
                        value: t * 100.0,
                    }
                })
                .collect();

            // --- CPU closure binding ---
            let mut sel_cpu: Selection<ScatterPoint, Circle> = Selection::from_data(data.clone());
            sel_cpu
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("radius", |d: &ScatterPoint| {
                    // Manual CPU-side linear scale: domain [0,100] → range [0.001, 0.01]
                    let normalised = d.value / 100.0;
                    0.001 + normalised * 0.009
                })
                .attr("fill_color", |_: &ScatterPoint| [1.0f32, 0.0, 0.0, 1.0]);

            let cpu_start = Instant::now();
            sel_cpu
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("CPU prepare");
            let cpu_elapsed = cpu_start.elapsed();

            // --- GPU shader function binding ---
            let mut sel_gpu: Selection<ScatterPoint, Circle> = Selection::from_data(data);
            let radius_scale = LinearScale::new(0.0, 100.0, 0.001, 0.01);
            sel_gpu
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("fill_color", |_: &ScatterPoint| [0.0f32, 0.0, 1.0, 1.0]);
            sel_gpu.attr_shader("radius", |d: &ScatterPoint| d.value, radius_scale);

            let gpu_start = Instant::now();
            sel_gpu
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("GPU prepare");
            let gpu_elapsed = gpu_start.elapsed();

            eprintln!(
                "100K points: CPU closure = {:?}, GPU shader fn = {:?}",
                cpu_elapsed, gpu_elapsed
            );

            // Both should complete in reasonable time (< 5 seconds).
            assert!(
                cpu_elapsed.as_secs() < 5,
                "CPU binding took too long: {cpu_elapsed:?}"
            );
            assert!(
                gpu_elapsed.as_secs() < 5,
                "GPU binding took too long: {gpu_elapsed:?}"
            );

            // Verify both produce valid render state.
            assert!(sel_cpu.is_render_ready());
            assert!(sel_gpu.is_render_ready());
        });
    }

    // --- update_shader_uniforms tests ---

    #[test]
    fn update_shader_uniforms_unit_not_found() {
        use crate::shader_function::LinearScale;

        let data = vec![ScatterPoint {
            x: 0.0,
            y: 0.0,
            value: 50.0,
        }];
        let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);
        let scale = LinearScale::new(0.0, 100.0, 0.01, 0.2);
        selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

        // Verify the method API: shader binding is stored, but not CPU bindings.
        assert!(selection.shader_bound_attributes().contains(&"radius"));
        assert!(!selection.shader_bound_attributes().contains(&"nonexistent"));
    }

    #[test]
    fn gpu_update_shader_uniforms_basic() {
        use crate::shader_function::LinearScale;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                ScatterPoint {
                    x: -0.3,
                    y: 0.0,
                    value: 50.0,
                },
                ScatterPoint {
                    x: 0.3,
                    y: 0.0,
                    value: 80.0,
                },
            ];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);
            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("fill_color", |_: &ScatterPoint| [1.0f32, 0.0, 0.0, 1.0]);

            let scale = LinearScale::new(0.0, 100.0, 0.02, 0.2);
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

            // First: full pipeline creation.
            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("initial prepare_render_bound");

            assert!(selection.is_render_ready());

            // Now: update only the uniform (different range).
            let new_scale = LinearScale::new(0.0, 100.0, 0.05, 0.5);
            selection
                .update_shader_uniforms("radius", new_scale, &context.queue)
                .expect("update_shader_uniforms should succeed");

            // Render to verify the pipeline is still valid.
            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");
            {
                let mut pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut pass)
                    .expect("render after uniform update");
            }
            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_update_shader_uniforms_not_found_error() {
        use crate::shader_function::LinearScale;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![ScatterPoint {
                x: 0.0,
                y: 0.0,
                value: 50.0,
            }];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);
            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("fill_color", |_: &ScatterPoint| [1.0f32, 0.0, 0.0, 1.0]);

            let scale = LinearScale::new(0.0, 100.0, 0.02, 0.2);
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound");

            // Try to update a non-existent attribute.
            let new_scale = LinearScale::new(0.0, 100.0, 0.05, 0.5);
            let result = selection.update_shader_uniforms("nonexistent", new_scale, &context.queue);
            assert!(result.is_err(), "Should error for unknown attribute");
            let err_msg = format!("{}", result.unwrap_err());
            assert!(
                err_msg.contains("No GPU shader binding found"),
                "Error should mention missing binding: {err_msg}"
            );
        });
    }

    #[test]
    fn gpu_update_shader_uniforms_not_gpu_bound_error() {
        use crate::shader_function::LinearScale;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![ScatterPoint {
                x: 0.0,
                y: 0.0,
                value: 50.0,
            }];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);
            // "center" is CPU-bound, not GPU-bound.
            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("fill_color", |_: &ScatterPoint| [1.0f32, 0.0, 0.0, 1.0]);

            let scale = LinearScale::new(0.0, 100.0, 0.02, 0.2);
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound");

            // Try to update "center" which is a CPU binding, not a shader binding.
            let center_scale = LinearScale::new(-1.0, 1.0, -0.5, 0.5);
            let result = selection.update_shader_uniforms("center", center_scale, &context.queue);
            assert!(result.is_err(), "Should error for CPU-bound attribute");
        });
    }

    #[test]
    fn gpu_update_shader_uniforms_before_prepare_error() {
        use crate::shader_function::LinearScale;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![ScatterPoint {
                x: 0.0,
                y: 0.0,
                value: 50.0,
            }];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);
            let scale = LinearScale::new(0.0, 100.0, 0.02, 0.2);
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

            // Don't call prepare_render_bound — render state is None.
            let new_scale = LinearScale::new(0.0, 100.0, 0.05, 0.5);
            let result = selection.update_shader_uniforms("radius", new_scale, &context.queue);
            assert!(
                result.is_err(),
                "Should error when render state not initialised"
            );
            let err_msg = format!("{}", result.unwrap_err());
            assert!(
                err_msg.contains("prepare_render_bound"),
                "Error should mention prepare_render_bound: {err_msg}"
            );
        });
    }

    #[test]
    fn gpu_update_shader_uniforms_type_mismatch_error() {
        use crate::shader_function::{ColorMap, LinearScale};

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![ScatterPoint {
                x: 0.0,
                y: 0.0,
                value: 50.0,
            }];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);
            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("fill_color", |_: &ScatterPoint| [1.0f32, 0.0, 0.0, 1.0]);

            let scale = LinearScale::new(0.0, 100.0, 0.02, 0.2);
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound");

            // Try updating with a ColorMap (outputs vec4<f32>) — type mismatch
            // with the radius attribute which expects f32.
            let color_map = ColorMap::new(
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
                Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            );
            let result = selection.update_shader_uniforms("radius", color_map, &context.queue);
            assert!(result.is_err(), "Should reject type-mismatched update");
            let err_msg = format!("{}", result.unwrap_err());
            assert!(
                err_msg.contains("mismatch"),
                "Error should mention type mismatch: {err_msg}"
            );
        });
    }

    #[test]
    fn gpu_update_shader_uniforms_performance() {
        use crate::shader_function::LinearScale;
        use std::time::Instant;

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU perf test — no adapter available");
                    return;
                }
            };

            const N: usize = 100_000;
            let data: Vec<ScatterPoint> = (0..N)
                .map(|i| {
                    let t = i as f32 / N as f32;
                    ScatterPoint {
                        x: -1.0 + 2.0 * t,
                        y: (t * std::f32::consts::TAU).sin() * 0.8,
                        value: t * 100.0,
                    }
                })
                .collect();

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);
            let scale = LinearScale::new(0.0, 100.0, 0.001, 0.01);
            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("fill_color", |_: &ScatterPoint| [0.0f32, 0.0, 1.0, 1.0]);
            selection.attr_shader("radius", |d: &ScatterPoint| d.value, scale);

            // Full prepare (uploads instance data + creates pipeline).
            let prepare_start = Instant::now();
            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("initial prepare");
            let prepare_elapsed = prepare_start.elapsed();

            // Uniform-only update.
            let new_scale = LinearScale::new(0.0, 100.0, 0.005, 0.02);
            let update_start = Instant::now();
            selection
                .update_shader_uniforms("radius", new_scale, &context.queue)
                .expect("uniform update");
            let update_elapsed = update_start.elapsed();

            eprintln!(
                "100K points: full prepare = {:?}, uniform update = {:?} (speedup = {:.1}x)",
                prepare_elapsed,
                update_elapsed,
                prepare_elapsed.as_secs_f64() / update_elapsed.as_secs_f64()
            );

            // Uniform update should be fast (buffer write only, no data rebuild).
            // Debug builds are slower; use generous thresholds.
            #[cfg(debug_assertions)]
            let threshold_us: u128 = 5000;
            #[cfg(not(debug_assertions))]
            let threshold_us: u128 = 1000;

            assert!(
                update_elapsed.as_micros() < threshold_us,
                "Uniform update took too long: {update_elapsed:?} (should be <{threshold_us}μs)"
            );

            // Verify render still works.
            assert!(selection.is_render_ready());
        });
    }

    // --- FunctionChain binding tests (GUP-180) ---

    #[test]
    fn shader_fn_info_captures_function_chain_metadata() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 1.0,
            },
            Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        );
        let chain = scale.compose(color_map);
        let info = shader_fn_info_from(&chain);

        assert_eq!(info.function_name, "composed_chain");
        assert_eq!(info.input_wgsl_type, "f32");
        assert_eq!(info.output_wgsl_type, "vec4<f32>");
        // WGSL code must include both component functions.
        assert!(
            info.wgsl_code.contains("linear_scale"),
            "Missing first function: {}",
            info.wgsl_code
        );
        assert!(
            info.wgsl_code.contains("color_map"),
            "Missing second function: {}",
            info.wgsl_code
        );
        assert!(
            info.wgsl_code.contains("composed_chain"),
            "Missing composed entry point: {}",
            info.wgsl_code
        );
        // Uniform bytes must be non-empty (both functions have uniforms).
        assert!(
            !info.uniform_bytes.is_empty(),
            "ChainUniforms should serialise to non-empty bytes"
        );
        // Uniform type name should be the chain wrapper.
        assert_eq!(info.uniform_type_name, "ChainUniforms");
    }

    #[test]
    fn chain_uniform_struct_def_includes_nested_types() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        let scale = LinearScale::new(0.0, 1.0, 0.0, 1.0);
        let color_map = ColorMap::new(
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            },
        );
        let chain = scale.compose(color_map);
        let info = shader_fn_info_from(&chain);

        // The struct definition must include both nested struct definitions so
        // the WGSL is self-contained.
        assert!(
            info.uniform_struct_def.contains("LinearScaleUniforms"),
            "Missing LinearScaleUniforms struct: {}",
            info.uniform_struct_def
        );
        assert!(
            info.uniform_struct_def.contains("ColorMapUniforms"),
            "Missing ColorMapUniforms struct: {}",
            info.uniform_struct_def
        );
        assert!(
            info.uniform_struct_def.contains("struct ChainUniforms"),
            "Missing ChainUniforms struct: {}",
            info.uniform_struct_def
        );
    }

    #[test]
    fn generate_wgsl_with_function_chain_injects_all_code() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 1.0,
            },
            Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        );
        let chain = scale.compose(color_map);
        let info = shader_fn_info_from(&chain);

        let base_wgsl = r#"
struct CircleInstance {
    center: vec2<f32>,
    radius: f32,
    fill_color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<CircleInstance>;

@vertex
fn vs_main() -> VertexOutput {
    let instance = instances[input.instance_index];
    let c = instance.fill_color;
    return c;
}
"#;

        let bindings: Vec<(&str, &ShaderFnInfo)> = vec![("fill_color", &info)];
        let result = generate_shader_bound_vertex_wgsl(base_wgsl, &bindings);

        // Should contain all three functions.
        assert!(
            result.contains("fn linear_scale"),
            "Missing linear_scale fn: {result}"
        );
        assert!(
            result.contains("fn color_map"),
            "Missing color_map fn: {result}"
        );
        assert!(
            result.contains("fn composed_chain"),
            "Missing composed_chain fn: {result}"
        );
        // The uniform binding should reference ChainUniforms.
        assert!(
            result.contains("ChainUniforms"),
            "Missing ChainUniforms reference: {result}"
        );
        // The transformation variable should call composed_chain.
        assert!(
            result.contains("_gup_fill_color = composed_chain("),
            "Missing transformation call: {result}"
        );
    }

    #[test]
    fn attr_shader_stores_function_chain_binding() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        let data = vec![ScatterPoint {
            x: 0.0,
            y: 0.0,
            value: 50.0,
        }];
        let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

        let chain = LinearScale::new(0.0, 100.0, 0.0, 1.0).compose(ColorMap::new(
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 1.0,
            },
            Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        ));

        selection.attr_shader("fill_color", |d: &ScatterPoint| d.value, chain);

        assert!(selection.has_shader_bindings());
        assert_eq!(selection.shader_bound_attributes(), vec!["fill_color"]);
    }

    #[test]
    fn function_chain_type_safety_rejects_wrong_output() {
        use crate::shader_function::{ComposableFunction, LinearScale};

        // LinearScale composes f32 → f32, so a chain of two LinearScales
        // still outputs f32.  Binding to "fill_color" (vec4) should fail.
        let chain =
            LinearScale::new(0.0, 100.0, 0.0, 1.0).compose(LinearScale::new(0.0, 1.0, 0.0, 100.0));

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![ScatterPoint {
                x: 0.0,
                y: 0.0,
                value: 50.0,
            }];
            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);
            selection.attr_shader("fill_color", |d: &ScatterPoint| d.value, chain);

            let result = selection.prepare_render_bound(&context.device, &context.queue, None, None);
            assert!(
                result.is_err(),
                "Should reject f32 chain bound to vec4 attr"
            );
            let err_msg = format!("{}", result.unwrap_err());
            assert!(
                err_msg.contains("not compatible"),
                "Error should mention incompatibility: {err_msg}"
            );
        });
    }

    #[test]
    fn gpu_function_chain_render() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                ScatterPoint {
                    x: -0.3,
                    y: 0.0,
                    value: 20.0,
                },
                ScatterPoint {
                    x: 0.3,
                    y: 0.0,
                    value: 80.0,
                },
            ];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

            // CPU bindings for position and radius.
            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("radius", |_: &ScatterPoint| 0.1f32);

            // GPU binding: linear_scale → color_map for fill_color.
            let chain = LinearScale::new(0.0, 100.0, 0.0, 1.0).compose(ColorMap::new(
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
                Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            ));
            selection.attr_shader("fill_color", |d: &ScatterPoint| d.value, chain);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound with FunctionChain");

            assert!(selection.is_render_ready());

            // Render to verify pipeline compilation succeeds.
            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");
            {
                let mut pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut pass)
                    .expect("render with FunctionChain binding");
            }
            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn extract_wgsl_struct_name_parses_name() {
        assert_eq!(
            extract_wgsl_struct_name("struct Foo {\n    bar: f32,\n}"),
            Some("Foo")
        );
        assert_eq!(
            extract_wgsl_struct_name("struct LinearScaleUniforms {\n    domain_min: f32,\n}"),
            Some("LinearScaleUniforms")
        );
        // Brace on same line as struct keyword
        assert_eq!(
            extract_wgsl_struct_name("struct Compact{ x: f32, }"),
            Some("Compact")
        );
        // Not a struct definition
        assert_eq!(extract_wgsl_struct_name("fn foo() {}"), None);
        assert_eq!(extract_wgsl_struct_name(""), None);
    }

    #[test]
    fn split_wgsl_struct_definitions_single() {
        let input = "struct Foo {\n    bar: f32,\n}";
        let parts = split_wgsl_struct_definitions(input);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].starts_with("struct Foo"));
    }

    #[test]
    fn split_wgsl_struct_definitions_multiple() {
        let input = "struct A {\n    x: f32,\n}\nstruct B {\n    y: f32,\n}";
        let parts = split_wgsl_struct_definitions(input);
        assert_eq!(parts.len(), 2);
        assert_eq!(extract_wgsl_struct_name(parts[0]), Some("A"));
        assert_eq!(extract_wgsl_struct_name(parts[1]), Some("B"));
    }

    #[test]
    fn split_wgsl_struct_definitions_empty() {
        assert!(split_wgsl_struct_definitions("").is_empty());
        assert!(split_wgsl_struct_definitions("fn foo() {}").is_empty());
    }

    #[test]
    fn duplicate_struct_definitions_deduplicated() {
        use crate::shader_function::{ComposableFunction, LinearScale};

        // Two chains that both contain LinearScaleUniforms.
        let chain1 =
            LinearScale::new(0.0, 100.0, 0.0, 1.0).compose(LinearScale::new(0.0, 1.0, 0.0, 10.0));
        let chain2 =
            LinearScale::new(0.0, 50.0, 0.0, 1.0).compose(LinearScale::new(0.0, 1.0, 0.0, 5.0));

        let info1 = shader_fn_info_from(&chain1);
        let info2 = shader_fn_info_from(&chain2);

        // Verify both infos share the same nested struct name.
        assert!(
            info1
                .uniform_struct_def
                .contains("struct LinearScaleUniforms"),
            "info1 should reference LinearScaleUniforms: {}",
            info1.uniform_struct_def
        );
        assert!(
            info2
                .uniform_struct_def
                .contains("struct LinearScaleUniforms"),
            "info2 should reference LinearScaleUniforms: {}",
            info2.uniform_struct_def
        );

        let base_wgsl = r#"
struct CircleInstance {
    center: vec2<f32>,
    radius: f32,
    fill_color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<CircleInstance>;

@vertex
fn vs_main() -> VertexOutput {
    let instance = instances[input.instance_index];
    let r = instance.radius;
    let c = instance.fill_color;
    return r;
}
"#;

        let bindings: Vec<(&str, &ShaderFnInfo)> = vec![("radius", &info1), ("fill_color", &info2)];
        let result = generate_shader_bound_vertex_wgsl(base_wgsl, &bindings);

        // LinearScaleUniforms should appear exactly once.
        let count = result.matches("struct LinearScaleUniforms").count();
        assert_eq!(
            count, 1,
            "Expected exactly 1 LinearScaleUniforms definition, found {count}:\n{result}"
        );

        // ChainUniforms should appear exactly once too (both chains share
        // the same top-level type name).
        let chain_count = result.matches("struct ChainUniforms").count();
        assert_eq!(
            chain_count, 1,
            "Expected exactly 1 ChainUniforms definition, found {chain_count}:\n{result}"
        );
    }

    // --- Deep chain binding tests (GUP-219) ---

    #[test]
    fn deep_chain_shader_fn_info_metadata() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        let chain = LinearScale::new(0.0, 100.0, 0.0, 1.0)
            .compose(LinearScale::new(0.0, 1.0, -1.0, 1.0))
            .compose(ColorMap::new(
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
                Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            ));
        let info = shader_fn_info_from(&chain);

        // Entry point is always composed_chain.
        assert_eq!(info.function_name, "composed_chain");
        assert_eq!(info.input_wgsl_type, "f32");
        assert_eq!(info.output_wgsl_type, "vec4<f32>");
        // Uniform type is ChainUniforms (outermost).
        assert_eq!(info.uniform_type_name, "ChainUniforms");
        // Uniforms must be non-empty (all three functions have uniforms).
        assert!(
            !info.uniform_bytes.is_empty(),
            "Nested ChainUniforms should serialise"
        );
    }

    #[test]
    fn deep_chain_struct_def_includes_renamed_inner() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        let chain = LinearScale::new(0.0, 100.0, 0.0, 1.0)
            .compose(LinearScale::new(0.0, 1.0, -1.0, 1.0))
            .compose(ColorMap::new(
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
                Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            ));
        let info = shader_fn_info_from(&chain);

        // Inner chain struct should be renamed.
        assert!(
            info.uniform_struct_def.contains("struct ChainUniforms_1"),
            "Missing ChainUniforms_1:\n{}",
            info.uniform_struct_def
        );
        // Outer struct should keep the plain name.
        assert!(
            info.uniform_struct_def.contains("struct ChainUniforms {"),
            "Missing outer ChainUniforms:\n{}",
            info.uniform_struct_def
        );
    }

    #[test]
    fn deep_chain_wgsl_injection_produces_valid_shader() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        let chain = LinearScale::new(0.0, 100.0, 0.0, 1.0)
            .compose(LinearScale::new(0.0, 1.0, -1.0, 1.0))
            .compose(ColorMap::new(
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
                Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            ));
        let info = shader_fn_info_from(&chain);

        let base_wgsl = r#"
struct CircleInstance {
    center: vec2<f32>,
    radius: f32,
    fill_color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<CircleInstance>;

@vertex
fn vs_main() -> VertexOutput {
    let instance = instances[input.instance_index];
    let c = instance.fill_color;
    return c;
}
"#;

        let bindings: Vec<(&str, &ShaderFnInfo)> = vec![("fill_color", &info)];
        let result = generate_shader_bound_vertex_wgsl(base_wgsl, &bindings);

        // Must have inner renamed function.
        assert!(
            result.contains("fn composed_chain_1("),
            "Missing composed_chain_1 fn:\n{result}"
        );
        // Must have outer entry point.
        assert!(
            result.contains("fn composed_chain("),
            "Missing composed_chain fn:\n{result}"
        );
        // Must have inner renamed struct.
        assert!(
            result.contains("ChainUniforms_1"),
            "Missing ChainUniforms_1 struct:\n{result}"
        );
        // Must have outer struct.
        assert!(
            result.contains("struct ChainUniforms {"),
            "Missing ChainUniforms struct:\n{result}"
        );
        // The binding should reference ChainUniforms.
        assert!(
            result.contains("_gup_uniforms_0: ChainUniforms"),
            "Missing uniform binding:\n{result}"
        );
        // The transformation call should use composed_chain.
        assert!(
            result.contains("_gup_fill_color = composed_chain("),
            "Missing transformation call:\n{result}"
        );
    }

    #[test]
    fn gpu_deep_function_chain_render() {
        use crate::shader_function::{ColorMap, ComposableFunction, LinearScale};

        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let data = vec![
                ScatterPoint {
                    x: -0.3,
                    y: 0.0,
                    value: 20.0,
                },
                ScatterPoint {
                    x: 0.3,
                    y: 0.0,
                    value: 80.0,
                },
            ];

            let mut selection: Selection<ScatterPoint, Circle> = Selection::from_data(data);

            // CPU bindings for position and radius.
            selection
                .attr("center", |d: &ScatterPoint| [d.x, d.y])
                .attr("radius", |_: &ScatterPoint| 0.1f32);

            // GPU binding: 3-function deep chain for fill_color.
            let chain = LinearScale::new(0.0, 100.0, 0.0, 1.0)
                .compose(LinearScale::new(0.0, 1.0, -1.0, 1.0))
                .compose(ColorMap::new(
                    Vec4 {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                        w: 1.0,
                    },
                    Vec4 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                        w: 1.0,
                    },
                ));
            selection.attr_shader("fill_color", |d: &ScatterPoint| d.value, chain);

            selection
                .prepare_render_bound(&context.device, &context.queue, None, None)
                .expect("prepare_render_bound with deep FunctionChain");

            assert!(selection.is_render_ready());

            // Render to verify pipeline compilation succeeds.
            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");
            {
                let mut pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection
                    .render(&mut pass)
                    .expect("render with deep FunctionChain binding");
            }
            frame.finish().expect("finish frame");
        });
    }

    // --- BufferPool integration tests ---

    #[test]
    fn gpu_prepare_render_with_buffer_pool() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let mut pool = BufferPool::new(Arc::clone(&context.device));

            let data = vec![CircleAttributes {
                center: Vec2 { x: 0.0, y: 0.0 },
                radius: 0.1,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            }];

            let mut selection: Selection<CircleAttributes, Circle> =
                Selection::from_data(data);

            // Prepare with pool — first call is a miss (no pooled buffers).
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    Some(&mut pool),
                )
                .expect("prepare_render with pool");

            assert!(selection.is_render_ready());
            assert_eq!(pool.get_stats().total_allocated, 1);
            assert_eq!(pool.get_stats().pool_misses, 1);

            // Render to an offscreen frame to verify correctness.
            let mut ctx = Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");
            {
                let mut pass = frame.render_pass(Some(wgpu::Color::BLACK));
                selection.render(&mut pass).expect("render with pooled buffer");
            }
            frame.finish().expect("finish frame");
        });
    }

    #[test]
    fn gpu_release_to_pool_enables_reuse() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let mut pool = BufferPool::new(Arc::clone(&context.device));

            // Helper to create a selection with some data.
            let make_data = || {
                vec![CircleAttributes {
                    center: Vec2 { x: 0.0, y: 0.0 },
                    radius: 0.1,
                    fill_color: Vec4 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                        w: 1.0,
                    },
                    stroke_width: 0.0,
                    stroke_color: Vec4 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        w: 0.0,
                    },
                }]
            };

            // First selection: allocate from pool (miss).
            let mut sel1: Selection<CircleAttributes, Circle> =
                Selection::from_data(make_data());
            sel1.prepare_render(
                &context.device,
                &context.queue,
                |a| CircleInstance::from(a),
                None,
                Some(&mut pool),
            )
            .unwrap();

            assert_eq!(pool.get_stats().pool_misses, 1);
            assert_eq!(pool.get_stats().pool_hits, 0);

            // Release buffer back to pool.
            sel1.release_to_pool(&mut pool);
            assert!(!sel1.is_render_ready());
            assert_eq!(pool.get_stats().pooled_buffers, 1);

            // Second selection: allocate from pool (hit!).
            let mut sel2: Selection<CircleAttributes, Circle> =
                Selection::from_data(make_data());
            sel2.prepare_render(
                &context.device,
                &context.queue,
                |a| CircleInstance::from(a),
                None,
                Some(&mut pool),
            )
            .unwrap();

            assert_eq!(pool.get_stats().pool_hits, 1);
            assert_eq!(pool.get_stats().pooled_buffers, 0);

            // Clean up.
            sel2.release_to_pool(&mut pool);
        });
    }

    #[test]
    fn gpu_pool_reallocation_returns_old_buffer() {
        // Test the reallocation path: when the instance data grows beyond
        // the current buffer capacity without calling set_data (which
        // destroys the render state), the old pooled buffer is returned.
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let mut pool = BufferPool::new(Arc::clone(&context.device));

            let make_attr = |x: f32| CircleAttributes {
                center: Vec2 { x, y: 0.0 },
                radius: 0.05,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            };

            // Start with 2 items — small buffer.
            let mut selection: Selection<CircleAttributes, Circle> =
                Selection::from_data(vec![make_attr(-0.5), make_attr(0.5)]);
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    Some(&mut pool),
                )
                .unwrap();

            assert_eq!(pool.get_stats().total_allocated, 1);
            assert!(
                selection
                    .render_state
                    .as_ref()
                    .unwrap()
                    .pool_meta
                    .is_some(),
                "pool_meta should be set after pooled allocation"
            );

            // Directly replace data with 200 items (bypassing set_data which
            // clears render_state). This forces the reallocation path in
            // upload_instances because the new instance_bytes are much larger.
            selection.data = (0..200).map(|i| make_attr(i as f32 * 0.01)).collect();

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    Some(&mut pool),
                )
                .unwrap();

            // Old buffer returned to pool, new one allocated.
            assert_eq!(pool.get_stats().total_allocated, 2);
            assert_eq!(pool.get_stats().total_deallocated, 1);
            // Old (small) buffer is in the pool now.
            assert_eq!(pool.get_stats().pooled_buffers, 1);

            selection.release_to_pool(&mut pool);
        });
    }

    #[test]
    fn gpu_pool_create_destroy_cycle_benchmark() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let mut pool = BufferPool::new(Arc::clone(&context.device));
            let cycles = 100;

            let data = vec![CircleAttributes {
                center: Vec2 { x: 0.0, y: 0.0 },
                radius: 0.1,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            }];

            for _ in 0..cycles {
                let mut sel: Selection<CircleAttributes, Circle> =
                    Selection::from_data(data.clone());
                sel.prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    Some(&mut pool),
                )
                .unwrap();
                sel.release_to_pool(&mut pool);
            }

            let stats = pool.get_stats();
            // First cycle is a miss, all subsequent should be hits.
            assert_eq!(stats.pool_misses, 1, "only the first allocation should miss");
            assert_eq!(
                stats.pool_hits,
                cycles - 1,
                "subsequent allocations should hit"
            );
            assert_eq!(stats.total_allocated, cycles);
            assert_eq!(stats.total_deallocated, cycles);
        });
    }

    #[test]
    fn release_to_pool_noop_without_pool_meta() {
        // A selection prepared without a pool should be a no-op when
        // release_to_pool is called.
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter available");
                    return;
                }
            };

            let mut pool = BufferPool::new(Arc::clone(&context.device));

            let data = vec![CircleAttributes {
                center: Vec2 { x: 0.0, y: 0.0 },
                radius: 0.1,
                fill_color: Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            }];

            let mut selection: Selection<CircleAttributes, Circle> =
                Selection::from_data(data);

            // Prepare WITHOUT pool.
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    None,
                    None,
                )
                .unwrap();

            // Releasing to pool should not add anything (no pool_meta).
            let before = pool.get_stats().pooled_buffers;
            selection.release_to_pool(&mut pool);
            assert_eq!(pool.get_stats().pooled_buffers, before);
        });
    }
}
