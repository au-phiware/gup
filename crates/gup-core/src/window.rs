// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`WindowTarget`]: a winit window's surface as a [`RenderTarget`].

use crate::context::Context;
use crate::error::{Error, Result};
use crate::render::TargetDesc;
use crate::target::{Frame, Readback, RenderTarget};
use std::sync::Arc;
use winit::window::Window;

/// A winit window's surface as a [`RenderTarget`].
///
/// The surface is created from an `Arc<Window>`, so it is
/// `wgpu::Surface<'static>` and needs no lifetime parameter. Its format is
/// an 8-bit RGBA or BGRA format, rendered through a **non-sRGB view**
/// (`Bgra8Unorm` on a `Bgra8UnormSrgb` surface), exactly like
/// [`ImageTarget`](crate::ImageTarget)'s `Rgba8Unorm` texture: colours
/// are blended in sRGB space and the bytes on screen equal the bytes in a
/// PNG (RFC-001 §7).
///
/// [`capture_next_frame`](Self::capture_next_frame) copies the next
/// presented frame back to the CPU (the `GUP_SCREENSHOT_PATH` path of
/// [`show`](crate::show)).
pub struct WindowTarget {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    present_modes: Vec<wgpu::PresentMode>,
    can_capture: bool,
    capture_requested: bool,
    captured: Option<Readback>,
}

impl std::fmt::Debug for WindowTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowTarget")
            .field("format", &self.config.format)
            .field("view_format", &self.view_format())
            .field("size", &(self.config.width, self.config.height))
            .field("present_mode", &self.config.present_mode)
            .finish_non_exhaustive()
    }
}

impl WindowTarget {
    /// A target drawing into `window`, on `cx`'s device.
    ///
    /// `cx` must have created its own device ([`Context::new`]): the
    /// surface comes from its instance and must be supported by its
    /// adapter. Presents with `Fifo` (vsync) until
    /// [`set_present_mode`](Self::set_present_mode) says otherwise.
    pub fn new(cx: &Context, window: Arc<Window>) -> Result<Self> {
        let (instance, adapter) = cx.instance().zip(cx.adapter()).ok_or_else(|| {
            Error::config(
                "window target",
                "this Context wraps a host device (Context::from_wgpu) and has no wgpu \
                 instance to create a surface from; create it with Context::new",
            )
        })?;
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|e| Error::config("window target", format!("cannot create a surface: {e}")))?;
        let caps = surface.get_capabilities(adapter);
        // An 8-bit RGBA/BGRA format, preferring one whose non-sRGB view
        // format can be used on it.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f.remove_srgb_suffix(),
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .ok_or_else(|| {
                Error::config(
                    "window target",
                    format!(
                        "{} cannot present 8-bit RGBA or BGRA to this window (formats: {:?})",
                        adapter.get_info().name,
                        caps.formats
                    ),
                )
            })?;
        let view_format = format.remove_srgb_suffix();
        let can_capture = caps.usages.contains(wgpu::TextureUsages::COPY_SRC);
        let usage = if can_capture {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: if view_format == format {
                vec![]
            } else {
                vec![view_format]
            },
        };
        surface.configure(cx.device(), &config);
        Ok(Self {
            window,
            surface,
            config,
            present_modes: caps.present_modes,
            can_capture,
            capture_requested: false,
            captured: None,
        })
    }

    /// The window.
    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    /// The surface's physical size.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// The window's device-pixel ratio (its scale factor).
    pub fn dpr(&self) -> f32 {
        self.window.scale_factor() as f32
    }

    /// The format pipelines render into (never sRGB).
    fn view_format(&self) -> wgpu::TextureFormat {
        self.config.format.remove_srgb_suffix()
    }

    /// Follow a window resize (physical pixels). Zero sizes are ignored.
    pub fn resize(&mut self, cx: &Context, width: u32, height: u32) {
        if width == 0 || height == 0 || (width, height) == self.size() {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(cx.device(), &self.config);
    }

    /// The present modes the surface supports.
    pub fn present_modes(&self) -> &[wgpu::PresentMode] {
        &self.present_modes
    }

    /// Present with `mode` (`Fifo` is vsync; `Immediate` or `Mailbox`
    /// measure frame cost without vsync).
    pub fn set_present_mode(&mut self, cx: &Context, mode: wgpu::PresentMode) -> Result<()> {
        if !self.present_modes.contains(&mode) {
            return Err(Error::config(
                "present mode",
                format!(
                    "{mode:?} is not supported by this surface (supported: {:?})",
                    self.present_modes
                ),
            ));
        }
        self.config.present_mode = mode;
        self.surface.configure(cx.device(), &self.config);
        Ok(())
    }

    /// Copy the next presented frame back to the CPU; read it with
    /// [`take_capture`](Self::take_capture).
    pub fn capture_next_frame(&mut self) -> Result<()> {
        if !self.can_capture {
            return Err(Error::config(
                "window capture",
                "this surface does not support COPY_SRC, so its frames cannot be read back",
            ));
        }
        self.capture_requested = true;
        Ok(())
    }

    /// The captured frame, as straight-alpha RGBA, once a frame requested
    /// with [`capture_next_frame`](Self::capture_next_frame) was presented.
    pub async fn take_capture(&mut self, cx: &Context) -> Result<Option<image::RgbaImage>> {
        match self.captured.take() {
            Some(readback) => readback.read(cx).await.map(Some),
            None => Ok(None),
        }
    }
}

impl RenderTarget for WindowTarget {
    fn desc(&self) -> TargetDesc {
        TargetDesc {
            format: self.view_format(),
            width: self.config.width,
            height: self.config.height,
            samples: 1,
        }
    }

    fn acquire(&mut self, cx: &Context) -> Result<Frame> {
        let texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            // The window changed under us: reconfigure and try once more.
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                let size = self.window.inner_size();
                self.config.width = size.width.max(1);
                self.config.height = size.height.max(1);
                self.surface.configure(cx.device(), &self.config);
                self.surface
                    .get_current_texture()
                    .map_err(|e| Error::config("window target", format!("no frame: {e}")))?
            }
            Err(e) => return Err(Error::config("window target", format!("no frame: {e}"))),
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format()),
            ..Default::default()
        });
        Ok(Frame::new(texture.texture.clone(), view, Some(texture)))
    }

    fn present(&mut self, cx: &Context, frame: Frame, commands: wgpu::CommandBuffer) -> Result<()> {
        let mut buffers = vec![commands];
        if std::mem::take(&mut self.capture_requested) {
            let mut encoder = cx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gup window capture"),
                });
            self.captured = Some(Readback::encode(cx, frame.texture(), &mut encoder)?);
            buffers.push(encoder.finish());
        }
        cx.queue().submit(buffers);
        self.window.pre_present_notify();
        if let Some(surface) = frame.surface {
            surface.present();
        }
        Ok(())
    }
}
