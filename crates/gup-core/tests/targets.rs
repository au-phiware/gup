// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! One prepared scene, every target (RFC-001 §7, GUP-401 AC1/AC2): the
//! reference scatter drawn into an `ImageTarget`, a host-owned
//! `TextureTarget` and a host's own render pass (draw-in-pass) gives the
//! same pixels, at one sample and at four. Draw-in-pass never submits.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use gup_core::{
    ChartExt, Context, ImageTarget, RenderTarget, Renderer, Scene, TargetDesc, TargetOptions,
    TextureTarget,
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
    host_texture_in(cx, samples, label, wgpu::TextureFormat::Rgba8Unorm, &[])
}

/// A host texture of `format`, which may list `view_formats`.
fn host_texture_in(
    cx: &Context,
    samples: u32,
    label: &str,
    format: wgpu::TextureFormat,
    view_formats: &[wgpu::TextureFormat],
) -> wgpu::Texture {
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
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | if samples == 1 {
                wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING
            } else {
                wgpu::TextureUsages::empty()
            },
        view_formats,
    })
}

/// A host recording Gup's draws into its own pass, with its own
/// (optionally multisampled) attachment, and submitting itself.
///
/// `encode` records `Prepared::encode` before the pass, as a host that
/// wants translucent lines drawn once does (GUP-584).
fn draw_in_host_pass(cx: &Context, scene: &Scene, samples: u32, encode: bool) -> image::RgbaImage {
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
    if encode {
        prepared.encode(&mut encoder);
    }
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
        .scene
        .clone();
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

        let in_pass = draw_in_host_pass(&cx, &scene, samples, true);

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

/// The embedding guide's sRGB-surface recipe (`docs/EMBEDDING.md`, GUP-702):
/// an `Rgba8UnormSrgb` texture that lists `Rgba8Unorm` in `view_formats`
/// holds the same raw bytes as `ImageTarget`'s image, and one that does not
/// is refused by name instead of drawing wrong colours.
#[test]
fn srgb_texture_with_unorm_view_format_matches_image_target() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = scatter::plot();
    let image = chart.render(&cx, WIDTH, HEIGHT).unwrap();
    let drawn = image.pixels().filter(|p| p.0 != [255; 4]).count();
    assert!(drawn > 2_000, "only {drawn} pixels drawn");

    let texture = host_texture_in(
        &cx,
        1,
        "host sRGB texture",
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &[wgpu::TextureFormat::Rgba8Unorm],
    );
    let mut target = TextureTarget::new(&cx, texture, TargetOptions::default()).unwrap();
    assert_eq!(target.desc().format, wgpu::TextureFormat::Rgba8Unorm);
    let resolved = chart.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    Renderer::new()
        .render(&cx, &resolved.scene, &mut target)
        .unwrap();
    // The raw bytes: a copy does not encode or decode.
    let raw = read_texture(&cx, target.texture());
    assert_eq!(
        differing(&image, &raw),
        0,
        "the sRGB texture's bytes differ from ImageTarget's"
    );
}

/// An sRGB texture without its unorm twin in `view_formats` is refused with
/// an error that names `view_formats` (and keeps wgpu's own message).
#[test]
fn srgb_texture_without_view_formats_is_refused_by_name() {
    let cx = Context::new_blocking().unwrap();
    let refused = host_texture_in(
        &cx,
        1,
        "host sRGB texture without view_formats",
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &[],
    );
    let err = TextureTarget::new(&cx, refused, TargetOptions::default()).unwrap_err();
    assert!(
        err.to_string().contains("view_formats"),
        "the error does not name view_formats: {err}"
    );
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
        .scene
        .clone();
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
    let resolved = scatter::plot()
        .resolve(&cx, w as f32, h as f32)
        .unwrap()
        .clone();
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

/// A wide translucent line crossing itself at the middle of a bow tie.
fn bow_tie(cx: &Context) -> Scene {
    use gup_core::prelude::*;
    let rows: Vec<(f64, f64)> = vec![(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0)];
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<(f64, f64), Line>::new(rows))
        .x_with(x.encode(|r: &(f64, f64)| r.0))
        .y_with(y.encode(|r: &(f64, f64)| r.1))
        .width(Px(24.0))
        .color(Color {
            a: 0.5,
            ..Line::DEFAULT_COLOR
        });
    plot.resolve(cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .scene
        .clone()
}

/// GUP-584: a host that records `Prepared::encode` before its own pass
/// gets `Renderer::render`'s translucent line byte for byte; one that
/// does not still gets the line, drawn directly (blending twice at the
/// crossing). Neither submits on the host's behalf.
#[test]
fn host_pass_translucent_line_with_and_without_encode() {
    let cx = Context::new_blocking().unwrap();
    let scene = bow_tie(&cx);
    let image = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &scene)
        .unwrap();
    let encoded = draw_in_host_pass(&cx, &scene, 1, true);
    assert_eq!(
        differing(&image, &encoded),
        0,
        "prepare/encode/draw differs from Renderer::render"
    );
    let direct = draw_in_host_pass(&cx, &scene, 1, false);
    let drawn = direct.pixels().filter(|p| p.0 != [255; 4]).count();
    assert!(drawn > 2_000, "direct draw left {drawn} pixels drawn");
    let darker = differing(&image, &direct);
    assert!(
        darker > 0,
        "the direct draw should blend twice at the crossing"
    );
}

/// GUP-651: an `*Srgb` format blends translucent marks in linear light,
/// so `prepare` rejects it and names the fix.
#[test]
fn prepare_rejects_srgb_formats() {
    let cx = Context::new_blocking().unwrap();
    let scene = bow_tie(&cx);
    let desc = TargetDesc {
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        width: WIDTH,
        height: HEIGHT,
        dpr: 1.0,
        samples: 1,
    };
    let err = Renderer::new().prepare(&cx, &scene, &desc).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("sRGB") && message.contains("non-sRGB view"),
        "{message}"
    );
}
