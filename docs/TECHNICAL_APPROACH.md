# Technical Approach

How Gup works today. This document describes what is on `main`; it is not a
plan. The design log, with the reasons and the measurements, is
[RFC-001](./planning/rfcs/RFC-001_Core_Architecture.md); each section below
links the findings it summarises. How the work is organised is in
[IMPLEMENTATION_STRATEGY.md](./IMPLEMENTATION_STRATEGY.md).

The `gup` crate is a facade: `src/lib.rs` is `pub use gup_core::*;`. Everything
below lives in `crates/gup-core`, and every snippet is compiled as a doctest of
`gup` (`cargo test --doc -p gup`).

```rust
use gup::prelude::*;

struct Row { gdp: f64, life_exp: f64, continent: String, population: f64 }
let data = vec![
    Row { gdp: 1e3, life_exp: 55.0, continent: "Africa".into(), population: 3e7 },
    Row { gdp: 6e4, life_exp: 81.0, continent: "Asia".into(), population: 1e9 },
];

let mut chart = gup::scatter(data)
    .x(|d| d.gdp)
    .y(|d| d.life_exp)
    .color(|d| d.continent.as_str())
    .size(|d| d.population);
let image = chart.render(&Context::shared()?, 800, 500)?;
assert_eq!(image.dimensions(), (800, 500));
# Ok::<(), gup::Error>(())
```

## The idea: encodings are shader functions

Every mapping from data to a visual value (a scale, a transfer function, a
palette) is a `ShaderFn`: WGSL that runs per instance in the vertex stage, plus
its uniform `Params`. Each also has a `CpuMirror`, an exact f64 twin. The GPU
draws with the WGSL; axes, ticks, legends, picking and SVG use the mirror, so
they cannot disagree with the pixels. Zooming changes uniforms only.

`EncodeFn::then` chains functions, and `encode` binds a chain to a row accessor
(`f.encode(|row| row.value)`); `encode_key` binds one that takes a string or
integer key. A chain is one function in the shader: the glue calls each in turn.

You write your own with `#[wgsl_function]`. The WGSL is composed against Gup's
library when your crate compiles, so a syntax error, a wrong signature or a
`Params` member with no Rust twin is a compile error at your code.

```rust
use gup::prelude::*;

/// The energy an earthquake releases, in joules: log10 E = 1.5 M + 4.8.
#[derive(Clone)]
struct Energy;

// `path` names the WGSL module; a doctest needs it because its crate is `gup`.
#[gup::wgsl_function(monotonic, path = "arch_energy", wgsl = r#"
    struct Params { slope: f32, offset: f32 }

    fn energy(magnitude: f32, p: Params) -> f32 {
        return pow(10.0, p.slope * magnitude + p.offset);
    }
"#)]
impl ShaderFn for Energy {
    fn params(&self) -> EnergyParams {
        EnergyParams { slope: 1.5, offset: 4.8 }
    }
}

impl CpuMirror for Energy {
    fn eval(&self, magnitude: f64) -> f64 {
        10f64.powf(1.5 * magnitude + 4.8)
    }
}

// Area grows with energy: Energy, then a square-root scale onto 0..30 px.
let radius = Energy.then(Pow::sqrt().domain(0.0, Energy.eval(7.8)).range(Px(0.0), Px(30.0)));
assert_eq!(radius.eval(7.8), 30.0);
```

Composition happens at build time on every target (`crates/gup-wgsl`). naga_oil
flattens the WGSL library once, in `build.rs`; at run time a dependency-free
linker joins a typed, generated glue module to the flattened text. Neither naga
nor naga_oil is in a `gup-core` binary, and Gup never edits authored WGSL with
find-and-replace. Authoring and the calling convention are documented on
`gup::wgsl_function`.

Findings: [S0b][rfc-s0b]; GUP-406 (build-time linking); S5c
(`#[wgsl_function]`).

## Data: the GPU column store

Accessors run once. Their outputs are stored as columns, in one buffer per chunk
(each column at a 256-byte-aligned range), and bound as instance-rate vertex
buffers. A chunk holds `min(2^20, max_buffer_size / row stride)` rows and is
drawn by one instanced draw.

- **Relative origins.** A relative column stores `(v - origin) as f32` against
  its chunk's f64 origin, so timestamps and large coordinates keep f32
  precision. Deep zoom uses a hi/lo pair of `f32`s (double-single) per value.
- **Nulls.** A non-finite number is a null. A numeric column gets validity bits,
  one per row, in a small storage buffer that exists only once the store has a
  null. A key column stores a reserved code. A row with a null position is not
  drawn; a null colour is `NULL_COLOR`. Shaders never test for NaN.
- **Dictionaries.** String, integer and enum keys are dictionary-encoded
  (`encode_key`). The store holds `u32` codes and each channel holds its keys.
  Scales over codes (`Band`, `Point`, `Categorical`) take their domain from
  them.
- **Retention.** `Retain` says what CPU data to keep after upload: `Auto` (keep
  everything up to ten million rows), `Rows`, `Columns` or `GpuOnly`. Picking,
  SVG output and re-encoding need the CPU data; `GpuOnly` layers are picked on
  the GPU. Stats and dictionaries are always kept.
- **Appending.** `Selection::append`, or an `Appender` handle from any thread,
  evaluates only the new rows and uploads only their bytes. The handle wakes the
  host through `Chart::set_waker`.
- **Windows.** `Window::LastRows(n)` keeps at least the newest `n` rows,
  dropping whole chunks from the front (chunks are capped at `max(1024, n / 4)`
  rows).
- **Counting.** Every buffer and texture write goes through `Context` and is
  counted by kind (`Context::upload_stats`). A test fails if any source file
  writes to the GPU another way, so "zooming writes 0 column bytes" is provable.

```rust
use gup::prelude::*;

struct Tick { t: f64, price: f64 }
let tick = |i: u32| Tick { t: f64::from(i), price: 100.0 + f64::from(i % 17) };

// Keep about the last 10,000 ticks, fed from another thread.
let mut chart = gup::line(Vec::<Tick>::new())
    .x(|t| t.t)
    .y(|t| t.price)
    .window(Window::LastRows(10_000));
let feed = chart.appender();
std::thread::spawn(move || feed.extend((0..25_000).map(tick)).unwrap())
    .join()
    .unwrap();
let image = chart.render(&Context::shared()?, 640, 360)?;
assert_eq!(image.dimensions(), (640, 360));

# Ok::<(), gup::Error>(())
```

Findings: [S0a][rfc-s0a]; S4b (nulls); [S12][rfc-s12] (append, `Window`).

## Marks and typed channels

A mark is a struct of typed channels plus a WGSL module. `Circle`, `Rect`,
`Segment` and `Line` are built in; each is `#[derive(Mark)]`, which also exists
for your own. A channel is an associated constant,
`Circle::RADIUS: Channel<Circle, Px>`, and `attr` accepts a constant, a closure
(for position channels, through the plot's scale) or a `ShaderFn` chain. The
channel's visual type (`Px`, `Color`, `Unit`) must match what the value
produces. A typo, a channel of another mark or a value of the wrong type is a
compile error, with a message that says what the channel accepts:

```rust,compile_fail
use gup::prelude::*;

struct Row { v: f64 }
let mut plot = Plot::new();
// A colour channel needs a scale or a colour, not a bare closure.
plot.circles(vec![Row { v: 1.0 }]).fill(|r: &Row| r.v);
```

The derive also checks the WGSL input struct against the channels when your
crate compiles. A mark names its geometry (`Quad`, or polyline segments), its
antialiasing and how it is hit-tested (`Disc`, box, segment, unpickable); the
shader supplies the shape.

A complete external mark (a square, about 60 lines of WGSL and a four-channel
struct) is the compiled test
[`derive_mark.rs`](../crates/gup-core/tests/derive_mark.rs); the built-in marks
in `crates/gup-core/src/marks.rs` are the same derive.

A layer's draw order is its `z` (`-99..=99`; equal `z` draws in the order
added). Within a layer, rows draw in data order, or largest first when the size
channel is a column, so no mark hides under a larger one. To raise some rows,
give them a layer of their own over the same scales.

Findings: [S3][rfc-s3]; GUP-431 (draw order); S9a (hit shapes).

## Scales

One family; each member is a `ShaderFn` with a `CpuMirror`.

| Kind     | Scales                                                     |
| -------- | ---------------------------------------------------------- |
| Position | `Linear`, `Pow` (and `Pow::sqrt`), `Log`, `Symlog`, `Time` |
| Key      | `Band` (bars, groups), `Point` (categorical positions)     |
| Colour   | `Sequential`, `Diverging` (palette tables), `Categorical`  |
| Helper   | `Offset` (a pixel shift read at each resolve), for chains  |

A plot's `x()` and `y()` (and `y_right()`) return a `ScaleRef`, a shared handle
that layers encode through and that you can read, so the plot's domain is the
union over every layer and channel that uses it. Ticks and their labels come
from the mirror (`PositionScale::ticks`), as do legends (`ShaderFn::legend`: a
swatch row per key for `Categorical`, a palette bar with ticks for `Sequential`
and `Diverging`). Grouped bars place each bar with `band.sub_band(i, n)`, a
chain of the band scale and an `Offset`.

```rust
use gup::prelude::*;

let scale = Log::new().domain(1.0, 1000.0).range(Px(0.0), Px(300.0));
assert_eq!(scale.eval(10.0), 100.0); // the f64 mirror of what the GPU computes
let ticks = scale.ticks(4);
assert_eq!(ticks.values.len(), ticks.labels.len());
```

Findings: [S4a][rfc-s4a]; S5a and S5b (the family).

## Plot, layout, scene and targets

`Plot` is the universal container: shared position scales, any number of layers,
axes (left, bottom and an optional right), an optional title and axis titles,
grid lines, colour legends in a column at the right, and a `Theme`.
`Plot::resolve(cx, width, height)` lays everything out and returns `Resolved`:

- a `Layout` (logical pixels): the plot rectangle that marks are clipped to,
  every text run with its ink box, tick positions, the legend column and the
  tooltip box, so tests and hosts know where things went;
- a `Scene`: data marks, guide rules, flat rectangles, legend gradients and text
  runs. It is plain data. A target draws it; a vector exporter walks it.

Resolving is cached: it does no work and no GPU writes when nothing changed.
Layout is sized by measuring text with the same font the renderer uses, so
margins match across targets. Charts hold the `Chart` trait (`resolve`,
`layout`, `handle`, `pick_row`, `needs_frame`, `set_waker`), which is object
safe: `Box<dyn Chart>` renders through `ChartExt` (`render`, `save_png`).

A `Renderer` draws a `Scene` into any `RenderTarget` in one render pass:

| Target          | Output                                                       |
| --------------- | ------------------------------------------------------------ |
| `ImageTarget`   | An offscreen texture read back to an `RgbaImage` (PNG).      |
| `WindowTarget`  | A winit window's surface (feature `window`, default).        |
| `TextureTarget` | A texture the host owns, to show in egui or Bevy.            |
| `CanvasTarget`  | A `<canvas>` (feature `web`, wasm32).                        |
| `SvgTarget`     | An SVG document (`VectorTarget`; data marks: feature `svg`). |

Hosts that own the render pass skip targets and use `Renderer::prepare` then
`Prepared::draw` (draw-in-pass).

```rust
use gup::prelude::*;
use gup::{ImageTarget, SvgTarget, VectorTarget};

let cx = Context::shared()?;
let mut plot = Plot::new();
plot.title("Squares");
plot.grid(Axes::Both);
plot.circles((1..=5).map(|i| (f64::from(i), f64::from(i * i))))
    .x(|p| p.0)
    .y(|p| p.1);

let resolved = plot.resolve(&cx, 480.0, 320.0)?;
assert!(resolved.layout.plot.width < 480.0); // room for axes and the title

let png = ImageTarget::new(&cx, 480, 320)?.render_blocking(&cx, &resolved.scene)?;
let mut svg = SvgTarget::new();
svg.render(&resolved.scene)?;
assert!(svg.svg().contains("<circle"));
assert_eq!(png.dimensions(), (480, 320));
# Ok::<(), gup::Error>(())
```

**SVG** is written from the same scene: guides as `<line>`, `<rect>`,
`<linearGradient>` and `<text>`, and (feature `svg`, on by default) one `<g>`
per layer with a `<circle>`, `<rect>`, `<line>` or `<polyline>` per mark, in
draw order, from the CPU mirrors that picking uses. A layer that cannot be
evaluated on the CPU (a user-defined mark, or a `Retain` policy that dropped the
columns) is skipped with an XML comment, never an error. Fonts are named, and
optionally embedded (`SvgOptions::embed_font`). Without the feature, a scene
with a data layer is an error and the WASM binary is smaller.

**Colour policy.** Colours are sRGB-encoded with straight alpha and blend in
sRGB space. Gup renders into non-sRGB views, so the bytes on a window or canvas
equal the bytes in a PNG. **Antialiasing** is analytic for points, rules, boxes
and text, so a Gup-owned pass has one sample by default (`DEFAULT_SAMPLES`). A
target can ask for MSAA, and a scene with an area or polygon mark draws with
four samples on any target.

**Errors.** GPU failures are `Err(Error::Gpu)` naming the pipeline, layer or
target, never a panic or a blank image; a failed pipeline is not cached.

Findings: [S1][rfc-s1], [S2][rfc-s2], [GUP-410][rfc-gup-410],
[GUP-421][rfc-gup-421], [S10][rfc-s10].

### Context

One cloneable `Context` owns or wraps a wgpu device. `Context::shared()` is the
process-wide one, `Context::new_blocking()` makes a fresh headless one, and
`Context::from_wgpu(device, queue)` wraps a host's own (the wgpu major version
must match; `gup::wgpu` re-exports it). Pipelines are cached per context.

## Interaction

A host passes `Input` (pointer moved, left, down, up and wheel, in logical
pixels from the chart's top-left) to `Chart::handle`, which returns a
`Response { redraw }`. Charts never see winit, DOM or egui events.

- **Zoom** is the wheel; **pan** is a primary-button drag, by default.
  `Plot::drag` picks what a drag does (`Pan`, `Off`, `Brush` or `Zoom`), and
  `view_axes` limits which axes move. Both are uniform writes: `zoom` and `pan`
  change the scales' domains and nothing is re-uploaded. `reset_view` undoes
  them.
- **Brush** selects the rows whose mark position lies inside a dragged
  rectangle. `Plot::brush` returns it in data units with the `RowId`s;
  `on_brush` observes it. Brushed rows draw normally and the others are greyed
  (`Theme.unbrushed`); the brush is applied on the GPU through bits, so it costs
  no per-row CPU work.
- **Picking.** The CPU pick builds a uniform grid over the drawn marks from
  retained columns and hit-tests with each mark's signed distance, taking the
  topmost within a few pixels. The GPU pick (`Plot::request_pick`, `poll_picks`)
  draws pickable layers into a one-texel `Rg32Uint` target centred on the
  pointer with the same draw order and tolerance, for layers whose data the CPU
  cannot read. Both return a `RowId`; `Plot::row::<T>(id)` and `pick::<T>(at)`
  give back the row when it is retained.
- **Events and tooltips.** `Selection::on(Event::Enter | Leave | Click, ..)`
  handlers get the row; `tooltip(|row| String)` draws a box beside the hovered
  row as ordinary scene items.
- **Culling.** Draws, CPU picks and GPU picks share one result of chunk culling:
  chunks whose bounds lie outside the plot rectangle are not drawn
  (`Plot::chunk_culling`), so a sorted four-million-row layer draws a few
  chunks.
- **Waking.** A chart that is waiting for the GPU (a pick in flight,
  `needs_frame`) or fed from another thread (an `Appender`) wakes the host
  through the `Waker` the host gave `Chart::set_waker`. Hosts redraw on demand,
  not in a loop.

```rust
use gup::prelude::*;
use gup::geom::Point;
use gup::{Input, Response};

let cx = Context::shared()?;
let mut plot = Plot::new();
let x = plot.x(Linear::new().domain(0.0, 10.0));
let y = plot.y(Linear::new().domain(0.0, 10.0));
plot.circles([(2.0, 3.0), (7.0, 8.0)])
    .x_with(x.encode(|p: &(f64, f64)| p.0))
    .y_with(y.encode(|p: &(f64, f64)| p.1))
    .tooltip(|p| format!("{:.0}, {:.0}", p.0, p.1));
plot.drag(Drag::Brush);
plot.resolve(&cx, 400.0, 300.0)?;

// The mark's position in chart coordinates, from the scales' f64 mirrors.
let at = Point::new(x.read().eval(2.0) as f32, y.read().eval(3.0) as f32);
assert_eq!(plot.pick::<(f64, f64)>(at), Some(&(2.0, 3.0)));

// Input in, redraw request out.
let response = plot.handle(Input::PointerMoved { at });
assert_eq!(response, Response { redraw: true });
# Ok::<(), gup::Error>(())
```

Findings: [S8b][rfc-s8b]; S9a to S9f (picking, brush, culling); S12 (waker).

## Builders

`gup::scatter`, `gup::bar` and `gup::line` take rows and return a builder that
owns a `Plot`. Setters are typed (`.x(|d| d.gdp)`,
`.color(|d| d.region.as_str())`, `.size(..)`, `.title(..)`, `.tooltip(..)`,
`.window(..)`; bars add `.group(..)` and `.stack(..)`), and a builder is a
`Chart`, so `gup::show(chart)`, `render` and `save_png` all take it. Building is
cheap: columns are evaluated at the first resolve. A builder is not a ceiling:
`plot_mut()` and `layer_mut()` give the underlying `Plot` and typed
`Selection<T, M>`, and `into_plot()` ends the builder, so a chart can drop to
the low-level API without starting over.

```rust
use gup::prelude::*;

struct Sale { quarter: &'static str, region: &'static str, amount: f64 }
let sales = [("Q1", "North", 40.0), ("Q1", "South", 25.0), ("Q2", "North", 46.0)]
    .map(|(quarter, region, amount)| Sale { quarter, region, amount });

let chart = gup::bar(sales)
    .x(|s| s.quarter)
    .y(|s| s.amount)
    .color(|s| s.region)
    .title("Sales")
    .group(|s| s.region);

// Drop down a level: the plot underneath is ordinary.
let mut plot = chart.into_plot();
plot.y_title("units");
assert_eq!(plot.title_text(), Some("Sales"));
```

## Hosts

- **`gup::show(chart)`** opens a winit window (feature `window`) with hover,
  wheel zoom, drag pan and Escape to close. With `GUP_SCREENSHOT_PATH=out.png`
  it renders one frame and exits, which the examples use for headless captures.
- **`web::CanvasApp`** (feature `web`, wasm32) owns a chart, a `Context` and a
  `CanvasTarget`, translates DOM pointer and wheel events to `Input`, and
  redraws with one `requestAnimationFrame` per burst of input. WebGPU only, for
  now. The reference page is `crates/gup-core/wasm-size/scatter`.
- **`gup-egui`** (`GupWidget`) and **`gup-bevy`** (`GupPlugin`, `GupChart`)
  render a `Chart` into a texture on the host's own device (no CPU readback) and
  forward input. See [EGUI_INTEGRATION.md](./EGUI_INTEGRATION.md) and
  [BEVY_INTEGRATION.md](./BEVY_INTEGRATION.md). They depend on `gup-core`, the
  same types as `gup`. The iOS, Android and Tauri shims were deleted (strategic
  review 2026-11, Decision 3); a mobile or Tauri host would be built on
  `gup-core` when a user asks (track N6).

`gup::show` is [`examples/hello_world.rs`](../examples/hello_world.rs) (five
lines, run by `mask smoke-examples`).

## Gates

Claims here are checked, not asserted:

- **Budgets** (`crates/gup-core/PERF_BUDGETS.md`, `mask perf-budget`): GPU pass
  and CPU time for a steady 100,000-point zoom, exact upload counts per frame, a
  culled four-million-row zoom, pipeline creation time. Timings hold on the
  machine the file names; CI runs the deterministic proxies (uploads, draws,
  pipelines), since runners render in software.
- **WASM size** (`mask wasm-size`): the reference scatter is about 334 KB
  gzipped (about 302 KB over bare wgpu; ceiling 400,000 B, +2% tolerance).
- **Visual regression** (`mask visual-regression`): per chart, not blank, marks
  and text present, marks inside the plot rectangle, configured colours, and a
  golden image (`tests/golden`). Known-broken output is tracked in
  `tests/visual_regression/expected_failures.toml`.
- **Browser smoke test** (`mask wasm-browser`): the reference scatter in
  headless Chromium over WebGPU; any console error or WGSL diagnostic fails.
- **Dogfood** (`mask dogfood`, [`dogfood/`](../dogfood/README.md)): a detached
  crate on the public `gup` API only, six realistic tasks with pixel checks.
- **Examples** (`mask smoke-examples`): every headless example runs.

Findings: [GUP-417][rfc-gup-417] (budgets); [S14][rfc-s14] (the flip).

[rfc-s0b]:
  ./planning/rfcs/RFC-001_Core_Architecture.md#s0b-findings-2026-10-05-gup-396
[rfc-s0a]:
  ./planning/rfcs/RFC-001_Core_Architecture.md#s0a-findings-2026-10-05-gup-395
[rfc-s12]: ./planning/rfcs/RFC-001_Core_Architecture.md#s12-findings-gup-480482
[rfc-s3]:
  ./planning/rfcs/RFC-001_Core_Architecture.md#s3-findings-2026-10-06-gup-401
[rfc-s4a]:
  ./planning/rfcs/RFC-001_Core_Architecture.md#s4a-findings-2026-10-10-gup-414
[rfc-s1]:
  ./planning/rfcs/RFC-001_Core_Architecture.md#s1-findings-2026-10-05-gup-399
[rfc-s10]: ./planning/rfcs/RFC-001_Core_Architecture.md#s10-findings-gup-460466
[rfc-s8b]:
  ./planning/rfcs/RFC-001_Core_Architecture.md#s8b-findings-2026-10-17-gup-445
[rfc-gup-417]:
  ./planning/rfcs/RFC-001_Core_Architecture.md#gup-417-findings-2026-10-10
[rfc-s14]: ./planning/rfcs/RFC-001_Core_Architecture.md#s14-findings-gup-485488
