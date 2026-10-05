# GUP-402: Fix Accessibility Nested-Runtime Panic

## Story Overview

**Initiative**: Accessibility **Status**: 📋 Planned **Created**: 2026-10-05

## Context

`LinuxAccessibility::initialize()` (`src/accessibility/platform.rs:202-221`)
builds its own single-threaded Tokio runtime
(`tokio::runtime::Builder::new_current_thread().enable_all().build()`, line 204)
and calls `runtime.block_on(..)` on it (line 212) to connect to AT-SPI2 over
D-Bus. `update_accessibility_tree` (line 238) and `announce` (line 286) call
`block_on` on the same stored runtime again. Tokio panics with "Cannot start a
runtime from within a runtime" whenever `block_on` is called on a thread that is
already executing inside another Tokio runtime — which is exactly what happens
in any application built with `#[tokio::main]` that constructs an
`AccessibilitySystem` (which calls `LinuxAccessibility::initialize()`
internally).

This is not hypothetical: `examples/pattern_pipeline_demo.rs` has
`#[tokio::main]` (line 36) and constructs `AccessibilitySystem::new()` (line
141), and panics on Linux as a result. It is tracked today as a known failure in
`tests/visual_regression/expected_failures.toml`:

```toml
[[expected]]
case = "examples/pattern_pipeline_demo"
check = "render"
reason = "panics at src/accessibility/platform.rs:212 'Cannot start a runtime from within a runtime' (block_on inside the tokio main runtime)"
tracking = "RFC-001 S14 (old accessibility wiring replaced) / follow-up if the example survives"
```

`examples/pattern_rendering_demo.rs` and `examples/web_accessibility_demo.rs`
also construct `AccessibilitySystem` and are named in this story's source
request as affected, though only `pattern_pipeline_demo` currently has
`#[tokio::main]` wrapping the call — the other two are at risk if ever run
inside an async caller, and auditing them is part of this story's "scope the
class" requirement below.

This is a real bug reachable by any user of the public
`AccessibilitySystem`/`gup::accessibility` API on Linux inside an async
application — exactly the kind of WCAG-relevant defect the project's
accessibility conformance work (GUP-016, GUP-111, GUP-112, GUP-272) commits to.
Unlike most Linux-accessibility work, which is superseded by RFC-001 T5's
eventual accessibility redesign on the new core, this is a crash, not a design
gap, and the project's own tracking entry says it should not simply wait for T5:
"a real-user bug on a WCAG commitment, so it is not parked even though
accessibility gets redesigned in T5."

**Scope the class, not the instance.** A repo-wide search for
`tokio::runtime::Runtime`/`Builder`+`block_on` (as distinct from
`pollster::block_on`, which is not a Tokio runtime and does not have this
hazard) finds the same shape in two other places:

- `src/wasm_bench_interaction.rs:380-382`, `run_native_benchmarks()`: creates a
  `tokio::runtime::Runtime` and calls `block_on`. It is `pub`, callable from an
  external crate, and currently only invoked by `src/bin/wasm_bench_native.rs`
  (not nested in a runtime today), but as public library API it has the same
  latent hazard if a downstream consumer calls it from inside their own
  `#[tokio::main]`.
- `src/gpu_timer.rs:210-215, 253-258, 292-297`: the same
  `Builder::new_current_thread()...block_on` shape, but exclusively inside
  `#[cfg(test)] mod tests` — not reachable from any production code path, so it
  cannot panic a user's application, though it is the same pattern and worth a
  documented decision rather than silent exclusion.

`pollster::block_on` (used extensively elsewhere in `src/`, e.g. `selection.rs`,
`render.rs`, `pipeline_cache.rs`) is a different mechanism: it polls a future on
the current thread with a simple waker and does not itself start a "runtime" in
Tokio's sense, so it does not have Tokio's reentrancy guard and is not in scope
for this fix — those call sites are not part of the hazard class this story
addresses.

## User Story

> "As a developer embedding Gup charts in a `#[tokio::main]` application on
> Linux, I want `AccessibilitySystem::new()` to work (or fail gracefully)
> instead of panicking, so that accessibility support does not make my
> application unusable inside the async runtime I already have."

## Acceptance Criteria

### AC1: `LinuxAccessibility` no longer panics inside an existing Tokio runtime

- [ ] A new test constructs an `AccessibilitySystem` (or directly
      `LinuxAccessibility::initialize()`) from inside a `#[tokio::main]`-style
      async context (e.g. `#[tokio::test]` or an explicit
      `tokio::runtime::Runtime::new().block_on(async { .. })` wrapping the call)
      and asserts it does **not** panic with "Cannot start a runtime from within
      a runtime" — it either succeeds (if AT-SPI2/D-Bus is reachable in the test
      environment) or returns an
      `AccessibilityError::PlatformUnavailable`/`Other` `Err`, never a panic.
- [ ] The same guarantee holds when `LinuxAccessibility::initialize()` is called
      from a plain synchronous `main()` with no Tokio runtime active (the
      existing, currently-working case) — this fix must not regress the
      non-nested path.
- [ ] The chosen fix pattern is documented in a doc comment on
      `LinuxAccessibility` explaining why nested runtime creation was unsafe and
      what replaces it (see Risk Assessment for candidate approaches).

### AC2: The known example failure is resolved (user-visible)

- [ ] **AC (user-visible, required)**: `examples/pattern_pipeline_demo.rs` runs
      headlessly (`GUP_SCREENSHOT_PATH`) without panicking on Linux, and its
      entry is **removed** from
      `tests/visual_regression/expected_failures.toml`. Per the harness's XPASS
      rule, leaving a fixed expected-failure entry in place would itself now
      fail the smoke suite, so removal is required, not optional.
- [ ] `examples/pattern_rendering_demo.rs` and
      `examples/web_accessibility_demo.rs` are checked for the same hazard (per
      the Context section's audit) and confirmed not to panic when run
      headlessly; if either turns out to be wrapped in `#[tokio::main]` or
      otherwise reachable from inside a Tokio runtime, the fix in AC1 covers it
      without further example-specific changes (since the fix is in the shared
      `LinuxAccessibility` type).

### AC3: The other two `Runtime`/`block_on` sites are each given an explicit disposition

- [ ] `src/wasm_bench_interaction.rs::run_native_benchmarks()` is either fixed
      with the same pattern as AC1 (since it is public API with the same latent
      hazard), or explicitly documented with a doc comment stating it must not
      be called from inside an existing Tokio runtime and why a fix was not
      applied — a decision, not a silent gap.
- [ ] `src/gpu_timer.rs`'s three test-only `Runtime`/`block_on` sites are either
      left as-is with a one-line note recording why they are out of scope (not
      reachable from production code), or converted to `#[tokio::test]` /
      `pollster::block_on` for consistency with the rest of the test suite,
      whichever the implementer judges lower-risk — record the choice.

## Technical Tasks

- [ ] Reproduce the panic: run `pattern_pipeline_demo` on Linux and confirm the
      exact panic message and location match the tracked expected failure.
- [ ] Implement the fix in `src/accessibility/platform.rs` (see Risk Assessment
      for the two candidate approaches: a background OS thread with its own
      runtime plus a blocking channel handoff, or verifying `zbus`'s default
      executor does not require a Tokio reactor and switching to
      `pollster::block_on` directly). Apply it to all three call sites that
      currently call `runtime.block_on` (`initialize`,
      `update_accessibility_tree`, `announce`).
- [ ] Add the nested-runtime test (AC1) and the non-nested regression test.
- [ ] Re-run `pattern_pipeline_demo`, `pattern_rendering_demo` and
      `web_accessibility_demo` headlessly; remove the resolved entry from
      `tests/visual_regression/expected_failures.toml` (AC2).
- [ ] Audit and disposition `wasm_bench_interaction.rs` and `gpu_timer.rs`
      (AC3).
- [ ] Run the existing accessibility test suite
      (`cargo test accessibility -- --test-threads=1`) to confirm no regression
      on macOS/Windows stub backends or the Web backend.

## Dependencies

### Prerequisite Stories

- None.

### Enables Stories

- None known. This is a standalone bug fix; it does not block or get blocked by
  the RFC-001 migration (T5 eventually redesigns accessibility on the new core,
  per the project's own note that this fix should not wait for that).

## Testing Strategy

- **Unit tests**: `LinuxAccessibility::initialize()` called from inside an
  active Tokio runtime does not panic (AC1); called from a plain sync context
  still works as before (regression).
- **Integration tests**: `pattern_pipeline_demo` runs headlessly via
  `GUP_SCREENSHOT_PATH` without panicking (AC2), verified through
  `tests/examples_smoke.rs`'s existing mechanism.
- **Visual validation**: not applicable — this is a crash fix, not a rendering
  change; no new golden image is needed.

## Success Metrics

- [ ] `cargo test --test examples_smoke -- --include-ignored --test-threads=1`
      no longer lists `examples/pattern_pipeline_demo` as an expected failure,
      and does not report an XPASS for it either (because the stale entry was
      removed).
- [ ] A test proves `LinuxAccessibility::initialize()` is safe to call from
      inside an existing Tokio runtime.
- [ ] `wasm_bench_interaction.rs` and `gpu_timer.rs` each have an explicit,
      recorded disposition for the same hazard pattern.

## Risk Assessment

- **Medium**: the correct fix depends on whether `zbus` (the AT-SPI2/D-Bus
  client, `src/accessibility/atspi.rs`) requires an active Tokio reactor to
  drive its socket I/O, or whether (as the project's `zbus = "5.2"` dependency
  with no `tokio` feature flag suggests) it uses its own executor-agnostic I/O
  driver. If the latter, switching `block_on(async { .. })` to
  `pollster::block_on(async { .. })` (used everywhere else in this codebase for
  exactly this kind of "run one future to completion synchronously" need)
  removes the nested-runtime hazard entirely, since `pollster` is not a Tokio
  runtime and has no reentrancy guard. If `zbus` does need an active reactor,
  the fallback is to run the AT-SPI2 connection and each subsequent call on a
  dedicated background OS thread with its own single-threaded Tokio runtime,
  handing results back to the caller over a `std::sync::mpsc` channel with a
  plain blocking `recv()` (not `block_on`) on the calling thread — this works
  regardless of whether the caller is already inside a Tokio runtime, because
  the calling thread never tries to start or enter one. _Mitigation_: try the
  `pollster` approach first (smaller, more consistent with the rest of the
  codebase) and fall back to the background-thread pattern only if `zbus` calls
  actually hang or error without a reactor — verify with the nested-runtime test
  (AC1) against a real or mocked AT-SPI2 bus, not by inspection alone.
- **Low**: the fix changes behaviour on the only backend (`LinuxAccessibility`)
  that currently has real users on Linux desktop examples; a mistake here could
  trade a panic for a silent failure to connect to AT-SPI2. _Mitigation_: AC1
  requires the non-panicking path to still either succeed or return a typed
  `Err`, and the existing accessibility test suite must stay green.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `examples/pattern_pipeline_demo` runs headlessly without panicking and its
      expected-failure entry is removed
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
