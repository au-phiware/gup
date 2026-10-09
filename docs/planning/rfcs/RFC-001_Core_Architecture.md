# RFC-001: Core Architecture

**Status**: Accepted 2026-10-04 with the amendments in "Orchestrator review"
(owner decisions: accept; sRGB-space blending; wave 1 trimmed to surviving work)
and in "Decisions (2026-10-09)" (owner decisions: shader composition moves to
build time on every target; wasm targets WebGPU only, for now) **Date**:
2026-10-04 **Tracks**: T2 (One Context / Scene / RenderTarget) and T3 (Core data
model), from [STRATEGIC_REVIEW_2026-10](../STRATEGIC_REVIEW_2026-10.md)
**Supersedes on acceptance**: the architecture sections of
`TECHNICAL_APPROACH.md` and `IMPLEMENTATION_STRATEGY.md` (review decision 6)

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
                                                  // (superseded 2026-10-09: library modules are flattened
                                                  // at build time; see "Decisions (2026-10-09)")
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

> **Superseded 2026-10-09**: this recommendation composed library modules with
> naga_oil at runtime, preloaded per `Context`, with import-only concatenation
> kept only as a wasm fallback (risk 1, risk 10). The owner moved composition to
> build time on every target instead. See "Decisions (2026-10-09)".

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
  naga_oil ever blocks a wgpu upgrade. (Superseded 2026-10-09: this
  concatenation is now the runtime mechanism on every target, fed by naga_oil's
  output at build time, not a conditional fallback. See "Decisions
  (2026-10-09)".)

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
   the Bevy cadence. (Superseded 2026-10-09: naga_oil becomes a build-time-only
   dependency — see "Decisions (2026-10-09)" — so a lag no longer gates a wgpu
   upgrade of the runtime crate; the build-time tooling can upgrade on its own
   schedule. Done in GUP-406: naga_oil is a dependency of `gup-wgsl`'s
   build-time `compose` feature only; see "GUP-406 findings".)
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
    exist there. `naga_oil` adds binary size. _Recommendation_: async-only APIs
    on wasm, and measure binary size in S3 with a budget (≤ +400 KB gz). Fall
    back per risk 1 if it's over. (Superseded 2026-10-09: S3 found the cost was
    +897 KB gz, 2.2× over budget — see "S3 findings". The owner's fix is not a
    wasm-only fallback but build-time composition on every target, plus dropping
    the wasm `GL` backend default; see "Decisions (2026-10-09)". Resolved in
    GUP-406: the reference scatter is 392.9 KB gz in total, +351.1 KB over bare
    wgpu including the bundled Inter's 198.3 KB, so +152.9 KB without it; see
    "GUP-406 findings".)
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

---

## S0a findings (2026-10-05, GUP-395)

[GUP-395](../stories/GUP-395_Gup_Core_Vertical_Slice_Headless.md) built the
headless half of S0 in `crates/gup-core`: `Context` (`new_blocking`,
`from_wgpu`), a single-chunk column store with relative origins, the Circle
module and a typed glue emitter on naga_oil, `X`/`Y`/`RADIUS`/`FILL` channels,
`Linear`/`Log`/`Sequential` with CPU mirrors, and `Plot` → `Scene` →
`ImageTarget` → PNG with a title and tick labels. All numbers below come from an
Intel HD Graphics 630 (KBL GT2, Mesa 26.0.0 Vulkan) with rustc 1.93.1.

### Decision: GO on naga_oil

Keep naga_oil 0.20 for library modules plus the typed glue emitter (§6). The
fallback is not needed. Evidence:

1. **It integrates as a direct dependency.** naga_oil 0.20.0, naga 27.0.3 and
   wgpu 27.0.1 resolve together with no patches. The composed `naga::Module` is
   passed straight to wgpu (`ShaderSource::Naga`, wgpu's `naga-ir` feature), so
   WGSL is parsed once, by naga_oil.
2. **Every composition S0a needed worked.** Five library modules, the generated
   glue, two hand-written top-level guide shaders (rules, text) and a compute
   harness for the conformance tests. This covered `#import path as alias`,
   `#import path::{Item}`, imported structs as uniform members, textures and
   samplers as function arguments, an import alias that is also a WGSL builtin
   (`log`), and a module that imports another (`circle` imports `view`). Entry
   points come only from the top-level module, as §6 assumed.
3. **It is fast enough**: 2.75 ms median for compose + create in release, 13.5
   ms for the cold first run, against a ≤ 30 ms budget (table below).
4. **Errors map back to the source.** A wrong-arity call in generated glue:

   ```text
   shader composition failed for broken_glue:
   error: failed to build a valid final module: Function [1] 'f' is invalid
     ┌─ broken_glue:2:1
     │
   2 │ fn f() -> f32 { return gup::scale::linear::map_rel(1.0, 2.0); }
     │ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     │ │                      │
     │ │                      invalid function call
     │ naga::ir::Function [1]
     │
     = Call to [0] is invalid
     = Requires 3 arguments, but 2 are provided
   ```

   Errors in a library module point at that module's own source (see the
   identifier rule below).

Timings: the reference pipeline composed and created 20 times, each time with a
fresh composer and cache on one device
(`cargo test -p gup-core --lib pipeline_timings -- --ignored --nocapture`).

| Step (ms)                                         | Debug median | Release median | Release max |
| ------------------------------------------------- | -----------: | -------------: | ----------: |
| Glue emit (Rust string building)                  |        0.049 |          0.017 |       0.037 |
| naga_oil `make_naga_module`                       |         24.5 |           2.28 |        3.08 |
| `create_shader_module` + `create_render_pipeline` |         2.92 |           0.42 |        11.0 |
| **Compose + create**                              |     **27.3** |       **2.75** |    **13.5** |
| `Context` creation incl. library preload          |         71.1 |           6.55 |        8.96 |

The release maximum is the first, cold, run in the process. The §12 risk 2
budget (≤ 30 ms per pipeline on desktop) holds with 2–10× headroom in release
and is met even in a debug build. The per-`Context` cost of preloading the
library (6.6 ms release) is paid once per context.

The costs are real but acceptable: the flattened module is unreadable
(`ParamsX_naga_oil_mod_XM52XAOR2ONRWC3DFHI5GY2LOMVQXEX`), so the readable
artifact is the glue, not the composed output. The rules below also constrain
how library modules are written.

### Generated WGSL: the reference scatter

The emitter's output for linear x, log y, sequential fill and a constant radius.
It is checked in as `crates/gup-core/tests/fixtures/scatter_glue.wgsl`, and the
composed, flattened module as `scatter_composed.wgsl` next to it (158 lines; a
unit test fails if either changes without `GUP_BLESS=1`).

```wgsl
// gup glue: Circle {x: f32rel→gup::scale::linear::map_rel, y: f32→gup::scale::log::map, radius: const f32, fill: f32→gup::color::sequential::map(lut)}
// Generated by gup-core's glue emitter; do not edit.
#import gup::view::{View}
#import gup::marks::circle as circle
#import gup::scale::linear as linear
#import gup::scale::log as log
#import gup::color::sequential as sequential

struct Encodings {
    @align(16) x: linear::Params,
    @align(16) y: log::Params,
    @align(16) radius: f32,
    @align(16) fill: sequential::Params,
}

struct Chunk {
    row_base: u32,
    x_base: f32,
}

@group(0) @binding(0) var<uniform> u_view: View;
@group(1) @binding(0) var<uniform> enc: Encodings;
@group(1) @binding(1) var fill_lut: texture_2d<f32>;
@group(1) @binding(2) var fill_smp: sampler;
@group(2) @binding(0) var<uniform> chunk: Chunk;

struct Columns {
    @location(0) x: f32,
    @location(1) y: f32,
    @location(2) fill: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32, col: Columns) -> circle::Varyings {
    var m: circle::CircleIn;
    m.x = linear::map_rel(col.x, chunk.x_base, enc.x);
    m.y = log::map(col.y, enc.y);
    m.radius = enc.radius;
    m.fill = sequential::map(col.fill, enc.fill, fill_lut, fill_smp);
    return circle::vertex(m, vertex_index, chunk.row_base + instance_index, u_view);
}

@fragment
fn fs_main(v: circle::Varyings) -> @location(0) vec4<f32> {
    return circle::shade(v);
}
```

### Other evidence

- **GPU ≡ CPU conformance** (1,000 samples per scale, compute pass over columns
  stored as the column store stores them): linear over one day of Unix
  timestamps 1.0e-4 px, log over nine decades 6.8e-5 px, viridis 2.3e-5 per
  channel. A negative control shows the same timestamps as absolute f32 miss by
  **0.74 px**, so the relative-origin column format (§3) is what keeps linear
  inside 0.25 px.
- **Rescale is uniform-only.** Changing a `ScaleRef` domain and re-resolving
  reuses the composed program, hits the pipeline cache and reuses the same
  column buffer (`plot::tests::rescale_reuses_program_pipeline_and_columns`).
  Counting column bytes written while zooming is S0b's job.
- **The PNG is correct by construction.** `tests/golden/gup_core/scatter.png`
  (720×450) passes every GUP-388 structural check, using layout metadata from
  `gup_core::Layout`: title present, 13 x and 13 y tick labels present, marks
  confined to the plot rect, viridis end colours present within ΔE 3. The old
  path's PNG output has no text at all.
- **Compile errors** (trybuild, rustc 1.93.1; snapshots in
  `crates/gup-core/tests/compile_fail/`):

  ```text
  // wrong value type: attr(Circle::RADIUS, Sequential::viridis().encode(..))
  error[E0271]: type mismatch resolving `<Sequential as ShaderFn>::Out == Px`
     |     sel.attr(Circle::RADIUS, Sequential::viridis().encode(|r: &Row| r.temp));
     |         ----                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `Px`, found `Color`

  // unknown channel: Circle::RADUIS
  error[E0599]: no variant or associated item named `RADUIS` found for enum `gup_core::Circle` in the current scope
  help: there is an associated constant `RADIUS` with a similar name

  // wrong mark: Channel<Bar, Px> on Selection<_, Circle>
  error[E0308]: mismatched types
     |     sel.attr(Bar::WIDTH, Px(3.0));
     |         ---- ^^^^^^^^^^ expected `Channel<Circle, Px>`, found `Channel<Bar, Px>`

  // bare number: attr(Circle::RADIUS, 3.0f32)  (on_unimplemented applies here)
  error[E0277]: `f32` cannot drive a channel of visual type `Px`
     |     sel.attr(Circle::RADIUS, 3.0f32);
     |         ----                 ^^^^^^ this encoding does not produce `Px` for rows of type `Row`
     = note: a `Px` channel accepts a constant of type `Px` (such as `Px(3.0)`) or `f.encode(|row: &Row| …)` where `f: ShaderFn<Out = Px>`
  ```

### RFC assumptions that proved wrong or needed refining

1. **naga_oil forbids identifiers that end in a digit** in composable modules
   ("Composable module identifiers must not require substitution according to
   naga writeback rules: `r0`"). §5–6 use `r0`, `d0` and `log_d0`. These became
   `range_start`, `lo` and `log_lo`, and `View`'s padding field became
   `padding`. This is now an authoring rule for every library module and for
   `#[wgsl_function]` (S5) to check at macro expansion.
2. **"std140 layout by construction" covers each `Params`, not the packed
   `Encodings`.** WGSL requires struct-typed uniform members to be 16-byte
   aligned, and nothing aligns them automatically. The emitter puts `@align(16)`
   on every `Encodings` field. The uniform writer reads member offsets from
   naga's layout of the composed module and checks each encase `Params` against
   the space before the next member. A test compares the size of every encase
   `Params` with naga's size of the WGSL struct. Rust and WGSL still can't drift
   silently, but the guarantee comes from naga's layout, not from encase alone.
3. **`#[diagnostic::on_unimplemented]` does not rewrite the wrong-scale error**
   (§4, risk 11). The `Encoded` impl matches, and rustc then reports the failed
   associated-type equality (E0271) instead of an unimplemented trait. Routing
   the check through a `Produces<V>` marker trait made no difference: rustc sees
   through the blanket impl. The native E0271 message ("expected `Px`, found
   `Color`") is clear anyway. The attribute is stable (no nightly needed) and
   does fire for the E0277 cases, such as a bare `f32` on a `Px` channel.
4. **`ShaderFn::params(&self, cx: &ParamCx)` became three methods**:
   `params(&self)`, `chunk_base(&self, origin: f64) -> f32` (only relative
   inputs call it; `Linear` folds `origin - d0` in f64) and
   `fit_domain(&mut self, extent)` for data-driven domains.
5. **`encode` takes `&self` and clones.** By value it consumed the `ScaleRef`
   handle that zoom and pan need. An inherent `&self` method on `ScaleRef` can't
   fix this: method resolution picks the by-value trait method first.
6. **Risk 4 is real.** With `A: Fn(&T) -> D`, `|d: &Row| d.name.as_str()` fails
   with "lifetime may not live long enough". The proposed
   `for<'a> Fn(&'a T) -> &'a str` bound compiles, so S5 should add the dedicated
   key-encoding entry point (`color_key`/`encode_key`). S0a only needed numeric
   columns, so the north-star doctest uses numbers.
7. **`CircleIn` has one field per channel** (`x`, `y`, `radius`, `fill`) rather
   than §6's `center`, so the emitter stays generic: `m.<channel> = <expr>;`.
   `derive(Mark)` (S6) can then check `CircleIn` against `CHANNELS` mechanically
   (`marks::tests::wgsl_input_struct_matches_channels` already does this by
   composing a probe).
8. **The fallback is not a plain concatenation.** The library uses `as` aliases
   and `{Item}` imports, so the import-only fallback needs a small qualification
   rewrite (prefixing module names), not just concatenation. It is still viable
   because no module uses `#ifdef`.
9. **Fonts.** The bundled default is still the old path's `default.ttf` (Squada
   One, a condensed display face). Inter arrives with GUP-392/S2. That swap will
   re-bless the golden.

### Proposed adjustments to S0b and S1–S3

- **S0b (GUP-396):**
  - Add `PositionScale::set_domain` (today zoom means
    `*x.write() = Linear::new().domain(..)`).
  - Count column bytes in `ColumnStore::upload`, which is the only place columns
    are written.
  - Change `Plot::resolve` to write uniforms into the existing buffers
    (`queue.write_buffer`). Today it rebuilds the uniform buffers and bind
    groups on every resolve, which is fine for PNG but wasteful at 60 fps.
  - The window surface should use a `Bgra8UnormSrgb` surface with a `Bgra8Unorm`
    view, so the ΔE parity check compares like with like.
- **S1:** move `Context` into place as built. Note that each `Context` preloads
  the library (6.6 ms release), so `Context::shared()` matters for
  `save_png`-style calls. The `Mutex` ordering is pipelines → shaders; text is
  never held across either.
- **S2:** replace `gup_core::text` (≈300 lines, internal) with `gup-text`. Keep
  `measure` and the ink bounds that layout metadata uses. Cap-height anchors
  (`VAlign::Top`/`Middle`) worked well for tick labels.
- **S3:** turn `ImageTarget` and `encode_scene` into `RenderTarget` and
  `Renderer::prepare`/`draw`. One render pass, premultiplied output, scissor
  clipping and the non-sRGB target are already in place. MSAA, rects and
  gradients are still to do. Measure the WASM size cost of naga_oil (risk 10)
  here as planned.
- **S5/S6:** fold in assumptions 1–7. The trybuild snapshots match the toolchain
  pinned in `rust-toolchain.toml`, which the dev shell and every CI workflow use
  (GUP-403); re-bless them when that pin moves.

## S0b findings (2026-10-05, GUP-396)

[GUP-396](../stories/GUP-396_Gup_Core_Vertical_Slice_Window_Performance.md)
added the window half of S0: the §7 `RenderTarget` trait with
`Renderer::prepare` → `Prepared::draw`, `WindowTarget`, a minimal
`gup_core::show`, `Context::with_options`, upload counters, and
`PositionScale::set_domain`/`invert` with `Plot::zoom`. Same machine as S0a:
Intel HD Graphics 630 (KBL GT2), Mesa 26.0.0 Vulkan, niri (Wayland) on a
1920×1080 60.02 Hz panel at scale 1, rustc 1.93.1.

With S0a this closes the S0 gate: exit criteria (2) and (3) hold.

### Exit criterion 2: window/PNG parity

`show` with `GUP_SCREENSHOT_PATH` draws one frame into the window's surface,
copies that surface texture back (`COPY_SRC` surface usage) and writes it as
PNG. The surface is `Bgra8UnormSrgb`, rendered through a `Bgra8Unorm` view;
`ImageTarget` renders into `Rgba8Unorm`.
`cargo test -p gup-core --test window_parity -- --ignored --nocapture` compares
the window's 720×450 frame with an `ImageTarget` render of the same `Plot` and
with the golden `tests/golden/gup_core/scatter.png`, using CIEDE2000 from the
GUP-388 harness:

| Comparison                | Pixels  | ΔE max | ΔE mean | ΔE > 0 | ΔE ≥ 2 |
| ------------------------- | ------- | -----: | ------: | -----: | -----: |
| window vs `ImageTarget`   | 324,000 |      0 |       0 |      0 |      0 |
| window vs golden PNG      | 324,000 |      0 |       0 |      0 |      0 |
| control: double sRGB enc. | 324,000 |  28.67 |   0.866 | 16,397 | 16,397 |

The window is byte-identical to the PNG. The control encodes the window's pixels
a second time, which is what an sRGB view would do. It fails the threshold on
every anti-aliased and coloured pixel, so the zero is meaningful. The non-sRGB
view policy (§7) is what makes the window and the PNG equal.

### Exit criterion 3: 100K points at ≥ 60 fps, 0 column bytes

`cargo run -p gup-core --release --example zoom_bench -- …` runs 100,000 points
(linear x, log y, viridis fill, constant radius) in a borderless fullscreen
1920×1080 window. It sets both domains every frame: it zooms to 5% of the extent
and back, twice, over 60 warm-up and 600 measured frames. Each frame takes the
same path as `show`: `Plot::resolve`, `Renderer::prepare`, one pass with
`Prepared::draw`, then `WindowTarget::present`. Frame interval is the `Instant`
difference between consecutive frame starts. GPU time is the render pass
measured by `TIMESTAMP_QUERY` (resolved once after the run).

| Run (radius 3 px unless noted)  | Frame interval median / p95 (ms) | fps median / p95 | > 25 ms | CPU work median / p95 (ms) | GPU pass median / p95 (ms) |
| ------------------------------- | -------------------------------: | ---------------: | ------: | -------------------------: | -------------------------: |
| `Fifo` (vsync), redraw-paced    |                  16.661 / 16.887 |      60.0 / 59.2 |       0 |              0.754 / 0.806 |              3.039 / 5.564 |
| `Fifo`, radius 4.5              |                  16.654 / 16.861 |      60.0 / 59.3 |       0 |              0.768 / 0.894 |              3.058 / 7.477 |
| `Mailbox`, uncapped (no vsync)  |                    4.074 / 6.917 |    245.5 / 144.6 |       0 |              0.647 / 0.783 |              2.674 / 5.854 |
| same, second run                |                    4.327 / 5.477 |    231.1 / 182.6 |       0 |              0.646 / 0.797 |              2.922 / 4.498 |
| `Mailbox`, uncapped, radius 4.5 |                    4.615 / 6.520 |    216.7 / 153.4 |       0 |              0.674 / 0.825 |              3.061 / 5.559 |

- **The target is met with at least 2.4× headroom at p95** (145 fps uncapped).
  Under vsync the median is the panel's refresh (16.66 ms), and no frame missed
  a refresh. Without vsync the same loop runs at a median of 231–245 fps over
  two runs. The p95 varies more, between 145 and 183 fps.
- **It is GPU-bound, and the CPU is idle.** Uncapped, 3.4 ms of the 4.1 ms
  median frame is spent in `acquire`, waiting for a swapchain image. CPU work
  (resolve with ticks and text, prepare, encode, submit) is 0.65 ms. The GPU
  pass is 2.7 ms median and 5.9 ms p95; the p95 frames are the zoomed-out, dense
  ones.
- **Column bytes written during the 600 zoomed frames: 0, in 0 writes.** The
  only column write in the process is the startup upload (1,200,384 B, one
  write). Per frame the run writes 88 B of uniforms in 3 writes (`Encodings` 64
  B, `Chunk` 8 B, view 16 B) and about 9.4 KB of guide instances (axis rules and
  glyph quads, rebuilt because the ticks move). It wrote no textures.
- **How the counter works.** Every GPU write in `gup-core` goes through
  `Context::write_buffer`/`buffer_with_data`/`write_texture`, counted by
  `Upload` kind. `context::tests::every_gpu_write_is_counted` fails if any
  source file calls `queue.write_buffer`, `write_texture`, `create_buffer_init`
  or `create_texture_with_data`, or maps a buffer at creation, directly. The
  headless `tests/zoom_uploads.rs` repeats the claim in CI: 300 zoomed frames of
  100K points into an `ImageTarget` give 0 column bytes and exactly 3 uniform
  writes per frame.

`Sqrt` size is not in S0's scales. Whichever story adds it (S5) should bind
`Circle::RADIUS` through it and re-run `zoom_bench`.

### What changed in the S0a design

1. **Uniforms are written in place.** `Selection` keeps its `LayerGpu`. When the
   program, context, column chunk and LUTs are unchanged, `prepare` only calls
   `write_uniforms`. The `Renderer` keeps the view uniform, the atlas bind group
   and grow-only guide instance buffers. The rescale test asserts that the
   layer's `Arc<LayerGpu>` is reused.
2. **`RenderTarget::present` takes a `CommandBuffer`, as §7 says.** A readback
   target copies the frame out in a second command buffer, submitted in the same
   `submit`. `ImageTarget` and `WindowTarget` share one `Readback`, which
   handles BGRA as well as RGBA.
3. **Zoom is pixel-space and inverted.** `DynPositionScale::zoom` pulls the
   range ends towards the anchor and maps them back with `invert`. That is
   correct for any monotone scale, including log, without per-scale zoom code.
   S9's zoom and pan behaviours can build on `invert`.
4. **Teardown order is a real constraint.** wgpu's GL backend binds its EGL
   display to winit's Wayland connection when a surface is created, even when
   the Vulkan adapter is the one in use. If any wgpu object outlives the event
   loop, dropping the instance segfaults in `eglTerminate`. `show` drops the
   surface, the renderer and the plot's GPU state in
   `ApplicationHandler::exiting`. S8's `GupApp` must keep this rule. A `Context`
   that only ever enables the primary backends would avoid it.
5. **winit paces `RedrawRequested` to frame callbacks** on Wayland, even with a
   `Mailbox` swapchain. To measure without vsync, drive frames from
   `about_to_wait` under `ControlFlow::Poll` (`zoom_bench --uncapped`).
6. **The temporary atlas re-uploaded all 1 MB per new glyph.** It now uploads
   only the dirty rows (2 MB → 18 KB over the headless zoom test). `gup-text`
   (S2) should keep a dirty-rectangle upload.

### Proposed adjustments to S1–S3

- **S1 (`Context`):** keep `with_options` and the upload counters (`UploadStats`
  is cheap: eight relaxed atomics). Make the "every write is counted" test part
  of the crate's contract, so S4's tail appends are counted as `Upload::Column`.
  Consider defaulting `Context::new` to primary backends and keeping GL as an
  explicit fallback (finding 4).
- **S2 (`gup-text`):** keep a dirty-rectangle atlas upload. Text layout of 26
  tick labels and a title is part of the 0.2 ms resolve, so no glyph-run cache
  is needed yet.
- **S3 (`Scene`/`Renderer`/targets):** `RenderTarget`, `Frame`, `Renderer` and
  `Prepared` exist now with the §7 signatures, except that `TargetDesc` has no
  `dpr` yet: dpr is still `desc.width / scene.width`. What remains is
  `TextureTarget`, draw-in-pass for hosts (`Prepared::draw` already takes no
  locks and allocates nothing), MSAA, rects and gradients. `WindowTarget` needs
  no changes for S3. Its wasm canvas path is untested.
- **S8 (`GupApp`/`show`):** start from `gup_core::show`. It already handles
  resize, wheel zoom about the cursor, Escape, `GUP_SCREENSHOT_PATH` and safe
  teardown. winit allows one event loop per process, so `show` runs once. The
  `GupApp` design should decide whether to support re-entry
  (`run_app_on_demand`).

### Orchestrator review of S0b (2026-10-05)

Read the fullscreen window capture (`/tmp/gup396/scatter_window_full.png`,
1920×1052). Layout re-resolves correctly and marks/axes are crisp, but **tick
density does not adapt to available space**: the x axis shows 31 labels
(0–60000, step 2000) nearly touching at 1920 px, while the 720 px golden
shows 13. The tick count must be derived from the axis length in Px and the
measured label width (with a minimum gap), not a fixed target count. **Input for
S7** (`Layout` resolve and guide emitters): add an AC that no two tick labels on
an axis are closer than one em, verified at 400, 720 and 1920 px widths through
the GUP-388 harness's text regions.

## S1 findings (2026-10-05, GUP-399)

[GUP-399](../stories/GUP-399_RFC_001_S1_One_Context.md) made `gup_core::Context`
the one place the old and new paths get a device.

- **`Context::shared()`** is a process default behind a `OnceLock`.
  - It is created with `new_blocking` on the first call. Concurrent first calls
    are serialised, so they make one device.
  - Later calls are an `Arc` clone.
  - If creation fails, the error is returned and the next call tries again.
- **`RenderContext` and `GupContext` are now shims over it.**
  - `RenderContext::new`/`with_viewport`, `GupContext::new`/`headless`/
    `with_options` and `VisualTestUtils::new` take the instance, adapter, device
    and queue from the shared context when it meets the requested `GupOptions`.
  - Otherwise (low power, other backends, features or limits it lacks) they get
    a dedicated `gup_core::Context::with_options`.
  - Pools, multi-surface and recovery are unchanged. `attempt_recovery` still
    requests its own device until S14 deletes it.
  - The root crate depends on `gup-core` (not re-exported). This adds 8 crates
    (gup-core, naga_oil, encase and five helpers) and the `naga-ir` feature on
    the shared wgpu.
- **Backend default: primary, with GL as an explicit fallback.**
  - `ContextOptions::default()` asks for `Backends::PRIMARY` on native and
    `BROWSER_WEBGPU | GL` on wasm. (Superseded 2026-10-09, wasm half only:
    wgpu's `webgl` feature was never enabled, so `GL` here never had a working
    adapter; the wasm default becomes `Backends::BROWSER_WEBGPU` alone. See
    "Decisions (2026-10-09)". The native default, `Backends::PRIMARY` with GL as
    an explicit fallback, is unaffected.)
  - GL is tried only when the requested backends have no adapter and
    `WGPU_BACKEND` is unset.
  - On machines with Vulkan, Metal or DX12, no GL/EGL instance exists, so the
    S0b `eglTerminate` teardown crash (S0b finding 4) cannot happen. It is also
    what the old path already requested.
  - `ContextOptions` gained `backends`, `power_preference`, `required_features`
    and `required_limits`. `TIMESTAMP_QUERY` is optional by default, as the old
    `GupContext` did.
- **Limits.** The device starts from the WebGPU defaults where the adapter
  supports them (downlevel otherwise), then takes the adapter's buffer sizes and
  texture resolution. The old path asked for `Limits::default()`; S0a asked for
  downlevel defaults, which would have lowered the old path's uniform-binding
  and storage-buffer limits.
- **Lock order is checked, not just documented.**
  - Debug builds record which `Context` locks each thread holds and panic on any
    nesting other than `shaders` inside `pipelines`.
  - The whole gup-core suite runs under the check without a violation.
  - `parking_lot`'s deadlock detector was not used: it is a feature flag that
    would change the `parking_lot` wgpu itself uses.
- **`wgpu::Device ==` is not an identity check.**
  - It compares wgpu-core ids, and every `Instance` has its own id space.
  - Devices from two instances are both `Id(0,1)` and compare equal, so a naive
    "same device" assertion would have passed before S1.
  - `tests/one_device.rs` asks wgpu instead, with a bind group made on one
    device from a layout made on the other. Across instances this panics with
    "BindGroupLayout[Id(0,1)] does not exist", the `composite_*` panic.
  - A low-power `GupContext` with its own device is the negative control.
- **The `composite_*` panic is fixed by S1 alone**, but the output is not.
  - All four examples now run windowed without panicking, and their
    expected-failure entry is removed.
  - Their screenshots still show the S11 bugs: bars and areas overflow the plot
    rect, scatter points are black ellipses, and there is no text.
  - `composite_scatter_regression`'s trend line zig-zags because one-`T` layers
    share one data vector. That is S11's mixed-`T` layers.
- **gup-core builds for wasm32 again.** It had not since S0a, because
  `Layer: Send + Sync` and wgpu's web types are `!Sync`.
  - `Layer` is now bounded by `wgpu::WasmNotSendSync`, which is §2's "MaybeSend
    on wasm".
  - Enabling wgpu's `fragile-send-sync-non-atomic-wasm` instead was tried and
    rejected. Through feature unification it made the old path's surfaces
    require `Send + Sync` on wasm, and that broke the root wasm build.
- **One wgpu.** `Cargo.lock` resolves one wgpu 27.0.1, wgpu-core 27.0.3,
  wgpu-hal 27.0.4, wgpu-types 27.0.1 and naga 27.0.3. Before S1 the root and
  gup-core built wgpu with different feature sets into one target directory, and
  29 root doctests failed with "multiple different versions of crate
  `wgpu_types`". After S1 they pass (216 passed, 0 failed).
- **Old-path LOC**: 28882 → 28876 (−6). The `render.rs` device request (−23)
  outweighs the `core_context`/`core_handles` shim in `context.rs` (+17).

### Proposed adjustments to S2–S3 and later

- **S2 (`gup-text`):** give any new `Context`-held lock a rank in the debug
  check (`LockRank`), and keep `text` exclusive. Glyph runs should be laid out
  while holding `text` only, then drawn without it.
- **S3:** the WASM size measurement can run now that gup-core builds for wasm32.
  `ImageTarget`'s readback is native-only (`wait_idle`); the web path needs an
  async readback.
- **S8 (`GupApp`/`show`):** `src/wasm_api.rs` still creates its own device,
  because WebGL adapters need a `compatible_surface`. `Context` needs a way to
  request an adapter for a surface (or `from_wgpu_full`) before the wasm entry
  point can share it.
- **S13 (hosts):** never use `Device ==` to decide whether a host device is
  Gup's. Compare `ContextId`s, or wrap the host device once with `from_wgpu`.

## S2 findings (2026-10-06, GUP-400)

[GUP-400](../stories/GUP-400_RFC_001_S2_Extract_Gup_Text.md) extracted
`crates/gup-text`, a leaf crate that depends only on wgpu, fontdue, bytemuck and
thiserror. It replaced gup-core's temporary text module.

- **One `TextSystem` per `Context`.** It lives under the existing
  `LockRank::Text` lock, so S2 adds no new lock. `Renderer::prepare` lays out
  and prepares a text item while holding only that lock. The resulting
  `GlyphBatch` draws without it, allocates nothing and takes no locks.
- **The measuring API is unchanged for the harness.** `measure` returns width,
  cap height and descent; `ink_bounds` takes a run. `Layout`'s `PlacedText`
  keeps its shape. `Anchor`, `HAlign` and `VAlign` now live in `gup-text`, and
  `gup_core::scene` re-exports them.
- **Uploads are still counted.** gup-text writes through an `Uploader` trait,
  which gup-core implements with its counters. The atlas uploads only the dirty
  rectangle, packed tight (S0b finding 6): a 60-frame zoom wrote 220 B of texels
  in 2 writes.
- **Decision: bitmap rasterisation, no MSDF.** Glyphs are rasterised at
  `size × dpr` and drawn 1:1 on whole physical pixels. This is sharp at any dpr
  for the horizontal, fixed-size text every current caller draws. `msdf.rs` and
  `sdf_tuning.rs` (over 3,400 lines) were not ported. MSDF returns when a caller
  needs free rotation or continuously scaled text. 90° axis titles can use
  rotated bitmap quads.
- **Inter Regular 4.1 is the default face** (SIL OFL 1.1, in
  `crates/gup-text/fonts/`). The scatter golden was re-blessed and checked by
  eye, and the window frame is still byte-identical to it. The old path keeps
  Squada One (`assets/fonts/default.ttf` is unchanged). Two limits:
  - fontdue reads only the legacy `kern` table, and Inter has none, so text is
    unkerned.
  - Inter's default digits are proportional; tabular figures are the `tnum`
    feature.

  GUP-405 adds rustybuzz shaping for both.

- **Parsing Inter is not free**: 18 ms in release, 120 ms in debug, because
  fontdue parses every outline. It is parsed lazily, once per process, so a
  `Context` that never draws text does not pay for it.

### Proposed adjustments to S3 and later

- **S3 WASM budget:** the bundled Inter is 411 KB, or 194 KB gzipped. That is
  about half of the ≤ +400 KB gz budget (risk 10) before any code is counted.
  Report the font separately in the measurement. If the budget is tight, subset
  Inter to Latin and the symbols labels use. Squada One was 9 KB gzipped.
- **S3 `Renderer`:** a `GlyphBatch` binds its atlas at group 0 and carries
  clip-space quads, so it needs no view uniform. `Prepared::draw` rebinds the
  view for each mark and rule draw. When `TargetDesc` gains `dpr`, pass it to
  `Glyphs::new`/`clear`; today it is `desc.width / scene.width`. Text pipelines
  are cached in the `TextSystem` by `(format, samples)`, so MSAA needs no extra
  work in gup-text.
- **S3 `SvgTarget`:** measure SVG text with the same `gup_text::Font` so that
  layout matches. Name the font family `Inter`, and decide whether to embed it
  (`@font-face`, 194 KB gzipped) or accept a fallback face in viewers without
  Inter.
- **S7 (layout, guides):** the tick-density rule from the S0b review needs
  stable label widths. With proportional digits a label's width changes with its
  value. GUP-405's `tnum` fixes that.

## S3 findings (2026-10-06, GUP-401)

[GUP-401](../stories/GUP-401_RFC_001_S3_Scene_Renderer_RenderTarget.md)
completed §7's target family. Same machine as S0a/S0b (Intel HD Graphics 630,
Mesa 26.0.0 Vulkan, rustc 1.93.1) unless noted.

- **Targets.** `RenderTarget` is implemented by `ImageTarget`, `WindowTarget`
  and the new `TextureTarget` (a host-owned texture; Gup clears, draws, resolves
  and submits). Hosts that own the pass use `Renderer::prepare` and
  `Prepared::draw`. `Frame::color_attachment` builds the pass attachment
  (multisampled view plus resolve target), so the one `Renderer::render` call
  site and `zoom_bench` share it.
- **Equivalence.** `tests/targets.rs` draws the reference scatter through
  `ImageTarget`, `TextureTarget` and a host's own pass. The three are
  byte-identical at 1 and at 4 samples. The window frame (4×) is still
  byte-identical to the 4× `ImageTarget` and the golden (ΔE 0 on 324,000 px).
- **Gup never submits in draw-in-pass mode, provably.** Every gup-core submit
  goes through `Context::submit`, counted by `Context::submissions`.
  `every_submit_is_counted` enforces this the way `every_gpu_write_is_counted`
  does for uploads. The draw-in-pass test asserts the count does not move.
- **`TargetDesc` has `dpr`** and passes it to the glyph rasteriser. A dpr-2
  render rasterises the title at twice the size, not upscaled. Pipelines are
  keyed by `(kind, format, samples)`; dpr and size are uniforms only (tested).
  Pick variants (S9) use one sample; this is documented on `TargetDesc`.
- **MSAA 4× is the default** on Gup-owned targets (`TargetOptions`,
  `WindowTarget::set_samples`). It matters only for geometric edges. On a fan of
  1.5 px diagonal rules, 1× has 0 partially covered pixels (hard stair steps)
  and 4× has 597. On a rect at fractional coordinates the counts are 0 and 71.
  Circles antialias analytically, so 1× and 4× differ by at most 2/255 on their
  edges. Goldens: `tests/golden/gup_core/msaa_{1x,4x}.png`.
- **Rects and gradients.** `ItemKind::Rects` draws flat rectangles.
  `ItemKind::Gradient` draws a legend bar through the same
  `gup::color::sequential::map` and the scale's own LUT (an `Arc`, not a copy).
  `GradientBar::color_at` and the scale's CPU mirror share one LUT sampler.
  Golden: `gup_core/scene_items.png`, a tinted plot background under the marks
  and a viridis legend whose ends match the fill's domain extremes within ΔE 2.
- **`SvgTarget` (guides only).** It writes rules (`<line>`, square caps), rects,
  gradients (`<linearGradient>`, 64 stops), text (`<text>`) and `<clipPath>`. A
  scene with marks is an error that names `Scene::guides()`. Text is measured
  with the same `gup_text::Font` (`Font::baseline_origin`, now shared with the
  glyph walk) and snapped to whole pixels as on the GPU. Without the snap, 151
  pixels fell outside the PNG's text boxes; with it, all fall within 1 px. The
  SVG rasterised by resvg (with only Inter loaded) passes the GUP-388 structural
  checks against gup-core's own layout. Outside text it matches the GPU PNG to
  ΔE ≤ 0.77, and each label's ink is within ×0.96–×1.02 of the PNG's.
- **Font decision: referenced, not embedded.** SVG text uses
  `font-family="Inter, sans-serif"`, and `text-anchor` keeps fallback faces
  centred and right-aligned. Embedding the full face would add ~550 KB of base64
  to every file. A subset (24 KB gz, below) makes embedding reasonable; that is
  GUP-407.

### WASM size (§12 risk 10)

`mask wasm-size` builds three harnesses in `crates/gup-core/wasm-size` for
`wasm32-unknown-unknown`. They use the release profile (the workspace has no
custom one), then `wasm-bindgen --target web` without name or producers
sections, then `gzip -9`. Each harness is its own workspace, so gup-core's
features (wgpu `naga-ir`) cannot leak into the baselines:

| Build                                                               | Raw      | gzip -9       |
| ------------------------------------------------------------------- | -------- | ------------- |
| bare wgpu: instanced discs, uniform, offscreen pass, async readback | 111.8 KB | 41.7 KB       |
| the same, its shader composed through naga_oil (`--features`)       | 3,297 KB | 938.5 KB      |
| gup-core: reference scatter → `ImageTarget` (async) + `SvgTarget`   | 3,996 KB | 1,241.9 KB    |
| _bundled Inter, for reference_                                      | 411.6 KB | 198.3 KB      |
| **naga_oil-attributable (row 2 − row 1)**                           |          | **+896.7 KB** |

- **The budget (≤ +400 KB gz) is exceeded by 2.2×.** This needs a decision on
  the §6 fallback (GUP-406). `wasm-opt -Oz` (binaryen 129) shrinks raw size by
  14–15% but leaves gzip unchanged or slightly larger, so it is no way out.
- **Method.** Row 2 differs from row 1 only in passing the same shader through
  naga_oil (a library module plus a top-level shader, with codespan error
  reporting, as gup-core does) and giving wgpu a `ShaderSource::Naga`. The delta
  is everything that path brings: naga's WGSL front end, validator, compactor
  and WGSL back end (wgpu's WebGPU backend writes WGSL for the browser), plus
  naga_oil and its preprocessor. twiggy over the build with names splits the raw
  code as follows:
  - naga: 825 KB, plus 237 KB of `arrayvec` monomorphisations;
  - the regex family (regex-automata, regex-syntax, aho-corasick): 570 KB;
  - naga_oil: 136 KB;
  - data-encoding: 73 KB;
  - codespan-reporting: 47 KB;
  - `.rodata` (Unicode tables and more): ~600 KB.
- **The rest of gup-core is small.** Row 3 − row 2 = 303 KB gz, of which Inter
  is ~198 KB and gup-core, gup-text, fontdue and encase ~105 KB.
- **One cheap win was taken.** naga_oil's default `glsl` feature put naga's GLSL
  front and back ends into gup-core, which composes WGSL only.
  `default-features = false` cut the scatter from 1,431.6 to 1,241.2 KB gz.
- **Inter subset.** Basic Latin, Latin-1, dashes, quotes, arrows, math symbols,
  µ and €, keeping `kern`, `tnum` and `lnum` (pyftsubset), is 47.6 KB raw and
  23.8 KB gz, against 198.3 KB gz for the full face. That saves ~175 KB gz
  (GUP-407).

### gup-core in a browser

The scatter harness runs in headless Chromium through WebGPU
(`mask wasm-browser`): `Context::new`, MSAA, the async `ImageTarget` readback
and `SvgTarget`. It found two bugs that no native test could:

1. **`std::time::Instant::now` panics on wasm32-unknown-unknown.** The pipeline
   and compose timings now use `web-time`.
2. **The uniform layout differed in the browser.** wgpu hands browsers naga's
   WGSL output, which drops the glue's `@align(16)` attributes. Chrome
   (`uniform_buffer_standard_layout`) accepted the natural offsets, so the
   shader read the circle radius from the y scale's parameters and flooded the
   plot. Padding members cannot fix this: naga, like WGSL, requires
   `roundUp(16, size)` bytes after a struct member. **New authoring rule:**
   every uniform `Params` struct spans a multiple of 16 bytes, padded in WGSL
   and in its encase twin. The emitter asserts it and pads scalar constants with
   `u32`s. A unit test re-parses naga's WGSL output and requires every struct to
   lay out as composed.

Since GUP-408 this runs on every push, as the Visual regression workflow's
`browser` job, in Chrome for Testing on SwiftShader (Chrome's bundled software
Vulkan). Headless Chromium had been choosing SwiftShader locally too, so both
bugs above were found without a GPU; the test now forces that adapter and
records it. It fails on a console error, an uncaught exception or a WebGPU/WGSL
diagnostic as well as a bad render. Chrome compiles WGSL with its own compiler,
which is stricter than naga in places: a derivative in non-uniform control flow
passes naga and every native test but fails in Chrome.

### Proposed adjustments to S4 and later

- **Decided (2026-10-09), superseding this proposal:** the owner chose
  build-time composition on every target, not a wasm-only runtime fallback that
  keeps naga_oil at runtime on native. See "Decisions (2026-10-09)" and the
  rewritten [GUP-406](../stories/GUP-406_WGSL_Only_Shader_Path_On_Wasm.md).
- **S5 (`ShaderFn` v2, `#[wgsl_function]`):** enforce the 16-byte `Params` rule
  at macro expansion, alongside the no-trailing-digit identifier rule.
- **S4/S5 (`MarkBatch::vector()`):** `SvgTarget` already writes every guide
  kind. Mark export plugs into the `ItemKind::Marks` arm, which errors today.
- **S7 (layout):** a long title overflows a narrow chart (the 320 px browser
  render clips "…expectancy"). Layout should wrap, shrink or ellipsise it.
  Legends need a layout slot; the S3 test makes room by resolving the plot
  narrower than the scene.
- **S8 (`GupApp`, wasm entry):** start from the `wasm-size/scatter` harness. It
  is the first gup-core code proven in a browser.
- **S13 (hosts):** use `TextureTarget` for egui images and `Prepared::draw` for
  paint callbacks. Pass the host attachment's sample count in `TargetDesc`.

---

## Decisions (2026-10-09)

Two owner decisions, made after the S3 findings above and closing §12 risk 1
(naga_oil vs wgpu cadence) and risk 10 (WASM budget). Each supersedes specific
RFC text, marked inline above at the point it applies.

### Decision: shader composition moves to build time

S3 findings measured naga_oil's wasm cost at +897 KB gz against the ≤ +400 KB gz
budget (§12 risk 10) — 2.2× over. Three options were on the table:

(a) **Concatenate on wasm only**, keeping naga_oil at runtime on native (the
design S3 proposed and GUP-406 originally planned to build). Rejected: it is a
second composition path living next to the first, and this project's history is
that parallel systems never get deleted on their own (four scale systems, three
composition systems, three pipeline caches — Orchestrator review point 1). A
wasm-only fallback recreates exactly the pattern RFC-001 exists to end. (b)
**Concatenate everywhere**, with naga_oil moved to build time on every target.
One path, no parallel system. (c) **Accept the size.** Rejected outright: +897
KB gz is 2.2× the budget, and the budget itself was already a compromise (§12
risk 10).

The owner chose **(b)**:

- **Library modules are flattened at build time.** naga_oil runs in a build
  script (or a small build-time crate gup-core depends on at build time only)
  and flattens the library's WGSL modules (§6) — namespaced and mangled, the
  same transform naga_oil already does at runtime today — into plain WGSL text,
  checked into the build output, not into `gup-core`'s runtime `Context`.
- **At runtime, only the typed glue module is built**, exactly as §6's emitter
  already does (a ~300-line Rust emitter printing one top-level module per
  `(mark, encoding signature)` pair). That glue text is concatenated with the
  pre-flattened library text and handed to wgpu as `ShaderSource::Wgsl`. **One
  path runs on every target**: naga_oil is a build-time-only dependency of
  `gup-core`; it is never linked into a `gup-core` binary, native or wasm.
- **`#[wgsl_function]`** (§4, §6) user modules are flattened the same way, at
  macro-expansion time rather than at runtime, consistent with its existing
  compile-time WGSL validation.
- **Validation** is naga_oil's, at build time (source-mapped errors, as S0a
  demonstrated), plus wgpu's own validation at pipeline creation — through its
  internal naga on native, through the browser's own WGSL parser on WebGPU. No
  separate runtime validation step is needed, and none is added.

This supersedes, at the points marked above: §2's `shaders: Mutex<Composer>`
field; §6's "Recommendation" (naga_oil preloaded per `Context` at runtime, with
import-only concatenation as a conditional wasm fallback) and its "Fallback"
bullet; §12 risk 1's mitigation (an internal `Composer` trait choosing naga_oil
or concatenation at runtime) and risk 10's mitigation (concatenate on wasm only
if the budget is missed); and the S3 findings' "Decision needed (GUP-406)"
proposal.

[GUP-406](../stories/GUP-406_WGSL_Only_Shader_Path_On_Wasm.md) is rewritten to
this design.

### Decision: WASM targets WebGPU only, for now

S1 findings set `ContextOptions::default()`'s wasm backends to
`BROWSER_WEBGPU | GL`, following §1's north star of running everywhere. But
wgpu's `webgl` feature has never been enabled on `gup-core`, so the `GL` half of
that default has never had a working adapter behind it on wasm32 — the default
has always resolved to WebGPU alone in practice. Enabling `webgl` for real would
bring naga's WGSL front end and validator back into the wasm build regardless of
the decision above, because wgpu's GL backend lowers WGSL to GLSL through naga;
that cost would apply to a path nothing currently exercises.

The owner decided: **wasm targets WebGPU only, for now.**
`ContextOptions::default()`'s wasm backends become `Backends::BROWSER_WEBGPU`
alone, dropping `GL`. Revisit if a user needs a browser without WebGPU (for
example Safari before its WebGPU release, or an older browser).

This supersedes, at the point marked above, the wasm half of S1 findings'
"Backend default: primary, with GL as an explicit fallback" bullet. The native
default (`Backends::PRIMARY` with GL as an explicit fallback) is unaffected.

## GUP-406 findings (2026-10-09)

[GUP-406](../stories/GUP-406_WGSL_Only_Shader_Path_On_Wasm.md) moved shader
composition to build time on every target, as decided above. Same machine as
S0a–S3 (Intel HD Graphics 630, Mesa 26.0.0 Vulkan, rustc 1.93.1).

### What runs where

- **`gup-wgsl`** is a new crate with two halves. Without features it is the
  run-time half: `WgslModule` (an import path, flattened WGSL and its imports)
  and `link`, plain string handling with no dependencies. Its `compose` feature
  is the build-time half: naga_oil and naga.
- **`gup-core/build.rs`** reads `src/shaders` through `compose`. Each library
  module (a file with `#define_import_path`) becomes a `WgslModule` static in
  `$OUT_DIR` (`gup::scale::linear` → `SCALE_LINEAR`). Each other file becomes a
  completely flattened `<STEM>_SHADER` constant (the rule, rect and gradient
  guide shaders). Adding a module is adding a file; nothing is registered by
  hand.
- **At run time** the glue emitter is unchanged (`scatter_glue.wgsl` is
  byte-identical). `link` resolves its naga_oil-style `#import` lines against
  the flattened modules and appends them. wgpu gets `ShaderSource::Wgsl` on
  every target, and the `naga-ir` wgpu feature is gone.
  `cargo tree -p gup-core -e normal` has no naga_oil on either target, and no
  naga on wasm32. On native, naga appears only under wgpu-core and wgpu-hal
  (wgpu's own validation).
- **`Context` lost its shader-library mutex** and the lock order that guarded
  it. `pipelines` and `text` are each taken alone.
- **Uniform offsets come from the glue emitter.** Every `Encodings` field starts
  on a 16-byte boundary, so offsets follow from encase sizes. A native test
  checks them against naga's layout of the linked reference glue, and another
  checks the linked glue against naga_oil composing it directly: same entry
  points, all 9 struct layouts equal.

### Flattening: naga's writer, checked at build time

`naga_oil` returns a `naga::Module`, not text, so printing flattened WGSL needs
naga's WGSL writer. wgpu's WebGPU backend already used that writer for every
gup-core shader in S3. Items are renamed in the IR from `naga_oil`'s decorated
names to `gup_wgsl::flat_name(path, item)`, which replaces each `::` with an
underscore and appends the item (`gup_scale_linear_map_rel`). These names are
gup-wgsl's own contract, not naga_oil's internal mangling, and they survive
naga's namer. The build checks, for every module and guide shader:

- the flattened text, with its imports', parses and validates standalone;
- every struct has the same span, member offsets and member names as in
  naga_oil's composed module, so a dropped `@align`/`@size` fails the build
  (seeded test), as does a member the writer renames. naga renames identifiers
  that end in a digit: `rule.wgsl`'s `p0`/`p1` became `start`/`stop`;
- uniform `Params` structs span a multiple of 16 bytes;
- library modules use only `#define_import_path`/`#import`, since they are
  flattened once without shader defs;
- no two items share a flat name, and no decorated name survives renaming.

naga_oil itself rejects trailing-digit exported items and struct members (not
function arguments, which the writer renames harmlessly) and wrong-arity calls,
with source-mapped reports. A broken `circle.wgsl` fails `cargo build`:

```text
error: gup-core@0.1.0: gup-core's WGSL failed to compose at build time (src/shaders/circle.wgsl); naga_oil's report follows under stderr
...
  --- stderr
  shader composition failed for src/shaders/circle.wgsl:
  error: failed to build a valid final module: Function [2] 'gup::marks::circle::vertex' is invalid
     ┌─ src/shaders/circle.wgsl:43:1
  48 │ │     out.clip = gup::view::px_to_clip(vec2<f32>(m.x, m.y) + offset);
     │ │                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ invalid function call
     = Call to [0] is invalid
     = Requires 2 arguments, but 1 are provided
```

naga_oil only imports items a shader names. The flattening probe passes every
item, found by a lexical scan of the module's declarations, through
`NagaModuleDescriptor::additional_imports`.

### WASM size (§12 risk 10)

`mask wasm-size` now builds two harnesses; the naga_oil row is gone, because
nothing composes at run time any more. The scatter harness also draws a plot
background and a legend bar (below).

| Build                                                  | Raw      | gzip -9       |
| ------------------------------------------------------ | -------- | ------------- |
| bare wgpu (unchanged)                                  | 111.8 KB | 41.7 KB       |
| gup-core reference scatter (S3)                        | 3,996 KB | 1,241.9 KB    |
| **gup-core reference scatter (GUP-406)**               | 903.0 KB | **392.9 KB**  |
| _bundled Inter, for reference_                         | 411.6 KB | 198.3 KB      |
| **gup-core over bare wgpu**                            | 791.2 KB | **+351.1 KB** |
| gup-core over bare wgpu, Inter excluded (budget basis) |          | **+152.9 KB** |

The +400 KB gz budget holds with Inter counted, and with 247 KB to spare without
it. GUP-407's Inter subset (23.8 KB gz) would bring the scatter to about 218 KB
gz.

### Pipeline creation (§12 risk 2)

The S0a harness (`pipeline_timings`, release, 20 runs with a fresh cache):

| Step (ms)                                         | S0a median | GUP-406 median | GUP-406 max |
| ------------------------------------------------- | ---------: | -------------: | ----------: |
| Glue emit                                         |      0.017 |          0.016 |       0.023 |
| naga_oil compose → `gup_wgsl::link`               |       2.28 |          0.112 |       0.115 |
| `create_shader_module` + `create_render_pipeline` |       0.42 |          0.815 |        8.28 |
| **Compose/link + create**                         |   **2.75** |       **0.93** |    **8.39** |
| `Context` creation (library preload in S0a)       |       6.55 |          0.031 |       0.361 |

Linking is 20× cheaper than composing. Creation itself costs more (0.82 against
0.42 ms) because wgpu now parses WGSL where it used to receive naga IR. The
first, cold run is 8.4 ms against 13.5 ms.

### In the browser

`mask wasm-browser` passes. The harness now also draws a tinted plot background
(`ItemKind::Rects`) and a viridis legend bar (`ItemKind::Gradient`), so all five
pipeline kinds run in Chrome over WebGPU. The PNG shows the title, log y ticks,
linear x ticks, viridis circles, the tinted background and the legend bar
(purple at the bottom, yellow at the top). naga's writer adds
`@interpolate(flat)` to integer vertex _inputs_ (`GradientIn`); Chrome accepts
it.

### Proposed adjustments to S4 and later

- **S4 (column store v1):** unaffected. If a new column format needs a new
  entry-point shape (dictionary codes, validity bits), that is a glue-emitter
  change; the linker and the build step need nothing.
- **S5/S6 (the scale family, more marks):** a new scale, palette or mark is a
  `.wgsl` file in `src/shaders`; `build.rs` generates its static
  (`SCALE_<NAME>`, `MARKS_<NAME>`). Follow the authoring rules above, which now
  fail the build rather than a test. No composer work is needed.
- **S5 (`#[wgsl_function]`):** the proc-macro crate depends on `gup-wgsl` with
  `compose` and flattens the user module with `Library::new` at expansion,
  emitting a `WgslModule` static with the flat text. That gives the 16-byte and
  identifier checks for free. Three things to settle:
  1. **The import path.** A proc macro cannot evaluate `module_path!()`, and
     flat names need the path at expansion. Default to
     `<CARGO_CRATE_NAME>::<fn name>`, with a `path = "…"` override. `link`
     already refuses two different modules with one import path, so a collision
     fails at link time with a clear message, not inside wgpu.
  2. **Library sources at expansion.** A user module that imports `gup::view`
     needs that module's source to compose against. Move `src/shaders` (or its
     library half) into a crate both `gup-core/build.rs` and the macro can
     `include_str!` from: `gup-wgsl` itself, or a small `gup-shaders` data
     crate.
  3. **Error spans.** `compose::Error` reports point into the WGSL string. Emit
     them with `compile_error!` at the attribute's span, keeping naga_oil's
     report text.
  4. **The calling convention.** Check the entry function's signature against
     what the glue calls
     (`(In, [base: f32,] Params[, texture, sampler]) -> Out`) at expansion.
     naga_oil used to catch a mismatch when composing the glue at run time; now
     it would surface only as a wgpu validation error at pipeline creation.
- **S6 (`derive(Mark)`):** the generated `<NAME>In` check
  (`marks::tests::wgsl_input_struct_matches_channels`) now reads the flattened
  `gup_marks_<name>_<Name>In` struct. A derive can check channel order against
  the flattened text at build time instead of in a test.
- **Run-time errors.** `link` catches unknown modules and items in generated
  glue. Type errors in glue (a generator bug) now surface from wgpu's validation
  at pipeline creation (naga inside wgpu on native, the browser's compiler on
  wasm), not from naga_oil. No extra validation step was added, as decided.
- **S8 (wasm entry):** starts from 393 KB gz (about 218 KB with GUP-407).
- **GUP-408 (browser CI):** keep the legend and background in the harness, so CI
  runs every pipeline kind.

## GUP-410 findings (2026-10-10)

GPU errors from gup-core are now `Error::Gpu { what, message }` on every target,
not a native panic and not a silent blank image in a browser.

### Design

- **Where the scopes are.** `Context::scoped(what, f)` pushes validation and
  out-of-memory scopes (and internal, natively), runs `f` and pops them. It
  wraps glue program creation, mark and guide pipeline creation, the layer step
  of `Plot::resolve` (column upload, uniforms, bind groups),
  `Renderer::prepare`, `Renderer::render` (acquire, encode, present and submit),
  `ImageTarget` and `TextureTarget` creation, and `WindowTarget` surface
  configuration (`resize` now returns `Result`). Scopes nest: a pipeline's own
  scope names it, and the render scope names the target.
- **Pop timing.** wgpu-core resolves a popped scope immediately
  (`ready(scope.error)`), so natively the error comes back from the call that
  caused it, with no round trip. In a browser the pop is a promise. The scope is
  spawned (`wasm_bindgen_futures::spawn_local` into a oneshot), the next
  synchronous call on the context reports whatever has arrived, and
  `ImageTarget::read` (so `ImageTarget::render`) and
  `WindowTarget::take_capture` await every pending scope. So an awaited browser
  render returns its own error. A synchronous render into a texture or window
  reports it from the next frame.
- **No invalid pipeline is cached.** A cache miss creates and inserts inside the
  scope. Any scope error makes the context forget its programs and pipelines,
  and gup-text's (`TextSystem::forget_pipelines`), so the next render recreates
  them and reports the error again.
- **Threads.** wgpu-core keeps one scope stack per device, shared by threads and
  by every `Context` wrapping that device, so concurrent scopes would catch each
  other's errors. The outermost scope on a thread holds a process-wide lock
  until it pops. It is always taken before a context lock (debug builds check
  this). Cache misses no longer hold the pipeline-cache lock while they create.
- **Uncaptured errors.** On a device Gup creates in a browser, an
  `on_uncaptured_error` handler records errors outside every scope, and the next
  call reports them. Native devices keep wgpu's panic, and host devices
  (`from_wgpu`) keep the host's handler. wgpu's WebGPU backend panics on a
  `GPUInternalError` (`Error::from_js`), so browser scopes do not filter
  `Internal`.

### Cost

- **An empty scope** (all filters pushed and popped): about 250 ns natively,
  release build (`scope::tests::scope_cost`). A cached frame opens three (layer
  resolution, render, prepare).
- **`zoom_bench`, Mailbox uncapped, 100K points**: CPU work median 0.787, 0.794
  and 0.798 ms before, and 0.782, 0.800 and 0.776 ms after, in alternating runs.
  That is within noise.
- **Pipeline creation** (`pipeline_timings`, release): create median 0.739 ms
  before and 0.719 ms after.
- **WASM**: the reference scatter grew from 392,954 to 404,121 B gz (+11.2 KB
  gz, +37.7 KB raw) for the scopes, the spawned tasks and the error messages.
  gup-core over bare wgpu is now 362.4 KB gz, under the +400 KB budget.

### Proofs

- **Browser**: with GUP-408's seed 2 (`dpdx` in non-uniform control flow in
  `rect.wgsl`), `mask wasm-browser` fails with the page's own line:

  ```text
  GUP FAIL GPU error in guide pipeline `gup rects` (Rgba8Unorm, 4× MSAA):
  Error while parsing WGSL: :43:19 error: 'dpdx' must only be called from
  uniform control flow
  ```

  Chrome no longer logs the four "rendering" warnings, because the scopes
  capture the errors. Without the seed the page passes with unchanged pixel
  counts (white=30884 grey=16219 coloured=16897).

- **Native**: `scope::tests` seed a pipeline layout without group 2 (an `Err`
  from `ImageTarget::render` twice, never cached, after which the valid scene
  renders), broken glue WGSL (an `Err` from `Renderer::prepare` with naga's
  diagnostic) and a frame without `RENDER_ATTACHMENT` (an `Err` from
  `Renderer::render`).

### Proposed adjustments to S5 and later

- **S5 (`#[wgsl_function]`):** a user function that composes at expansion but
  fails pipeline creation (a calling-convention mismatch the macro missed, a
  browser-only uniformity error like seed 2) is an `Error::Gpu` naming the mark
  pipeline and its glue signature. Keep the user module's import path in the
  glue signature, so that the message names the user's function. Item 4 above
  (checking the calling convention at expansion) is still worth doing:
  expansion-time errors carry spans, and `Error::Gpu` messages point into the
  linked text. In a browser, test user WGSL through an awaited
  `ImageTarget::render`, which returns the error itself.
- **S8 (wasm entry):** awaited renders report their own errors. A canvas
  (`WindowTarget`) render loop reports a frame's error from the next frame, so
  the entry point should surface `render`'s `Err` (or await a readback once
  after the first frame). It starts from 404 KB gz.

## GUP-407 findings (2026-10-10)

[GUP-407](../stories/GUP-407_Subset_Inter_And_SVG_Font_Embedding.md) replaced
the bundled Inter with a subset and let `SvgTarget` embed it.

### The subset

- **Characters** (`crates/gup-text/fonts/inter-subset.txt`, 515): printable
  ASCII, Latin-1, Latin Extended-A and the Romanian comma-below letters, the
  basic Greek alphabet, typographic spaces, dashes, quotes, bullets, ellipsis,
  per mille and primes, superscript and subscript digits, 15 currency signs,
  letterlike symbols (℃ ℉ ℓ № ™ Ω K Å), vulgar fractions, ten arrows (← ↑ → ↓
  ↔ ↕ and the diagonals), common maths operators (U+2212 minus, ≈ ≠ ≤ ≥ √ ∞ ∑
  ∂ ∫), and the geometric shapes, stars and marks a legend uses. That is wider
  than the S3 estimate (Latin-1 only), for Central and Eastern European labels
  and Greek symbols.
- **Features**: GPOS `kern` and GSUB `tnum` only (Inter has no `lnum`). Every
  other feature (`calt`, `case`, `ccmp`, `locl`, `mark`, `mkmk`, `frac`, `sups`,
  `ss*`, `cv*`, `zero`, …) and all hinting are dropped. fontdue and resvg do not
  hint, and the glyphs are the full face's: a test rasterises all 515 characters
  at 11 and 16 px from both faces and compares the bitmaps.
- **Out-of-subset characters** draw as Inter's `.notdef` box, kept with
  `--notdef-outline` (pyftsubset otherwise empties it, so they would vanish).
  `Font::missing_glyphs` lists them; `Font::inter_full` is the full face for
  other scripts. There is no font fallback.
- **Recipe**: `mask subset-inter`, with fonttools 4.61.1 from the dev shell
  (pinned by `flake.lock`). The output is byte-for-byte reproducible. Inter's
  OFL declares no Reserved Font Name, so the subset keeps the name "Inter"; it
  keeps the copyright, trademark and licence name records.

### WASM size (§12 risk 10)

| Build                                   | Raw      | gzip -9       |
| --------------------------------------- | -------- | ------------- |
| bare wgpu (unchanged)                   | 111.8 KB | 41.8 KB       |
| gup-core reference scatter (GUP-410)    | 941.7 KB | 404.2 KB      |
| **gup-core reference scatter (subset)** | 594.7 KB | **238.5 KB**  |
| bundled Inter, full face                | 411.6 KB | 198.3 KB      |
| **bundled Inter, subset**               | 61.3 KB  | **30.2 KB**   |
| **gup-core over bare wgpu**             | 483.0 KB | **+196.7 KB** |

The scatter is 165.7 KB gz (41%) smaller. `INTER_REGULAR_FULL` is not linked
unless a program uses it. The subset (593 glyphs, with the `tnum` figures) is 6
KB gz larger than the S3 estimate: without Latin Extended-A it is 26.5 KB gz,
without Greek 26.6 KB gz.

### SVG

`SvgOptions { font, embed_font }` replaces `SvgTarget::with_font`. With
`embed_font`, a scene with text gets one
`@font-face{font-family:"Inter";src:url(data:font/ttf;base64,…)}` in `<defs>`:
82 KB more per file, 37 KB gzipped. Chromium draws the embedded face. resvg
ignores `@font-face`, so a resvg user still loads `INTER_REGULAR` into its font
database.

### Proposed adjustments

- **GUP-405 (shaping):** `kern` and `tnum` are in the subset; shaping with
  rustybuzz's default features finds no `calt`, `ccmp`, `locl`, `mark` or
  `mkmk`. The subset has precomposed letters and no combining marks, so only
  contextual alternates (`calt`) and localised forms (`locl`) are lost. Inter's
  `case` (raised hyphens and brackets between capitals) and `zero` (slashed
  zero) would each need adding to the recipe's `--layout-features`.
- **S8 (wasm entry):** starts from 238 KB gz.

## S4a findings (2026-10-10, GUP-414)

[GUP-414](../stories/GUP-414_RFC_001_S4a_Column_Store_Chunking_Append.md) made
the column store multi-chunk and added a counted tail append. Same machine as
S0a–S3 (Intel HD Graphics 630, Mesa 26.0.0 Vulkan, rustc 1.93.1) unless noted.

### Chunks

- **Size.** `ColumnStore::chunk_rows_for(limits, formats)` is
  `min(2^20, max_buffer_size / Σ stride)`, rounded down to whole 64-row blocks
  (256 bytes of a 4-byte column), so a full chunk has no padding. With default
  limits and the reference scatter's three columns it is 2^20. `Selection` reads
  the limits from the `Context` it resolves on and rebuilds its store if they
  give a different chunk size.
- **Capacity.** Every chunk but the last is full. A chunk built from data has
  room for its rows rounded up to a 64-row block, which is exactly the padding
  the S0a layout already had, so a store that fits one chunk has the S0a bytes,
  origins and stats (checked against a verbatim copy of the S0a layout for 1 to
  4096 rows). Each chunk has its own buffer, f64 origin per relative column (its
  first finite value; set by the first finite append if a chunk starts with
  nulls) and stats. `ColumnStore::stats(k)` merges them for domain fitting.
- **Test seam.** `Selection::max_chunk_rows(n)` caps the size below the
  device's. It is `pub` and `#[doc(hidden)]`: the integration tests, the wasm
  harness and `zoom_bench --chunk-rows` are other crates, so `#[cfg(test)]`
  cannot reach them.

### Drawing

`LayerGpu` is now an `Arc`'d `Encodings` half (uniform buffer, LUTs, bind group)
and a chunk half: one `Chunk` uniform buffer with an entry per chunk, 256 bytes
apart and written in one `write_buffer` (trimmed to `(n − 1) × 256 + span`
bytes), and one `ChunkDraw` per chunk (buffer, column ranges, instance count,
dynamic offset). `Prepared::draw` sets bind group 2 at each chunk's offset,
binds its column ranges and draws its instances. It still allocates nothing. Per
frame a single chunk writes 88 B of uniforms in 3 writes, as in S0b; 7 chunks
write 1,624 B, still in 3 writes.

- **Equivalence.** The golden scatter in 3 and in 4 chunks matches the
  single-chunk render. On Intel it is identical. On lavapipe (CI) 7 and 9 edge
  pixels differ by 1/255: each chunk stores x relative to its own origin, so
  positions round differently, by about 1e-5 px. The test allows 64 pixels at
  1/255. With every chunk drawn at dynamic offset 0 (seeded), 10,503 pixels
  differ. A first version compared with the golden PNG byte for byte; it passed
  on Intel and failed on lavapipe, so multi-chunk is now compared only with
  single-chunk, on the same device in the same run.
- **Browser.** The `mask wasm-browser` page renders the harness scatter with the
  device's chunk size and in 32-row chunks (7 draws) and compares them: 0 pixels
  differ on SwiftShader. With the seeded offset bug the page fails (51,351 bytes
  differ).

### Append

`ColumnStore::append(rows, columns)` writes the new rows into the last chunk's
CPU copy, doubling its capacity when it runs out (up to `chunk_rows`), then
opens new chunks. The next `upload` brings each chunk's buffer up to date:

- **Rows, not chunks.** Each column's missing rows are written at their offset
  through `Context::write_buffer` as `Upload::Column`. A first upload writes
  every row the same way, so column bytes written are always rows × stride and
  never padding. The 100K-point startup upload is now 1,200,000 B in 3 writes
  (S0b: 1,200,384 B in 1).
- **Growth moves rows on the GPU.** A grown chunk gets a larger buffer and
  `copy_buffer_to_buffer` moves the uploaded rows; one submit (counted), no CPU
  bytes. This happens in `Plot::resolve`. `Renderer::prepare` and
  `Prepared::draw` still never submit.
- **Evidence.** Batches of 1, 27, 36, 100, 300 and 5 rows appended to a 100-row
  scatter in 64-row chunks wrote exactly 469 × 8 = 3,752 column bytes. The auto
  x domain grew through the merged stats, and the final image equals a plot
  built from all 569 rows at once. At store level, 437 rows over 5 batches wrote
  exactly 3,496 B, with 2 growth copies, and the GPU bytes equal the CPU copy.
  At layer level an append writes no texture (the LUT is shared) and 2 uniform
  writes. `Selection::append` is `pub(crate)`: a handle to append to a selection
  inside a `Plot` is S12.
- **Fixed on the way.** A selection whose channels were all constant drew 0
  instances, because the store took its row count from its first column. Rows
  are now counted explicitly.

### Precision at a chunk boundary

Unix-second timestamps across three years (2020-09 to 2023-11) in 64-row chunks:
three coarse chunks, two dense chunks (80 rows a second) meeting at 1.7e9, a
coarse tail. The x domain is one second over 1000 px, straddling the boundary.
The scale's WGSL runs over each uploaded chunk column (bound from the chunk
buffer) with that chunk's base, against the f64 `CpuMirror`, as in S0a:

| Store                                         | Points | max \|GPU − CPU\| |
| --------------------------------------------- | -----: | ----------------: |
| `F32Relative`, 6 chunks of 64                 |     80 |        2.67e-5 px |
| negative control: absolute `F32`              |     80 |          993.8 px |
| control: `F32Relative`, one chunk (3 years)   |     80 |          993.8 px |
| S5 limit: one full chunk of 2^20 seconds rows |     16 |           39.9 px |

The per-chunk origin, not the relative format alone, keeps the boundary within
0.25 px. A chunk's precision is bounded by its value span: the last row of a
full default chunk of one-per-second samples is 12 days (about 1e6 s, a 1/16 s
ULP) from its origin, so a one-second zoom there misses by 40 px.

### Performance

`zoom_bench`, 100K points, 1920×1080, `Mailbox` uncapped, 600 frames,
alternating runs against the pre-S4a tree (`b176400`, which has GUP-407's font):

| Build                    | Frame interval median / p95 (ms) | fps median / p95 | CPU work median / p95 (ms) | GPU pass median / p95 (ms) |
| ------------------------ | -------------------------------: | ---------------: | -------------------------: | -------------------------: |
| before S4a, run 1        |                    4.568 / 7.494 |    218.9 / 133.4 |              0.583 / 0.842 |              3.673 / 6.711 |
| before S4a, run 2        |                    4.840 / 7.317 |    206.6 / 136.7 |              0.626 / 0.934 |              3.820 / 6.665 |
| S4a, 1 chunk, run 1      |                    4.510 / 7.984 |    221.7 / 125.2 |              0.598 / 0.896 |              3.548 / 7.264 |
| S4a, 1 chunk, run 2      |                    4.867 / 7.278 |    205.4 / 137.4 |              0.690 / 0.965 |              3.857 / 6.501 |
| S4a, 7 chunks (16384), 1 |                    4.840 / 8.279 |    206.6 / 120.8 |              0.768 / 0.969 |              3.839 / 7.085 |
| S4a, 7 chunks (16384), 2 |                   4.829 / 7.524¹ |    207.1 / 132.9 |              0.644 / 0.949 |              3.860 / 6.933 |

¹ Derived from that run's fps line (its interval line was not captured).

`Fifo` gives 60.0 / 59.3 fps (one chunk) and 60.0 / 59.2 (7 chunks), with no
missed refresh. Every run whose upload line was kept (one chunk, `Fifo` and
`Mailbox`; 7 chunks, `Mailbox`) wrote 0 column bytes over its 600 zoomed frames.
S4a costs nothing measurable, and seven draws instead of one cost nothing
either. Both trees' GPU pass (about 3.7 ms median) is slower than S0b's 2.7–3.0
ms, before and after this story alike, so that is not S4a's; GUP-407's text
changes or the machine's state are the candidates. The window frame is still
byte-identical to the 4× `ImageTarget` and the golden (ΔE 0 on 324,000 px).

### Proposed adjustments to S4b, S5 and later

- **S4b (validity bits, dictionaries, `Retain`).** The chunk code assumes 4-byte
  strides: capacities are 64-row blocks, `chunk_rows_for` divides bytes, and
  tail writes rely on `write_buffer`'s 4-byte alignment. A 1-bit validity column
  needs a stride in bits, capacities that are whole 32-row words, and a tail
  write that rewrites the last partial word (so validity bytes written are
  slightly more than rows/8; count and document them). Dictionary codes (`U32`)
  fit as they are; keep the dictionary per store, append-only, so appended rows
  never renumber codes and a new key only grows the domain (a uniform write).
  `Retain::GpuOnly` cannot keep `Chunk::bytes`: growth already moves rows on the
  GPU, but upload to a second context and the dirty tail need the CPU copy, so
  drop each chunk's bytes once it is full and uploaded, and make re-binding to
  another context an error in that mode.
- **S5 (`Time`).** A per-chunk origin is not enough for deep zoom into a large
  chunk: one-per-second samples lose the 0.25 px budget at a one-second, 1000 px
  zoom once a value is more than a few thousand seconds from its origin (the 40
  px row above). Options: cap a time column's chunk by value span (split when
  `|v − origin|` would exceed about 4,000 s at the finest zoom offered), a hi/lo
  f32 pair (double-single) column format, or origins per sub-chunk. Decide in S5
  with the real `Time` proof, which can reuse `conformance::chunk_boundary` (it
  binds uploaded chunk columns).
- **S9 (culling, picking).** Per-chunk f64 stats are there for culling, which
  can be a filter on the chunk draws at prepare time; `Prepared::draw` needs no
  change. `row_base` is in every chunk's uniform, so picked instance ids are
  store rows. `row_base` is a `u32`; prepare errors past 2^32 rows.
- **S12 (append handles, `Window`).** Build on `Selection::append` (evaluates
  only new rows) and `ColumnStore::append`. Evicting whole chunks leaves
  `row_base` values that do not start at 0; picking must map rows through the
  window. A growth copy submits during resolve; hosts that resolve inside their
  own frame graph should know.
