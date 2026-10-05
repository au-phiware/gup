// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! One prepared scene, every target (RFC-001 §7, GUP-401 AC1/AC2): the
//! reference scatter drawn into an `ImageTarget`, a host-owned
//! `TextureTarget` and a host's own render pass (draw-in-pass) gives the
//! same pixels, at one sample and at four. Draw-in-pass never submits.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use gup_core::{
    Context, ImageTarget, RenderTarget, Renderer, Scene, TargetDesc, TargetOptions, TextureTarget,
};

/// What a host does to look at its own texture: copy it to a buffer,
/// submit on its queue and map it. Straight copy, no un-premultiply (the
/// scene is opaque, so premultiplied and straight bytes are equal).
fn read_texture(cx: &Context, texture: &wgpu::Texture) -> image::RgbaImage {
    let (w, h) = (texture.width(), texture.height());
    let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = cx.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("host readback"),
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = cx.device().create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        texture.size(),
    );
    cx.queue().submit([encoder.finish()]);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    cx.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for line in mapped.chunks_exact(row as usize) {
        pixels.extend_from_slice(&line[..(w * 4) as usize]);
    }
    image::RgbaImage::from_raw(w, h, pixels).unwrap()
}

fn host_texture(cx: &Context, samples: u32, label: &str) -> wgpu::Texture {
    cx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | if samples == 1 {
                wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING
            } else {
                wgpu::TextureUsages::empty()
            },
        view_formats: &[],
    })
}

/// A host recording Gup's draws into its own pass, with its own
/// (optionally multisampled) attachment, and submitting itself.
fn draw_in_host_pass(cx: &Context, scene: &Scene, samples: u32) -> image::RgbaImage {
    let resolved = host_texture(cx, 1, "host colour");
    let msaa = (samples > 1).then(|| host_texture(cx, samples, "host msaa"));
    let desc = TargetDesc {
        format: wgpu::TextureFormat::Rgba8Unorm,
        width: WIDTH,
        height: HEIGHT,
        dpr: 1.0,
        samples,
    };
    let before = cx.submissions();
    let mut renderer = Renderer::new();
    let prepared = renderer.prepare(cx, scene, &desc).unwrap();
    let resolved_view = resolved.create_view(&Default::default());
    let msaa_view = msaa.as_ref().map(|t| t.create_view(&Default::default()));
    let mut encoder = cx.device().create_command_encoder(&Default::default());
    {
        let (view, resolve_target) = match &msaa_view {
            Some(m) => (m, Some(&resolved_view)),
            None => (&resolved_view, None),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("host pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(prepared.clear_color()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        prepared.draw(&mut pass);
    }
    assert_eq!(
        cx.submissions(),
        before,
        "Renderer::prepare / Prepared::draw submitted on the host's behalf"
    );
    cx.queue().submit([encoder.finish()]);
    read_texture(cx, &resolved)
}

fn differing(a: &image::RgbaImage, b: &image::RgbaImage) -> usize {
    a.pixels().zip(b.pixels()).filter(|(p, q)| p != q).count()
}

#[test]
fn image_texture_and_draw_in_pass_agree() {
    let cx = Context::new_blocking().unwrap();
    let scene = scatter::plot()
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .scene;
    for samples in [1, 4] {
        let options = TargetOptions {
            samples,
            ..TargetOptions::default()
        };
        let image = ImageTarget::with_options(&cx, WIDTH, HEIGHT, options)
            .unwrap()
            .render_blocking(&cx, &scene)
            .unwrap();
        let drawn = image.pixels().filter(|p| p.0 != [255; 4]).count();
        assert!(drawn > 2_000, "{samples}×: only {drawn} pixels drawn");

        let mut texture_target =
            TextureTarget::new(&cx, host_texture(&cx, 1, "host texture"), options).unwrap();
        assert_eq!(texture_target.desc().samples, samples);
        let before = cx.submissions();
        Renderer::new()
            .render(&cx, &scene, &mut texture_target)
            .unwrap();
        assert_eq!(cx.submissions(), before + 1, "TextureTarget submits once");
        let texture = read_texture(&cx, texture_target.texture());

        let in_pass = draw_in_host_pass(&cx, &scene, samples);

        assert_eq!(
            differing(&image, &texture),
            0,
            "{samples}×: TextureTarget differs from ImageTarget"
        );
        assert_eq!(
            differing(&image, &in_pass),
            0,
            "{samples}×: draw-in-pass differs from ImageTarget"
        );
    }
}

/// The sample count is part of every pipeline's key (RFC-001 §6–7): the
/// first 4× prepare creates a new pipeline for each draw kind, the second
/// creates none. Uniform-only state (size, dpr) never is.
#[test]
fn samples_are_part_of_the_pipeline_key() {
    let cx = Context::new_blocking().unwrap();
    let scene = scatter::plot()
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .scene;
    let desc = |samples, dpr| TargetDesc {
        format: wgpu::TextureFormat::Rgba8Unorm,
        width: WIDTH,
        height: HEIGHT,
        dpr,
        samples,
    };
    let mut renderer = Renderer::new();
    let created = |cx: &Context| cx.pipeline_stats().pipelines_created;
    renderer.prepare(&cx, &scene, &desc(1, 1.0)).unwrap();
    let one = created(&cx);
    renderer.prepare(&cx, &scene, &desc(4, 1.0)).unwrap();
    let four = created(&cx);
    // The mark and rule pipelines (text pipelines live in gup-text's own
    // cache, keyed the same way).
    assert_eq!(four - one, 2, "4× needs its own mark and rule pipelines");
    renderer.prepare(&cx, &scene, &desc(4, 2.0)).unwrap();
    renderer.prepare(&cx, &scene, &desc(1, 1.5)).unwrap();
    assert_eq!(created(&cx), four, "dpr is not part of the key");
}

#[test]
fn texture_target_rejects_unusable_textures() {
    let cx = Context::new_blocking().unwrap();
    let sampled_only = cx.device().create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let err = TextureTarget::new(&cx, sampled_only, TargetOptions::default()).unwrap_err();
    assert!(err.to_string().contains("RENDER_ATTACHMENT"), "{err}");
    let multisampled = host_texture(&cx, 4, "msaa");
    let err = TextureTarget::new(&cx, multisampled, TargetOptions::default()).unwrap_err();
    assert!(err.to_string().contains("Prepared::draw"), "{err}");
    let err = TextureTarget::new(
        &cx,
        host_texture(&cx, 1, "ok"),
        TargetOptions {
            samples: 3,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("use 1 or 4"), "{err}");
}

/// `TargetDesc::dpr` reaches layout and the glyph rasteriser: the same
/// logical scene at dpr 2 is twice the size in every direction, title
/// included, rather than a stretched dpr-1 render.
#[test]
fn dpr_scales_geometry_and_text() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (WIDTH / 2, HEIGHT / 2);
    let resolved = scatter::plot().resolve(&cx, w as f32, h as f32).unwrap();
    let one = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let two = ImageTarget::with_options(&cx, 2 * w, 2 * h, TargetOptions::default().with_dpr(2.0))
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    assert_eq!(two.dimensions(), (2 * w, 2 * h));

    // The title's ink box, found as the non-white rows/columns above the
    // plot rect.
    let title = resolved
        .layout
        .texts
        .iter()
        .find(|t| t.run.role == gup_core::scene::TextRole::Title)
        .unwrap();
    let ink = |img: &image::RgbaImage, scale: u32| {
        let top = (resolved.layout.plot.top() as u32).saturating_sub(4) * scale;
        let (mut x0, mut x1, mut y0, mut y1) = (u32::MAX, 0, u32::MAX, 0);
        for (x, y, p) in img.enumerate_pixels() {
            if y < top && p.0[..3] != [255, 255, 255] {
                (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
            }
        }
        (x1 - x0 + 1, y1 - y0 + 1)
    };
    let (w1, h1) = ink(&one, 1);
    let (w2, h2) = ink(&two, 2);
    assert!(
        (w1 as f32 - title.bounds.width).abs() <= 2.0,
        "{w1} vs {title:?}"
    );
    // Rasterised at 2× (not upscaled): within a few pixels of twice.
    assert!(
        (w2 as i32 - 2 * w1 as i32).abs() <= 4,
        "{w1}×{h1} → {w2}×{h2}"
    );
    assert!(
        (h2 as i32 - 2 * h1 as i32).abs() <= 3,
        "{w1}×{h1} → {w2}×{h2}"
    );
}
