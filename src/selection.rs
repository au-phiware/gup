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
//! selection.prepare_render(&device, &queue, |attrs| CircleInstance::from(attrs))?;
//!
//! // Later, in a render pass:
//! selection.render(&mut render_pass)?;
//! ```

use crate::interaction::{InteractionElement, InteractionEvent, Renderable};
use crate::mark::{MarkInfo, MarkInfoImpl};
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
    /// # Errors
    ///
    /// Returns an error if pipeline or buffer creation fails.
    pub fn prepare_render<I>(
        &mut self,
        device: &Device,
        queue: &Queue,
        mapper: impl Fn(&T) -> I,
    ) -> GupResult<()>
    where
        I: bytemuck::Pod + bytemuck::Zeroable,
    {
        // Convert data items to GPU instances.
        let instances: Vec<I> = self.data.iter().map(&mapper).collect();
        let instance_bytes: &[u8] = bytemuck::cast_slice(&instances);
        let instance_count = instances.len() as u32;

        self.upload_instances(device, queue, instance_bytes, instance_count)
    }

    /// Prepare GPU resources using the attribute bindings stored via
    /// [`attr`](Self::attr) / [`attr_parallel`](Self::attr_parallel).
    ///
    /// This method replaces the manual `mapper` closure required by
    /// [`prepare_render`](Self::prepare_render). Instead, it evaluates the
    /// stored attribute bindings for each data item and uses the mark's
    /// [`MarkInstanceBuilder`] to construct GPU instance data.
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
    ///     .prepare_render_bound(&device, &queue)?;
    /// ```
    pub fn prepare_render_bound(&mut self, device: &Device, queue: &Queue) -> GupResult<()>
    where
        M: MarkInstanceBuilder,
    {
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

        self.upload_instances(device, queue, instance_bytes, instance_count)
    }

    /// Get the names of currently bound attributes.
    pub fn bound_attributes(&self) -> Vec<&str> {
        self.attr_bindings.iter().map(|b| b.name.as_str()).collect()
    }

    /// Returns `true` if any attribute bindings have been set.
    pub fn has_attr_bindings(&self) -> bool {
        !self.attr_bindings.is_empty()
    }

    /// Internal helper: upload pre-computed instance bytes to the GPU.
    fn upload_instances(
        &mut self,
        device: &Device,
        queue: &Queue,
        instance_bytes: &[u8],
        instance_count: u32,
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
            let state = SelectionRenderState::new::<M>(device, instance_bytes, instance_count)?;
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

/// Internal GPU state for rendering a Selection's data.
///
/// Holds the render pipeline, vertex/index/instance buffers, and bind group.
/// Created by [`Selection::prepare_render`] and consumed by
/// [`Selection::render`].
struct SelectionRenderState {
    /// Render pipeline created from the mark's shaders.
    pipeline: wgpu::RenderPipeline,
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
}

impl SelectionRenderState {
    /// Create a complete render state for a mark type.
    fn new<M: Mark>(
        device: &Device,
        instance_bytes: &[u8],
        instance_count: u32,
    ) -> GupResult<Self> {
        // --- Pipeline ------------------------------------------------
        let mark_info = MarkInfoImpl::<M>::new();
        let pipeline = mark_info.create_render_pipeline(device)?;

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
}

/// Placeholder types for shader functions (will be properly implemented)
pub struct PositionShaderFunction<F, T> {
    _function: PhantomData<F>,
    _data: PhantomData<T>,
}

impl<F, T> PositionShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 2] + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    pub fn new(_function: F) -> Self {
        Self {
            _function: PhantomData,
            _data: PhantomData,
        }
    }
}

pub struct ColorShaderFunction<F, T> {
    _function: PhantomData<F>,
    _data: PhantomData<T>,
}

impl<F, T> ColorShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 4] + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    pub fn new(_function: F) -> Self {
        Self {
            _function: PhantomData,
            _data: PhantomData,
        }
    }
}

// Note: Stub Line and LineAttributes types removed as they are no longer needed.
// The Selection system will be properly implemented in the future.

/// Implement Renderable trait for Selection to enable GPU interaction queries.
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
                .prepare_render(&context.device, &context.queue, |a| CircleInstance::from(a))
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
                .prepare_render(&context.device, &context.queue, |a| {
                    RectangleInstance::from(a)
                })
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
                .prepare_render(&context.device, &context.queue, |a| CircleInstance::from(a))
                .expect("first prepare_render");

            assert!(selection.is_render_ready());

            // Second prepare with same-sized data — reuses pipeline + buffers.
            selection
                .prepare_render(&context.device, &context.queue, |a| CircleInstance::from(a))
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
                .prepare_render(&context.device, &context.queue, |a| CircleInstance::from(a))
                .expect("first prepare");

            // Grow to 10 items — triggers buffer resize.
            let large_data: Vec<_> = (0..10).map(|i| make_attr(-0.9 + i as f32 * 0.2)).collect();
            selection.set_data(large_data);

            selection
                .prepare_render(&context.device, &context.queue, |a| CircleInstance::from(a))
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
                .prepare_render(&context.device, &context.queue, |a| CircleInstance::from(a))
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
                .prepare_render(&context.device, &context.queue, |a| {
                    RectangleInstance::from(a)
                })
                .expect("rect prepare");
            circle_sel
                .prepare_render(&context.device, &context.queue, |a| CircleInstance::from(a))
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
                .prepare_render(&context.device, &context.queue, |a| {
                    BoxPlotInstance::from(a)
                })
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
                .prepare_render(&context.device, &context.queue, |a| {
                    BoxPlotInstance::from(a)
                })
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
                .prepare_render(&context.device, &context.queue, |a| {
                    BoxPlotInstance::from(a)
                })
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
}
