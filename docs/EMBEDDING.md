# Embedding Gup In Your Own wgpu App

How to draw a Gup chart into a texture of your own wgpu renderer, or into your
own render pass, resize it and forward input to it. For egui and Bevy, use
[their guides](EGUI_INTEGRATION.md) instead; this page is for a host that owns
its wgpu device and its frame loop (a game, an editor, a tool with its own
compositor).

Gup 0.1 uses wgpu 27 (`Cargo.lock`) and needs Rust 1.93 (`Cargo.toml`
`rust-version`). Every Rust block below is compiled and run as a doctest of
`gup`, on a device made in the block's hidden lines (lines starting with `#`),
without a window or a surface.

The short version, for a host that composites a texture:

1. `Context::from_wgpu(device, queue)` once.
2. A texture and a `TextureTarget` over it, sized in physical pixels.
3. Each frame, `chart.resolve(..)` at the logical size, then
   `renderer.render(..)` into the target. Then sample the texture.

## Your device

Make the device the way your renderer does, and wrap it:

```rust
use gup::prelude::*;
use gup::wgpu;

// Your renderer's own setup, shown for wgpu 27: its `DeviceDescriptor` has an
// `experimental_features` field, so a struct literal that omits it (and has no
// `..Default::default()`) does not compile.
async fn host_device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("no adapter");
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("host device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        })
        .await
        .expect("no device")
}

# fn block_on<F: std::future::Future>(f: F) -> F::Output {
#     use std::sync::Arc;
#     use std::task::{Context as TaskContext, Poll, Wake, Waker};
#     struct Unpark(std::thread::Thread);
#     impl Wake for Unpark {
#         fn wake(self: Arc<Self>) {
#             self.0.unpark();
#         }
#     }
#     let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
#     let mut f = std::pin::pin!(f);
#     loop {
#         if let Poll::Ready(v) = f.as_mut().poll(&mut TaskContext::from_waker(&waker)) {
#             return v;
#         }
#         std::thread::park();
#     }
# }
# fn main() -> gup::Result<()> {
# let (device, queue) = block_on(host_device());
let cx = Context::from_wgpu(device, queue);
# // Default limits are enough for a chart.
# let mut chart = gup::scatter(vec![(1.0, 2.0), (3.0, 1.0), (5.0, 4.0)]).x(|r| r.0).y(|r| r.1);
# let image = chart.render(&cx, 200, 150)?;
# assert!(image.pixels().any(|p| p.0 != [255; 4]));
# Ok(())
# }
```

- The major version of wgpu must match: `gup::wgpu` re-exports the one Gup was
  built with, and a `wgpu::Device` of another major version is a different type.
- `from_wgpu` requests nothing from the adapter. Gup reads its `Caps` (limits
  and features) from the device, so a device made with default limits gets
  default-limit chunk sizes, and charts still draw.
- Clone the `Context` freely; it is a handle.

## A texture and a target

The chart is drawn into a texture you own. There are two recipes, and which one
you want depends on how your own shader samples it and where you write.

**`Rgba8Unorm`** is what `gup-egui` uses: your shader works on gamma-encoded
bytes (egui's does), so you sample the bytes raw.

```rust
# use gup::prelude::*;
use gup::{TargetOptions, TextureTarget, wgpu};

fn unorm_target(cx: &Context, px: (u32, u32), dpr: f32) -> gup::Result<TextureTarget> {
    let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("chart"),
        size: wgpu::Extent3d { width: px.0, height: px.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    TextureTarget::new(cx, texture, TargetOptions::default().with_dpr(dpr))
}
# fn main() -> gup::Result<()> {
# let cx = Context::new_blocking()?;
# unorm_target(&cx, (640, 400), 2.0)?;
# Ok(())
# }
```

**`Rgba8UnormSrgb`** is for a host whose shader samples through hardware sRGB
decode and writes to an sRGB surface (the usual wgpu setup): the decode and the
surface's encode cancel, and the chart's bytes reach the screen unchanged. List
the non-sRGB twin in `view_formats`, because Gup draws through a view of that
format:

```rust
# use gup::prelude::*;
use gup::{TargetOptions, TextureTarget, wgpu};

fn srgb_target(cx: &Context, px: (u32, u32), dpr: f32) -> gup::Result<TextureTarget> {
    let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("chart"),
        size: wgpu::Extent3d { width: px.0, height: px.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
    });
    TextureTarget::new(cx, texture, TargetOptions::default().with_dpr(dpr))
}
# fn main() -> gup::Result<()> {
# let cx = Context::new_blocking()?;
# srgb_target(&cx, (640, 400), 2.0)?;
# Ok(())
# }
```

Why: Gup writes gamma-encoded bytes and blends in that space, the same bytes as
a PNG, so the target draws through the non-sRGB view whatever the texture's
format. A texture without the twin in `view_formats` is refused by
`TextureTarget::new` with an error that names `view_formats`. The test
`srgb_texture_with_unorm_view_format_matches_image_target`
(`crates/gup-core/tests/targets.rs`) draws one chart both ways and asserts the
sRGB texture's bytes equal the image target's, pixel for pixel.

The texture must be 2D, single-sampled and have `RENDER_ATTACHMENT`; add
`TEXTURE_BINDING` to sample it (and `COPY_SRC` to read it back). For
anti-aliased areas and polygons, `TargetOptions::with_samples(4)` makes Gup
resolve its own multisampled texture into yours.

A host whose colours are in linear light (a renderer that does its lighting in
linear space) converts the chart's own colours when it sets them:
`Color::from_linear(r, g, b)` takes linear components and returns the
sRGB-encoded colour Gup draws with.

## The frame loop

Keep one `Renderer` for the life of the chart, so pipelines and glyphs are built
once. Each frame:

```rust
use gup::prelude::*;
use gup::{Renderer, TargetOptions, TextureTarget, wgpu};

fn draw(
    cx: &Context,
    renderer: &mut Renderer,
    chart: &mut dyn Chart,
    logical: (f32, f32),
    target: &mut TextureTarget,
) -> gup::Result<()> {
    let resolved = chart.resolve(cx, logical.0, logical.1)?;
    renderer.render(cx, &resolved.scene, target)
}
# fn main() -> gup::Result<()> {
# let cx = Context::new_blocking()?;
# let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
#     label: Some("chart"),
#     size: wgpu::Extent3d { width: 640, height: 400, depth_or_array_layers: 1 },
#     mip_level_count: 1,
#     sample_count: 1,
#     dimension: wgpu::TextureDimension::D2,
#     format: wgpu::TextureFormat::Rgba8Unorm,
#     usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
#     view_formats: &[],
# });
# let mut target = TextureTarget::new(&cx, texture, TargetOptions::default().with_dpr(2.0))?;
# let mut chart = gup::scatter(vec![(1.0, 2.0), (3.0, 1.0), (5.0, 4.0)]).x(|r| r.0).y(|r| r.1);
let mut renderer = Renderer::new();
draw(&cx, &mut renderer, &mut chart, (320.0, 200.0), &mut target)?;
draw(&cx, &mut renderer, &mut chart, (320.0, 200.0), &mut target)?; // a second frame
# Ok(())
# }
```

- A clean chart resolves to the same scene at no GPU cost (no re-upload), so
  drawing every frame is cheap, and you may draw only when something changed.
- `render` runs Gup's own render pass into the target and submits once. After it
  returns, your pass can sample the texture, as it would any texture another
  pass wrote.

## Logical and physical size

The chart is sized in **logical** pixels (`320 × 200` above). The texture is
`round(logical × dpr)` **physical** pixels (`640 × 400` at a dpr of 2), and
`TargetOptions::with_dpr` takes that same `dpr`, so text and lines come out as
sharp as the display allows. Input points are logical pixels from the chart's
top-left corner.

## Resize

There is no `TextureTarget::resize`. When the size or the dpr changes, make a
new texture and a new `TextureTarget` (as `gup-egui` does), and keep the
`Renderer` and the chart. Then re-register the new texture's view with your own
bind group, wherever you sampled the old one.

```rust
# use gup::prelude::*;
use gup::{Renderer, TargetOptions, TextureTarget, wgpu};

struct ChartTexture {
    target: TextureTarget,
    px: (u32, u32),
    dpr: f32,
}

fn physical(logical: (f32, f32), dpr: f32) -> (u32, u32) {
    (
        (logical.0 * dpr).round().max(1.0) as u32,
        (logical.1 * dpr).round().max(1.0) as u32,
    )
}

fn ensure(
    cx: &Context,
    current: Option<ChartTexture>,
    logical: (f32, f32),
    dpr: f32,
) -> gup::Result<ChartTexture> {
    let px = physical(logical, dpr);
    match current {
        Some(t) if t.px == px && t.dpr == dpr => Ok(t),
        _ => {
            let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("chart"),
                size: wgpu::Extent3d { width: px.0, height: px.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let target = TextureTarget::new(cx, texture, TargetOptions::default().with_dpr(dpr))?;
            // Here: create a view of `target.texture()` and update your bind group.
            Ok(ChartTexture { target, px, dpr })
        }
    }
}
# fn main() -> gup::Result<()> {
# let cx = Context::new_blocking()?;
# let t = ensure(&cx, None, (320.0, 200.0), 2.0)?;
# assert_eq!(t.px, (640, 400));
# let t = ensure(&cx, Some(t), (300.0, 200.0), 2.0)?;
# assert_eq!(t.px, (600, 400));
# let mut chart = gup::scatter(vec![(1.0, 2.0), (3.0, 1.0)]).x(|r| r.0).y(|r| r.1);
# let mut renderer = Renderer::new();
# let mut t = t;
# let resolved = chart.resolve(&cx, 300.0, 200.0)?;
# renderer.render(&cx, &resolved.scene, &mut t.target)?;
# Ok(())
# }
```

## Input

Forward your window's events as `Input` values, in logical pixels from the
chart's top-left corner, and give them to the chart:

```rust
use gup::prelude::*;
use gup::{Renderer, TargetOptions, TextureTarget, wgpu};

# fn main() -> gup::Result<()> {
# let cx = Context::new_blocking()?;
# let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
#     label: Some("chart"),
#     size: wgpu::Extent3d { width: 640, height: 400, depth_or_array_layers: 1 },
#     mip_level_count: 1,
#     sample_count: 1,
#     dimension: wgpu::TextureDimension::D2,
#     format: wgpu::TextureFormat::Rgba8Unorm,
#     usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
#     view_formats: &[],
# });
# let mut target = TextureTarget::new(&cx, texture, TargetOptions::default().with_dpr(2.0))?;
# let mut renderer = Renderer::new();
let mut line = gup::line((0..50).map(|i| (i as f64, (i as f64 / 5.0).sin())))
    .x(|r| r.0)
    .y(|r| r.1);
// Wheel and drag move only x (a plain `Plot` method, so a builder reaches it
// through `plot_mut`, and it is set before the chart is boxed).
line.plot_mut().view_axes(Axes::X);
let mut chart: Box<dyn Chart> = Box::new(line);
# let waker = std::task::Waker::noop().clone();
// An appender wakes the host through this, when rows arrive.
chart.set_waker(waker);

let (w, h) = (320.0, 200.0);
let resolved = chart.resolve(&cx, w, h)?;
renderer.render(&cx, &resolved.scene, &mut target)?;

let at = Point::new(160.0, 100.0);
let events = [
    Input::pointer_moved(at),
    Input::pointer_down(at, Button::Primary),
    Input::pointer_moved(Point::new(180.0, 100.0)),
    Input::pointer_up(Point::new(180.0, 100.0), Button::Primary),
    Input::wheel(at, 1.0),
    Input::key_with(Key::ResetView, Modifiers::NONE),
];
let mut redraw = false;
for input in events {
    // `Response` is `#[must_use]`: it says whether the chart looks different.
    redraw |= chart.handle(input).redraw;
}
// Draw again when the chart changed, or while it waits on the GPU.
if redraw || chart.needs_frame() {
    let resolved = chart.resolve(&cx, w, h)?;
    renderer.render(&cx, &resolved.scene, &mut target)?;
}
# Ok(())
# }
```

`Chart::set_waker` and `Chart::needs_frame` are the two hooks for hosts that do
not redraw every frame: the first is how a chart with an appender wakes you, the
second is true while a pick is in flight.

## Rebuilding a live chart

If your data changes by rebuilding the chart, do not rebuild one the user has
zoomed or panned: `Chart::view_is_home` is true until they have, and
`Chart::x_view` and `Chart::y_view` give the visible ranges after the last
resolve, so you can carry a view over.

```rust
use gup::prelude::*;

# fn main() -> gup::Result<()> {
# let cx = Context::new_blocking()?;
let rows = || (0..20).map(|i| (i as f64, 500.0 + 10.0 * i as f64));
let mut chart: Box<dyn Chart> = Box::new(
    gup::line(rows())
        .x(|r| r.0)
        .y(|r| r.1)
        .y_scale(Linear::new().include_zero()),
);
chart.resolve(&cx, 320.0, 200.0)?;
// Nothing moved the view, so replacing the chart discards nothing.
if chart.view_is_home() {
    chart = Box::new(gup::line(rows()).x(|r| r.0).y(|r| r.1));
} else {
    let (x0, x1) = chart.x_view().expect("resolved");
    let (y0, y1) = chart.y_view().expect("resolved");
    println!("the user is looking at x {x0}..{x1}, y {y0}..{y1}");
}
# Ok(())
# }
```

(`Linear::include_zero` keeps zero on an automatic axis, so a live line that
grows from 500 does not float off the axis.)

## Your own pass (advanced)

If you want Gup's draws inside **your** render pass (say, in a UI layer you
already record), skip `TextureTarget`. `Renderer::prepare` uploads what the
scene needs, and `Prepared::draw` records the draws into your pass. Describe
your colour attachment with a `TargetDesc`:

```rust
use gup::prelude::*;
use gup::{Renderer, TargetDesc, wgpu};

# fn main() -> gup::Result<()> {
# let cx = Context::new_blocking()?;
# let (device, queue) = (cx.device().clone(), cx.queue().clone());
# let cx = Context::from_wgpu(device, queue);
# let mut chart = gup::scatter(vec![(1.0, 2.0), (3.0, 1.0), (5.0, 4.0)]).x(|r| r.0).y(|r| r.1);
# let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
#     label: Some("host attachment"),
#     size: wgpu::Extent3d { width: 320, height: 200, depth_or_array_layers: 1 },
#     mip_level_count: 1,
#     sample_count: 1,
#     dimension: wgpu::TextureDimension::D2,
#     format: wgpu::TextureFormat::Rgba8UnormSrgb,
#     usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
#     view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
# });
// The attachment's colour view is non-sRGB, even if the texture is sRGB.
let view = texture.create_view(&wgpu::TextureViewDescriptor {
    format: Some(wgpu::TextureFormat::Rgba8Unorm),
    ..Default::default()
});
let desc = TargetDesc {
    format: wgpu::TextureFormat::Rgba8Unorm,
    width: 320,
    height: 200,
    dpr: 1.0,
    samples: 1,
};

let mut renderer = Renderer::new();
let resolved = chart.resolve(&cx, 320.0, 200.0)?;
let prepared = renderer.prepare(&cx, &resolved.scene, &desc)?;

let mut encoder = cx.device().create_command_encoder(&Default::default());
prepared.encode(&mut encoder); // optional, before the host's pass
{
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("host pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(prepared.clear_color()),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    // ... your own draws ...
    prepared.draw(&mut pass); // inside the host's pass
}
cx.queue().submit([encoder.finish()]); // you submit; Gup never does here
# Ok(())
# }
```

- `prepare` rejects an `*Srgb` format: the pass's colour view must be non-sRGB.
  For an sRGB surface or texture, list the non-sRGB format in `view_formats` and
  create the view with it, as above. (An sRGB view would blend translucent marks
  in linear light and draw them lighter.)
- Gup never submits in this path; you do, after your pass.
- `Prepared::encode` is not "record your pass". It records only the offscreen
  pre-pass of a translucent `Line` layer, which has to be drawn before your
  pass. If you skip it, `draw` still draws that layer, directly, and the layer
  blends twice where its segments overlap. Without a translucent line, `encode`
  records nothing.
- The scene's size and the `TargetDesc` are separate: resolve at the logical
  size, describe the attachment in physical pixels with the same `dpr`.
