# gup-core performance budgets

What `gup-core` is expected to cost, measured, and the checks that keep it there
(GUP-417). Every number below was measured on the machine named with it; none is
a target picked in advance.

- **Locally**, `mask perf-budget` (`scripts/perf_budget.sh`) runs the three
  wall-clock tools, compares their `metric` lines with the
  [budget table](#budget-table) and exits non-zero on any failure. It needs a
  display and a real GPU, so it is a local, pre-merge check, not a CI job. Run
  it for any story that touches `gup-core` rendering, uploads, pipeline creation
  or WASM size, and record the result in the story's Definition-of-Done
  evidence.
- **In CI**, absolute GPU and CPU times mean nothing: the runners render on
  lavapipe, a software rasteriser. CI instead runs the
  [deterministic proxies](#ci-side-proxies), which fail on an extra upload, an
  extra draw or an extra pipeline whatever the hardware, and enforces the WASM
  size budget.

## The development machine

All timings were measured on 2026-10-10 (GUP-417) and, for the GPU pass at the
new 1-sample default, on 2026-10-16 (GUP-421), on:

| Part           | Value                                                            |
| -------------- | ---------------------------------------------------------------- |
| GPU            | Intel HD Graphics 630 (KBL GT2), integrated, 350–1100 MHz        |
| Driver         | Mesa 26.0.0, Vulkan (wgpu 27), Mesa's on-disk shader cache on    |
| CPU            | Intel Core i7-7700HQ, 4 cores / 8 threads                        |
| OS and display | Linux 6.18.54–55, niri (Wayland), 1920×1080 at 60.02 Hz, scale 1 |
| Toolchain      | rustc 1.93.1 (`rust-toolchain.toml`), release profile            |
| Story          | GUP-421; `DEFAULT_SAMPLES` = 1 (analytic antialiasing, no MSAA)  |

On another machine the timing budgets do not apply: measure there first and
compare against your own numbers, not these.

### The GPU clock decides the GPU pass time

The i915 driver picks the GPU clock from how busy the GPU is, and `zoom_bench`
sits near its thresholds. A run either stays at the 350 MHz floor for most
frames or is boosted to ~1050 MHz, and the boosted runs' GPU pass median is
about half (1.9 against 3.9 ms at 4× MSAA). `mask perf-budget` samples
`/sys/class/drm/card*/gt_act_freq_mhz` during `zoom_bench` and prints the median
as `zoom.gpu_clock.median_mhz`. The budgets below were recorded at the floor
(clock median 350–483 MHz), the slow state, so a boosted run can only pass; the
script says when that happened and its timing checks were weak. Under `Fifo`
(vsync) the clock stayed at the floor in every run.

## Budget table

`scripts/perf_budget.sh` reads the first three columns of this table. A check is
`exact` (the same value), `+N%` (at most N% over the recorded value; faster is a
pass, flagged for re-recording when it is more than N% faster), `<= X` (at most
X) or `info` (printed, never fails).

| Metric                             | Recorded  | Check     | Measured by                                         |
| ---------------------------------- | --------- | --------- | --------------------------------------------------- |
| `zoom.size`                        | 1920x1080 | exact     | `zoom_bench` (fullscreen; other sizes: re-record)   |
| `zoom.samples`                     | 1         | exact     | `zoom_bench` (`DEFAULT_SAMPLES`)                    |
| `zoom.gpu_clock.median_mhz`        | 350       | info      | `scripts/perf_budget.sh` (i915 sysfs)               |
| `zoom.gpu_pass.median_ms`          | 2.95      | +20%      | `zoom_bench`, timestamp queries                     |
| `zoom.gpu_pass.p95_ms`             | 5.60      | +25%      | `zoom_bench`, timestamp queries                     |
| `zoom.cpu.median_ms`               | 0.69      | +20%      | `zoom_bench`, resolve + prepare + submit            |
| `zoom.cpu.p95_ms`                  | 0.86      | +25%      | `zoom_bench`, resolve + prepare + submit            |
| `zoom.interval.median_ms`          | 4.91      | info      | `zoom_bench` (GPU-bound: follows the GPU pass)      |
| `zoom.columns.bytes`               | 0         | exact     | `zoom_bench`, over the 600 measured frames          |
| `zoom.columns.writes`              | 0         | exact     | `zoom_bench`                                        |
| `zoom.validity.bytes`              | 0         | exact     | `zoom_bench`                                        |
| `zoom.uniforms.bytes_per_frame`    | 136       | exact     | `zoom_bench`                                        |
| `zoom.uniforms.writes_per_frame`   | 3         | exact     | `zoom_bench`                                        |
| `zoom.instances.bytes`             | 5,631,168 | exact     | `zoom_bench`, over the 600 measured frames          |
| `zoom.instances.writes_per_frame`  | 3         | exact     | `zoom_bench`                                        |
| `zoom.textures.bytes`              | 0         | exact     | `zoom_bench`                                        |
| `zoom_culled.draws.median`         | 14        | exact     | `zoom_bench` on 4M sorted rows, 64 chunks (GUP-457) |
| `zoom_culled.gpu_clock.median_mhz` | 350       | info      | `scripts/perf_budget.sh` (i915 sysfs)               |
| `zoom_culled.gpu_pass.median_ms`   | 9.90      | +20%      | `zoom_bench` on 4M sorted rows, 64 chunks           |
| `zoom_culled.cpu.median_ms`        | 0.75      | +20%      | `zoom_bench` on 4M sorted rows, 64 chunks           |
| `zoom_culled.columns.bytes`        | 0         | exact     | `zoom_bench` on 4M sorted rows, 64 chunks           |
| `pipeline.link_create.median_ms`   | 1.11      | +20%      | `pipeline_timings`, 20 runs, fresh cache each       |
| `pipeline.link_create.cold_ms`     | 1.04      | <= 10     | `pipeline_timings`, its first run                   |
| `pipeline.link_create.max_ms`      | 1.06      | info      | `pipeline_timings`                                  |
| `wasm.wgpu.gz_bytes`               | 32,360    | info      | `scripts/wasm_size.sh`, bare wgpu                   |
| `wasm.scatter.gz_bytes`            | 315,804   | +2%       | `scripts/wasm_size.sh`, the reference scatter       |
| `wasm.over_wgpu.gz_bytes`          | 283,442   | <= 400000 | `scripts/wasm_size.sh`, RFC-001 §12 risk 10         |
| `wasm.per_row_type.gz_bytes`       | 1,730     | <= 2600   | `scripts/wasm_size.sh`, one more Selection row type |

A legitimate change that moves a number (a new guide, a bigger feature) records
the new value here, in the same commit, with the reason in the story.

## The five categories

### GPU render-pass time

```bash
cargo run -p gup-core --release --example zoom_bench -- --present mailbox --uncapped
```

100,000 points (linear x, log y, viridis fill, radius 3 px) in a borderless
fullscreen window, both domains zoomed every frame, 60 warm-up and 600 measured
frames; the render pass is timed with `TIMESTAMP_QUERY`. Recorded (GUP-421): the
median of six runs' medians (2.61–3.00 ms) and p95s (4.67–5.88 ms) at the
default single sample, at the clock floor (350–417 MHz), close to GUP-417's 1×
figures (3.02 / 5.87 ms). Four runs at `--samples 4` alternating with them
measured 4.52–4.60 ms median and 8.50–8.87 ms p95 (clock 367–400 MHz). Tolerance
+20% on the median and +25% on the p95, whose runs spread by up to 15% between
sessions; that still catches a regression the size of S0b→S4a's (+30%, which
[GUP-417](#history) traced to MSAA).

| Configuration (same machine, Mailbox uncapped, 6 runs each) | GPU pass median (ms) | GPU pass p95 (ms) |
| ----------------------------------------------------------- | -------------------: | ----------------: |
| S0b's tree (`e6bba47`, before MSAA)                         |          2.76 – 3.01 |       5.36 – 6.36 |
| this tree, `--samples 1`                                    |          2.94 – 3.04 |       4.62 – 6.38 |
| this tree, `--samples 4` (the default until GUP-421)        |          3.73 – 4.14 |       5.97 – 6.79 |

### CPU time per frame

The same `zoom_bench` run: `Plot::resolve` + `Renderer::prepare` + encode,
submit and present, without the wait for a swapchain image. Recorded 0.69 ms
median, 0.86 ms p95: GUP-417's six runs at one sample, which GUP-421 made the
default (at 4×: 0.76 and 1.13 ms). GUP-421's `perf-budget` runs measured 0.58
and 0.70 ms. The CPU is mostly idle: the loop is GPU-bound.

### Uploads per frame during a steady zoom

The same `zoom_bench` run counts every GPU write by `Upload` kind
(`Context::upload_stats`). Over the 600 zoomed frames: **0 column bytes in 0
writes** and 0 validity bytes (the columns were uploaded once, 1,200,000 B in 3
writes, before the zoom); **136 uniform bytes in 3 writes per frame**
(`Encodings` 112 B, `Chunk` 8 B, view 16 B); **3 guide-instance writes per
frame**, 5,629,476 B in all (axis rules and glyph quads, rebuilt because the
ticks move; exact at 1920×1080, other sizes differ); 0 texture bytes (every
glyph is in the atlas after warm-up). Deterministic, so checked exactly.

### Pipeline creation time

```bash
cargo test -p gup-core --release --lib pipeline_timings -- --ignored --nocapture
```

Link (`gup_wgsl::link`) plus `create_shader_module` and `create_render_pipeline`
of the reference scatter, 20 runs, each with a fresh pipeline cache on one
device. Recorded over twelve runs: median 0.84–0.97 ms (0.88 recorded, +20%).
The first (cold) run is a single sample and too noisy for a relative budget:
0.94–1.23 ms in eleven runs, 4.47 ms in one (the first after `zoom_bench`). It
has an absolute ceiling instead, 10 ms: below S0a's 13.5 ms cold run, so a
return to composing shaders at run time would fail it. Both numbers depend on
Mesa's on-disk shader cache: with `MESA_SHADER_CACHE_DISABLE=true` the median is
5.0 ms and the cold run 5.3–7.8 ms, which explains GUP-406's 8.4 ms cold run.
Measure with the cache on, as a user's machine has it.

### WASM size

```bash
mask wasm-size   # ./scripts/wasm_size.sh
```

The reference scatter harness against bare wgpu, both release
`wasm32-unknown-unknown`, `wasm-bindgen --target web` without name or producers
sections, `gzip -9`. Recorded: scatter 300,913 B gz, bare wgpu 41,712 B gz, so
**gup-core costs 259,201 B gz over bare wgpu** against RFC-001 §12 risk 10's
ceiling of +400 KB gz, read as 400,000 B (the RFC's KB are 1,000 B). Sizes
depend only on the tree and toolchain, so the scatter is held to +2% (about 5 KB
gz, smaller than one story's typical growth: S4b added 6.4 KB). CI enforces both
the scatter row and the ceiling (`mask wasm-size`, GUP-438), so a story that
changes the size re-records the row in the same commit, against current `main`.

The harness is also the browser smoke test, so its scenes count: GUP-418 (S5a)
re-recorded the scatter at 271,706 B (from 252,925). Its library changes (the
chain-aware glue, `EncodeFn`, the hi/lo column format) cost 974 B (+0.4%,
measured before the new scene), and the scene that draws `Time`, `Symlog`,
`Sqrt` and a `then` chain, with its CPU-mirror checks, 17,807 B (+7.0%): code a
chart using those scales pays, and the reference scatter alone does not.

GUP-419 (S5b) re-recorded it at 294,333 B (from 271,706). Its library changes,
measured before the new scenes, cost 7,985 B (+2.9%): mostly the categorical
palette (a LUT read by texel, its generated extension past the palette and the
key domain), the legend types and the generic key dictionaries, reached by the
existing dictionary scene. Two new scenes, a `Band` axis with an 11-key
`Categorical`, its swatch legend and a hidden null key, and a `Diverging` scale
with its ramp, each with CPU-mirror checks, cost 14,642 B (+5.2%), which
includes `Band`, `Point` and `Diverging` themselves.

GUP-420 (S5c) re-recorded it at 300,913 B (from 294,333). Its library changes,
measured without the new scene, cost 1,714 B (+0.6%): the import-path clash
check and its message, each shader function's type name for it, and `Color` as a
WGSL `vec4<f32>`. The macro and naga_oil run at compile time and add nothing.
The scene, a user transfer function chained into `Pow::sqrt()` and a user colour
function whose `Params` hold colours, written with `#[wgsl_function]` and
checked against their mirrors, costs 4,866 B (+1.6%).

GUP-421 re-recorded it at 302,929 B (from 300,913): the `gup::coverage` library
module and the rule and rect shaders that use it (+2,016 B, +0.7%).

GUP-431 re-recorded it at 312,306 B (from 302,929). At the commit GUP-431
started from, the scatter was already 308,740 B (+1.9%, within the +2%): the
stories merged since GUP-421 (Segment, Rect, stroke and opacity, theme, chart
traits, overhang domains) had not re-recorded it. GUP-431's size-descending draw
order (`order.rs`: the batch sort, the run merge, compaction, and draws over
instance ranges) adds 3,566 B (+1.2%).

Merging GUP-426 (legends, +10,242 B on its own) with GUP-431 gives 316,565 B
(over bare wgpu 274,852), measured with `mask wasm-size` on the merge
(2026-10-17). That is the recorded figure.

### Where the bytes go (GUP-450)

Measured 2026-10-18 at commit bf9e8b8 plus this story's harness feature (the
default build is byte-identical), with `mask wasm-attrib`: the scatter as
`wasm-bindgen --target web` writes it, names kept, read by twiggy 0.8.0. The
file is 1,415,901 B raw; its 263,564 B name section is not shipped and is left
out of the tables. Raw bytes, not gz: use them to find targets, and judge a cut
by `mask wasm-size`.

Ten largest crates (`data` is every data segment, mostly `.rodata`, which holds
the two Inter subsets):

| Crate                  | Raw B   | Share of file |
| ---------------------- | ------- | ------------- |
| `gup_core`             | 383,096 | 27.06%        |
| `data`                 | 230,954 | 16.31%        |
| `core`                 | 139,401 | 9.85%         |
| `alloc`                | 65,994  | 4.66%         |
| `ttf_parser`           | 60,704  | 4.29%         |
| `other`                | 57,125  | 4.03%         |
| `wasm_bindgen_futures` | 49,778  | 3.52%         |
| `wgpu`                 | 31,887  | 2.25%         |
| `fontdue`              | 21,305  | 1.50%         |
| `hashbrown`            | 20,786  | 1.47%         |

Ten largest generic groups (instantiation hash removed; groups of two or more):

| Group                                                                                           | Instantiations | Raw B  |
| ----------------------------------------------------------------------------------------------- | -------------- | ------ |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::prepare`                  | 7              | 53,431 |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`                             | 8              | 44,076 |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::hit_items`                | 7              | 25,557 |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::fit_domains`              | 7              | 24,675 |
| `gup_core::selection::Selection<T,M>::attr`                                                     | 35             | 20,880 |
| `gup_core::scope::<impl gup_core::context::Context>::scoped`                                    | 7              | 15,112 |
| `hashbrown::raw::RawTable<T,A>::reserve_rehash`                                                 | 8              | 14,092 |
| `<alloc::vec::Vec<T> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<T,I>>::from_iter` | 23             | 12,118 |
| `<alloc::vec::Vec<T> as alloc::vec::spec_from_iter::SpecFromIter<T,I>>::from_iter`              | 22             | 9,262  |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::bind_axes`                | 7              | 8,995  |

Ten largest single items:

| Item                                                                           | Raw B   | Share of file |
| ------------------------------------------------------------------------------ | ------- | ------------- |
| `data segment ".rodata"`                                                       | 230,826 | 16.30%        |
| `gup_core::shader::glue::emit`                                                 | 14,867  | 1.05%         |
| `gup_core::plot::Plot::resolve`                                                | 13,342  | 0.94%         |
| `ttf_parser::tables::cff::cff1::_parse_char_string`                            | 12,654  | 0.89%         |
| `wasm_bindgen_futures::future_to_promise::{{closure}}::{{closure}}`            | 9,693   | 0.68%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::prepare` | 7,633   | 0.54%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::prepare` | 7,633   | 0.54%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::prepare` | 7,633   | 0.54%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::prepare` | 7,633   | 0.54%         |
| `<gup_core::selection::Selection<T,M> as gup_core::selection::Layer>::prepare` | 7,633   | 0.54%         |

These agree with the Context figures of GUP-450 within about 5% on the
per-instantiation sizes (`Layer::prepare` 7,633 B here against 8,177 B,
`hit_items` 3,651 B against 3,842 B, `fit_domains` 3,525 B against 3,712 B,
`bind_axes` 1,285 B against 1,404 B, all with 7 instantiations); the sum of
those four, 112,658 B, is below the scribe's 159,137 B for all `Layer` methods
because it leaves out the others (`attr`, `scoped`, the `hit_items` closures).
`ttf_parser` is 60.7 KB against about 64 KB, and the `wasm_bindgen_futures`
closures 44.1 KB (the scribe's 59 KB included inlined scene functions that
twiggy attributes to their callees). The scribe's figures were taken before
wasm-bindgen, so they differ by the describe functions it removes.

One more `Selection` row type (`wasm.per_row_type.gz_bytes`, the scatter built
with and without the harness's `extra-row-type` feature) costs **3,674 B gz**,
0.92% of the scatter's 398,627 B gz. That is a floor for an interactive layer of
`Circle`s with a tooltip and a click handler: a row type that also carries other
encodings or marks instantiates more of `attr`.

## CI-side proxies

Deterministic checks that run on every push (the Tests and Visual regression
workflows) and do not depend on the GPU's speed. Each was seen to fail on a
seeded violation (GUP-417 retrospective).

| Proxy                  | What it pins                                                                                                                                                                                                                                 | Enforced by                                                                             |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| No extra upload        | 300 zoomed frames of 100K points: 0 column and validity bytes; exactly 3 uniform writes and 136 B (1 chunk) or 1,672 B (7 chunks) per frame; one instance write per guide draw and no more guide bytes than recorded; < 128 KB of atlas rows | `tests/zoom_uploads.rs` (`zooming_100k_points_writes_no_column_bytes`, `…_in_chunks_…`) |
| No extra draw          | one instanced draw per column chunk (`MarkBatch::chunks`) for a layer without appends, and the same number of draw calls (`Prepared::draw_calls`) every frame: 4 for 1 chunk, 10 for 7                                                       | `tests/zoom_uploads.rs`, same tests                                                     |
| No extra pipeline      | no glue program linked and no pipeline created (`Context::pipeline_stats`) after the first of 301 frames with an unchanged encoding                                                                                                          | `tests/zoom_uploads.rs`, same tests; `plot::tests` for one zoom                         |
| Every write is counted | no source file writes to the GPU except through the counted `Context` methods                                                                                                                                                                | `context::tests::every_gpu_write_is_counted`                                            |
| WASM size              | gup-core ≤ 400,000 B gz over bare wgpu                                                                                                                                                                                                       | `scripts/wasm_size.sh`, Visual regression workflow's browser job                        |

## History

- **S0b (GUP-396)**: GPU pass 2.7–3.0 ms median at 1 sample.
- **S3 (GUP-401)** made 4× MSAA the default without re-running `zoom_bench`.
- **S4a (GUP-414)** measured ~3.7 ms and left the cause open.
- **GUP-417** measured S0b's tree, this tree at 1× and this tree at 4× on the
  same machine, alternating: S0b and 1× agree within 2%, and 4× costs +0.9 ms
  median uncapped (+29%) and +1.7 ms under vsync at the clock floor (+56%). MSAA
  accounts for all of the S0b→S4a gap. Whether to keep 4× as the default is an
  owner decision recorded in RFC-001's GUP-417 findings.
- **GUP-430 (S6d)** re-recorded `zoom.uniforms.bytes_per_frame` 88 → 136: the
  reference `Circle` gained three constant channels (`fill_opacity`, `stroke`,
  `stroke_width`), so `Encodings` grew 64 → 112 B. Measured 136 B; timings
  within budget.
- **GUP-418 (S5a)** re-recorded the WASM scatter (+18,781 B gz, almost all of it
  the new browser scene for `Time`, `Symlog`, `Sqrt` and `then`). Its
  `perf-budget` run (GPU clock median 517 MHz) passed every other budget: GPU
  pass 3.26 ms median, CPU 0.74 ms, 88 uniform bytes and 0 column bytes per
  frame.
- **GUP-419 (S5b)** re-recorded the WASM scatter (+22,627 B gz: 7,985 B of
  library reached by the existing scenes, 14,642 B of new browser scenes for
  `Band`, the full `Categorical` and `Diverging`). Its `perf-budget` run (GPU
  clock median 583 MHz, above the floor) passed every other budget: GPU pass
  3.14 ms median, CPU 0.67 ms, 88 uniform bytes and 0 column bytes per frame,
  link and create 0.91 ms median (cold 4.5 ms).
- **GUP-420 (S5c)** re-recorded the WASM scatter (+6,580 B gz: 1,714 B of
  library, 4,866 B for the browser scene of user `#[wgsl_function]`s). Its first
  `perf-budget` run failed only `pipeline.link_create.cold_ms` (10.8 ms, Mesa's
  shader cache cold after the rebuild, as GUP-417 found); the second (GPU clock
  median 500 MHz) passed every timing budget: GPU pass 3.75 ms median, CPU 0.63
  ms, 88 uniform bytes and 0 column bytes per frame, link and create 0.94 ms
  median (cold 1.08 ms). A third, complete run passed every check with the clock
  boosted (750 MHz). The reference scatter's glue is unchanged.
- **GUP-421** made rules and rects antialias analytically and `DEFAULT_SAMPLES`
  1, and re-recorded the GPU pass (2.95 ms median, 5.60 ms p95, from 3.89 and
  6.30 at 4×), the CPU time (GUP-417's 1× figures) and the WASM scatter (+2,016
  B gz). Two complete `perf-budget` runs (GPU clock median 367 MHz) passed every
  check: GPU pass 3.00 and 2.95 ms median, CPU 0.58 ms, 88 uniform bytes and 0
  column bytes per frame, link and create 1.04 and 0.96 ms median.
- **GUP-426 (S7c)** re-recorded the WASM scatter (+10,242 B gz, 302,929 to
  313,171; over bare wgpu 261,217 to 271,458, under the 400,000 cap). It is the
  library's legend code (`guide.rs` layout and drawing, the `ShaderFn::legend`
  hook and its overrides, `Layer::legends`, and the legend text the harness's
  plot now lays out). The harness change is one `.unwrap()` on the legend
  accessor, a few bytes.
- **GUP-432 (S6f)** re-recorded the WASM scatter (+4,474 B gz, 316,983 to
  321,457, both measured with `mask wasm-size` on 2026-10-17, the parent tree
  first; over bare wgpu 279,744, under the 400,000 cap). The parent measured 418
  B above the recorded 316,565. The growth is shared code the scatter runs
  through although it draws no line: the glue emitter's next-row reads, draw
  entries (`order::draw_entries`), per-slot vertex buffer bindings
  (`ChunkDraw::slots`) and the vertex-slot and storage-buffer checks in prepare.
- **Merge of GUP-435, 437 and 432 (2026-10-17)**, measured by the orchestrator
  with `mask perf-budget` on the merged tree (GPU clock median 433 MHz, load
  5.2):
  - `zoom.instances.bytes` 5,629,476 to 5,646,396 (+16,920 B): the guide
    instances from GUP-435's tick changes (thinned and re-nice'd labels on
    widened domains). `tests/zoom_uploads.rs` moved with it in GUP-435.
  - WASM scatter 321,457 to 329,548 B gz (+8,091), and over bare wgpu 279,744 to
    287,835. That is GUP-435's tick thinning, domain widening and log nice, plus
    GUP-437's chain legends; measured on its own base (316,565) at +6,462 and
    +2,618 respectively.
  - `pipeline.link_create.median_ms` read 1.139 against 1.056 at load 5.2: a
    timing failure under load, not re-recorded.
- **GUP-435's widening fix (1d9b9ce, 2026-10-17)**: re-measured by the
  orchestrator. Widened domains are no longer rounded a second time, so there
  are fewer guide instances: `zoom.instances.bytes` drops from 5,646,396 to
  5,631,168. WASM scatter is 329,486 B gz, within its +2%.
  - `pipeline.link_create.median_ms` read 1.085, and has read 1.07–1.16 in most
    runs since GUP-429, one run passing at 0.98, with the load at 4–6 every
    time. It is not re-recorded. Confirm it on an idle machine before treating
    it as a regression in the glue (GUP-431's draw entries and GUP-432's
    next-row reads are the candidates).
- **GUP-436 (S7g), merged 2026-10-17**: the orchestrator re-measured the merged
  tree. The WASM scatter grows from 329,548 to 361,531 B gz (+31,983, +9.7%),
  and over bare wgpu from 287,835 to 319,818, against the 400,000 cap. Almost
  all of it is the bundled Inter SemiBold subset that year ticks use: the
  story's 40,000 B allowance for a second weight. The Regular subset line in
  `wasm_size.sh`'s table does not list the SemiBold subset yet.
- **link_create re-recorded after an A/B (2026-10-17, orchestrator)**: with no
  agents running (load 4 to 5 from ZFS writes), `mask perf-budget` measured
  `pipeline.link_create.median_ms` at 0.926 at GUP-421's commit (b263068) and at
  1.137 and 1.091 at main after GUP-447 (20d18ad), both at a GPU clock median of
  367 to 450 MHz. The +0.2 ms (+18 to 23%) built up as the generated glue grew:
  draw entries (GUP-431), next-row reads (GUP-432), legend hooks (GUP-426/437)
  and the closures bound to position channels (GUP-447). A pipeline is created
  once per layer, so the cost is not visible to users. Recorded at 1.11 (the
  mean of the two runs). In the same A/B, CPU per frame went from 0.695 to 0.73
  to 0.76 ms and the GPU pass from 2.94 to 3.11 ms, both within their budgets.
- **S8 so far: GUP-444, 447 and 445 (2026-10-17)**, re-measured by the
  orchestrator on main at 087856c. WASM scatter 361,531 to 372,092 B gz
  (+10,561); over bare wgpu 319,818 to 330,379, against the 400,000 cap:
  - GUP-444 (cached resolve, `Input`/`handle`) and GUP-447 (adders, default
    axes, position closures) measured 366,318 together (+4,787).
  - GUP-445 (CPU picking) adds 5,774. Most of that is two new `Layer` methods
    (`hit_items`, `row_any`) instantiated for every `Selection<T, M>`, so each
    distinct row/mark type in a program pays for them.
  - That leaves 69,621 B of headroom under the cap. Per-instantiation code is
    now the growth to watch, ahead of S9 to S12.
- **S8 complete: GUP-446, 448 and 449 merged (2026-10-18)**, re-measured by the
  orchestrator on the merge. WASM scatter 372,092 to 398,627 B gz (+26,535);
  over bare wgpu 330,379 to 356,914, leaving **43,086 B of headroom** under the
  400,000 cap:
  - GUP-446 (hover, click, tooltips) and GUP-448 (`GupApp`) measured 376,211
    together (+4,119).
  - GUP-449 (web canvas host) adds the rest. Its browser scene now drives hover,
    tooltip, click and wheel, so picking and tooltip code that the scatter
    harness never reached before is now linked as well as the host's DOM
    listeners. The split between the two is not measured.
  - S9 to S12 cannot fit in this headroom at this rate. A WASM size story, to
    attribute the bytes with `twiggy` and cut them, comes before S9.
- **GUP-451, WASM size cuts (2026-10-18)**, re-measured by the orchestrator.
  Over bare wgpu: 356,914 to 263,544 B gz (−93,370, −26%), with no change to
  rendered output (`mask wasm-browser` pixel counts identical; native goldens
  unblessed):
  - `z` + LTO + `codegen-units = 1` in both harnesses: −64,322.
  - ASCII-only SemiBold subset, with per-glyph Regular fallback: −21,385.
  - `to_ascii_uppercase` for channel names, which drops the Unicode case tables:
    −7,664.
  - The per-row-type cost falls to 941 B gz, so moving `Selection` behind a
    T-free trait (AC3) was not needed. Bare wgpu itself is 32,360 under the same
    profile.
  - Headroom under the 400,000 cap is now 136,456. These figures assume the
    harnesses' release profile, which `web.rs` documents for users.
  - perf_budget.sh now reads only the "Budget table" section, so the crate
    breakdown above no longer shows as unmeasured budget rows.
- **S9a to S9d merged: GUP-452, 453, 454 and 455 (2026-10-18)**, re-measured by
  the orchestrator on the merge. WASM scatter 295,904 to 310,638 B gz (+14,734);
  over bare wgpu 263,544 to 278,278, leaving 121,722 under the cap:
  - GUP-452 (held rect): +227. GUP-453 (pan): +825. GUP-454 (brush, drag-zoom):
    +4,152. Each includes its browser-harness steps.
  - GUP-455 (GPU pick): +9,502. Under 1 KB of that is library WGSL and glue; the
    rest is the harness now linking the pick pass for the browser test.
  - `wasm.per_row_type.gz_bytes` 941 to 1,598. GUP-453 to 455 each saw it move
    with no new `Layer` method: inlining shifts in generic code between the one-
    and two-row-type builds. At this size it is noisy, and stays `info`.
- **GUP-459, per-row-type cost measured cleanly and budgeted (2026-10-18)**.
  `wasm.per_row_type.gz_bytes` went 941 (GUP-451), 1,598 (S9a to S9d), 2,569
  (GUP-456, at `b3f87b5`: 312,441 B gz without the feature, 315,010 with it).
  Diffing `twiggy top` of the two builds by item gave 5,144 B raw outside the
  name section, about 3.3 KB of it the measuring function, not the library:
  - 2,510 `future_to_promise::{{closure}}::{{closure}}` (+1), and 277 for
    `future_to_promise::{{closure}}` (+1) with ~240 of drops and shims: the
    harness's async `render_extra_row_type` state machine, which inlines
    resolve, pick and render.
  - 464 `<Resolved as Clone>::clone` (+1), -534 `InteractiveScatter::resolved`:
    an inlining shift caused by the second caller.
  - Library, per row type: 802 `Selection::bind_axes`, 234 `tooltip_text`, 204
    `AxisEncoding<T>::evaluate`, 151 `dispatch`, and ~300 for `row_any`,
    `as_any_mut`, `type_id`, vtable shims and closures.
  - The harness export is now the synchronous `extra_row_type`, which boxes a
    `Plot` as `dyn Chart` and runs nothing: M0 = 1,647 B gz. `bind_axes` now
    delegates its role match, messages and `reads_keys` check to the non-generic
    `axis_scale_for`: its instantiation falls from 802 to 538 B raw. The gz
    figure M = 1,730 is 83 B above M0 (inlining noise at this size).
  - The row is a budget: `<= 2600`, B = ceil(1.5 M / 100) x 100. The seeded
    proof is in the GUP-459 retrospective. `scripts/wasm_attrib.sh --row-type`
    attributes the figure by item.
- **GUP-457, 459 and 460 merged (2026-10-18)**, recorded by the orchestrator
  from CI's `mask wasm-size` on 7983fe8, the reference per GUP-438. WASM scatter
  310,638 to 315,804 B gz (+5,166); over bare wgpu 278,278 to 283,442. Most of
  it is GUP-457's chunk culling (+3,181 measured on its own base, accepted for
  an 83% GPU-pass cut on 4M rows). GUP-456's GPU-picked hover had not been
  re-recorded (+1,803). CI measures `wasm.per_row_type.gz_bytes` at 764 where a
  local build gives 1,730: the row is noisy across machines, and its `<= 2600`
  bound holds on both.
