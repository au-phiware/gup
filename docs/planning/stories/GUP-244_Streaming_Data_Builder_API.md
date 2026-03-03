# GUP-244: Streaming Data Builder API

## Story Overview

**Initiative**: Debug & Development Tools **Status**: 🚧 In Progress **Created**:
2026-03-01

## Context

GUP-015 establishes the low-level infrastructure for real-time data streaming:
ring buffers, double-buffered GPU memory, dirty-region tracking, and the
`StreamUpdate<T>` enum that drives incremental buffer writes. That foundation is
intentionally mechanical — it handles the _how_ of moving bytes to the GPU with
minimal latency, but it does not address the _what_ a developer needs to express
when wiring up a live data source.

Today a developer who wants to build a live monitoring dashboard or a financial
tick chart must manually instantiate `DataStream<T>`, choose buffer sizes, wire
up the update queue, and hand-roll whatever eviction or backpressure logic their
use-case demands. This is error-prone and verbose. A fluent builder layer that
captures these choices declaratively will make the correct patterns easy to
reach and the wrong patterns hard to fall into.

GUP-002's `Selection<T, M>` is the rendering unit developers interact with most
often. Adding a `.stream(data_stream)` entry-point keeps the streaming story
consistent with the existing `set_data()` / `join()` vocabulary, so developers
can adopt streaming incrementally without rewriting their rendering code. An
observable subscriber callback (`stream.subscribe(|update| ...)`) rounds out the
API by letting application code react to incoming data — for example, to
auto-scroll a time axis or update a legend.

## User Story

> "As a visualization developer, I want a fluent builder API for configuring a
> `DataStream<T>` so that I can declaratively specify buffer capacity, update
> mode, and backpressure strategy — and connect the stream to a `Selection` with
> a single call — without having to manage low-level GPU buffer details myself."

## Acceptance Criteria

### AC1: Fluent Builder Constructs a Valid `DataStream<T>`

- [ ] `DataStream::builder()` returns a `DataStreamBuilder<T>` in the
      builder-pattern style.
- [ ] `.capacity(n: usize)` sets the maximum number of data points the stream
      can hold before backpressure or eviction kicks in.
- [ ] `.mode(StreamMode)` accepts one of `StreamMode::AppendOnly`,
      `StreamMode::SlidingWindow`, and `StreamMode::RingBuffer`; each variant
      changes the eviction/overwrite semantics for the underlying GUP-015
      buffer.
- [ ] `.backpressure(BackpressureStrategy)` accepts at least `Block`, `Drop`,
      and `YieldOldest` variants, and the chosen strategy is enforced when the
      stream is at capacity.
- [ ] `.build(context: &GupContext)` consumes the builder, validates all
      parameters (returns `Err` for zero capacity or conflicting options), and
      returns a `DataStream<T>`.
- [ ] All builder methods are chainable and compile without ergonomic friction
      (no unnecessary `mut`, no `Arc` wrapping required at call sites).

### AC2: `Selection` Integration via `.stream()`

- [ ] `Selection<T, M>` gains a `.stream(data_stream: DataStream<T>)` method
      that replaces (or supplements) `set_data()` for live data sources.
- [ ] After `.stream()` is called, subsequent pushes to the `DataStream` trigger
      incremental GPU buffer updates via the GUP-015 primitive without requiring
      a full `set_data()` / re-join cycle.
- [ ] A `Selection` with an active stream renders correctly when `.render()` is
      called on successive frames with interleaved `push` and `push_batch`
      calls.
- [ ] Calling `.stream()` on a `Selection` that already has static data set via
      `set_data()` replaces the static binding with the stream, and the old data
      is dropped.

### AC3: Observable Subscriber Pattern

- [ ] `DataStream<T>` exposes a
      `.subscribe(callback: impl Fn(&StreamUpdate<T>)     + Send + 'static)`
      method that registers a callback invoked for every committed update.
- [ ] Multiple subscribers can be registered on the same stream; all are called
      in registration order.
- [ ] The callback receives a reference to the `StreamUpdate<T>` after it has
      been applied to the GPU buffer, so subscribers observe the post-commit
      state.
- [ ] `.unsubscribe()` or an equivalent handle-based API is provided to
      deregister a subscriber without dropping the stream.

### AC4: Performance — Builder Overhead ≤ 1 ms

- [ ] A microbenchmark demonstrates that the cost of constructing a
      `DataStreamBuilder`, calling `.capacity()`, `.mode()`, `.backpressure()`,
      and `.build()` adds no more than 1 ms of wall-clock time beyond the
      baseline cost of calling the equivalent GUP-015 primitives directly.
- [ ] The per-push overhead introduced by the subscriber dispatch loop (for a
      stream with zero subscribers) is not measurable above noise in a criterion
      benchmark.

### AC5: Error Handling and Ergonomics

- [ ] `DataStreamBuilder::build()` returns
      `Result<DataStream<T>, DataStreamError>` with distinct error variants for
      invalid capacity, unsupported mode/backpressure combinations, and missing
      `GupContext`.
- [ ] `DataStreamError` implements `std::error::Error` and produces
      human-readable messages suitable for `?` propagation in example code.
- [ ] All public API items carry `///` doc-comments with at least one usage
      example in doctests.

## Technical Tasks

- [ ] Define `StreamMode` enum (`AppendOnly`, `SlidingWindow`, `RingBuffer`) in
      `src/streaming/mode.rs`; map each variant to the correct GUP-015 buffer
      configuration.
- [ ] Define `BackpressureStrategy` enum (`Block`, `Drop`, `YieldOldest`) in
      `src/streaming/backpressure.rs` with associated logic for each strategy.
- [ ] Implement `DataStreamBuilder<T>` struct in `src/streaming/builder.rs` with
      chainable setter methods and a `build()` method that validates parameters
      and delegates to GUP-015's `DataStream::new`.
- [ ] Extend `DataStream<T>` in `src/streaming/stream.rs` with: - A
      `subscribers: Vec<Box<dyn Fn(&StreamUpdate<T>) + Send + 'static>>`
      field. - `subscribe()` and `unsubscribe()` / `SubscriberHandle` methods. -
      Subscriber dispatch after each committed update.
- [ ] Add `Selection::stream(data_stream: DataStream<T>)` in `src/selection.rs`;
      set the rendering mode to `Streaming` and hook up the incremental buffer
      path from GUP-015.
- [ ] Write unit tests for `DataStreamBuilder` covering valid construction, each
      error variant, and all combinations of `StreamMode` ×
      `BackpressureStrategy`.
- [ ] Write unit tests for the subscriber pattern: zero subscribers, one
      subscriber, multiple subscribers, and unsubscribe.
- [ ] Write an integration test that constructs a `Selection` with
      `.stream(...)`, pushes a batch, and asserts the GPU buffer length is
      correct.
- [ ] Add a criterion benchmark (`benches/streaming_builder.rs`) measuring
      builder construction overhead and per-push subscriber dispatch cost.
- [ ] Add an `examples/streaming_live_chart.rs` demonstrating end-to-end usage:
      builder → `DataStream` → `.stream()` on a `Selection` → render loop with
      simulated incoming data.
- [ ] Update `///` doc-comments and add doctests for all public items.

## Dependencies

### Prerequisite Stories

- GUP-002: Core Selection Type ✅ — provides `Selection<T, M>` which the
  `.stream()` integration method extends.
- GUP-015: Real-Time Data Streaming Core 📋 — provides `DataStream<T>`,
  `StreamUpdate<T>`, `StreamingBufferManager`, and the incremental GPU buffer
  update primitive that the builder wraps.

### Enables Stories

- GUP-258: Streaming Data Manager for LOD — the LOD manager builds directly on
  the `DataStreamBuilder` API established here to configure per-level stream
  modes and backpressure.

## Testing Strategy

- **Unit tests**: `DataStreamBuilder` construction paths (valid, each error
  variant); `StreamMode` and `BackpressureStrategy` mapping to GUP-015
  primitives; subscriber registration, dispatch order, and unsubscription.
- **Integration tests**: `Selection::stream()` replaces static data binding;
  interleaved `push` / `push_batch` calls produce the correct GPU buffer state
  across multiple render frames.
- **Doctest compilation**: All `///` examples compile and pass under
  `cargo test --doc`.
- **Performance**: Criterion benchmark in `benches/streaming_builder.rs`
  verifies that builder-layer overhead is ≤ 1 ms and per-push subscriber
  dispatch with zero subscribers is noise-level.
- **Visual validation**: `examples/streaming_live_chart.rs` compiles and, when
  run manually, renders a live-updating chart without GPU validation errors or
  visible tearing.

## Success Metrics

- [ ] `DataStreamBuilder` covers all three `StreamMode` variants and all three
      `BackpressureStrategy` variants with tests for each combination.
- [ ] `Selection::stream()` integration test passes and demonstrates that no
      full re-join is triggered on incremental pushes.
- [ ] Criterion benchmark confirms builder overhead ≤ 1 ms over the GUP-015
      baseline on a representative development machine.
- [ ] `examples/streaming_live_chart.rs` compiles and runs without GPU
      validation errors.
- [ ] All public API items have doc-comments; `cargo doc --no-deps` produces no
      warnings.

## Risk Assessment

- **Medium**: The precise mapping from `StreamMode` and `BackpressureStrategy`
  to GUP-015's internal buffer and eviction primitives depends on API details
  that GUP-015 has not yet finalised (it is 📋 Planned). The builder design may
  need adjustment once GUP-015's public surface stabilises. _Mitigation_: Draft
  the builder against a minimal trait abstraction (`StreamingBackend`) so that
  the surface can be swapped if GUP-015 internals change before implementation
  begins.

- **Low**: The subscriber dispatch loop, if implemented naively (e.g. with a
  `Mutex`-guarded `Vec`), could introduce contention on high-throughput streams.
  _Mitigation_: Use `RwLock` for the subscriber list (reads are far more
  frequent than writes), and document that subscribers must be cheap — expensive
  work should be deferred to a channel.

- **Low**: Adding `.stream()` to `Selection<T, M>` could create confusion with
  the existing `set_data()` lifecycle (especially if both are called). Clear
  documentation and a runtime assertion (or `debug_assert!`) will prevent
  accidental misuse. _Mitigation_: Document the contract explicitly; consider
  making `.stream()` an associated function on a `StreamingSelection` newtype if
  the API surface becomes ambiguous.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
