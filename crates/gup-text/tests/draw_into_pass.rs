// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! External-crate test of `gup-text`'s public API: construct a
//! `TextSystem` from a plain wgpu device, measure a run, record its glyphs
//! into a render pass the test opened itself, and read the pixels back.

use gup_text::{
    Anchor, DrawTarget, GlyphBuffer, Glyphs, HAlign, Run, TextSystem, Uploader, VAlign,
};
use std::cell::Cell;

/// 256 px wide so a row of RGBA8 is 1024 bytes, a multiple of
/// `COPY_BYTES_PER_ROW_ALIGNMENT`.
const WIDTH: u32 = 256;
const HEIGHT: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn device() -> (wgpu::Device, wgpu::Queue) {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("a GPU adapter (set WGPU_BACKEND to choose one)");
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("gup-text test"),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .expect("a device")
    })
}

/// Counts what `gup-text` uploads, then forwards to the queue: the hook a
/// host uses to account for GPU writes.
struct Counting<'a> {
    queue: &'a wgpu::Queue,
    buffer_bytes: Cell<usize>,
    texture_bytes: Cell<usize>,
}

impl Uploader for Counting<'_> {
    fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
        self.buffer_bytes.set(self.buffer_bytes.get() + data.len());
        self.queue.write_buffer(buffer, offset, data);
    }

    fn write_texture(
        &self,
        texture: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        self.texture_bytes
            .set(self.texture_bytes.get() + data.len());
        self.queue.write_texture(texture, data, layout, size);
    }
}

/// Run `draw` in a pass over a white background and read back RGBA.
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
) -> Vec<u8> {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(WIDTH * HEIGHT * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("the caller's pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        draw(&mut pass);
    }
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH * 4),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU finished");
    let pixels = readback.slice(..).get_mapped_range().to_vec();
    readback.unmap();
    pixels
}

#[test]
fn measures_and_draws_a_run_into_a_callers_pass() {
    let (device, queue) = device();
    let mut text = TextSystem::new(&device);
    assert_eq!(text.font().name(), "Inter");

    let run = Run {
        text: "Hello, 42",
        size: 24.0,
        at: [WIDTH as f32 / 2.0, 16.0],
        anchor: Anchor::new(HAlign::Middle, VAlign::Top),
    };
    let metrics = text.measure(run.text, run.size);
    assert!(metrics.width > 60.0 && metrics.width < 200.0, "{metrics:?}");
    let ink = text.ink_bounds(&run);
    assert!(
        (ink.y - 16.0).abs() <= 1.0,
        "cap height on the anchor: {ink:?}"
    );
    assert!(
        (ink.x + ink.width / 2.0 - WIDTH as f32 / 2.0).abs() <= 2.0,
        "{ink:?}"
    );

    let mut glyphs = Glyphs::new(1.0);
    text.layout(&run, [0.0, 0.0, 0.0, 1.0], &mut glyphs)
        .expect("layout");
    // Eight inked glyphs: the space has none.
    assert_eq!(glyphs.len(), 8);

    let target = DrawTarget {
        format: FORMAT,
        samples: 1,
        width: WIDTH,
        height: HEIGHT,
    };
    let uploads = Counting {
        queue: &queue,
        buffer_bytes: Cell::new(0),
        texture_bytes: Cell::new(0),
    };
    let mut buffer = GlyphBuffer::new();
    let batch = text
        .prepare(&uploads, &mut buffer, &glyphs, &target)
        .expect("a batch");
    assert_eq!(batch.len(), 8);
    assert_eq!(uploads.buffer_bytes.get(), 8 * 48);
    // Only the new glyphs' rectangle, not the 1 MB atlas.
    let first = uploads.texture_bytes.get();
    assert!(first > 0 && first < 16 << 10, "{first} atlas bytes");

    let pixels = render(&device, &queue, |pass| batch.draw(pass));
    let px = |x: u32, y: u32| &pixels[((y * WIDTH + x) * 4) as usize..][..4];
    // Dark ink inside the ink box, and only there.
    let (mut inside, mut outside) = (0, 0);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if px(x, y)[0] < 128 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                if fx >= ink.x && fx <= ink.x + ink.width && fy >= ink.y && fy <= ink.y + ink.height
                {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
        }
    }
    assert!(inside > 100, "{inside} dark pixels inside {ink:?}");
    assert_eq!(outside, 0, "dark pixels outside {ink:?}");

    // The same glyphs again upload no texels.
    uploads.texture_bytes.set(0);
    glyphs.clear(1.0);
    text.layout(&run, [0.0, 0.0, 0.0, 1.0], &mut glyphs)
        .expect("layout");
    text.prepare(&uploads, &mut buffer, &glyphs, &target)
        .expect("a batch");
    assert_eq!(uploads.texture_bytes.get(), 0);
}

#[test]
fn a_device_pixel_ratio_of_two_draws_twice_the_size() {
    let (device, queue) = device();
    let mut text = TextSystem::new(&device);
    let run = Run {
        text: "88",
        size: 12.0,
        at: [10.0, 10.0],
        anchor: Anchor::new(HAlign::Start, VAlign::Top),
    };
    let ink = |g: &Glyphs| {
        g.rects()
            .fold([f32::MAX, f32::MAX, 0.0f32, 0.0f32], |a, [x, y, w, h]| {
                [a[0].min(x), a[1].min(y), a[2].max(x + w), a[3].max(y + h)]
            })
    };
    let mut one = Glyphs::new(1.0);
    let mut two = Glyphs::new(2.0);
    text.layout(&run, [0.0; 4], &mut one).unwrap();
    text.layout(&run, [0.0; 4], &mut two).unwrap();
    let (a, b) = (ink(&one), ink(&two));
    let (wa, wb) = (a[2] - a[0], b[2] - b[0]);
    assert!((wb - 2.0 * wa).abs() <= 2.0, "{a:?} {b:?}");
    // Rasterised at 24 px, not scaled up from 12 px: two sizes in the atlas.
    assert_eq!(text.atlas_glyphs(), 2);
    let target = DrawTarget {
        format: FORMAT,
        samples: 1,
        width: WIDTH,
        height: HEIGHT,
    };
    assert!(
        text.prepare(&queue, &mut GlyphBuffer::new(), &two, &target)
            .is_some()
    );
    assert!(
        text.prepare(&queue, &mut GlyphBuffer::new(), &Glyphs::new(2.0), &target)
            .is_none()
    );
}
