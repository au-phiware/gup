# CI/CD workflows

This directory contains the GitHub Actions workflows. The old `performance.yml`
(Criterion benchmarks and timing tests of the pre-flip render core) was deleted
in GUP-488; `gup-core`'s budgets are `mask perf-budget` and the deterministic
proxies in `crates/gup-core/PERF_BUDGETS.md`.

## Workflows

### Tests (`tests.yml`)

Runs `cargo test --workspace --exclude gup-bevy -- --test-threads=1` on lavapipe
(`gup-bevy` has its own `integrations` job). Wall-clock budgets stay out of it:
tests that assert a budget are `#[ignore]`d with a "wall-clock budget ...
(GUP-398)" reason, and `gup-core`'s budgets are `mask perf-budget`.
`mask ci tests` runs it locally.

### Lint (`lint.yml`)

Runs `mask all-check`, the full local gate, in the Nix dev shell: strict clippy
on every workspace member and target (default and all features), rustfmt,
whitespace, nixfmt/statix, prettier/mdl, gallery sync and the pre-commit hook's
scoping tests. The local hook (`mask pre-commit`) scopes its checks to the
staged files; this workflow never does. `mask ci lint` runs it locally.

### Visual regression (`visual-regression.yml`)

The `visual-regression` job renders the chart-builder goldens, gup-core, and
gup-text on lavapipe and smoke-runs the examples. The `browser` job (GUP-408)
builds gup-core's `wasm-size/scatter` harness for wasm32 and renders it in
headless Chrome for Testing (a pinned version) through WebGPU on SwiftShader,
Chrome's bundled software adapter. It fails on `GUP FAIL`, a timeout, a console
error, an uncaught exception, a failed load or a WebGPU/WGSL diagnostic; the
step summary records the browser and adapter, and the render is uploaded as the
`browser-smoke` artifact. The same job then enforces gup-core's WASM size budget
(GUP-417, `scripts/wasm_size.sh`): it fails when the scatter costs more than
400,000 B gz over bare wgpu. `mask ci visual-regression` runs both jobs locally;
`mask wasm-browser` and `mask wasm-size` run the browser job's two checks alone.

### WASM compilation (`wasm.yml`)

Checks that the `gup` facade builds for `wasm32-unknown-unknown`
(`cargo build --target wasm32-unknown-unknown --lib`). The browser path is
`gup-core`'s harness, run by the Visual regression workflow.

### No workflow scopes by changed files

No workflow has a `paths:` filter, and none reads the pre-commit hook's scoping
rules: a workflow that runs runs all of its checks, whatever changed.

### Every workflow cancels its superseded runs

Every workflow has a `concurrency:` group (`<workflow>-${{ github.ref }}`) with
`cancel-in-progress: true`, so a second push to a branch cancels the first
push's runs instead of queueing beside them (GUP-409: queued jobs piling up
produced "job was not acquired by Runner" failures).

### Workflow files are linted

`actionlint` (with shellcheck on every `run:` script) checks every file in this
directory. It runs in `mask all-check` (so in the Lint workflow) and in the
pre-commit hook whenever a workflow file is staged. A malformed workflow does
not fail a run on GitHub: the run never starts, so nothing reports it.

## Toolchain and GPU (all workflows)

- The Rust toolchain is pinned in `rust-toolchain.toml`. Workflows install it
  with `rustup toolchain install` or run in the Nix dev shell, which reads the
  same file. Never use a floating `stable` toolchain in a workflow.
- The runners have no GPU. Workflows install Mesa's lavapipe (software Vulkan),
  or set `GUP_SOFTWARE_GPU=1` for the Nix dev shell. The browser job uses
  Chrome's SwiftShader instead (`scripts/browser_smoke.mjs` forces it).
- `mask ci` (or `mask ci <workflow>`) runs the push-to-main workflows locally on
  lavapipe.
