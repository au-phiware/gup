# GUP-412: Lint the wasm32 Target in the Gates

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: ✅ Complete
(2026-10-10) **Created**: 2026-10-10

## Context

Every clippy gate (`mask all-check`, the pre-commit hook, CI's Lint workflow)
lints the host target only. Code behind `#[cfg(target_arch = "wasm32")]` is
compiled by the Wasm workflow (`cargo build --target wasm32-unknown-unknown`)
but never linted, and wasm32 changes what some lints see: on wasm32 wgpu's types
are not `Send`/`Sync`.

[GUP-410](GUP-410_Surface_WebGPU_Errors_From_Gup_Core.md) found one such
finding, and GUP-411 confirmed it on 2026-10-10:

```text
$ cargo clippy -p gup-core --target wasm32-unknown-unknown -- -D warnings
error: usage of an `Arc` that is not `Send` and `Sync`
   --> crates/gup-core/src/selection.rs:251:19
error: could not compile `gup-core` (lib) due to 1 previous error
```

`cargo clippy --workspace --target wasm32-unknown-unknown` stops at the same
error, because every other member depends on gup-core, so how many findings lie
beyond it is unknown. gup-core is the crate whose browser path (RFC-001 S8) is
next, so its wasm32 code will grow.

## User Story

> "As a contributor changing code that runs in the browser, I want the gates to
> lint the wasm32 target, so that a lint that only fires there is caught before
> it reaches main."

## Acceptance Criteria

- [x] `selection.rs:251` is fixed (or allowed with a reason, if the `Arc` is
      deliberate on wasm32, where there are no threads), and every further
      wasm32 finding in the workspace is fixed or allowed with a reason. (Libs
      of the wasm32 members; gup-culling-lod is excluded as a crate, with its
      reason, and its 16 findings are left: see the summary.)
- [x] `mask all-check` runs `cargo clippy --target wasm32-unknown-unknown` with
      `-D warnings` over the members that build for wasm32 (at least gup-core,
      gup-text and gup; decide `--all-targets` and features per crate and record
      why).
- [x] CI's Lint workflow gets the same check (it runs `mask all-check`).
- [x] The pre-commit hook's scoped mode adds the wasm32 pass for touched crates
      that build for wasm32, or the story records why it is left to full mode
      and CI (time budget, GUP-398).
- [x] Seeded proof (GUP-398's convention): a wasm32-only lint (e.g. the current
      `Arc` finding, reintroduced) fails `mask all-check` and passes a host-only
      clippy run.

## Technical Tasks

- [x] List wasm32 findings crate by crate
      (`cargo clippy -p <crate> --target wasm32-unknown-unknown`), fixing
      gup-core first.
- [x] Add the pass to `maskfile.md`'s `all-check` (and `all-fix`/`lint` for
      consistency) and, if cheap enough, to `scripts/pre_commit.sh`.
- [x] Measure the added time, warm and cold, and the disk for wasm32 check
      artifacts in the shared build directory (GUP-411).

## Dependencies

### Prerequisite Stories

- GUP-398 ✅: the gate conventions and seeded-proof rule.
- GUP-410 ✅: found the finding.

### Enables Stories

- RFC-001 S8 (gup-core's wasm entry point) lands on a linted target.

## Testing Strategy

- **Seeded proof**: one wasm32-only lint, observed failing the new pass and
  passing the host pass.
- **Gate timing**: warm `mask all-check` before and after.

## Success Metrics

- [x] Zero wasm32 clippy findings in the workspace, enforced by the gates (the
      libs of the wasm32 members; not gup-culling-lod or test targets).
- [x] Warm `mask all-check` grows by no more than about 20 s (32.3 s to 34.7 s).

## Risk Assessment

- **Medium**: findings beyond gup-core may be many in the root crate's wasm
  code. Mitigation: start with gup-core and gup-text, and add the root crate in
  a follow-up if it is large.
- **Low**: a wasm32 check build adds disk. Mitigation: check mode only (rmeta).

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] Lint and format clean: `mask all-check`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`scripts/clippy_wasm32.sh` runs strict clippy (`-D warnings`) on
`wasm32-unknown-unknown`, with default features and with `--all-features`, over
the lib of every workspace member except those it lists as excluded, each with
its reason. `mask all-check` (and so CI's Lint workflow), `all-fix`, `lint` and
`lint-check` run it on every wasm32 member. The pre-commit hook's scoped mode
runs it on the wasm32 members among the crates it lints and prints them in its
plan (`wasm32=[...]`). A docs-only commit is unchanged: no Rust check.

### What is linted, and why (AC2)

| Member                  | wasm32 pass       | Why                                                                                                                                                                                                           |
| ----------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `gup`                   | lib, default, all | The browser build. `--all-targets` fails in 103 targets (criterion, tokio's runtime, blocking gup-core APIs, the lib test target itself; GUP-285B). `--all-features` builds on wasm32, `wasm-start` included. |
| `gup-core`              | lib, default, all | Tests and examples use `Context::new_blocking`/`render_blocking`, which are native-only by design. All features is default (`window`).                                                                        |
| `gup-text`, `gup-wgsl`  | lib, default, all | Their tests also lint clean on wasm32, but they run on the host, so linting them there only repeats the host lint.                                                                                            |
| `gup-visual-regression` | lib, default, all | Declares itself target-agnostic; clean.                                                                                                                                                                       |
| `gup-macros`            | excluded          | A proc-macro: it always compiles for the host.                                                                                                                                                                |
| `gup-culling-lod`       | excluded          | The quarantined parts bin (GUP-390): nothing builds it for wasm32, and code ported from it into gup-core is linted there. It has 16 `arc_with_non_send_sync` findings, left as they are.                      |

A new member is linted on wasm32 unless it is added to the exclusion list with a
reason.

### Findings and how each was resolved (AC1)

gup-core: 1. gup-text, gup-wgsl, gup-visual-regression: 0. gup lib: 30 (default
features; `wasm-start` and `--all-features` added none).

| Finding                                                                                                                                                | Resolution                                                                                                                                                                                                                                                                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `arc_with_non_send_sync`, gup-core `selection.rs` (`Arc<LayerGpu>`)                                                                                    | `#[cfg_attr(target_arch = "wasm32", expect(..., reason))]`. `Layer: wgpu::WasmNotSendSync` needs `Send + Sync` on native, so it must be `Arc` there; on wasm32 wgpu's handles are neither, so neither is the `Arc`, and it cannot cross a thread. A false positive for the target.                                                                        |
| `arc_with_non_send_sync` ×9, gup: `app.rs` ×2, `context.rs` ×2, `interaction.rs` ×2, `layout/treemap.rs`, `test_utils.rs`, `wasm_bench_interaction.rs` | Same reasoning and the same wasm32-only `expect` at each site: `Arc<GupContext>`, `Arc<RenderContext>` and `Arc<wgpu::Buffer>` are public old-path types that native shares. On the statement where possible (two assignments became `let` bindings, since attributes on assignments are unstable); on the function where the `Arc` is a tail expression. |
| `type_complexity` ×6: choropleth's region-id and tooltip closures ×3, `AccessorToShaderFunction` ×2, the stream subscriber ×1                          | The native twins carried `#[expect(clippy::type_complexity)]` and the wasm32 twins did not. A cfg'd type alias per closure type (`RegionIdFn`, `TooltipFormatter`, `SubscriberCallback`) replaces both twins, so neither target needs an `expect` (three native `expect`s removed). `AccessorToShaderFunction` had no implementor or caller: deleted.     |
| `dead_code`: five `WebDomOverlay` methods and two fields                                                                                               | The old instance-method forwarding path, replaced by `handle_*_event_static`. Deleted. It was the only code that read `DomOverlayConfig::deduplicate_events`, which was therefore a silent no-op: the option, its test assertions and its section in `docs/EVENT_FORWARDING.md` are deleted.                                                              |
| `dead_code`: `GupOptions::is_met_by`                                                                                                                   | `#[cfg(not(target_arch = "wasm32"))]`: its only caller is native-only.                                                                                                                                                                                                                                                                                    |
| `dead_code`: `wasm_api::CanvasState::surface_config`                                                                                                   | Never read (nothing reconfigures the surface): field removed.                                                                                                                                                                                                                                                                                             |
| `unused_imports` ×4, `unused_variables`, `collapsible_if` ×4, `new_without_default`, `unused_parens`                                                   | Fixed directly (the `unused_parens` was in a deleted method).                                                                                                                                                                                                                                                                                             |

So 10 wasm32-only `expect`s, every one `arc_with_non_send_sync` with the reason
above; everything else is fixed.

Considered and rejected: wgpu's `fragile-send-sync-non-atomic-wasm` feature,
which marks wgpu's types `Send + Sync` on non-atomic wasm32 and would remove the
`Arc` findings at the source. It changes the threading contract of every wgpu
type for the browser path RFC-001 S8 is about to build on, and the workspace
already designs around `WasmNotSendSync`. That is a design decision for S8, not
a lint story.

### Hook (AC4)

The scoped mode runs the wasm32 pass, not only full mode, because it is cheap:
after a one-line edit, warm, the pass adds about 15 s (16 s for a gup-core edit,
where the host clippy took 67 s; 15 s for a gup-text edit, host 44 s). A change
to `scripts/clippy_wasm32.sh` forces full mode (it is outside every crate).
`test_pre_commit.sh` gains six cases for the `wasm32=[...]` plan.

### Seeded proof (AC5)

| Seed                                                                                                                  | Host clippy (`-p <crate> --all-targets`, default and all features) | `mask all-check`                                                                                            | Hook, scoped (`scripts/pre_commit.sh <path>`)             |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| A: the `selection.rs` `expect` deleted (the original finding)                                                         | exit 0, exit 0                                                     | exit 1: `[rs] error: usage of an Arc that is not Send and Sync --> crates/gup-core/src/selection.rs:251:19` | exit 1, plan `wasm32=[gup gup-core]`, same error          |
| B: `#[cfg(target_arch = "wasm32")] pub mod gup412_seed { pub fn len(v: &Vec<u32>) -> usize { v.len() } }` in gup-text | exit 0, exit 0                                                     | exit 1: `[rs] error: writing &Vec instead of &[_] ... --> crates/gup-text/src/lib.rs:69:19`                 | exit 1, plan `wasm32=[gup gup-core gup-text]`, same error |

Both seeds were reverted.

### Timing and disk

- Warm `mask all-check`, no change: 32.3 s before, 34.7 s after (the wasm32 pass
  alone: 0.6 s).
- Cold wasm32 pass, fresh build directory: 103 s (52.6 s default features, 50.6
  s all features; feature unification differs, so the two share little).
- Disk: about 250 MB of wasm32 check artifacts (rmeta), plus host build-script
  and proc-macro output that host builds already share.
- CI: `actions/cache` saves only on a key miss, so the Lint cache key gains a
  `wasm32` segment; otherwise its existing cache would never hold the wasm32
  artifacts and every run would lint wasm32 cold. The restore prefix still
  restores the old host cache once.

### Validation

`mask all-check` passes (all eight groups). `cargo test -p gup --lib` (2475
passed, 31 ignored), `streaming_builder_integration` (15) and
`treemap_window_tests` (3) pass. `mask ci wasm` (the Wasm workflow's build and
wasm-pack steps) passes. `choropleth_world_population`, which uses `region_id`,
runs and joins its regions. No rendered output changed: the changes are
wasm32-only code, type aliases equal to the old native types, and deletions of
dead code.

### Commits

- `6ff600c` Expect `arc_with_non_send_sync` on wasm32 for gup-core's layer GPU
  handle
- `a992408` Fix wasm32 lints in the web accessibility overlay
- `746e5d0` Fix the root crate's wasm32 clippy findings
- `71c8f1c` Lint the wasm32 target in the gates
- `bd5a5ba` Re-key the Lint cache so it saves the wasm32 artifacts

### Key files

- `scripts/clippy_wasm32.sh` (new): the pass and its member rule.
- `maskfile.md`, `scripts/pre_commit.sh`, `scripts/test_pre_commit.sh`,
  `.github/workflows/lint.yml`: wiring.
- `crates/gup-core/src/selection.rs`;
  `src/accessibility/{platform,web_overlay}.rs`,
  `src/chart_builder/builders{.rs,/choropleth.rs}`, `src/streaming/stream.rs`,
  `src/{app,context,interaction,lib,test_utils,wasm_api,wasm_bench_interaction}.rs`,
  `src/layout/treemap.rs`: the findings.
