// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public WASM API for rendering Gup charts from JavaScript.
//!
//! This module provides `#[wasm_bindgen]`-exported functions that JavaScript
//! callers (including Tauri WebView frontends) can use to render GPU-accelerated
//! charts inside HTML `<canvas>` elements.
//!
//! # Usage from JavaScript
//!
//! ```js
//! import init, { render_scatter } from './gup.js';
//!
//! await init();
//! const data = JSON.stringify([
//!   { x: 1.0, y: 2.0 },
//!   { x: 3.0, y: 4.0 },
//! ]);
//! await render_scatter('my-canvas', data);
//! ```
//!
//! # Architecture
//!
//! Each call to `render_scatter` performs a complete GPU render cycle:
//!
//! 1. Obtain the `<canvas>` element from the DOM.
//! 2. Create (or reuse) a WebGPU surface, adapter, device, and queue.
//! 3. Parse the JSON data into scatter points.
//! 4. Build circle instance data and GPU buffers.
//! 5. Render circles in a single render pass to the surface texture.
//! 6. Present the frame.
//!
//! GPU resources are cached in a module-level `RefCell` so that repeated
//! calls with different data reuse the device and pipeline, avoiding canvas
//! or GPU resource leaks.

#[cfg(target_arch = "wasm32")]
mod inner {
    use serde::Deserialize;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use wasm_bindgen::prelude::*;

    // -----------------------------------------------------------------------
    // Public data types
    // -----------------------------------------------------------------------

    /// A single scatter data point as received from JavaScript via JSON.
    #[derive(Debug, Clone, Deserialize)]
    pub struct ScatterPoint {
        /// X-coordinate value.
        pub x: f32,
        /// Y-coordinate value.
        pub y: f32,
    }

    // -----------------------------------------------------------------------
    // Cached GPU state (per-canvas)
    // -----------------------------------------------------------------------

    /// GPU resources tied to a single canvas element.
    struct CanvasState {
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
        device: wgpu::Device,
        queue: wgpu::Queue,
        pipeline: wgpu::RenderPipeline,
    }

    thread_local! {
        /// Per-canvas GPU state, keyed by canvas element ID.
        static CANVAS_STATES: RefCell<HashMap<String, CanvasState>> = RefCell::new(HashMap::new());
    }

    // -----------------------------------------------------------------------
    // WGSL circle shader (same algorithm as the scatter window example)
    // -----------------------------------------------------------------------

    const CIRCLE_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) center: vec2<f32>,
    @location(3) radius: f32,
    @location(4) fill_color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = position * radius + center;
    out.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    out.color = fill_color;
    out.local_pos = local_pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = length(in.local_pos);
    let alpha = 1.0 - smoothstep(0.9, 1.0, dist);
    if (alpha < 0.01) {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;

    // -----------------------------------------------------------------------
    // GPU vertex / instance layout
    // -----------------------------------------------------------------------

    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct QuadVertex {
        position: [f32; 2],
        local_pos: [f32; 2],
    }

    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct CircleInstance {
        center: [f32; 2],
        radius: f32,
        _pad: f32,
        fill_color: [f32; 4],
    }

    /// Unit quad vertices for instanced circle rendering.
    const QUAD_VERTICES: &[QuadVertex] = &[
        QuadVertex {
            position: [-1.0, -1.0],
            local_pos: [-1.0, -1.0],
        },
        QuadVertex {
            position: [1.0, -1.0],
            local_pos: [1.0, -1.0],
        },
        QuadVertex {
            position: [1.0, 1.0],
            local_pos: [1.0, 1.0],
        },
        QuadVertex {
            position: [-1.0, 1.0],
            local_pos: [-1.0, 1.0],
        },
    ];

    const QUAD_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

    // -----------------------------------------------------------------------
    // Data → GPU instance conversion
    // -----------------------------------------------------------------------

    /// Compute axis-aligned bounding box with 10 % padding.
    fn compute_data_range(points: &[ScatterPoint]) -> (f32, f32, f32, f32) {
        let x_min = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let x_max = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let y_min = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let y_max = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        let x_pad = (x_max - x_min).max(0.001) * 0.1;
        let y_pad = (y_max - y_min).max(0.001) * 0.1;
        (x_min - x_pad, x_max + x_pad, y_min - y_pad, y_max + y_pad)
    }

    fn points_to_instances(points: &[ScatterPoint]) -> Vec<CircleInstance> {
        if points.is_empty() {
            return Vec::new();
        }
        let (x_min, x_max, y_min, y_max) = compute_data_range(points);

        points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // Map to clip space [-0.85, 0.85] with padding for visual margin.
                let nx = ((p.x - x_min) / (x_max - x_min)) * 1.7 - 0.85;
                let ny = ((p.y - y_min) / (y_max - y_min)) * 1.7 - 0.85;

                // Simple colour ramp: hue varies across points.
                let t = i as f32 / (points.len().max(1) as f32);
                let r = 0.2 + 0.6 * t;
                let g = 0.4 * (1.0 - t);
                let b = 0.8 * (1.0 - t);

                CircleInstance {
                    center: [nx, ny],
                    radius: 0.04,
                    _pad: 0.0,
                    fill_color: [r, g, b, 0.9],
                }
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Pipeline creation
    // -----------------------------------------------------------------------

    fn create_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gup_scatter_wasm_shader"),
            source: wgpu::ShaderSource::Wgsl(CIRCLE_SHADER.into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gup_scatter_wasm_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gup_scatter_wasm_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Slot 0: per-vertex quad data
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    },
                    // Slot 1: per-instance circle data
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<CircleInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    // -----------------------------------------------------------------------
    // Canvas initialisation
    // -----------------------------------------------------------------------

    async fn init_canvas(canvas_id: &str) -> Result<CanvasState, JsValue> {
        let document = web_sys::window()
            .ok_or_else(|| JsValue::from_str("No global `window` found"))?
            .document()
            .ok_or_else(|| JsValue::from_str("No `document` found"))?;

        let element = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("Canvas element '{canvas_id}' not found")))?;

        let canvas: web_sys::HtmlCanvasElement = element
            .dyn_into()
            .map_err(|_| JsValue::from_str("Element is not an HtmlCanvasElement"))?;

        // Create wgpu instance targeting the browser WebGPU backend.
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&format!("Failed to create surface: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("No suitable GPU adapter: {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("gup_wasm_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: Default::default(),
                experimental_features: Default::default(),
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to create device: {e}")))?;

        let width = canvas.width().max(1);
        let height = canvas.height().max(1);

        let surface_config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| JsValue::from_str("Surface configuration not supported"))?;
        surface.configure(&device, &surface_config);

        let pipeline = create_pipeline(&device, surface_config.format);

        Ok(CanvasState {
            surface,
            surface_config,
            device,
            queue,
            pipeline,
        })
    }

    // -----------------------------------------------------------------------
    // Render a single frame
    // -----------------------------------------------------------------------

    fn render_frame(state: &CanvasState, instances: &[CircleInstance]) -> Result<(), JsValue> {
        use wgpu::util::DeviceExt;

        let frame = state
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("Failed to get surface texture: {e}")))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Upload buffers
        let vertex_buffer = state
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scatter_vertex_buf"),
                contents: bytemuck::cast_slice(QUAD_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = state
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scatter_index_buf"),
                contents: bytemuck::cast_slice(QUAD_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        let instance_buffer = state
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scatter_instance_buf"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scatter_encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scatter_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.98,
                            g: 0.98,
                            b: 0.98,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&state.pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, instance_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..QUAD_INDICES.len() as u32, 0, 0..instances.len() as u32);
        }

        state.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Public JS-callable API
    // -----------------------------------------------------------------------

    /// Render a scatter plot to an HTML `<canvas>` element.
    ///
    /// # Arguments
    ///
    /// * `canvas_id` — The DOM `id` attribute of the target `<canvas>` element.
    /// * `data_json` — A JSON string encoding an array of `{x, y}` objects,
    ///   e.g. `[{"x": 1.0, "y": 2.0}, {"x": 3.0, "y": 4.0}]`.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error string if:
    /// - The canvas element cannot be found.
    /// - WebGPU is not available in the current browser / WebView.
    /// - The JSON data is malformed or empty.
    ///
    /// # Repeated Calls
    ///
    /// Calling this function multiple times with the same `canvas_id` reuses
    /// the underlying GPU device and render pipeline, updating the chart
    /// in place without leaking resources.
    #[wasm_bindgen]
    pub async fn render_scatter(canvas_id: &str, data_json: &str) -> Result<(), JsValue> {
        // Parse data.
        let points: Vec<ScatterPoint> = serde_json::from_str(data_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid JSON data: {e}")))?;

        if points.is_empty() {
            return Err(JsValue::from_str("Data array must not be empty"));
        }

        let instances = points_to_instances(&points);

        // Initialise canvas GPU state if this is the first call for this ID.
        let needs_init = CANVAS_STATES.with(|states| !states.borrow().contains_key(canvas_id));

        if needs_init {
            let state = init_canvas(canvas_id).await?;
            CANVAS_STATES.with(|states| {
                states.borrow_mut().insert(canvas_id.to_string(), state);
            });
        }

        // Render the frame.
        CANVAS_STATES.with(|states| {
            let states = states.borrow();
            let state = states
                .get(canvas_id)
                .ok_or_else(|| JsValue::from_str("Canvas state unexpectedly missing"))?;
            render_frame(state, &instances)
        })
    }

    /// Render a chart from a [`ChartBundle`] or [`ChartSnapshot`] JSON string.
    ///
    /// This is the primary entry point for the HTML export round-trip.  The
    /// JavaScript bootstrap reads the embedded `#gup-chart-data` JSON block
    /// and passes it to this function, which parses the configuration and
    /// data, then renders a scatter plot onto the specified canvas.
    ///
    /// The function accepts two JSON formats:
    ///
    /// * **`ChartBundle`** — `{"config": {…}, "data": [{…}, …]}` — uses the
    ///   embedded data array as scatter points.
    /// * **`ChartSnapshot`** — `{"title": "…", "width": 800, …}` — falls
    ///   back to a placeholder when no data is present.
    ///
    /// # Arguments
    ///
    /// * `canvas_id` — DOM `id` of the target `<canvas>` element.
    /// * `bundle_json` — JSON string in either `ChartBundle` or
    ///   `ChartSnapshot` format.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if the JSON cannot be parsed or the
    /// canvas / WebGPU is unavailable.
    #[wasm_bindgen]
    pub async fn render_from_bundle(canvas_id: &str, bundle_json: &str) -> Result<(), JsValue> {
        use crate::export::html::{ChartBundle, ChartSnapshot};

        // Try ChartBundle first, then fall back to ChartSnapshot.
        let data_json = if let Ok(bundle) = serde_json::from_str::<ChartBundle>(bundle_json) {
            // Extract scatter data from the bundle's data array.
            if let Some(data) = &bundle.data {
                serde_json::to_string(data)
                    .map_err(|e| JsValue::from_str(&format!("Failed to re-serialise data: {e}")))?
            } else {
                // Config-only bundle: no data to render.
                return Err(JsValue::from_str(
                    "ChartBundle has no data array — nothing to render",
                ));
            }
        } else if serde_json::from_str::<ChartSnapshot>(bundle_json).is_ok() {
            // ChartSnapshot without data.
            return Err(JsValue::from_str(
                "ChartSnapshot has no data — use render_scatter with explicit data instead",
            ));
        } else {
            return Err(JsValue::from_str(&format!(
                "Failed to parse JSON as ChartBundle or ChartSnapshot: {}",
                &bundle_json[..bundle_json.len().min(200)]
            )));
        };

        // Delegate to render_scatter.
        render_scatter(canvas_id, &data_json).await
    }
}

// Re-export the public API at crate level (only on WASM).
#[cfg(target_arch = "wasm32")]
pub use inner::*;

// -----------------------------------------------------------------------
// Non-WASM helper types (for testing deserialization on native)
// -----------------------------------------------------------------------

/// A single scatter data point.
///
/// This type is used by both the WASM public API and native test
/// utilities. On WASM targets it is re-exported from the `inner` module;
/// on native targets the definition here keeps the JSON parsing logic
/// testable without requiring a browser.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ScatterPoint {
    /// X-coordinate value.
    pub x: f32,
    /// Y-coordinate value.
    pub y: f32,
}

/// Parse a JSON string as a [`ChartBundle`] or [`ChartSnapshot`].
///
/// This function implements the same parsing logic used by the WASM
/// [`render_from_bundle`] function, making it possible to test the
/// round-trip pipeline on native targets without a browser.
///
/// # Returns
///
/// A tuple of `(Option<ChartSnapshot>, Option<Vec<serde_json::Value>>)`
/// representing the config snapshot and optional data array.
///
/// # Errors
///
/// Returns an error if the JSON cannot be parsed as either format.
pub fn parse_bundle_json(
    json: &str,
) -> Result<
    (
        crate::export::html::ChartSnapshot,
        Option<Vec<serde_json::Value>>,
    ),
    String,
> {
    use crate::export::html::{ChartBundle, ChartSnapshot};

    // Try ChartBundle first.
    if let Ok(bundle) = serde_json::from_str::<ChartBundle>(json) {
        return Ok((bundle.config, bundle.data));
    }

    // Fall back to ChartSnapshot.
    if let Ok(snapshot) = serde_json::from_str::<ChartSnapshot>(json) {
        return Ok((snapshot, None));
    }

    Err(format!(
        "Failed to parse JSON as ChartBundle or ChartSnapshot: {}",
        &json[..json.len().min(200)]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scatter_data_basic() {
        let json = r#"[{"x": 1.0, "y": 2.0}, {"x": 3.0, "y": 4.0}]"#;
        let points: Vec<ScatterPoint> = serde_json::from_str(json).unwrap();
        assert_eq!(points.len(), 2);
        assert!((points[0].x - 1.0).abs() < f32::EPSILON);
        assert!((points[0].y - 2.0).abs() < f32::EPSILON);
        assert!((points[1].x - 3.0).abs() < f32::EPSILON);
        assert!((points[1].y - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_scatter_data_integers() {
        let json = r#"[{"x": 10, "y": 20}]"#;
        let points: Vec<ScatterPoint> = serde_json::from_str(json).unwrap();
        assert_eq!(points.len(), 1);
        assert!((points[0].x - 10.0).abs() < f32::EPSILON);
        assert!((points[0].y - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_scatter_data_empty_array() {
        let json = "[]";
        let points: Vec<ScatterPoint> = serde_json::from_str(json).unwrap();
        assert!(points.is_empty());
    }

    #[test]
    fn test_parse_scatter_data_malformed() {
        let json = r#"[{"x": 1.0}]"#; // missing y
        let result: Result<Vec<ScatterPoint>, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_scatter_data_not_array() {
        let json = r#"{"x": 1.0, "y": 2.0}"#;
        let result: Result<Vec<ScatterPoint>, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_scatter_data_large_values() {
        let json = r#"[{"x": 1e6, "y": -1e6}]"#;
        let points: Vec<ScatterPoint> = serde_json::from_str(json).unwrap();
        assert!((points[0].x - 1_000_000.0).abs() < 1.0);
        assert!((points[0].y - (-1_000_000.0)).abs() < 1.0);
    }

    // -- parse_bundle_json tests -------------------------------------------

    #[test]
    fn test_parse_bundle_json_with_chart_bundle() {
        let json = r#"{
            "config": {
                "title": "Test",
                "subtitle": null,
                "width": 800.0,
                "height": 600.0,
                "margins": {"top": 60.0, "right": 40.0, "bottom": 60.0, "left": 60.0},
                "background_color": null,
                "show_axes": true,
                "show_grid": true
            },
            "data": [{"x": 1.0, "y": 2.0}, {"x": 3.0, "y": 4.0}]
        }"#;

        let (snapshot, data) = parse_bundle_json(json).unwrap();
        assert_eq!(snapshot.title.as_deref(), Some("Test"));
        assert_eq!(snapshot.width, 800.0);
        assert!(data.is_some());
        assert_eq!(data.unwrap().len(), 2);
    }

    #[test]
    fn test_parse_bundle_json_with_chart_snapshot() {
        let json = r#"{
            "title": "Snapshot Only",
            "subtitle": null,
            "width": 640.0,
            "height": 480.0,
            "margins": {"top": 10.0, "right": 10.0, "bottom": 10.0, "left": 10.0},
            "background_color": null,
            "show_axes": false,
            "show_grid": false
        }"#;

        let (snapshot, data) = parse_bundle_json(json).unwrap();
        assert_eq!(snapshot.title.as_deref(), Some("Snapshot Only"));
        assert_eq!(snapshot.width, 640.0);
        assert!(data.is_none());
    }

    #[test]
    fn test_parse_bundle_json_invalid() {
        let json = r#"{"not": "valid"}"#;
        let result = parse_bundle_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_bundle_json_config_only_bundle() {
        let json = r#"{
            "config": {
                "title": "Config Only",
                "subtitle": null,
                "width": 400.0,
                "height": 300.0,
                "margins": {"top": 0.0, "right": 0.0, "bottom": 0.0, "left": 0.0},
                "background_color": null,
                "show_axes": false,
                "show_grid": false
            }
        }"#;

        let (snapshot, data) = parse_bundle_json(json).unwrap();
        assert_eq!(snapshot.title.as_deref(), Some("Config Only"));
        assert!(data.is_none());
    }
}
