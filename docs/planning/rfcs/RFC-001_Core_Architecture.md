# RFC-001: Core Architecture

**Status**: Accepted 2026-10-04 with the amendments in "Orchestrator review"
(owner decisions: accept; sRGB-space blending; wave 1 trimmed to surviving work)
**Date**: 2026-10-04 **Tracks**: T2 (One Context / Scene / RenderTarget) and T3
(Core data model), from
[STRATEGIC_REVIEW_2026-10](../STRATEGIC_REVIEW_2026-10.md) **Supersedes on
acceptance**: the architecture sections of `TECHNICAL_APPROACH.md` and
`IMPLEMENTATION_STRATEGY.md` (review decision 6)

## Summary

Gup's main idea is that data encodings are composable GPU functions. That idea
is right, but today it runs only in tests. This RFC moves it onto the production
path with seven decisions:

1. **One `gup::Context`**, which can be cloned cheaply. It is created by Gup, or
   wraps a device that the host owns.
2. **A GPU column store.** Accessors run once and their outputs are uploaded as
   chunked structure-of-arrays. Each chunk stores values relative to its own
   origin, so f32 stays precise.
3. **Typed channels.** `Circle::RADIUS: Channel<Circle, Px>` replaces string
   attribute names. `attr` accepts a constant, a closure or a scale/`ShaderFn`.
   If the types don't match, it is a compile error.
4. **One scale family.** Each scale has a WGSL implementation and an exact f64
   CPU mirror, used for axes, ticks, legends, picking and SVG. Zooming changes
   uniforms only.
5. **Shader composition** uses naga_oil modules plus a small typed generator for
   the entry-point "glue" module. Gup never edits authored WGSL with
   find-and-replace again.
6. **A resolved `Scene`** contains marks, guides and text runs. It is drawn by a
   `RenderTarget` (window, texture, PNG, canvas) and walked by vector exporters
   (SVG, PDF).
7. **One object-safe `Chart` trait.** `Plot` is the universal container.
   Builders produce a `Plot` and give access to the typed `Selection<T, M>`
   underneath.

The first step is a scatter-only spike that proves all seven together.

---

## 1. Goals and non-goals

### Goals

- **North star, verbatim** (review §North star): about 5–10 lines produce a
  publication-quality chart that looks the same in a window, a PNG, an SVG and
  WASM. Encodings run on the GPU. Users can drop to `Selection` without starting
  over.
- **Performance**:
  - 100K points at 60 fps at 1080p on an integrated GPU (Iris Xe class), with x
    linear, y log, colour sequential and size sqrt.
  - Zooming or panning writes **0 column bytes**: uniforms only, well under 64
    KiB per frame.
- **No structural blocker to 1B points.** Chunks are the unit of upload,
  culling, LOD, paging and picking. Readback is async only. Data can be kept on
  the GPU only.
- **Errors at compile time.** Unknown channels, wrong value types and a channel
  used on the wrong mark do not compile.
- **One render path.** PNG, window, egui/bevy texture and WASM canvas all draw
  the same `Scene`. SVG and PDF walk the same `Scene`.

### Non-goals (this RFC)

- Porting all builders. That is T5; this RFC fixes the contract they port onto.
- Building 1B-point LOD or paging. That is T7; this RFC only guarantees the
  structure allows it.
- Graph and treemap layouts, 3D, mobile integrations (GUP-312–315/371–373 are
  re-evaluated against this RFC).
- Device-loss recovery, multi-surface management and pooling. These are deleted
  and come back later only when needed.
- A Rust-to-WGSL transpiler. T1 deletes it. CPU mirrors are hand-written and
  tested against the GPU.

---

## 2. Context

### Today

There are two device-owning contexts that can't work together:

- `RenderContext` (`src/render.rs:13`) is used by charts and `Selection`.
- `GupContext` (`src/context.rs:1155`) is used by `GupApp`
  (`src/app.rs:49,64-67`) and `RenderFrame`.

Chart text can only be drawn through a `GupContext` `RenderFrame`
(`src/chart_builder.rs:2211`). PNG export goes through `RenderContext`
(`src/chart_builder.rs:2715`). The result is that PNGs have no text and builder
charts can't be shown in a window.

`GupContext` wraps `Arc<Device>`/`Arc<Queue>` (`context.rs:1157-1158`). That
wrapping is unnecessary because wgpu 27 handles are already `Clone`.

### Design

```rust
pub use wgpu; // exact re-export; hosts must match the wgpu major version

#[derive(Clone)]                       // Arc<Inner>: cheap to clone, Send + Sync (MaybeSend on wasm)
pub struct Context { inner: Arc<Inner> }

struct Inner {
    id: ContextId,
    instance: Option<wgpu::Instance>, adapter: Option<wgpu::Adapter>,
    device: wgpu::Device, queue: wgpu::Queue,
    caps: Caps,                                  // limits, features, chunk sizing, MSAA support
    pipelines: Mutex<PipelineCache>,             // keyed by (mark, encoding signature, TargetDesc, variant)
    text: Mutex<gup_text::TextSystem>,           // one glyph atlas + shaper per device
    shaders: Mutex<Composer>,                    // naga_oil composer with gup library modules preloaded
}

impl Context {
    pub async fn new() -> Result<Self>;                         // all targets
    pub async fn with_options(o: ContextOptions) -> Result<Self>;
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_blocking() -> Result<Self>;                      // pollster
    pub fn from_wgpu(device: wgpu::Device, queue: wgpu::Queue) -> Self;   // egui, bevy
    pub fn from_wgpu_full(i: wgpu::Instance, a: wgpu::Adapter, d: wgpu::Device, q: wgpu::Queue) -> Self;
    #[cfg(not(target_arch = "wasm32"))]
    pub fn shared() -> Result<Self>;    // lazily-initialised process default (OnceLock)
    pub fn device(&self) -> &wgpu::Device;  pub fn queue(&self) -> &wgpu::Queue;
}
```

- **Ownership.** Charts hold no context until they are first prepared. GPU state
  in a layer is tagged with a `ContextId`.
  - If a chart is prepared on a different context, it re-uploads from its
    CPU-retained columns (§3).
  - If no columns were retained, it fails with `Error::ContextMismatch`.
- **North-star calls.** `save_png` and `gup::show` run without an explicit
  context: `save_png` uses `Context::shared()`, and `show` uses the window's
  context.
- **`from_wgpu`.** This constructor requests nothing.
  - `Caps` is read from `device.limits()`/`features()`.
  - Optional features such as `TIMESTAMP_QUERY` are used only if the host
    enabled them.
  - Gup never submits on the host's behalf in draw-in-pass mode (§7).
- **Adapter limits.** `new()` asks for the adapter's own `max_buffer_size` and
  `max_storage_buffer_binding_size`, rather than the defaults, so chunks can be
  larger on capable hardware.

---

## 3. Data model: the GPU column store

### Today

- `prepare_render_bound` builds a `Vec<(&str, AttrValue)>` **for every row**,
  then string-matches it into an array-of-structs `CircleInstance`
  (`selection.rs:1387-1397`, `circle.rs:472-505`).
- Unknown names are silently dropped (`circle.rs:501`).
- Any `attr` call throws the GPU state away (`selection.rs:686`).
- Streaming (`streaming/streaming_buffer.rs:96`) and LOD (`lod/mod.rs:62,112`)
  are separate array-of-structs stores with no chunking.

### Design

```rust
pub struct ColumnStore {
    rows: u64,
    chunks: Vec<Chunk>,              // fixed capacity, chunk_rows each (last chunk partially filled)
    columns: Vec<ColumnMeta>,        // one per encoded channel input
    retain: Retain,
}
struct Chunk {
    buffer: Option<(ContextId, wgpu::Buffer)>, // ONE buffer per chunk, columns at fixed 256-aligned sub-ranges
    len: u32,                                   // rows filled
    origin: SmallVec<[f64; 4]>,                 // per relative column
    stats: SmallVec<[ColumnStats; 4]>,          // min/max/nulls, in f64; drives domains + chunk culling
    gpu_dirty: Range<u32>,                      // rows not yet uploaded (tail)
}
pub enum ColumnFormat { F32, F32Relative, U32, Unorm8x4 /* colours */, F16 /* opt-in */ }
pub enum Retain { Auto, Rows, Columns, GpuOnly }
```

**Columns are instance-rate vertex buffers.** Each chunk's buffer has
`VERTEX | STORAGE | COPY_DST | COPY_SRC` usage.

- **Drawing.** Columns are bound with
  `set_vertex_buffer(slot, buf.slice(col_range))`. Hardware fetch does the
  format conversion (`Unorm8x4`, `Float16`). This avoids the storage-binding
  limits (`max_storage_buffers_per_shader_stage` = 8, 128 MiB binding size) and
  keeps a WebGL2 fallback possible.
- **Quad corners** come from `vertex_index`, so all 8 `max_vertex_buffers` slots
  are free for data.
- **Line segments** bind the same column twice, offset by one element, which
  gives a previous/next pair per instance.
- **Compute passes** (culling, LOD, GPU picking) bind the same buffers as
  storage.

**Chunking.** The chunk row count is chosen as follows:

```
chunk_rows = min(2^20, max_buffer_size / Σ column_stride)
```

With default limits, 1M rows × 4 columns × 4 B is 16 MiB per chunk. Each chunk
is one instanced draw.

- The draw sets a per-chunk dynamic-offset uniform holding `row_base` and the
  relative-column bases.
- Chunk stats in f64 give CPU frustum culling for free.
- Chunks are also the future unit for LOD tiers and paging under `MemoryBudget`
  (`src/lod/streaming.rs`).
- This replaces the 16.7M-instance ceiling (`compute_instance_filter.rs:139`)
  and its blocking readback (`compute_instance_filter.rs:634-646`).

**Precision.** `F32Relative` stores `(v - chunk.origin) as f32`, with the origin
taken from the chunk's first value.

- On the CPU, the scale computes `base = (origin - domain_lo) as f32` in f64 for
  each chunk and writes it to the chunk uniform.
- The shader computes `r0 + (v + base) * k`.
- **Example.** Unix seconds (~1.7e9) have a 128 s ULP as absolute f32. Relative
  to a chunk spanning one day, the ULP is about 8 ms.
- Linear and Time scales request `F32Relative`.
- Log scales request absolute `F32`, because log is scale-invariant and keeps
  relative precision.
- Band and categorical scales use `U32` codes. Strings and other `Hash + Eq`
  keys are dictionary-encoded on the CPU, and the dictionary order becomes the
  domain.

**Append.** `append(rows)` runs the stored accessors on the new rows only.

- It writes the tail of the last chunk with `queue.write_buffer` at the column
  sub-range offset. `DirtyRegionTracker` (`streaming/dirty_region.rs`) is
  reused.
- When the chunk is full, it opens a new chunk with a fresh origin.
- It updates the f64 stats. If an auto domain grows, that is a uniform write
  plus a guide re-resolve; column data is never re-uploaded.
- Windowing (`Window::LastRows(n)`) evicts whole chunks.

**CPU retention.** Retained data has four uses: (1) picking returns `&T`, (2)
SVG/PDF, (3) accessibility descriptions, (4) re-binding to another context.

- `Retain::Auto` keeps the user's rows (`Arc<[T]>`) and the CPU columns while
  `rows ≤ 10M`, and is GPU-only above that.
- `GpuOnly` keeps only stats and dictionaries. In that mode, `pick` returns a
  `RowId`, and SVG either rasterises the marks layer into an embedded image or
  fails with a clear error.
- **Data ownership.** Builders accept `impl Into<Data<T>>`:
  - `Vec<T>` is moved in.
  - `Arc<[T]>` is shared.
  - `&[T]`/`&Vec<T>` with `T: Clone` is copied once (documented).
  - This keeps charts `'static`, which `show()` and append need, while still
    supporting the north-star `&data`.
- **Nulls.** Shaders never test for NaN; WGSL implementations may treat NaN as
  undefined.
  - Position channels with nulls get a 1-bit-per-row validity column, and the
    vertex stage emits a degenerate quad for null rows.
  - Colour channels map null to `theme.null_color` through a reserved code.

---

## 4. Typed channels

```rust
pub struct Channel<M, V> { slot: u8, name: &'static str, _p: PhantomData<fn() -> (M, V)> }

// Visual (post-encoding) types: what a mark consumes
pub struct Px(pub f32);  pub struct Unit(pub f32);  pub struct Color { /* sRGB-encoded, straight alpha */ }
pub struct Angle(pub f32);

impl<T, M: Mark> Selection<T, M> {
    pub fn attr<V: Visual, E, K>(&mut self, ch: Channel<M, V>, enc: E) -> &mut Self
    where E: IntoEncoding<T, V, K>;
}
```

`IntoEncoding<T, V, K>` uses the "marker type" pattern (as in Bevy's
`IntoSystem<Marker>`) so that blanket impls don't overlap:

| Argument                                       | Impl (marker `K`)                                                                                  | GPU result                                                                                                               |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| constant `Px(3.0)`, `Color::BLACK`             | concrete impl for each `Visual` type                                                               | uniform field, no column                                                                                                 |
| closure `Fn(&T) -> V`                          | `PreEncoded`                                                                                       | column of `V` with identity encoding. This is the escape hatch: rescaling needs re-evaluation, so builders never use it. |
| `scale.encode(acc)` / `encode(acc, shader_fn)` | `Encoded<A, S>` where `A: Fn(&T) -> D`, `D: ColumnValue`, `S: ShaderFn<In = D::Gpu, Out = V::Gpu>` | column of `D` plus a GPU function                                                                                        |

- **Type checking** is done with associated-type equality.
  - `attr(Circle::RADIUS, viridis.encode(..))` fails with
    `type mismatch: <Sequential as ShaderFn>::Out == Color, expected Px`.
  - `#[diagnostic::on_unimplemented]` on `IntoEncoding` rewrites this as
    "_`Sequential` produces `Color`, but channel `Circle::RADIUS` needs `Px`_".
- **Unknown channels** don't exist as associated consts: `Circle::RADUIS` is "no
  associated item".
- **Wrong mark**: `Channel<Rect, Px>` does not unify with `Channel<Circle, _>`.
- **Tests.** All three error cases get `trybuild` compile-fail tests, which run
  in T0's harness.

**What `#[derive(Mark)]` generates.** It is rewritten. No `*Instance` struct is
generated, because there are no instances any more.

```rust
#[derive(Mark)]
#[mark(module = "gup::marks::circle", geometry = Quad)]
pub struct Circle {
    #[channel(role = X)]                           x: Px,
    #[channel(role = Y)]                           y: Px,
    #[channel(default = Px(3.5))]                  radius: Px,
    #[channel(default = Theme::FILL)]              fill: Color,
    #[channel(default = Color::TRANSPARENT)]       stroke: Color,
    #[channel(default = Px(0.0))]                  stroke_width: Px,
}
// generates:
//   impl Circle { pub const X: Channel<Circle, Px>; ... pub const STROKE_WIDTH: Channel<Circle, Px>; }
//   impl Mark for Circle { const MODULE; const CHANNELS: &[ChannelDesc] /* name, slot, role, wgsl type, default */; }
//   pub trait CircleAttrs<T> { fn radius<E, K>(&mut self, e: E) -> &mut Self where E: IntoEncoding<T, Px, K>; ... }
//   a compile-time check (naga in the proc macro) that the module's `CircleIn` struct matches CHANNELS
```

Roles (`X`, `Y`, `Color`, `Size`) tell `Plot` which shared scale slot a channel
joins and which range to assign (for example, the plot rect for `X`).

---

## 5. Scales and encodings: shader functions with CPU mirrors

### Today

There are four scale systems:

- `shader_function/math.rs:97` `LinearScale`
- `chart_builder.rs:496` `AxisScale`, with a CPU re-implementation at `:551`
- `tick_generator.rs:40,120,675` `Scale`/`LinearScale`/`TimeScale`
- `scale.rs:275,316` `Integrated*`

Uniform WGSL structs are hand-written strings that drift from the Rust struct;
for example, Rust field `clamp` vs WGSL field `clamp_flag` (`math.rs:52,63`).

`ColorScale` reads globals `gradient_colors`/`gradient_stops`
(`shader_function/color.rs:793-815`) that `Selection` never binds. It binds one
uniform per attribute (`selection.rs:1681-1703`). The scatter builder's
`color_scale` is documented as GPU-wired, but it only feeds the colorbar
(`scatter.rs:189-206`, `chart_builder.rs:1412`).

### Design

```rust
pub trait ShaderFn: Clone + MaybeSend + 'static {
    type In: GpuType; type Out: GpuType;
    type Params: encase::ShaderType + encase::internal::WriteInto;   // std140 layout by construction
    const MODULE: &'static WgslModule;    // import path + source
    const ENTRY: &'static str;            // `fn ENTRY(x: In, p: Params [, resources]) -> Out`
    fn params(&self, cx: &ParamCx) -> Self::Params;  // cx carries per-chunk origins for relative inputs
    fn resources(&self) -> &[Resource] { &[] }       // e.g. palette LUT texture
    fn input_format(&self) -> ColumnFormat;          // Linear/Time → F32Relative, Log → F32
}
pub trait CpuMirror: ShaderFn { fn eval(&self, x: f64) -> <Self::Out as GpuType>::Cpu; }

pub trait PositionScale: ShaderFn<In = f32, Out = Px> + CpuMirror {
    fn domain(&self) -> (f64, f64); fn set_domain(&mut self, d: (f64, f64)); fn set_range(&mut self, r: (Px, Px));
    fn invert(&self, px: Px) -> ScaleValue; fn ticks(&self, n: usize) -> Ticks; fn nice(&mut self);
}
pub trait ColorScale: ShaderFn<Out = Color> + CpuMirror { fn legend(&self) -> Legend; }
```

- **Family.** The scales are `Linear`, `Pow`/`Sqrt`, `Log` (with `Symlog`),
  `Time` (Linear over f64 seconds with calendar ticks), `Band`, `Point`,
  `Sequential` (256-entry LUT texture), `Diverging` and `Categorical` (uniform
  `array<vec4,16>`, Okabe-Ito by default).
- **Composition.** `a.then(b)` (`Then<A, B>` where `A::Out == B::In`) emits
  `b(a(x, pa), pb)` in the glue module. Each `Params` value is a separate field
  in a packed per-layer `Encodings` uniform. `FunctionChain`/`ChainUniforms` and
  their renaming tricks go away (`core.rs:1286-1324`, `selection.rs:2911`).
- **What changes on rescale or zoom.** Only the `Encodings` uniform (≤ 256 B)
  and the per-chunk base uniforms (16 B × chunks) change.
- **Shared scales.** Scales live in shared `ScaleRef<S>` handles, so axes,
  ticks, legend, `layout.invert` and the GPU always agree.
- **Log base** affects ticks only: the shader uses `log2` ratios.
- **CPU mirrors** are the existing tick algorithms (`tick_generator.rs`) moved
  behind `PositionScale::ticks`, evaluated in f64.
- **Conformance tests.** A T0 harness test dispatches each scale's WGSL in a
  compute pass over 10K samples and asserts that the GPU result matches
  `CpuMirror::eval` within 0.25 px (or 1/255 for colour).

---

## 6. Shader composition

### Today

Generated shaders are built by editing hand-written WGSL as text:

- The generator splits at `@vertex`, finds the marker
  `"let instance = instances["`, and rewrites `instance.<attr>` with
  `str::replace` (`selection.rs:2728,2798-2832`).
- Duplicate definitions are removed by brace-counting (`core.rs:1196`) and
  renamed by regex-like scans (`selection.rs:2863`).
- Every mark has a second hand-built `String` shader (`circle.rs:179-366`) that
  nothing calls.

### Options

|                    | naga_oil                                                                                               | Custom naga IR builder                  | Improved text templating  |
| ------------------ | ------------------------------------------------------------------------------------------------------ | --------------------------------------- | ------------------------- |
| Namespacing, dedup | Built in (`#define_import_path`, mangling)                                                             | Full control                            | Manual prefixes           |
| Error spans        | Mapped to the source module                                                                            | Poor (IR-level)                         | Point into generated text |
| Maintenance        | Bevy-maintained. 0.20 pairs with naga 27 and is **already in our Cargo.lock** via gup-bevy (bevy 0.18) | naga IR API breaks every release; large | Lowest                    |
| Expressiveness     | Modules and shader defs; entry glue still needed                                                       | Anything                                | Glue only                 |
| WASM cost          | regex + codespan                                                                                       | none                                    | none                      |

### Recommendation

Use **naga_oil for library modules, plus a small typed glue emitter**.

- **Library modules.** Marks, scales, palettes, the view transform and colour
  helpers are plain WGSL files with `#define_import_path gup::…`, preloaded into
  `Context.shaders`.
- **Glue emitter.** For each `(mark, encoding signature)` pair, a ~300-line Rust
  emitter builds a small expression tree and prints **one** top-level module.
  That module declares vertex inputs, the `Encodings` struct, bind groups, calls
  the encoders, and holds the entry points. naga_oil keeps entry points only
  from the top-level shader, so entry points live only in the glue.
- **Hard rule: no string edits to authored WGSL.**
- **Pipeline cache key**:
  `(MarkId, EncodingSignature, TargetDesc{format, samples}, Variant{Draw, Pick})`.
  Uniform values are not part of the key.
- **`#[wgsl_function]`** now emits a `ShaderFn` impl plus a module whose import
  path comes from `module_path!()`. It validates the WGSL with naga **at macro
  expansion**, so errors appear at compile time in external crates instead of as
  a runtime panic.
- **Fallback.** Library modules use only `#define_import_path`/`#import` (no
  `#ifdef`), so a plain concatenation of namespaced modules stays possible if
  naga_oil ever blocks a wgpu upgrade.

### Generated glue: scatter with linear x, log y, sequential fill

```wgsl
// gup glue: Circle {x: f32rel→linear, y: f32→log, fill: f32→sequential(lut), radius/stroke: const}
#import gup::view::{View, px_to_clip}
#import gup::scale::linear as linear
#import gup::scale::log as log
#import gup::color::sequential as seq
#import gup::marks::circle as circle

struct Encodings {            // packed std140 from each ShaderFn::Params + constant channels
    x: linear::Params,        // { k, r0, flags }
    y: log::Params,           // { log_d0, inv_log_span, r0, r_span }
    fill: seq::Params,        // { d0, inv_span, reverse }
    radius: f32, stroke_width: f32,
    stroke: vec4<f32>,
}
struct Chunk { row_base: u32, x_base: f32 }        // x_base = f32(origin - domain_lo), computed in f64

@group(0) @binding(0) var<uniform> view: View;     // size_px, dpr, plot clip
@group(1) @binding(0) var<uniform> enc: Encodings;
@group(1) @binding(1) var fill_lut: texture_2d<f32>;
@group(1) @binding(2) var fill_smp: sampler;
@group(2) @binding(0) var<uniform> chunk: Chunk;   // dynamic offset per chunk

struct Columns { @location(0) x: f32, @location(1) y: f32, @location(2) fill: f32 }

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32, col: Columns)
        -> circle::Varyings {
    var c: circle::CircleIn;
    c.center = vec2<f32>(linear::map_rel(col.x, chunk.x_base, enc.x), log::map(col.y, enc.y));
    c.radius = enc.radius;
    c.fill = seq::map(col.fill, enc.fill, fill_lut, fill_smp);   // textureSampleLevel inside
    c.stroke = enc.stroke;
    c.stroke_width = enc.stroke_width;
    return circle::vertex(c, circle::corner(vi), chunk.row_base + ii, view);
}
@fragment fn fs_main(v: circle::Varyings) -> @location(0) vec4<f32> { return circle::shade(v); }
// Variant::Pick adds: @fragment fn fs_pick(v) -> @location(0) vec2<u32> { return vec2(LAYER_ID, v.row + 1u); }
```

`circle::vertex` works in **Px**. It expands `corner * radius` in pixel space
and then calls `px_to_clip`, which makes circles aspect-correct. Today the
shader scales the radius by `scale_x` in clip units
(`mark/shaders/circle.vert.wgsl:44,52`).

---

## 7. Scene and RenderTarget

### Today

- `ComposedChart` owns its pipelines (`chart_builder.rs:1005-1023`).
- `render_to_rgba` has no text pass (`:2761-2770`).
- `render_to_texture_view` ignores its size arguments (`:2830-2831`).
- `render_to_svg` passes an empty mark slice (`:2645`).
- The pipeline colour format is hard-coded to `Bgra8UnormSrgb`
  (`selection.rs:2527`, `export/png.rs:259`).

### Design

```rust
pub struct Scene {
    pub size: Size<Px>, pub background: Color,
    pub items: Vec<Item>,            // stable-sorted by z at build
    pub clips: Vec<Rect<Px>>,        // plot rect etc.
}
pub struct Item { pub z: i32, pub clip: Option<ClipId>, pub kind: ItemKind }
pub enum ItemKind {
    Marks(MarkBatch),        // Arc<LayerGpu> (pipeline key, chunks, uniforms) + Arc<dyn VectorSource>
    Rules(Vec<Rule>),        // axis lines, ticks, grid: p0, p1, width: Px, color, dash
    Rects(Vec<RectPrim>),    // plot background, legend swatches
    Gradient(GradientBar),   // colour legend
    Text(Vec<TextRun>),
}
pub struct TextRun { pub text: Arc<str>, pub at: Point<Px>, pub anchor: Anchor, pub angle: Angle,
                     pub style: TextStyle, pub role: TextRole /* Title, TickLabel, Legend… */ }
```

- **Default z-order**: grid −100, data layers 0…n (or a user `z`), axes 100,
  legend and title 200, tooltip 1000.
- **Text is first-class.** `gup-text` (extracted from `src/text/`: font, layout,
  msdf, atlas survive) shapes every run. The same measurements drive both layout
  (§9) and every target, so margins match in PNG and SVG.
- **Default font.** Inter is bundled (OFL), per decision 4.

```rust
pub struct TargetDesc { pub format: wgpu::TextureFormat, pub size: PhysicalSize<u32>, pub dpr: f32, pub samples: u32 }
pub trait RenderTarget {
    fn desc(&self) -> TargetDesc;
    fn acquire(&mut self, cx: &Context) -> Result<Frame>;                 // color (+ MSAA) views
    fn present(&mut self, cx: &Context, f: Frame, cmd: wgpu::CommandBuffer) -> Result<()>;
}
// WindowTarget (winit surface; on wasm the HtmlCanvasElement surface), ImageTarget (offscreen + async
// readback → RgbaImage/PNG), TextureTarget (host-owned texture).
// For host paint callbacks (egui_wgpu::CallbackTrait, bevy render graph):
impl Renderer { pub fn prepare(&mut self, cx: &Context, s: &Scene, d: &TargetDesc) -> Prepared; }
impl Prepared { pub fn draw<'p>(&'p self, pass: &mut wgpu::RenderPass<'p>); }      // no submit
pub trait VectorTarget { fn render(&mut self, scene: &Scene) -> Result<()>; }    // SvgTarget, PdfTarget
```

- **Vector output** draws guides directly. `MarkBatch::vector()` evaluates CPU
  mirrors over the retained columns. Above `SvgOptions::max_vector_marks`
  (default 200K), the marks layer is rasterised as an embedded PNG.
- **Clipping** uses a scissor rect on the GPU and `<clipPath>` in SVG.
- **Colour policy: sRGB-space end to end, matching browsers, SVG and PDF.**
  - Public colours are sRGB-encoded.
  - GPU targets render into a **non-sRGB view**. For example, a `Bgra8UnormSrgb`
    surface is configured with a `Bgra8Unorm` view format; WebGPU canvases are
    non-sRGB already. Blending therefore happens in sRGB space, as it does in
    browsers.
  - Pipelines output premultiplied alpha, which egui needs and which makes
    transparent PNGs correct; the PNG writer un-premultiplies.
  - If a host forces an sRGB view, a pipeline shader-def encodes to linear on
    output.
  - Palette LUTs are `Rgba8Unorm` (no hardware linearisation). Colours are
    interpolated in OKLab when building LUTs.
  - This ends the double gamma encoding by construction. The T4a stopgap stays
    valid for the old path.
- **Antialiasing.** Points, lines and text use analytic AA (fwidth or SDF).
  Polygons and areas use **MSAA 4×** by default on Gup-owned targets (WebGPU
  guarantees 1 and 4). Pick passes use 1 sample. Hosts set the sample count in
  draw-in-pass mode, and it is part of the pipeline key.
- **Units.** `Px` is logical pixels; `Ndc` exists only inside `gup::view`. `dpr`
  belongs to the target. Guides snap hairlines to physical-pixel centres.
  `save_png(path, w, h)` uses dpr 1, and `save_png_scaled(..., 2.0)` doubles the
  physical size.

---

## 8. Chart trait and composition

### Today

`Mixable`, `ComposedChart`, `CompositeChart` and egui's `DynChart`
(`gup-egui/src/widget.rs:22`) all exist. Every builder returns
`ComposedChart<T, Circle>`, and so does the composite's "primary" layer
(`scatter.rs:314`, `composite.rs:885`).

### Design

```rust
pub trait Chart: MaybeSend + 'static {                      // object-safe
    fn resolve(&mut self, cx: &mut ResolveCx<'_>) -> Result<&Resolved>; // cached; cheap when clean
    fn handle(&mut self, input: &Input, cx: &mut InputCx<'_>) -> Response;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
pub struct Resolved { pub layout: Layout, pub scene: Scene }

pub trait ChartExt: Chart {                                  // blanket impl, also for dyn Chart
    fn save_png(&mut self, path: impl AsRef<Path>, w: u32, h: u32) -> Result<()>;
    fn to_rgba(&mut self, cx: &Context, w: u32, h: u32) -> impl Future<Output = Result<RgbaImage>>;
    fn save_svg(&mut self, path: impl AsRef<Path>, w: u32, h: u32) -> Result<()>;
    fn layout(&self) -> Option<&Layout>;
}
```

- **`Plot`** is the universal chart. It holds `Vec<Box<dyn Layer>>`, the shared
  `Scales` registry (keyed by role: x, y, y2, color, size), guide configuration
  and a `Theme`. `Selection<T, M>` implements the internal `Layer` trait.
- **Shared domains.** Layers that bind the same role share a scale, and its
  domain is the union of their f64 chunk stats.
- **Z-order** is per layer (`.z(i32)`), with insertion order as the tiebreak.
- **Builders.** `ScatterChart<T>` and the others are thin typed newtypes over
  `Plot`. The builder _is_ the chart: there is no `build()`, and accessors are
  evaluated lazily at first `resolve`, after the scale choice is known.
- **Drop-down access:**

```rust
impl<T: 'static> ScatterChart<T> { pub fn layer_mut<M: Mark>(&mut self) -> Option<&mut Selection<T, M>>; }
impl Plot { pub fn layer_mut<T: 'static, M: Mark>(&mut self) -> Option<&mut Selection<T, M>>; } // first match
```

- **Multi-panel and facets** (later) are `Chart`s holding `Box<dyn Chart>`
  children. `gup-egui`'s widget takes `Box<dyn Chart>`.

---

## 9. Layout, picking and events

**Resolve pipeline.** This runs only when the size, data or a domain changes:

1. Domains come from column stats, then `nice()`.
2. Ticks come from the CPU mirrors.
3. Tick labels, titles and the legend are measured with `gup-text`, which gives
   the margins.
4. The plot rect is computed.
5. Scale ranges are set from the plot rect.
6. Uniforms are written.
7. The scene is emitted.

```rust
pub struct Layout {
    pub size: Size<Px>, pub plot: Rect<Px>, pub margins: Insets<Px>,
    pub x: ScaleSnapshot, pub y: ScaleSnapshot, pub bands: Option<Bands>, pub legend: Option<Rect<Px>>,
}
impl Layout {
    pub fn invert(&self, p: Point<Px>) -> Option<(ScaleValue, ScaleValue)>;  // None outside plot
    pub fn to_px(&self, x: f64, y: f64) -> Point<Px>;
}
```

**Picking.**

- **`chart.pick(px) -> Option<&T>`** is synchronous and runs on the CPU. It
  evaluates CPU mirrors over the retained columns into a lazily built uniform
  grid in Px; rebuilding is O(n) and happens only on rescale.
  `pick_row(px) -> Option<RowId>` always works.
- **`pick_async(px)`** uses the `Pick` pipeline variant. It draws a scissored
  5×5 px region into an `Rg32Uint` target holding `(layer, row+1)`, then uses
  `map_async` without blocking and gets the result one or two frames later.
- **Above the `Retain` threshold**, only the GPU path is available.
- **What it replaces.** Today `InteractionData` (`selection.rs:308`) hit-tests
  positions the user supplies, not the positions that were drawn.

**Events.**

- `GupApp`, `gup::show`, the egui widget and the WASM canvas translate host
  input into `gup::Input` and call `chart.handle(...)`.
- `Plot` resolves the hit (CPU, or the latest GPU pick) and dispatches it to
  handlers registered with `sel.on(Event::Click, |ev, d: &T| …)`. The manual
  `trigger_event` (`selection.rs:581`) goes away.
- Built-in behaviours (tooltip, wheel zoom, drag pan, brush) mutate scale
  domains, so they cost uniform writes only.

---

## 10. Public API sketches

**(a) Scatter to PNG**

```rust
use gup::prelude::*;
#[derive(Clone)] struct Row { gdp: f64, life_exp: f64, continent: String, population: f64 }

fn main() -> gup::Result<()> {
    let data: Vec<Row> = load_rows()?;
    gup::scatter(&data)
        .x(|d: &Row| d.gdp).x_scale(Log::new())
        .y(|d: &Row| d.life_exp)
        .color(|d: &Row| d.continent.clone())   // dictionary-encoded → Categorical (Okabe-Ito)
        .size(|d: &Row| d.population)           // Sqrt scale → Px
        .title("Wealth and health of nations")
        .save_png("out.png", 800, 500)          // &mut self on a temporary; Context::shared()
}
```

**(b) Scatter in a window**

```rust
let chart = gup::scatter(data)
    .x(|d: &Row| d.gdp).y(|d: &Row| d.life_exp)
    .tooltip(|d: &Row| format!("{}: {:.1} years", d.continent, d.life_exp));
gup::show(chart)?;   // = GupApp::new(chart).title("Gup").size(800, 500).run()
```

**(c) Low-level Selection with typed channels and a shader-fn colour scale**

```rust
let mut plot = gup::plot();
let (x, y) = (plot.x(Time::new()), plot.y(Log::new()));      // ScaleRef handles shared with axes
let heat = Sequential::viridis().domain(-10.0, 40.0);
plot.add(Selection::<Reading, Circle>::new(readings))
    .attr(Circle::X, x.encode(|r: &Reading| r.timestamp))
    .attr(Circle::Y, y.encode(|r: &Reading| r.value))
    .attr(Circle::FILL, heat.encode(|r: &Reading| r.temp))
    .attr(Circle::RADIUS, Px(2.5))
    .on(Event::Click, |_, r: &Reading| println!("{r:?}"));
// .attr(Circle::RADIUS, heat.encode(..))  // ✗ compile error: Sequential produces Color, RADIUS needs Px
// .attr(Rect::WIDTH, Px(3.0))             // ✗ compile error: Channel<Rect, _> on Selection<_, Circle>
plot.save_png("readings.png", 1200, 600)?;
```

**(d) Composite chart**

```rust
let mut chart = gup::plot()
    .layer(gup::area(&forecast).x(|f: &Fc| f.date).y0(|f: &Fc| f.lo).y1(|f: &Fc| f.hi).opacity(0.25))
    .layer(gup::line(&forecast).x(|f: &Fc| f.date).y(|f: &Fc| f.mid))
    .layer(gup::scatter(&observed).x(|o: &Obs| o.date).y(|o: &Obs| o.value).z(10))
    .title("Forecast vs observed");                 // x/y scales shared, domains unioned
chart.layer_mut::<Obs, Circle>().unwrap().attr(Circle::STROKE, Color::BLACK);
gup::show(chart)?;
```

**(e) Live append**

```rust
let chart = gup::line(Vec::<Tick>::new())
    .x(|t: &Tick| t.time).y(|t: &Tick| t.price)
    .window(Window::LastRows(1_000_000));           // whole-chunk eviction
let feed = chart.appender();                        // Clone + Send handle (channel)
std::thread::spawn(move || for t in market_feed() { feed.push(t); });
gup::show(chart)?;   // per frame: drain → tail-only write_buffer; domain change → uniforms + ticks
```

**(f) Host-owned device** (egui/bevy)

```rust
let cx = gup::Context::from_wgpu(render_state.device.clone(), render_state.queue.clone());
```

---

## 11. Migration plan

### Where the code lives

The new core goes into a new workspace crate, `crates/gup-core`, which is not
re-exported until the flip story.

- This enforces boundaries: no reaching into old modules.
- It gives T7's split a head start.
- It makes "external crate" tests the natural default.
- The old path stays compiled but **frozen** (review §Backlog triage) until it
  is deleted.
- `gup-macros` already supports overriding the crate path (`crate_path.rs`).

### Survive, adapt, delete

The table respects the T1 prune list: nothing T1 deletes is migrated.

| Fate                | Code                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Survive** (move)  | `text/{font,layout,msdf,atlas,sdf_tuning,style}` → `gup-text`; tick algorithms in `tick_generator.rs` → scale CPU mirrors; WGSL bodies of `LinearScale`/`LogScale`/`color_scale` normalisation and palette data (`shader_function/{math,color}.rs`) → WGSL modules/LUTs; `export/png.rs` encoding, `export/svg` writer; `streaming/dirty_region.rs`; `gpu_timer.rs`; LOD and culling → experimental crate (decision 1)                                                                                                                                                                                                                        |
| **Adapt** (rewrite) | `Selection` (`selection.rs`: keep the data/events/transition surface, rebuild the internals); `Mark` trait and `#[derive(Mark)]`; `Circle` (WGSL module contract, Px radius); `#[wgsl_function]` (emits a module and validates at compile time); `app.rs` (`GupApp` on `Chart`); axis and grid → guide emitters; `interaction.rs` event types → `Input`/`Event`; accessibility descriptions from channels; `gup-egui`/`gup-bevy` re-wired through `from_wgpu` and draw-in-pass                                                                                                                                                                |
| **Delete**          | `AttrValue`/`IntoAttrValue`/`attr_parallel`/`attr_shader`/`ShaderFnInfo` (`selection.rs:72-264`); WGSL splicing (`selection.rs:2706-2975`); `MarkInstanceBuilder` and every `*Instance`; `Mark::generate_*_shader*` (`circle.rs:171-366` and siblings); `shader_pipeline.rs`; `FunctionChain`/`ChainUniforms`/`ParallelComposition`/`ConditionalFunction` and the text helpers (`core.rs:1134-1900`); `AxisScale`, `scale.rs`, the tick `Scale` trait; `RenderContext`, `GupContext` (including multi-surface, recovery, pools); `ComposedChart`, `CompositeChart`, `DynChart`, `ConfigurableBuilder`; `ViewportUniforms`/`ViewportTransform` |

### Story sequence

Each story merges on its own and keeps main green. "Done" means a golden image
the agent has looked at, examples running headless, and dogfood compiling.

| #      | Story                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Track |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| **S0** | **Vertical-slice spike** in `gup-core`, scatter only. Context (`new_blocking`, `from_wgpu`); single-chunk column store with relative origin; Circle module + glue emitter on naga_oil; channels X/Y/RADIUS/FILL; Linear, Log, Sequential with CPU mirrors; Scene with axis rules, tick-label and title `TextRun`s via `gup-text`; ImageTarget→PNG and WindowTarget via a minimal `show()`. **Exit**: (1) golden PNG with title and tick labels; (2) window screenshot (`GUP_SCREENSHOT_PATH`) within ΔE < 2 of the PNG; (3) 100K points at ≥60 fps while zooming, with a buffer-write counter showing 0 column bytes; (4) `trybuild` failures for wrong type, unknown channel, wrong mark; (5) north-star snippet compiles as a doctest; (6) generated WGSL and pipeline-compile timings attached to this RFC. **Gate**: naga_oil yes/no. | T3    |
| S1     | `gup::Context` in `gup-core`; old `gup` builds `RenderContext`/`GupContext` as thin shims over it; `pub use wgpu`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | T2    |
| S2     | Extract `gup-text` (leaf crate), one `TextSystem` per Context, measuring API, glyph-run draw into a pass, Inter bundled                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | T2    |
| S3     | `Scene`, `Renderer`, `RenderTarget` (Image, Window, Texture, draw-in-pass), `SvgTarget` for guides; units and colour policy; MSAA                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | T2    |
| S4     | Column store v1: chunking, origins, stats, `Retain`, dictionary encoding, validity bits, tail append; precision tests at chunk boundaries                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | T3    |
| S5     | `ShaderFn` v2 + encase + `then`; full scale family with CPU mirrors; GPU≡CPU conformance harness                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | T3    |
| S6     | `#[derive(Mark)]` v2 + typed channels + compile-fail suite; marks Circle, Rect, Rule, Segment/Line                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | T3    |
| S7     | `Selection` v2, `Plot`, `Chart`/`ChartExt`, `Layout` resolve, axes/grid/legend/title guides, `Theme` (Inter, Okabe-Ito, viridis)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | T2/T3 |
| S8     | `GupApp`/`show` on `Chart`; `Input` dispatch; CPU `pick`; tooltips; `hello_world` in ≤10 lines                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | T2    |
| S9     | GPU pick variant (async); zoom/pan/brush behaviours                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | T3    |
| S10    | Port `scatter`, `line`, `bar` builders onto `Plot` + `layer_mut`; dogfood tasks 1–3 pass                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | T5    |
| S11    | Composite / mixed-`T` layers, secondary y; delete `ComposedChart`/`CompositeChart`/`DynChart`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | T2/T5 |
| S12    | `append`/`appender`/`Window`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | T5    |
| S13    | Re-wire `gup-egui` and `gup-bevy` through `from_wgpu` and `Prepared::draw`; back into the workspace and CI                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | T2    |
| S14    | **Flip**: `gup` re-exports the `gup-core` prelude; delete the old selection/mark/shader_function/shader_pipeline/chart_builder/context/render; revise the TECHNICAL_APPROACH and IMPLEMENTATION_STRATEGY docs                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | T2/T3 |

The remaining builders (area, boxplot, violin, density, heatmap, choropleth)
follow S14 as T5 stories.

---

## 12. Risks and open questions

1. **naga_oil may lag behind wgpu releases.** _Recommendation_: keep it behind
   an internal `Composer` trait and write library modules in the import-only
   subset so concatenation remains a fallback. Accept that wgpu upgrades follow
   the Bevy cadence.
2. **Too many pipeline variants and slow compiles**, since every encoding
   signature is a pipeline. _Recommendation_: keep uniforms out of the cache
   key, build pipelines asynchronously, pre-warm builder defaults, and measure
   compose-plus-create time in S0. Budget: ≤30 ms per pipeline on desktop.
3. **sRGB-space vs linear blending.** _Recommendation_: sRGB-space, for parity
   with browsers, SVG and PDF (§7). Linear blending is physically nicer for
   dense overplotting, so offer it as an opt-in `Theme::blend_linear` later.
   This needs owner sign-off because it refines T4a.
4. **`|d: &Row| d.continent.as_str()` won't infer.** A closure returning a
   borrow of its argument can't satisfy `Fn(&T) -> V`, because `V` can't depend
   on the argument's lifetime. _Recommendation_: support owned keys generically
   and add `.color_key(|d| d.continent.as_str())`, bounded directly by
   `for<'a> Fn(&'a T) -> &'a str`. S0 decides, and the north-star snippet is
   adjusted to whatever compiles.
5. **Ergonomics of `&mut self`.** `let chart = …; chart.save_png()` needs
   `let mut chart`. _Recommendation_: accept this. The cached GPU state lives in
   the chart, and chained calls on temporaries still work.
6. **f32 precision on deep zoom inside a chunk.** _Recommendation_: chunk
   origins cover timestamps and typical zoom ranges. Add an opt-in `F32x2`
   (hi/lo, "df64") column format later; the glue already supports it per
   channel.
7. **8 vertex-buffer slots.** _Recommendation_: constants are uniforms, and a
   mark with more than 7 data columns is an error when the pipeline is built. If
   a real mark needs more, add a storage-buffer fallback.
8. **Memory for CPU retention at scale.** _Recommendation_: use the
   `Retain::Auto` threshold. SVG/PDF above the threshold rasterises the marks
   layer. Document that `pick` returns a `RowId` in `GpuOnly` mode.
9. **Closure `attr` defeats GPU rescaling.** _Recommendation_: builders always
   use `scale.encode(..)`. Document `attr(ch, closure)` as "pre-encoded visual
   values".
10. **WASM.** There is no blocking, so `save_png` and `Context::shared` don't
    exist there. naga*oil adds binary size. \_Recommendation*: async-only APIs
    on wasm, and measure binary size in S3 with a budget (≤ +400 KB gz). Fall
    back per risk 1 if it's over.
11. **Compile-error quality.** _Recommendation_: use
    `#[diagnostic::on_unimplemented]` on `IntoEncoding`/`Visual`, and check the
    `trybuild` snapshot output in review.
12. **Device loss** is dropped together with `GupContext`'s recovery code.
    _Open_: re-introduce later as "rebuild Context and re-upload from retained
    columns". Not before S14.
13. **Transitions.** Domain tweens are uniform-only and come free. Data
    transitions need previous and next column sets plus a `t` uniform mixed in
    the glue. _Recommendation_: defer to after S14; the design allows it.
14. **Accessibility** today depends on string attributes (`AccessibleMark`,
    `selection.rs:382`). _Recommendation_: generate descriptions from channel
    roles plus scale formatters over the retained rows. This becomes a T5 story.
15. **Draw-call count at 1B points** (1,000 chunks × layers). _Recommendation_:
    CPU chunk culling first, render bundles per layer second, and
    `multi_draw_indirect` where the feature exists. This belongs to T7.
16. **Should `Plot` be generic over `T`?** _Recommendation_: no. `Plot` stays
    untyped so that mixed-`T` layers are free, and typed newtypes
    (`ScatterChart<T>`) give `pick -> Option<&T>`.

---

## Appendix: current-code evidence

| Claim                                                    | Location                                                                     |
| -------------------------------------------------------- | ---------------------------------------------------------------------------- |
| String-named attrs; any `attr` throws away GPU state     | `src/selection.rs:676-690`                                                   |
| Per-row `Vec<(&str, AttrValue)>` and string match        | `src/selection.rs:1387-1397`, `src/mark/circle.rs:472-505`                   |
| Unknown attribute silently ignored                       | `src/mark/circle.rs:501`                                                     |
| Runtime type check through WGSL type-name strings        | `src/selection.rs:1497-1508`                                                 |
| WGSL text splicing via marker and `str::replace`         | `src/selection.rs:2728, 2798-2832`                                           |
| Uniform buffers created then dropped; one per binding    | `src/selection.rs:1681-1716`                                                 |
| `ColorScale` globals never bound                         | `src/shader_function/color.rs:793-815`                                       |
| Builder `color_scale` feeds only the colorbar            | `src/chart_builder/builders/scatter.rs:189-206`, `src/chart_builder.rs:1412` |
| CPU NDC mapping in closures; radius `× ndc_width × 0.01` | `src/chart_builder/builders.rs:664-689, 746-748`                             |
| No-op builder methods                                    | `src/chart_builder/builders/scatter.rs:156,163`                              |
| Hard-coded `Bgra8UnormSrgb`                              | `src/selection.rs:2527`, `src/export/png.rs:259`                             |
| Elliptical circles (radius × `scale_x` in clip space)    | `src/mark/shaders/circle.vert.wgsl:44,52`                                    |
| PNG path has no text; text only via `RenderFrame`        | `src/chart_builder.rs:2761-2770, 2211`                                       |
| Hand-written uniform WGSL drifts from Rust               | `src/shader_function/math.rs:52,63`                                          |
| Four scale systems                                       | `math.rs:97`, `chart_builder.rs:496`, `tick_generator.rs:40`, `scale.rs:275` |
| Primary composite layer hard-coded to `Circle`           | `src/chart_builder/builders/composite.rs:885`                                |
| Culling ceiling of 16.7M; blocking readback              | `src/mark/compute_instance_filter.rs:139, 634-646`                           |
| LOD: one unchunked array-of-structs buffer per level     | `src/lod/mod.rs:62,112`                                                      |
| naga 27.0.3 / naga_oil 0.20.0 / encase 0.12 in lockfile  | `Cargo.lock` (via `gup-bevy`, bevy 0.18)                                     |

### Critical files for implementation

- `/home/corin/src/github.com/au-phiware/gup/src/selection.rs` (binding model
  and WGSL splicing to replace; event and transition surface to keep)
- `/home/corin/src/github.com/au-phiware/gup/src/mark/circle.rs` and
  `/home/corin/src/github.com/au-phiware/gup/src/mark/shaders/circle.vert.wgsl`
  (first mark ported to the module contract)
- `/home/corin/src/github.com/au-phiware/gup/src/shader_function/math.rs` and
  `/home/corin/src/github.com/au-phiware/gup/src/shader_function/color.rs`
  (scale WGSL and palette data to move into modules with CPU mirrors)
- `/home/corin/src/github.com/au-phiware/gup/src/context.rs`,
  `/home/corin/src/github.com/au-phiware/gup/src/render.rs` and
  `/home/corin/src/github.com/au-phiware/gup/src/app.rs` (merged into
  `gup::Context`; `GupApp` on `Chart`)
- `/home/corin/src/github.com/au-phiware/gup/gup-macros/src/mark_derive.rs` and
  `/home/corin/src/github.com/au-phiware/gup/gup-macros/src/wgsl_function.rs`
  (typed-channel derive; compile-time WGSL validation)
- `/home/corin/src/github.com/au-phiware/gup/src/chart_builder.rs`
  (`render_to_rgba` ~2715 and SVG ~2645, to be replaced by Scene/RenderTarget)

---

## Orchestrator review (2026-10-04)

**Verdict**: accept the direction. The RFC answers every structural finding in
the strategic review, and its migration plan keeps `main` green. Five points
need resolving before S0 starts.

1. **Parallel-system risk.** This project's history shows that systems running
   side by side never get deleted (four scale systems, three composition
   systems, three pipeline caches). A frozen old `gup` next to `gup-core`
   repeats that pattern unless the flip is enforced. **Amendment**:
   - No feature work on the old path from S0 onward. Bug fixes only if they
     block T0 tooling.
   - S14 (flip and delete) must land before any T5 builder beyond
     scatter/line/bar is ported.
   - The story index tracks "LOC remaining in old path" as a metric after each
     S-story.
2. **Knock-on effects for wave-1 stories.**
   - **T1 public-surface purge**: largely superseded. The `gup-core` prelude is
     designed fresh (S7) and becomes `gup`'s at the flip. Recommend parking it,
     or cutting it down to the deletions T1 prune already covers.
   - **T4a correct visual defaults**: keep only what survives into `gup-core`:
     - Inter font (moves with `gup-text`)
     - tick precision (tick algorithms become CPU mirrors)
     - Okabe-Ito palette data
     - theme colour values

     Drop the old-path colour-pipeline change, because §7's sRGB-space policy
     replaces it.

   - **T4a mark fidelity**: park. Circle radius, MSAA and clipping are rebuilt
     in S0/S3. The area-stroke and composite-panic fixes on the frozen path have
     no users.
   - **T0 harness**: must be target-agnostic (assert on an RGBA image plus
     layout metadata), so it serves the old builders now and `gup-core` from S0
     on.
   - **T1 prune**: unaffected. It should land before S0 so `gup-core` doesn't
     model itself on dead code.

3. **S0 is large.** It covers context, columns, naga_oil glue, channels, three
   scales, a text scene, PNG and window, a perf benchmark and trybuild.
   **Amendment**: split it into S0a and S0b.
   - **S0a**: headless PNG path with title and tick labels, plus the trybuild
     compile-fail tests.
   - **S0b**: window, `show()`, the 100K-point zoom benchmark and the window/PNG
     parity check.

   The naga_oil gate is decided at the end of S0a.

4. **Colour policy**: owner chose **sRGB-space blending** (2026-10-04). Linear
   blending may come later as an opt-in theme flag. sRGB space matches browsers,
   SVG and PDF; linear space is physically correct and nicer for dense
   overplotting. (Risk 3.)
5. **Dependency note.** naga_oil 0.20 is in `Cargo.lock` only through
   `gup-bevy`, which is now parked. The compatible version still stands; it just
   becomes a direct dependency of `gup-core`.
