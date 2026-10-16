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

| Metric                            | Recorded  | Check     | Measured by                                       |
| --------------------------------- | --------- | --------- | ------------------------------------------------- |
| `zoom.size`                       | 1920x1080 | exact     | `zoom_bench` (fullscreen; other sizes: re-record) |
| `zoom.samples`                    | 1         | exact     | `zoom_bench` (`DEFAULT_SAMPLES`)                  |
| `zoom.gpu_clock.median_mhz`       | 350       | info      | `scripts/perf_budget.sh` (i915 sysfs)             |
| `zoom.gpu_pass.median_ms`         | 2.95      | +20%      | `zoom_bench`, timestamp queries                   |
| `zoom.gpu_pass.p95_ms`            | 5.60      | +25%      | `zoom_bench`, timestamp queries                   |
| `zoom.cpu.median_ms`              | 0.69      | +20%      | `zoom_bench`, resolve + prepare + submit          |
| `zoom.cpu.p95_ms`                 | 0.86      | +25%      | `zoom_bench`, resolve + prepare + submit          |
| `zoom.interval.median_ms`         | 4.91      | info      | `zoom_bench` (GPU-bound: follows the GPU pass)    |
| `zoom.columns.bytes`              | 0         | exact     | `zoom_bench`, over the 600 measured frames        |
| `zoom.columns.writes`             | 0         | exact     | `zoom_bench`                                      |
| `zoom.validity.bytes`             | 0         | exact     | `zoom_bench`                                      |
| `zoom.uniforms.bytes_per_frame`   | 136       | exact     | `zoom_bench`                                      |
| `zoom.uniforms.writes_per_frame`  | 3         | exact     | `zoom_bench`                                      |
| `zoom.instances.bytes`            | 5,629,476 | exact     | `zoom_bench`, over the 600 measured frames        |
| `zoom.instances.writes_per_frame` | 3         | exact     | `zoom_bench`                                      |
| `zoom.textures.bytes`             | 0         | exact     | `zoom_bench`                                      |
| `pipeline.link_create.median_ms`  | 0.88      | +20%      | `pipeline_timings`, 20 runs, fresh cache each     |
| `pipeline.link_create.cold_ms`    | 1.04      | <= 10     | `pipeline_timings`, its first run                 |
| `pipeline.link_create.max_ms`     | 1.06      | info      | `pipeline_timings`                                |
| `wasm.wgpu.gz_bytes`              | 41,712    | info      | `scripts/wasm_size.sh`, bare wgpu                 |
| `wasm.scatter.gz_bytes`           | 312,306   | +2%       | `scripts/wasm_size.sh`, the reference scatter     |
| `wasm.over_wgpu.gz_bytes`         | 270,593   | <= 400000 | `scripts/wasm_size.sh`, RFC-001 §12 risk 10       |

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
gz, smaller than one story's typical growth: S4b added 6.4 KB) and the ceiling
is enforced in CI.

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
