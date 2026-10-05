# Development Tasks

## build

Build all

```bash
cargo build
```

## check

Check all without building

```bash
cargo check
cargo check --tests
cargo check --examples
cargo check --benches
```

## test

Run tests (single-threaded to avoid GPU resource conflicts)

```bash
cargo test -- --test-threads=1
```

## visual-regression

Run the chart-builder golden-image tests (set `GUP_BLESS=1` to re-bless; see
`tests/golden/README.md`)

```bash
cargo test --lib visual_regression -- --test-threads=1
```

## gup-core-window

The gup-core checks that need a display (GUP-396): window/PNG ΔE parity, then
the 100K-point zoom benchmark with vsync and uncapped (release build)

```bash
cargo test -p gup-core --test window_parity -- --ignored --nocapture
cargo run -p gup-core --release --example zoom_bench -- --present fifo
cargo run -p gup-core --release --example zoom_bench -- --present mailbox --uncapped
```

## smoke-examples

Build every example and run the headless ones (see `tests/examples_smoke.rs` for
the `GUP_SMOKE_*` knobs)

Examples are built without debug info, stripped, into a separate target
directory: with debug info the 109 example binaries take about 17 GB.

```bash
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}/smoke"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_STRIP=true CARGO_INCREMENTAL=0
cargo build --examples --all-features -p gup -p gup-culling-lod
cargo test --all-features --test examples_smoke -- --include-ignored --test-threads=1
```

## test-tutorials

Compile-check tutorial code snippets (doctests + integration tests)

```bash
cargo test --doc -p gup -- tutorial_doctests && cargo test --test tutorial_snippet_tests -- --test-threads=1
```

## lint

Run linters

```bash
concurrently --group --names clippy,statix,mdl \
   'cargo clippy --allow-no-vcs --fix --all-targets --all-features -- -D warnings' \
   'statix fix flake.nix' \
   'mdl --git-recurse .'
```

The `mdl` tool has no automatic fixer.

## lint-check

Run clippy linter without writing fixes

```bash
concurrently --group --names clippy,statix,mdl \
   'cargo clippy --all-targets --all-features -- -D warnings' \
   'statix check flake.nix' \
   'mdl --git-recurse .'
```

## fmt

Format all code

```bash
shopt -qs globstar
concurrently --group --names rs,nix,md \
   'git grep -lz --untracked "[[:space:]]\+$" -- "*.rs" | xargs -0 -r sed -i "/[[:space:]]\+$/s///" && cargo fmt --all && cargo fmt --manifest-path dogfood/Cargo.toml' \
   'nixfmt flake.nix' \
   'prettier --cache --log-level warn --write "**/*.md"'
```

## fmt-check

Check if code is formatted

```bash
shopt -qs globstar
concurrently --group --names '\s,rs,nix,md' \
   '! git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" -- "*.rs"' \
   'cargo fmt --all -- --check && cargo fmt --manifest-path dogfood/Cargo.toml -- --check' \
   'nixfmt --check flake.nix' \
   'prettier --cache --log-level warn --check "**/*.md"'
```

## all-fix

Run Rust check, linters and formatters' checks.

```bash
shopt -qs globstar
concurrently --group --names rs,nix,md \
   'git grep -lz --untracked "[[:space:]]\+$" -- "*.rs" | xargs -0 -r sed -i "/[[:space:]]\+$/s///" && cargo fmt --all && cargo fmt --manifest-path dogfood/Cargo.toml && cargo clippy --allow-no-vcs --fix --all-targets --all-features -- -D warnings && cargo check' \
   'nixfmt flake.nix && statix fix flake.nix' \
   'prettier --cache --log-level warn --write "**/*.md" && mdl --git-recurse .'
```

## all-check

Run Rust check, linters and formatters' checks. `gup-core` is linted without
`--fix` so its warnings fail the gate (workspace-wide fix: GUP-398). The gallery
sync check keeps `scripts/gallery_config.toml` and `examples/INDEX.md` in step
with the Cargo examples (the Gallery workflow fails on drift).

```bash
shopt -qs globstar
concurrently --group --names '\s,rs,nix,md,marks,gallery' \
   '! git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" -- "*.rs"' \
   'mask check && cargo fmt --all -- --check && cargo fmt --manifest-path dogfood/Cargo.toml -- --check && cargo clippy --allow-no-vcs --fix --all-targets --all-features -- -D warnings && cargo clippy -p gup-core --all-targets -- -D warnings' \
   'nixfmt --check flake.nix && statix check flake.nix' \
   'prettier --cache --log-level warn --check "**/*.md" && mdl --git-recurse .' \
   'mask validate-marks' \
   './scripts/check_gallery_sync.sh'
```

## dogfood

Build the detached `dogfood/` crate against this checkout and run its
realistic-usage tasks with pixel checks (see `dogfood/README.md`). Windowed
tasks need a display: use `xvfb-run -a mask dogfood` when headless, or set
`DOGFOOD_SKIP_WINDOWED=1`.

```bash
./dogfood/run_all.sh
```

## ci

> Run the push-to-main CI workflows locally, on software Vulkan (GUP-403)

GitHub's runners have no GPU, so every subcommand forces Mesa's lavapipe, as the
workflows do (`WGPU_BACKEND=vulkan` also keeps `gup-core`, which accepts every
backend, off a hardware GL adapter). Each subcommand mirrors one file in
`.github/workflows/`; change both together. Run this before pushing: the hook
(`mask all-check`) is fast but does not render. Expect 30-60 minutes from cold;
`mask ci <workflow>` runs one. Not covered: the weekly Comprehensive
Benchmarking job (316 Criterion benchmarks; `--list` checks every bench target
accepts Criterion's flags), the Gallery deploy, and the manual-dispatch mobile
workflows.

```bash
set -euo pipefail
mask ci gallery
mask ci wasm
mask ci visual-regression
mask ci tests
mask ci dogfood
mask ci performance
echo "mask ci: all workflows passed locally"
```

### ci gallery

> Gallery workflow: config sync, headless thumbnails, HTML and link checks

```bash
set -euo pipefail
export VK_ICD_FILENAMES="${GUP_LAVAPIPE_ICD:-$LIBGL_DRIVERS_PATH/../../share/vulkan/icd.d/lvp_icd.x86_64.json}"
export WGPU_BACKEND=vulkan
export CARGO_PROFILE_RELEASE_STRIP=true
./scripts/check_gallery_sync.sh
cargo build --release --examples
./scripts/generate_gallery.sh
./scripts/generate_gallery_html.sh
./scripts/check_gallery_links.sh
```

### ci wasm

> WASM Compilation Check workflow (the known GUP-285B test failure is skipped)

```bash
set -euo pipefail
cargo build --target wasm32-unknown-unknown --lib
wasm-pack build --target web --out-dir "${CARGO_TARGET_DIR:-target}/ci-pkg" -- --features wasm-start
grep -q run_wasm_axis_benchmarks "${CARGO_TARGET_DIR:-target}/ci-pkg/gup.js"
```

### ci visual-regression

> Visual regression workflow: goldens, gup-core, culling/LOD, examples smoke

```bash
set -euo pipefail
export VK_ICD_FILENAMES="${GUP_LAVAPIPE_ICD:-$LIBGL_DRIVERS_PATH/../../share/vulkan/icd.d/lvp_icd.x86_64.json}"
export WGPU_BACKEND=vulkan
cargo test -p gup-visual-regression
cargo test --lib visual_regression -- --test-threads=1
cargo check -p gup-culling-lod --all-targets --all-features
cargo test -p gup-culling-lod -- --test-threads=1
cargo clippy -p gup-core --all-targets -- -D warnings
cargo test -p gup-core --lib --test scatter_png -- --test-threads=1
cargo test -p gup-core --doc
cargo test -p gup-core --test compile_fail
mask smoke-examples
```

### ci tests

> Tests workflow: every workspace member's tests, then the `debug` feature

The only CI job that runs the root crate's full suite. Built without debug info
(as on CI) so the 111 integration-test binaries fit on disk; even so, check that
`CARGO_TARGET_DIR` has about 10 GB free.

```bash
set -euo pipefail
export VK_ICD_FILENAMES="${GUP_LAVAPIPE_ICD:-$LIBGL_DRIVERS_PATH/../../share/vulkan/icd.d/lvp_icd.x86_64.json}"
export WGPU_BACKEND=vulkan
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_STRIP=debuginfo CARGO_INCREMENTAL=0
unset WAYLAND_DISPLAY
xvfb-run -a cargo test --workspace -- --test-threads=1
./scripts/test_debug_feature.sh
```

### ci dogfood

> Dogfood workflow, with the windowed tasks under Xvfb as on CI

The windowed tasks' pixel checks assume the window keeps its requested size. A
tiling Wayland compositor resizes it and the checks fail, so this always uses
Xvfb (no window manager), like the workflow.

```bash
set -euo pipefail
export VK_ICD_FILENAMES="${GUP_LAVAPIPE_ICD:-$LIBGL_DRIVERS_PATH/../../share/vulkan/icd.d/lvp_icd.x86_64.json}"
export WGPU_BACKEND=vulkan
unset WAYLAND_DISPLAY
(cd dogfood && cargo fmt -- --check && cargo test --release --lib --bin dogfood_check)
xvfb-run -a -s "-screen 0 1920x1080x24" ./dogfood/run_all.sh
```

### ci performance

> Performance Testing workflow's push-to-main jobs (benchmarks listed, not run)

```bash
set -euo pipefail
export VK_ICD_FILENAMES="${GUP_LAVAPIPE_ICD:-$LIBGL_DRIVERS_PATH/../../share/vulkan/icd.d/lvp_icd.x86_64.json}"
export WGPU_BACKEND=vulkan
mask perf-check
cargo test --features debug --test performance_ci_tests -- --test-threads=1
cargo test --test cross_platform_axis_performance_tests -- --test-threads=1
cargo test -p gup --lib wasm_bench_axis -- --test-threads=1
# Every bench target must accept Criterion's flags (the lib's libtest harness
# once rejected --save-baseline); --list checks that without running them.
cargo bench --all-features -- --list --save-baseline ci
```

## clean

Clean build artifacts

```bash
cargo clean
```

## doc

Build API reference and fail on any doc warnings

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

## watch

Watch for changes and rebuild

```bash
cargo watch -x check -x test -x "clippy --all-targets --all-features -- -D warnings"
```

## audit

Check dependencies for security vulnerabilities

```bash
cargo audit
```

## pack (project)

Build WebAssembly package for web target

```bash
wasm-pack build ${project} --target web
```

## run (project)

Run a specific project

```bash
cargo run --bin ${project}
```

## serve (project)

Serve a project on port 8080

OPTIONS

- port
  - flags: -p --port
  - type: string
  - desc: Port to serve on (default: 8080)

```bash
miniserve --index index.html --port ${port:-8080} --spa ${project}
```

## start (project)

Serve a project on port 8080

OPTIONS

- port
  - flags: -p --port
  - type: string
  - desc: Port to serve on (default: 8080)

```bash
mprocs --names '📦 pack,🌐 serve,🚀 launch' \
       "cargo watch --watch ${project}/src --shell 'mask pack ${project}'" \
       "mask serve --port ${port:-8080} ${project}" \
       "chromium-webgpu --app=http://localhost:${port:-8080}"
```

## deps

Update dependencies

```bash
cargo update
```

## tree

Show dependency tree

```bash
cargo tree
```

## old-path-loc

Count non-test LOC remaining in the frozen old render path (RFC-001 metric)

```bash
scripts/old_path_loc.pl | tail -1
```

## validate-marks

Validate all built-in mark types (CI gate)

```bash
cargo run --bin validate_marks
```

## bench

Run performance benchmarks

```bash
cargo bench
```

## bench-interaction

Run interaction system benchmarks only

```bash
cargo bench --bench interaction_benchmarks --bench interaction_memory_benchmarks
```

## bench-wasm-native

Run native benchmarks in WASM-compatible format (JSON output)

```bash
scripts/wasm_benchmark.sh native
```

## bench-wasm-build

Build WASM benchmark package for browser testing

```bash
scripts/wasm_benchmark.sh build
```

## bench-wasm-serve

Build WASM and serve the benchmark runner

```bash
scripts/wasm_benchmark.sh serve
```

## perf-check

Run performance regression tests (CI-friendly)

```bash
cargo test --test interaction_performance_tests -- --test-threads=1
```

## perf-alert

Run performance alert system (threshold tests + report generation)

```bash
scripts/perf_alert.sh --skip-benchmarks "$@"
```

## perf-trend-record

Record a performance trend data point

```bash
scripts/perf_trend.sh record
```

## perf-trend-report

Generate a performance trend report

```bash
scripts/perf_trend.sh report "${1:-10}"
```

## tauri-example

Build the Gup WASM package and launch the Tauri example application

```bash
echo "Building Gup WASM package..."
wasm-pack build --target web --out-dir examples/gup-tauri/ui/pkg

echo "Installing npm dependencies..."
(cd examples/gup-tauri && npm install)

echo "Launching Tauri dev server..."
(cd examples/gup-tauri && cargo tauri dev)
```

## ios-build

Build the gup-ios static library for the iOS Simulator (requires macOS with
Xcode).

```bash
echo "Building gup-ios for aarch64-apple-ios-sim..."
CARGO_TARGET_DIR=target cargo build --manifest-path gup-ios/Cargo.toml --target aarch64-apple-ios-sim --release
echo "Library at target/aarch64-apple-ios-sim/release/libgup_ios.a"
```
