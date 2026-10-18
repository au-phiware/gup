---
implementer: sonnet
implementer-reason:
  types, signatures, the drain point, the waker plumbing and the dogfood ports
  are fixed in Decisions; builds on GUP-414's existing `Selection::append`
---

# GUP-480: RFC-001 Step S12a: Public `append` and the `Appender` Handle

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-19)
**Created**: 2026-10-19

## Context

GUP-414 (S4a) built tail-only appends: `Selection::append` evaluates only the
new rows and the next resolve writes only their bytes, at the tail of the last
chunk or in new chunks. Auto domains refit with a uniform write. GUP-431 merges
appended batches into the size draw order, GUP-432 extends lines across chunks,
and GUP-458 grows the brush bits. All of that is tested, but `append` is
`pub(crate)` ("a handle to append to a selection already in a plot is RFC-001
S12").

RFC-001 §8 example (e) is the target:

```rust
let feed = chart.appender();   // Clone + Send handle (channel)
std::thread::spawn(move || for t in market_feed() { feed.push(t); });
gup::show(chart)?;             // per frame: drain → tail-only write_buffer
```

Today the dogfood live task (`t5_live`) rebuilds both charts from all rows on
every tick, and `t5_stream` still runs on the old path's `DataStream` (expected
failure `STREAM_NEVER_READY`). This story makes appending public, adds the
cross-thread handle, wakes the window when rows arrive, and ports both tasks.
Eviction (`Window::LastRows`) is GUP-482; the browser host's waker is GUP-481.

## Read First

- RFC-001 lines 226–236 ("Append") and 724–733 (example (e)); "S4a findings"
  lines 2100–2115 and 2190–2195 (the S12 notes).
- `crates/gup-core/src/selection.rs`: the struct (lines 112–150), `append`
  (lines 202–245), the `Layer` trait (lines 620–700) and `revision` (lines
  854–864).
- `crates/gup-core/src/plot.rs`: `resolve` (lines 815–980, the cache check),
  `revision_sum` (lines 1517–1528), the `Chart` impl (lines 327–355), and the
  append tests (lines 2186–2330) as the pattern for new tests.
- `crates/gup-core/src/chart.rs` (the `Chart` trait, `needs_frame`).
- `crates/gup-core/src/app.rs` lines 95–135 (event loop builder) and 285–360
  (`redraw_again`, `ApplicationHandler`).
- `crates/gup-core/src/builder/scatter.rs` lines 240–319 and `builder/line.rs`
  lines 186–304 (`materialise`, `split`, `Chart` impls).
- `dogfood/src/bin/t5_live.rs`, `dogfood/src/bin/t5_stream.rs`,
  `dogfood/src/suite.rs` `fn t5_stream` (line 806) and `fn t5_live` (line 1008),
  `dogfood/README.md` "Task 5" (lines 143–155).

## User Story

> "As a developer with live data, I want to push rows to a chart from my data
> thread and see them drawn on the next frame, with only the new rows uploaded,
> so that a streaming chart costs per frame what the new data costs."

## Acceptance Criteria

### AC1: `append` is public

- [x] `Selection::append` is `pub`, its doc says what it uploads, and the
      `expect(dead_code)` attribute is gone.
- [x] Appending bumps the selection's revision (a new `appended: u64` counter in
      `revision()`), so a plot holding the selection re-resolves: a test appends
      through `plot.layer_mut::<T, M>()` and gets a non-cached resolve that
      draws the new rows.

### AC2: `Appender`

- [x] `gup_core::Appender<T>` and `gup_core::Closed` exist as in Decisions;
      `Appender<T>: Clone + Send + Sync` for `T: Send` (a `static_assertions`
      style compile check in a test).
- [x] `Selection::appender`, `ScatterChart::appender` and `LineChart::appender`
      exist as in Decisions, each with a doctest that pushes from a spawned
      thread and renders (external crate).
- [x] A plot test: a thread pushes 1,000 rows in 10 `extend` calls into a
      64-row-chunk layer; after joining, `needs_frame()` is true; one resolve
      draws all rows; column bytes written equal exactly 1,000 × stride; the
      image equals a plot built from all rows at once (byte-identical, as
      `appended_rows_upload_only_their_bytes` checks); `needs_frame()` is then
      false.
- [x] A line chart with `.color(key)`: pushing rows of an existing key extends
      that key's line; a new key adds a new line layer with the same width and
      the next palette colour and a legend entry (golden-free pixel check of the
      new line's colour and the legend's text run).
- [x] Dropping the chart makes `push` return `Err(Closed)`.

### AC3: Hosts wake on new rows

- [x] `Chart::set_waker` exists (Decisions); `Plot`, `ScatterChart`, `LineChart`
      and `BarChart` implement or forward it.
- [x] A test with a counting `std::task::Wake` impl: after `set_waker`, each
      `push`/`extend` wakes it exactly once; an appender created before
      `set_waker` also wakes it.
- [x] `GupApp` installs a waker that redraws the window (Decisions).
- [x] A new example `crates/gup-core/examples/live_append.rs` (at most 40
      lines): a line whose rows a thread pushes every 50 ms for 3 s, shown by
      `gup_core::show`. Run it under the display for 3 s, capture the window
      (story-worker's screenshot procedure) with no pointer input, and read the
      image: the line has grown well past its first rows.

### AC4: Dogfood

- [x] `t5_live` creates each chart once and feeds it through `appender()` every
      tick; the `line_chart(&pts)` / `scatter_chart(&pts)` rebuilds are gone.
      Its three checks pass unchanged.
- [x] `t5_stream` is rewritten on `gup_core` (Decisions) and imports nothing
      from `gup`. The gap `STREAM_NEVER_READY` is deleted; its two checks are
      plain checks with the same thresholds, and `t5_stream_2.png` gains three
      checks, one per batch, as in Decisions.
- [x] `xvfb-run -a ./dogfood/run_all.sh`: no FAIL, no XPASS. Paste the t5 lines.
- [x] `dogfood/README.md` "Task 5" describes the appender and drops "task 5's
      missing append" and the rebuild note.

## Decisions

- **New module** `crates/gup-core/src/append.rs`:

  ```rust
  /// Appends rows to a chart from any thread. Rows pushed between two
  /// frames are evaluated and uploaded together at the next resolve.
  pub struct Appender<T> { /* tx: mpsc::Sender<Vec<T>>, shared: Arc<Feed> */ }
  impl<T> Clone for Appender<T> { .. }
  impl<T: Send + 'static> Appender<T> {
      pub fn push(&self, row: T) -> Result<(), Closed>;
      pub fn extend(&self, rows: impl IntoIterator<Item = T>) -> Result<(), Closed>;
  }
  /// The chart that owned an [`Appender`] was dropped.
  #[derive(Copy, Clone, Debug, PartialEq, Eq)]
  pub struct Closed;   // + Display ("the chart was dropped") + std::error::Error
  ```

  `Feed` (private) holds `pending: AtomicUsize` (rows queued) and
  `waker: Mutex<Option<Waker>>`. `push` and `extend` send one `Vec<T>` message,
  add its length to `pending`, then `wake_by_ref` the waker if set.
  `std::sync::mpsc`: no new dependency, and it works on wasm32.

- **`Selection`**:
  - `pub fn appender(&mut self) -> Appender<T>`: creates the channel on first
    call (stored as `feed: Option<(Receiver<Vec<T>>, Arc<Feed>)>`), then clones
    the sender.
  - `Layer` gains `fn drain_feed(&mut self) -> Result<bool>` (concatenates every
    queued message into **one** `append`, so one draw-order batch; resets
    `pending`; true if rows came), `fn feed_pending(&self) -> bool`, and
    `fn set_waker(&mut self, waker: Option<&Waker>)`.
- **Where rows land**: `Plot::resolve` drains every layer first, before the
  cache check. A drain that appended changes the revision sum, so the cache
  misses. An error from `append` is returned from `resolve`.
- **`Chart` trait** gains, with a doc and a default no-op:

  ```rust
  /// Called by hosts with a waker that schedules a frame. Charts with
  /// appenders wake it when rows arrive.
  fn set_waker(&mut self, waker: std::task::Waker) {}
  ```

  `Plot` stores it and passes it to every layer, now and on `add`. `Plot`'s
  `needs_frame` is also true while any layer has pending rows.

- **Builders**:
  - `ScatterChart::appender(&mut self) -> Appender<T>`: materialises, then the
    layer's appender. Layer setters panic afterwards, as after `layer_mut`.
  - `LineChart::appender(&mut self) -> Appender<T>`: without `.color(key)`, the
    single layer's appender. With a key, a builder-level feed:
    `LineChart::resolve` drains it before `plot.resolve`, splits the rows by key
    (`split`), appends each key's rows to that key's layer, and adds a layer for
    a new key exactly as `materialise` does. So `materialise` keeps `ops` (no
    `clear`), and the colour `ScaleRef` and the key → layer index map become
    fields. `needs_frame` and `set_waker` include the builder feed.
  - `BarChart`: no appender. Bars are aggregated per category and stack on the
    CPU at build time; `set_waker` forwards to the plot.
- **`GupApp` waker**: the event loop is built with a user event
  (`EventLoop::<Wake>::with_user_event()`, `struct Wake;`). The waker is an
  `Arc` of a private `std::task::Wake` impl holding the `EventLoopProxy<Wake>`
  (inside a `Mutex` if the proxy is not `Sync`); waking sends `Wake`, and
  `user_event` requests a redraw. `set_waker` is called once when the window
  opens, before the first frame.
- **`t5_live` port**: build `line_chart(&[])`-style charts once with the
  existing settings (fixed domains), keep `Appender`s, and `extend` them each
  tick with that tick's new samples. `ChartTexture` stays.
- **`t5_stream` port**: `gup_core::scatter(Vec::<S>::new())` with `.x(|s| s.i)`,
  `.y(|s| s.v)` where `i` is the row index as `f64` and
  `v = (12.0 * i / 200.0).sin() * 0.7`, `.fill(Color::hex(0xd62728))`,
  `.radius(3.0)`; push batches of 50 rows (0–49, 50–99, 100–149) through an
  appender, and `save_png` 400×300 after each to the same file names. No
  `tokio`. New checks on `t5_stream_2.png`: "batch 1 drawn", "batch 2 drawn",
  "batch 3 drawn", each `Measure::Hue { rgb: RED, min_px: 50 }` on the left,
  middle and right third of the plot rect, regions from a fresh render's
  `Layout` (as GUP-465 did).
- **Not here**: eviction (GUP-482), the web canvas waker (GUP-481), egui's
  repaint waker (GUP-483).

## Escalation Triggers

- `wasm.per_row_type.gz_bytes` (GUP-459) fails because the new `Layer` methods
  add per-row-type code beyond its budget.
- Draining inside `resolve` breaks the GUP-444 invariant "a clean resolve writes
  nothing" for a chart with an appender but no new rows.
- `EventLoop::with_user_event` changes winit behaviour seen by
  `tests/app_once.rs` or the screenshot path.
- A line key appended for the first time cannot share the existing colour
  `ScaleRef` without re-resolving every line layer's colour encoding.

## Out of Scope

- `Window::LastRows` eviction: GUP-482.
- `CanvasApp`'s waker and a browser append scene: GUP-481.
- `gup-egui`'s waker: GUP-483.
- Appending to bar charts.

## Technical Tasks

- [x] `append.rs`: `Appender`, `Closed`, `Feed`; exports.
- [x] `Selection`: `pub append`, `appended` revision, `appender`, the three
      `Layer` methods.
- [x] `Plot`: drain before the cache check, `needs_frame`, `set_waker`.
- [x] `Chart::set_waker`; builders' `appender` and forwarding; `LineChart` key
      routing.
- [x] `GupApp` user-event waker; `live_append` example; screenshot.
- [x] Tests (AC1–AC3); dogfood t5 ports; README; suite run.

## Dependencies

### Prerequisite Stories

- GUP-414: S4a Column Store Chunking and Append ✅.
- GUP-462, GUP-463: scatter and line builders ✅.
- GUP-465: dogfood on gup-core ✅.

### Enables Stories

- GUP-481: S12b Web Canvas Waker — needs `Chart::set_waker`.
- GUP-482: S12c Window Eviction — evicts what appenders add.
- GUP-483: S13a gup-egui — wires egui's repaint as the waker.
- GUP-486: S14b — `t5_stream` no longer uses `ComposedChart`.

## Testing Strategy

- **Unit tests**: `Appender` (closed, pending count, waking), `drain_feed`
  batching into one append.
- **Integration tests**: AC2–AC3 in `plot.rs` tests and
  `crates/gup-core/tests/`.
- **Visual validation**: the `live_append` window screenshot; dogfood
  `t5_live.png`, `t5_stream_2.png`.
- **Performance**: `mask perf-budget`; zoom rows unchanged.

## Success Metrics

- [x] The RFC's example (e), minus `window`, works as written on `gup_core`.
- [x] `STREAM_NEVER_READY` is gone and both task 5 binaries use the appender.

## Risk Assessment

- **Medium**: `LineChart`'s key routing duplicates `materialise`. _Mitigation_:
  factor one `add_series(rows)` used by both.
- **Low**: a busy feeder wakes the window on every push. _Mitigation_: winit
  coalesces redraw requests; resolve drains everything queued at once.
- **Medium**: trybuild `.stderr` churn: the snapshots quote line numbers in
  `src/selection.rs`, which this story edits near the top. _Mitigation_:
  re-bless with `TRYBUILD=overwrite` only for line-number diffs; escalate on
  message changes.

## Definition of Done

- [x] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] `xvfb-run -a ./dogfood/run_all.sh` passes (no FAIL, no XPASS)
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples` and
      `cargo check -p gup-core --examples`
- [x] `mask wasm-browser` passes (this story touches the host input loop and
      code compiled to wasm)
- [x] `mask wasm-size` run and the scatter and per-row-type figures reported in
      the Implementation Summary (the orchestrator re-records the budget)
- [x] `mask perf-budget` table pasted (story-worker step 9)
- [x] Rendered output verified by eye: the `live_append` screenshot,
      `t5_live.png`, `t5_stream_2.png`
- [x] `mask old-path-loc` recorded (expected unchanged)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/append.rs`: `Appender<T>` (Clone, Send, Sync), `Closed`,
  and the private feed (mpsc channel, pending count, waker).
- `Selection::append` is public with a doc; `appended` counter in the revision;
  `Selection::appender`; `Layer::{drain_feed, feed_pending, set_waker}`.
  `Plot::resolve` drains every layer before the cache check; `Plot::needs_frame`
  is true while rows are pending; `Chart::set_waker`.
- `ScatterChart::appender`, `LineChart::appender` (keyed charts route rows to
  per-key layers through a builder feed and one shared colour `ScaleRef`),
  `BarChart` forwards the waker only.
- `GupApp`: user-event loop (`EventLoop::<Wake>`), `ProxyWaker`, installed in
  `open`.
- `crates/gup-core/examples/live_append.rs`; `tests/append.rs` (12 tests: thread
  feed with exact bytes, draw order, culling, brush bits, picks, SVG, cache
  miss, idle resolve writes nothing, closed, waker counts, keyed line); 3
  doctests.
- Dogfood: `t5_live` builds each chart once and `extend`s each tick; `t5_stream`
  is on `gup_core` with three batch checks; `STREAM_NEVER_READY` deleted; README
  updated. Suite: 8 pass, 3 XFAIL, 0 FAIL, 0 XPASS.
- Evidence: `mask wasm-size`: scatter 324205 B gz (recorded 319085, +1.6%, limit
  +2%), over-wgpu 291845, per-row-type 728 (budget 2600). `mask perf-budget`:
  first run 1 timing FAIL (`pipeline.link_create.median_ms` 1.347 vs 1.332) at
  load average 18.6; second run all pass at load 8 (`uptime` after: load 4.18).
  `mask old-path-loc`: TOTAL 26601. `mask wasm-browser`: PASS. Screenshots read:
  `/tmp/live_append.png` (line grown to t = 2.9 s with no input), `t5_live.png`,
  `t5_stream_2.png`.

## Retrospective

**Completed**: 2026-10-19

### Key Technical Learnings

#### Appends compose with the landed systems without extra code

- **Challenge**: appends touch chunking, draw order, culling, brush bits, picks
  and SVG.
- **Solution**: one drain point (`Plot::resolve`, before the cache check)
  funnels every append into `Selection::append`, which GUP-414 and GUP-431
  already tested. Tests compare the appended chart to a chart built at once.
- **Pattern**: test composition by "appended == built at once" image equality.

#### Brush semantics after appends

- **Challenge**: a brush holds data rows chosen at drag time.
- **Solution**: appended rows are not in the brush and render dimmed until the
  next drag; the test asserts that rather than inventing re-brushing.
- **Pattern**: say what the semantics are in the test name.

### Architectural Decisions

#### `Receiver` behind a `Mutex`

- **Decision**: `FeedRx` wraps the `Receiver` in a `Mutex` because layers must
  be `Sync`.
- **Reasoning**: only the owner drains, so the lock is uncontended.
- **Trade-off**: one uncontended lock per resolve for charts with a feed.

### Development Workflow Insights

- A commit hook that failed printed its error only deep in the log; commit with
  output redirected to a file and grep it.
- Windowed screenshot: run the example under `xvfb-run` with
  `import -window root` after a sleep.
- The first commit attempt had a misplaced `#[cfg]` (my insert landed between an
  attribute and its item); the hook caught it on the wasm32 clippy.

### Follow-up Stories

None. GUP-482 (eviction), GUP-481 and GUP-483 (wakers) already exist.
