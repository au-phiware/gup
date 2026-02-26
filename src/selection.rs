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
//! selection.prepare_render(&device, &queue, |attrs| CircleInstance::from(attrs), None)?;
//!
//! // Later, in a render pass:
//! selection.render(&mut render_pass)?;
//! ```

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
    /// # Errors
    ///
    /// Returns an error if pipeline or buffer creation fails.
    pub fn prepare_render<I>(
        &mut self,
        device: &Device,
        queue: &Queue,
        mapper: impl Fn(&T) -> I,
        cache: Option<&mut PipelineCache>,
    ) -> GupResult<()>
    where
        I: bytemuck::Pod + bytemuck::Zeroable,
    {
        // Convert data items to GPU instances.
        let instances: Vec<I> = self.data.iter().map(&mapper).collect();
        let instance_bytes: &[u8] = bytemuck::cast_slice(&instances);
        let instance_count = instances.len() as u32;

        self.upload_instances(device, queue, instance_bytes, instance_count, cache)
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
    ///     .prepare_render_bound(&device, &queue, None)?;
    /// ```
    pub fn prepare_render_bound(
        &mut self,
        device: &Device,
        queue: &Queue,
        cache: Option<&mut PipelineCache>,
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

        self.upload_instances(device, queue, instance_bytes, instance_count, cache)
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
                for sb in gpu_bindings {
                    attr_values.push((sb.name.as_str(), (sb.extractor)(t)));
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
    ) -> GupResult<()> {
        if let Some(ref mut state) = self.render_state {
            // Re-use existing pipeline and vertex buffers.
            // If the instance buffer is too small, reallocate.
            if instance_bytes.len() > state.instance_buffer_capacity {
                let (instance_buffer, bind_group) =
                    SelectionRenderState::create_instance_buffer_and_bind_group(
                        device,
                        &state.pipeline,
                        instance_bytes,
                    );
                state.instance_buffer = instance_buffer;
                state.bind_group = bind_group;
                state.instance_buffer_capacity = instance_bytes.len();
            } else {
                // Buffer large enough: just re-upload.
                queue.write_buffer(&state.instance_buffer, 0, instance_bytes);
            }
            state.instance_count = instance_count;
        } else {
            // First-time setup: create everything.
            let state =
                SelectionRenderState::new::<M>(device, instance_bytes, instance_count, cache)?;
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

        // Remove any previous registration for this selection.
        if let Some(old_root) = self.aria_root_node.take() {
            aria_tree.remove_subtree(old_root);
        }

        let total = self.data.len();
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

    /// Automatically register ARIA from the [`RenderContext`]'s accessibility
    /// system.
    ///
    /// This is the recommended way to enable automatic ARIA registration.
    /// If `auto_aria` is enabled (the default), and the selection's
    /// [`RenderContext`] has an [`AccessibilitySystem`] attached, this method
    /// generates and registers the ARIA tree.
    ///
    /// Call this after [`prepare_render`](Self::prepare_render) or
    /// [`prepare_render_bound`](Self::prepare_render_bound) to ensure the
    /// ARIA tree reflects the current data.
    ///
    /// Returns `true` if a new ARIA tree was registered.
    pub fn sync_aria_from_context(&mut self) -> bool
    where
        M: AccessibleMark,
    {
        if !self.auto_aria {
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
}

impl<T, M: Mark> Drop for Selection<T, M> {
    fn drop(&mut self) {
        // Automatically deregister ARIA tree when selection is dropped.
        if let Some(root) = self.aria_root_node.take()
            && let Some(acc) = self
                .context
                .as_ref()
                .and_then(|ctx| ctx.accessibility().cloned())
                && let Ok(mut system) = acc.lock() {
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
    /// Uniform buffers for GPU shader function bindings (empty when no shader
    /// functions are used).
    uniform_buffers: Vec<wgpu::Buffer>,
}

impl SelectionRenderState {
    /// Create a complete render state for a mark type.
    fn new<M: Mark>(
        device: &Device,
        instance_bytes: &[u8],
        instance_count: u32,
        cache: Option<&mut PipelineCache>,
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

        // --- Instance buffer + bind group ----------------------------
        let (instance_buffer, bind_group) =
            Self::create_instance_buffer_and_bind_group(device, &pipeline, instance_bytes);

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
            uniform_buffers: Vec::new(),
        })
    }

    /// Create (or recreate) the instance storage buffer and matching bind group.
    fn create_instance_buffer_and_bind_group(
        device: &Device,
        pipeline: &wgpu::RenderPipeline,
        instance_bytes: &[u8],
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        // Ensure at least 16 bytes for wgpu minimum buffer size requirements.
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("selection_instance_buffer"),
            contents: if instance_bytes.is_empty() {
                &[0u8; 16]
            } else {
                instance_bytes
            },
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Derive bind group layout from the pipeline (guaranteed to match).
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("selection_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: instance_buffer.as_entire_binding(),
            }],
        });

        (instance_buffer, bind_group)
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
            uniform_buffers,
        })
    }
}

// ---------------------------------------------------------------------------
// WGSL generation for shader function attribute bindings
// ---------------------------------------------------------------------------

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
    for (i, (_attr_name, info)) in bindings.iter().enumerate() {
        // Uniform struct definition (skip if empty / primitive type).
        let struct_def = &info.uniform_struct_def;
        if !struct_def.is_empty() && struct_def != "f32" && struct_def != "i32" {
            result.push_str(struct_def);
            result.push('\n');
        }
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
            result.push_str(&format!(
                "    let _gup_{attr} = {fn_name}(instance.{attr}, _gup_uniforms_{i});\n",
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
    use crate::mark::{Circle, Rectangle};
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
                )
                .expect("rect prepare");
            circle_sel
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
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
                stroke_width: 0.01,
                outlier_radius: 0.03,
                ..Default::default()
            }];

            let mut selection: Selection<BoxPlotAttributes, BoxPlot> = Selection::from_data(data);

            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| BoxPlotInstance::from(a),
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
                        stroke_width: 0.005,
                        outlier_radius: 0.02,
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
                stroke_width: 0.01,
                outlier_radius: 0.03,
                ..Default::default()
            }];

            let mut selection: Selection<BoxPlotAttributes, BoxPlot> = Selection::from_data(data);
            selection
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| BoxPlotInstance::from(a),
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
            let result = selection.prepare_render_bound(&context.device, &context.queue, None);
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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

            let result = selection.prepare_render_bound(&context.device, &context.queue, None);
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
                .prepare_render_bound(&context.device, &context.queue, None)
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
}
