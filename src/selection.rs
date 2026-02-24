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

    /// Set an attribute on the selection.
    pub fn attr<V>(&mut self, _name: &str, _value: V) -> &mut Self
    where
        V: Send + Sync + 'static,
    {
        // Placeholder implementation
        self
    }

    /// Set multiple attributes from a parallel composition.
    ///
    /// This method enables efficient multi-attribute binding where a single shader
    /// function computes multiple outputs (e.g., position and color) from the same
    /// input data.
    ///
    /// # Arguments
    ///
    /// * `parallel_function` - A ParallelComposition that computes multiple outputs
    /// * `attribute_names` - Array of attribute names matching the outputs (e.g., ["position", "color"])
    ///
    /// # Type Safety
    ///
    /// The compiler ensures that the parallel function outputs are compatible with
    /// the mark's attribute types.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use gup::prelude::*;
    ///
    /// // Create parallel composition: data -> (position, color)
    /// let position_fn = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    /// let color_fn = ColorMap::new(min_color, max_color);
    /// let parallel = position_fn.parallel(color_fn);
    ///
    /// // Bind both attributes in single call
    /// selection.attr_parallel(parallel, ["position", "color"]);
    /// ```
    ///
    /// ```rust,ignore
    /// // 3-way parallel binding (position XY + color + size)
    /// let xy_and_color = x_scale.parallel(color_fn);
    /// let all_three = xy_and_color.parallel(size_fn);
    /// selection.attr_parallel(all_three, ["position", "color", "size"]);
    /// ```
    pub fn attr_parallel<P, const N: usize>(
        &mut self,
        _parallel_function: P,
        _attribute_names: [&str; N],
    ) -> &mut Self
    where
        P: Send + Sync + 'static,
    {
        // Placeholder implementation - will be integrated with mark rendering system
        // when the full attribute binding pipeline is implemented
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
