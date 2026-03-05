// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`GupWidget`] stateful widget and [`DynChart`] trait.

use egui::{ColorImage, TextureHandle, TextureOptions};
use gup::chart_builder::ComposedChart;
use gup::error::GupResult;
use gup::render::RenderContext;

use crate::bridge;

// ---------------------------------------------------------------------------
// DynChart — object-safe chart rendering trait
// ---------------------------------------------------------------------------

/// Object-safe trait that allows type-erased chart rendering.
///
/// Implemented automatically for any [`ComposedChart<T, M>`] whose type
/// parameters satisfy the required bounds, so users never need to implement
/// this trait manually.
pub trait DynChart: Send + Sync + 'static {
    /// Prepare GPU resources and record draw commands.
    fn render(&mut self, context: &mut RenderContext) -> GupResult<()>;

    /// Render the chart to tightly-packed RGBA pixels at the given dimensions.
    fn render_to_rgba(&mut self, width: u32, height: u32) -> GupResult<Vec<u8>>;

    /// Render the chart to PNG bytes at the given pixel dimensions.
    fn render_to_png(&mut self, width: u32, height: u32) -> GupResult<Vec<u8>>;

    /// Render the chart directly into the provided [`wgpu::TextureView`].
    ///
    /// This is the zero-copy path: all draw commands target the supplied view
    /// and are submitted to the GPU queue. No readback or encoding takes
    /// place — the rendered pixels stay on the GPU.
    fn render_to_texture_view(
        &mut self,
        view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> GupResult<()>;
}

impl<T, M> DynChart for ComposedChart<T, M>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
    M: gup::mark::Mark,
{
    fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        ComposedChart::render(self, context)
    }

    fn render_to_rgba(&mut self, width: u32, height: u32) -> GupResult<Vec<u8>> {
        ComposedChart::render_to_rgba(self, width, height)
    }

    fn render_to_png(&mut self, width: u32, height: u32) -> GupResult<Vec<u8>> {
        ComposedChart::render_to_png(self, width, height)
    }

    fn render_to_texture_view(
        &mut self,
        view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> GupResult<()> {
        ComposedChart::render_to_texture_view(self, view, surface_format, width, height)
    }
}

// ---------------------------------------------------------------------------
// GupWidget — stateful egui widget
// ---------------------------------------------------------------------------

/// A stateful egui widget that renders a Gup chart inside any egui panel.
///
/// Supports two rendering paths:
///
/// - **Pixel-buffer (fallback)** — created via [`GupWidget::new`]. The chart
///   renders to its own GPU device and the pixels are read back to the CPU,
///   then uploaded to egui as a [`ColorImage`].
/// - **Shared device (zero-copy)** — created via
///   [`GupWidget::with_render_state`]. The chart renders on egui's GPU
///   device; the resulting texture is registered directly with egui's
///   renderer, eliminating CPU readback.
///
/// # Usage
///
/// ```rust,ignore
/// // Pixel-buffer fallback:
/// let mut widget = GupWidget::new(my_chart);
///
/// // Zero-copy shared device (preferred when using wgpu backend):
/// let render_state = cc.wgpu_render_state.as_ref().unwrap();
/// let egui_ctx = GupEguiContext::from_render_state(render_state);
/// let chart = scatter().build_with_data(data, egui_ctx.render_context().clone())?;
/// let mut widget = GupWidget::with_render_state(chart, render_state.clone());
///
/// // Either way, usage is the same:
/// ui.add(&mut widget);
/// widget.mark_dirty();
/// ```
pub struct GupWidget {
    /// The type-erased chart.
    chart: Box<dyn DynChart>,
    /// Internal dirty flag — set when the chart needs re-rendering.
    dirty: bool,
    /// The last rendered size (width, height) in physical pixels.
    last_size: Option<[u32; 2]>,
    /// Cached egui texture handle from the previous render (pixel-buffer path).
    texture_handle: Option<TextureHandle>,
    /// Translated interaction events from the most recent frame.
    pending_events: Vec<gup::interaction::InteractionEvent>,
    /// Shared-device state (Some when using the zero-copy path).
    shared_state: Option<SharedDeviceState>,
}

/// State for the zero-copy shared-device rendering path.
struct SharedDeviceState {
    /// The egui_wgpu render state (provides device/queue and renderer).
    render_state: eframe::egui_wgpu::RenderState,
    /// The offscreen texture the chart renders into.
    offscreen_texture: Option<OffscreenTexture>,
    /// The egui texture ID registered for the offscreen texture.
    egui_texture_id: Option<egui::TextureId>,
}

impl Drop for SharedDeviceState {
    fn drop(&mut self) {
        // Free the texture from egui's renderer to avoid a resource leak.
        if let Some(id) = self.egui_texture_id.take() {
            self.render_state.renderer.write().free_texture(&id);
        }
    }
}

/// An offscreen texture on the shared device.
struct OffscreenTexture {
    /// The GPU texture (Rgba8UnormSrgb format, RENDER_ATTACHMENT | TEXTURE_BINDING).
    #[allow(dead_code)]
    texture: wgpu::Texture,
    /// View in Rgba8UnormSrgb format for chart rendering.
    render_view: wgpu::TextureView,
    /// View in Rgba8Unorm format for egui sampling.
    sample_view: wgpu::TextureView,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
}

/// The texture format the chart renders into (sRGB for correct gamma).
const CHART_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The texture format egui samples from (linear — egui expects gamma-space
/// bytes without hardware sRGB decode).
const EGUI_SAMPLE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl OffscreenTexture {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gup_egui_offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CHART_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            // Allow creating a Rgba8Unorm view for egui sampling.
            view_formats: &[EGUI_SAMPLE_FORMAT],
        });

        let render_view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(CHART_TEXTURE_FORMAT),
            ..Default::default()
        });

        let sample_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("gup_egui_sample_view"),
            format: Some(EGUI_SAMPLE_FORMAT),
            ..Default::default()
        });

        Self {
            texture,
            render_view,
            sample_view,
            width,
            height,
        }
    }
}

impl GupWidget {
    /// Create a new `GupWidget` wrapping any chart that implements [`DynChart`].
    ///
    /// Uses the **pixel-buffer fallback** path: the chart creates its own GPU
    /// device and pixels are read back to the CPU each frame.
    ///
    /// The widget starts in the dirty state so the first frame triggers a
    /// render.
    pub fn new(chart: impl DynChart) -> Self {
        Self {
            chart: Box::new(chart),
            dirty: true,
            last_size: None,
            texture_handle: None,
            pending_events: Vec::new(),
            shared_state: None,
        }
    }

    /// Create a new `GupWidget` using the **zero-copy shared-device** path.
    ///
    /// The chart must have been built with a [`RenderContext`] created from the
    /// same render state (see [`GupEguiContext::from_render_state`]).
    ///
    /// The chart texture is registered directly with egui's renderer — no CPU
    /// readback, no second GPU device.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let render_state = cc.wgpu_render_state.as_ref().unwrap();
    /// let egui_ctx = GupEguiContext::from_render_state(render_state);
    /// let chart = scatter()
    ///     .build_with_data(data, egui_ctx.render_context().clone())?;
    /// let widget = GupWidget::with_render_state(chart, render_state.clone());
    /// ```
    pub fn with_render_state(
        chart: impl DynChart,
        render_state: eframe::egui_wgpu::RenderState,
    ) -> Self {
        Self {
            chart: Box::new(chart),
            dirty: true,
            last_size: None,
            texture_handle: None,
            pending_events: Vec::new(),
            shared_state: Some(SharedDeviceState {
                render_state,
                offscreen_texture: None,
                egui_texture_id: None,
            }),
        }
    }

    /// Returns `true` when using the zero-copy shared-device path.
    pub fn is_shared_device(&self) -> bool {
        self.shared_state.is_some()
    }

    /// Mark the chart as needing a re-render on the next frame.
    ///
    /// Call this whenever the underlying chart data has changed.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Returns `true` if the chart will be re-rendered on the next frame.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Borrow the inner chart for direct manipulation.
    pub fn chart(&self) -> &dyn DynChart {
        &*self.chart
    }

    /// Mutably borrow the inner chart.
    pub fn chart_mut(&mut self) -> &mut dyn DynChart {
        &mut *self.chart
    }

    /// Replace the inner chart with a new one, marking the widget dirty.
    pub fn set_chart(&mut self, chart: impl DynChart) {
        self.chart = Box::new(chart);
        self.dirty = true;
    }

    /// Drain and return all interaction events translated during the last
    /// frame.
    ///
    /// Returns events that were generated by translating egui pointer events
    /// into Gup [`InteractionEvent`](gup::interaction::InteractionEvent)
    /// types. The vector is cleared after calling this method.
    pub fn take_events(&mut self) -> Vec<gup::interaction::InteractionEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Render the widget inside the given egui [`Ui`](egui::Ui).
    ///
    /// This is the main entry point. It:
    /// 1. Determines the available size in the panel.
    /// 2. Re-renders the chart if dirty or the size changed.
    /// 3. Displays the chart texture via [`egui::Image`].
    /// 4. Translates pointer events from the [`egui::Response`].
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        // Clear pending events from the previous frame.
        self.pending_events.clear();

        // Determine the available size in logical points.
        let available = ui.available_size();
        let ppp = ui.ctx().pixels_per_point();

        // Convert to physical pixels for GPU rendering.
        let phys_width = (available.x * ppp).round().max(1.0) as u32;
        let phys_height = (available.y * ppp).round().max(1.0) as u32;

        // Check if the size changed.
        let size_changed = self
            .last_size
            .map_or(true, |s| s[0] != phys_width || s[1] != phys_height);

        // Re-render if dirty or size changed.
        if self.dirty || size_changed {
            if self.shared_state.is_some() {
                self.rerender_shared(phys_width, phys_height);
            } else {
                self.rerender_pixel_buffer(ui, phys_width, phys_height);
            }
            self.last_size = Some([phys_width, phys_height]);
            self.dirty = false;
        }

        // Display the texture (or a placeholder).
        let size = egui::vec2(available.x, available.y);
        let response = if let Some(tex_id) = self.egui_texture_id() {
            let image = egui::Image::new(egui::load::SizedTexture::new(tex_id, size))
                .fit_to_exact_size(size);
            ui.add(image)
        } else {
            // No texture yet — draw a placeholder rectangle.
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_gray(240));
            response
        };

        // Translate egui pointer events into Gup interaction events.
        let chart_rect = response.rect;
        let events = bridge::translate_response(&response, chart_rect, ppp);
        self.pending_events = events;

        response
    }

    /// Return the egui texture id from whichever path is active.
    fn egui_texture_id(&self) -> Option<egui::TextureId> {
        if let Some(state) = &self.shared_state {
            state.egui_texture_id
        } else {
            self.texture_handle.as_ref().map(TextureHandle::id)
        }
    }

    // ----- Pixel-buffer fallback path ----------------------------------------

    /// Perform the off-screen render and upload the result to egui as a
    /// [`ColorImage`] (CPU readback).
    fn rerender_pixel_buffer(&mut self, ui: &mut egui::Ui, width: u32, height: u32) {
        // Render the chart to raw RGBA pixels (no PNG encode/decode).
        let pixels = match self.chart.render_to_rgba(width, height) {
            Ok(px) => px,
            Err(e) => {
                log::warn!("GupWidget render failed: {e}");
                return;
            }
        };

        // Build an egui ColorImage from the pixel data.
        let color_image =
            ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);

        // Upload (or replace) the egui texture.
        match &mut self.texture_handle {
            Some(handle) => {
                handle.set(color_image, TextureOptions::LINEAR);
            }
            None => {
                let handle =
                    ui.ctx()
                        .load_texture("gup_chart", color_image, TextureOptions::LINEAR);
                self.texture_handle = Some(handle);
            }
        }
    }

    // ----- Shared-device zero-copy path --------------------------------------

    /// Render the chart into a shared offscreen texture and register it with
    /// egui's renderer (no CPU readback).
    fn rerender_shared(&mut self, width: u32, height: u32) {
        let state = self.shared_state.as_mut().expect("shared_state is Some");
        let device = &state.render_state.device;

        // (Re)create the offscreen texture when dimensions change.
        let need_new_texture = state
            .offscreen_texture
            .as_ref()
            .map_or(true, |t| t.width != width || t.height != height);

        if need_new_texture {
            let offscreen = OffscreenTexture::new(device, width, height);

            // Register (or update) the texture with egui's renderer.
            let mut renderer = state.render_state.renderer.write();
            match state.egui_texture_id {
                Some(id) => {
                    renderer.update_egui_texture_from_wgpu_texture(
                        device,
                        &offscreen.sample_view,
                        wgpu::FilterMode::Linear,
                        id,
                    );
                }
                None => {
                    let id = renderer.register_native_texture(
                        device,
                        &offscreen.sample_view,
                        wgpu::FilterMode::Linear,
                    );
                    state.egui_texture_id = Some(id);
                }
            }

            state.offscreen_texture = Some(offscreen);
        }

        // Render the chart into the offscreen texture.
        let offscreen = state.offscreen_texture.as_ref().unwrap();
        if let Err(e) = self.chart.render_to_texture_view(
            &offscreen.render_view,
            CHART_TEXTURE_FORMAT,
            width,
            height,
        ) {
            log::warn!("GupWidget shared render failed: {e}");
            return;
        }

        // When the texture was not recreated but only re-rendered, we still
        // need to update the egui bind group so it picks up the new content.
        // For textures that keep the same wgpu::TextureView, the bind group
        // is unchanged, so no extra update is necessary — the GPU writes are
        // visible on the next frame automatically.
    }
}

/// Implement [`egui::Widget`] for `&mut GupWidget` so it can be passed to
/// [`Ui::add`](egui::Ui::add).
///
/// ```rust,ignore
/// ui.add(&mut my_widget);
/// ```
impl egui::Widget for &mut GupWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui)
    }
}
