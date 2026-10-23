// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The widget's render path draws the chart's own image: byte-identical to
//! `ChartExt::render` at dpr 1 and to an `ImageTarget` at dpr 2.

use gup_core::prelude::*;
use gup_core::wgpu;
use gup_core::{ChartExt, Context, ImageTarget, Renderer, TargetOptions, TextureTarget};

#[path = "../src/render.rs"]
mod render;

fn chart() -> gup_core::ScatterChart<(f64, f64)> {
    let pts: Vec<(f64, f64)> = (0..40).map(|i| (i as f64, ((i * 7) % 13) as f64)).collect();
    gup_core::scatter(pts)
        .x(|p| p.0)
        .y(|p| p.1)
        .fill(Color::hex(0xd62728))
        .radius(6.0)
        .title("parity")
}

/// Draw with the widget's path into an sRGB texture of `px` pixels and read
/// it back.
fn widget_pixels(cx: &Context, size: (u32, u32), dpr: f32) -> image::RgbaImage {
    let px = ((size.0 as f32 * dpr) as u32, (size.1 as f32 * dpr) as u32);
    let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("parity"),
        size: wgpu::Extent3d {
            width: px.0,
            height: px.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
    });
    let mut target =
        TextureTarget::new(cx, texture.clone(), TargetOptions::default().with_dpr(dpr)).unwrap();
    render::render_into(
        cx,
        &mut Renderer::new(),
        &mut chart(),
        (size.0 as f32, size.1 as f32),
        &mut target,
    )
    .unwrap();
    // Read back, 256-byte aligned rows.
    let row = (px.0 * 4).next_multiple_of(256);
    let buffer = cx.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * px.1) as u64,
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
                rows_per_image: Some(px.1),
            },
        },
        wgpu::Extent3d {
            width: px.0,
            height: px.1,
            depth_or_array_layers: 1,
        },
    );
    cx.queue().submit([encoder.finish()]);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    cx.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let data = buffer.slice(..).get_mapped_range();
    let mut out = Vec::with_capacity((px.0 * px.1 * 4) as usize);
    for y in 0..px.1 {
        let start = (y * row) as usize;
        out.extend_from_slice(&data[start..start + (px.0 * 4) as usize]);
    }
    image::RgbaImage::from_raw(px.0, px.1, out).unwrap()
}

#[test]
fn dpr_1_equals_chart_render() {
    let cx = Context::new_blocking().unwrap();
    let want = chart().render(&cx, 300, 200).unwrap();
    let got = widget_pixels(&cx, (300, 200), 1.0);
    assert!(
        got == want,
        "the widget's texture differs from ChartExt::render"
    );
}

#[test]
fn dpr_2_equals_image_target() {
    let cx = Context::new_blocking().unwrap();
    let mut c = chart();
    let scene = c.resolve(&cx, 300.0, 200.0).unwrap().scene.clone();
    let mut target =
        ImageTarget::with_options(&cx, 600, 400, TargetOptions::default().with_dpr(2.0)).unwrap();
    let want = target.render_blocking(&cx, &scene).unwrap();
    let got = widget_pixels(&cx, (300, 200), 2.0);
    assert!(
        got == want,
        "the widget's texture differs from an ImageTarget at dpr 2"
    );
}
