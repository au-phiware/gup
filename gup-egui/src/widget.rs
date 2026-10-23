// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;
use std::task::{Wake, Waker};

use eframe::egui_wgpu::RenderState;
use gup_core::{Chart, Context, Input, Renderer, TargetOptions, TextureTarget};

use crate::input::to_input;
use crate::render::render_into;

/// The `Context` kept in an egui renderer's `callback_resources`.
struct SharedContext(Context);

/// The `gup_core::Context` over egui's device: made once per render state
/// with [`Context::from_wgpu`] and kept in its renderer's
/// `callback_resources`, so every widget (and the app, for building charts
/// that upload eagerly) shares one set of pipelines and caches.
pub fn context(render_state: &RenderState) -> Context {
    let mut renderer = render_state.renderer.write();
    if let Some(shared) = renderer.callback_resources.get::<SharedContext>() {
        return shared.0.clone();
    }
    let cx = Context::from_wgpu(render_state.device.clone(), render_state.queue.clone());
    renderer
        .callback_resources
        .insert(SharedContext(cx.clone()));
    cx
}

/// Asks egui for a frame when a chart's data changes off the UI thread.
struct Repaint(egui::Context);

impl Wake for Repaint {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint();
    }
}

/// The texture Gup draws into and egui samples.
struct ChartTexture {
    target: TextureTarget,
    id: egui::TextureId,
    /// Physical pixels.
    size: (u32, u32),
    ppp: f32,
}

/// A Gup chart in egui: renders `chart` into a texture on egui's device
/// and forwards pointer input to it. Keep one per chart.
pub struct GupWidget {
    render_state: RenderState,
    cx: Context,
    renderer: Renderer,
    texture: Option<ChartTexture>,
    /// A button went down inside the widget and has not come up.
    pressed: bool,
    /// The pointer was last reported inside the widget.
    inside: bool,
}

impl GupWidget {
    /// A widget drawing on `render_state`'s device (from
    /// `eframe::CreationContext::wgpu_render_state`).
    pub fn new(render_state: &RenderState) -> Self {
        Self {
            render_state: render_state.clone(),
            cx: context(render_state),
            renderer: Renderer::new(),
            texture: None,
            pressed: false,
            inside: false,
        }
    }

    /// Draw `chart` in a `size` (points) rect and forward input to it.
    ///
    /// Renders on every call: egui repaints only on input or request, and
    /// a chart that has not changed resolves to the same scene and uploads
    /// nothing. Asks egui for another frame while the chart says it needs
    /// one, and when data is appended from another thread.
    ///
    /// A render error is logged to stderr and leaves the previous image.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        size: egui::Vec2,
        chart: &mut dyn Chart,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        chart.set_waker(Waker::from(Arc::new(Repaint(ui.ctx().clone()))));

        let mut redraw = self.forward_input(ui, rect, &response, chart);

        let ppp = ui.ctx().pixels_per_point();
        if let Err(err) = self.render(chart, size, ppp) {
            eprintln!("gup-egui: {err}");
        }
        redraw |= chart.needs_frame();
        if redraw {
            ui.ctx().request_repaint();
        }

        if let Some(tex) = &self.texture {
            ui.painter().image(
                tex.id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        response
    }

    /// Feed this frame's pointer events to `chart`; whether it wants a redraw.
    fn forward_input(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        response: &egui::Response,
        chart: &mut dyn Chart,
    ) -> bool {
        let events = ui.input(|i| i.events.clone());
        let mut redraw = false;
        for event in &events {
            let Some(input) = to_input(event, rect) else {
                continue;
            };
            let input = match input {
                Input::PointerMoved { at } => {
                    let inside = rect.contains(rect.min + egui::vec2(at.x, at.y));
                    if inside || self.pressed {
                        self.inside = inside;
                        input
                    } else if std::mem::take(&mut self.inside) {
                        Input::PointerLeft
                    } else {
                        continue;
                    }
                }
                Input::PointerDown { .. } => {
                    self.pressed = true;
                    self.inside = true;
                    input
                }
                Input::PointerUp { .. } => {
                    if !std::mem::take(&mut self.pressed) {
                        continue;
                    }
                    input
                }
                Input::PointerLeft => {
                    self.inside = false;
                    if self.pressed {
                        continue;
                    }
                    input
                }
                Input::Wheel { lines, .. } => match response.hover_pos() {
                    Some(p) if response.hovered() => Input::Wheel {
                        at: gup_core::geom::Point::new(p.x - rect.min.x, p.y - rect.min.y),
                        lines,
                    },
                    _ => continue,
                },
                other => other,
            };
            redraw |= chart.handle(input).redraw;
        }
        redraw
    }

    /// Resolve and draw `chart` into the texture, recreating it on resize.
    fn render(
        &mut self,
        chart: &mut dyn Chart,
        size: egui::Vec2,
        ppp: f32,
    ) -> gup_core::Result<()> {
        let px = (
            (size.x * ppp).round().max(1.0) as u32,
            (size.y * ppp).round().max(1.0) as u32,
        );
        let stale = self
            .texture
            .as_ref()
            .is_none_or(|t| t.size != px || t.ppp != ppp);
        if stale {
            self.resize(px, ppp)?;
        }
        let tex = self.texture.as_mut().expect("resized above");
        render_into(
            &self.cx,
            &mut self.renderer,
            chart,
            (size.x, size.y),
            &mut tex.target,
        )
    }

    fn resize(&mut self, px: (u32, u32), ppp: f32) -> gup_core::Result<()> {
        use gup_core::wgpu;
        let texture = self.cx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("gup-egui chart"),
            size: wgpu::Extent3d {
                width: px.0,
                height: px.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        // Gup writes sRGB bytes and egui's shader works on gamma bytes:
        // sample them raw (an sRGB view would decode them a second time).
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let target = TextureTarget::new(&self.cx, texture, TargetOptions::default().with_dpr(ppp))?;
        let mut egui_renderer = self.render_state.renderer.write();
        let device = &self.render_state.device;
        let id = match self.texture.take() {
            Some(old) => {
                egui_renderer.update_egui_texture_from_wgpu_texture(
                    device,
                    &view,
                    wgpu::FilterMode::Linear,
                    old.id,
                );
                old.id
            }
            None => egui_renderer.register_native_texture(device, &view, wgpu::FilterMode::Linear),
        };
        self.texture = Some(ChartTexture {
            target,
            id,
            size: px,
            ppp,
        });
        Ok(())
    }
}

impl Drop for GupWidget {
    fn drop(&mut self) {
        if let Some(tex) = self.texture.take() {
            self.render_state.renderer.write().free_texture(&tex.id);
        }
    }
}
