# Development Tasks

## build

Build all

```bash
cargo build
```

## check

Type-check every target of every workspace member without building. This only
warns; the strict clippy runs in `all-check` are the gate.

```bash
cargo check --workspace --all-targets
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

## wasm-size

gup-core's gzipped WASM cost (GUP-401, RFC-001 §12 risk 10), with its budget
enforced (GUP-417). Builds two `crates/gup-core/wasm-size` harnesses in release
for `wasm32-unknown-unknown`: bare wgpu, and the reference scatter through
gup-core (whose shaders are composed at build time, GUP-406). Each goes through
`wasm-bindgen --target web` without name sections, then `gzip -9`. Prints each
size, gup-core's cost over bare wgpu and the bundled Inter subset, and **fails**
when gup-core costs more than 400,000 B gz over bare wgpu
(`GUP_WASM_OVER_WGPU_MAX_GZ` overrides the budget). The Visual regression
workflow's browser job runs the same script. Needs `wasm-bindgen` at the
harnesses' `Cargo.lock` version. Delete
`$CARGO_TARGET_DIR/wasm32-unknown-unknown/release` afterwards if disk is tight.
See `scripts/wasm_size.sh` and `crates/gup-core/PERF_BUDGETS.md`.

```bash
./scripts/wasm_size.sh
```

## perf-budget

gup-core's performance budgets (GUP-417): run `zoom_bench` (100K points,
`Mailbox` uncapped, release, fullscreen), `pipeline_timings` (release) and
`scripts/wasm_size.sh`, and compare each `metric` they print with the table in
`crates/gup-core/PERF_BUDGETS.md` (±20–25% for GPU/CPU timings, exact for upload
counts, +2% and the 400,000 B ceiling for WASM size). Prints a pass/fail table
and exits non-zero on any failure. **Needs a display and a real GPU** (it opens
a fullscreen window, like `mask gup-core-window`) and `wasm-bindgen` like
`mask wasm-size`, so it is a local, pre-merge check, not a CI job: run it for
any story that touches gup-core rendering, uploads, pipeline creation or WASM
size. The timing budgets hold only on the machine `PERF_BUDGETS.md` names; a
failure on timing alone warrants a second run. Run `scripts/perf_budget.sh`
directly for its options (`--skip-wasm`, `--budgets FILE`, `--metrics FILE`).

```bash
./scripts/perf_budget.sh
```

## wasm-browser

Run gup-core in a real browser (GUP-401, GUP-408): build the `wasm-size/scatter`
harness, serve it, and render the reference scatter in headless Chromium through
WebGPU on SwiftShader, Chromium's bundled software adapter (no GPU needed, the
same adapter as on CI). Fails unless the page reports `GUP PASS` within 90 s
with no console error, uncaught exception, failed load or WebGPU/WGSL
diagnostic. Prints the browser version and adapter, and writes the browser's
pixels to `$CARGO_TARGET_DIR/visual-regression/gup_core/browser_scatter.png`.
Needs `chromium` (or `GUP_CHROMIUM`), Node 22+ and `wasm-bindgen` at the
harness's `Cargo.lock` version. See `scripts/wasm_browser.sh` and
`scripts/browser_smoke.mjs`.

```bash
./scripts/wasm_browser.sh
```

## subset-inter

Regenerate `gup-text`'s bundled font (GUP-407): subset the full Inter Regular to
the characters listed in `crates/gup-text/fonts/inter-subset.txt`. Keeps GPOS
`kern` and GSUB `tnum` (for shaping, GUP-405) and every other OpenType feature
is dropped, as is hinting (fontdue and resvg don't hint). The missing-glyph box
keeps its outline, so a character outside the subset draws as a visible box, not
as nothing. The copyright, trademark and licence names (IDs 0, 7, 13 and 14) are
kept, so an embedded copy carries its OFL notice. The output is byte-for-byte
reproducible with the dev shell's fonttools (pinned by `flake.lock`). Commit the
result with the character list.

```bash
set -euo pipefail
fonts=crates/gup-text/fonts
pyftsubset "$fonts/Inter-Regular.ttf" \
  --unicodes-file="$fonts/inter-subset.txt" \
  --layout-features=kern,tnum \
  --no-hinting \
  --notdef-outline \
  --name-IDs=0,1,2,3,4,5,6,7,13,14 \
  --output-file="$fonts/Inter-Regular-Subset.ttf"
for f in "$fonts/Inter-Regular.ttf" "$fonts/Inter-Regular-Subset.ttf"; do
  printf '%-44s %7d B raw %6d B gz\n' "$f" "$(stat -c %s "$f")" \
    "$(gzip -9 -c "$f" | wc -c)"
done
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

Apply clippy's automatic fixes, then lint strictly. `cargo clippy --fix` exits 0
whatever it cannot fix, so only the strict runs after it can fail.

```bash
concurrently --group --names clippy,statix,mdl,gha \
   'cargo clippy --allow-no-vcs --fix --workspace --all-targets --all-features && cargo clippy --workspace --all-targets -- -D warnings && cargo clippy --workspace --all-targets --all-features -- -D warnings && ./scripts/clippy_wasm32.sh' \
   'statix fix flake.nix' \
   'mdl --git-recurse .' \
   'actionlint .github/workflows/*.y*ml'
```

Neither `mdl` nor `actionlint` (workflow files) has an automatic fixer.

## lint-check

Lint strictly without writing fixes: every workspace member and target, with
default features and with all features, then the wasm32 libs
(`scripts/clippy_wasm32.sh`)

```bash
concurrently --group --names clippy,statix,mdl,gha \
   'cargo clippy --workspace --all-targets -- -D warnings && cargo clippy --workspace --all-targets --all-features -- -D warnings && ./scripts/clippy_wasm32.sh' \
   'statix check flake.nix' \
   'mdl --git-recurse .' \
   'actionlint .github/workflows/*.y*ml'
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
   'git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" -- "*.rs"; test $? -eq 1' \
   'cargo fmt --all -- --check && cargo fmt --manifest-path dogfood/Cargo.toml -- --check' \
   'nixfmt --check flake.nix' \
   'prettier --cache --log-level warn --check "**/*.md"'
```

## all-fix

Apply every automatic fix (whitespace, rustfmt, clippy, nixfmt, statix,
prettier), then run the strict checks. `cargo clippy --fix` exits 0 whatever it
cannot fix, so only the strict clippy runs after it can fail. `actionlint` has
no fixer, so it only checks.

```bash
shopt -qs globstar
concurrently --group --names rs,nix,md,gha \
   'git grep -lz --untracked "[[:space:]]\+$" -- "*.rs" | xargs -0 -r sed -i "/[[:space:]]\+$/s///" && cargo fmt --all && cargo fmt --manifest-path dogfood/Cargo.toml && cargo clippy --allow-no-vcs --fix --workspace --all-targets --all-features && cargo clippy --workspace --all-targets -- -D warnings && cargo clippy --workspace --all-targets --all-features -- -D warnings && ./scripts/clippy_wasm32.sh' \
   'nixfmt flake.nix && statix fix flake.nix' \
   'prettier --cache --log-level warn --write "**/*.md" && mdl --git-recurse .' \
   'actionlint .github/workflows/*.y*ml'
```

## all-check

The full, unscoped local gate (GUP-398). It never modifies files, and each of
its checks has been seen to fail on a seeded violation (GUP-398 retrospective).
Clippy covers every workspace member and target, with default features and with
all features, under `-D warnings`; it type-checks too, so there is no separate
`cargo check`. `scripts/clippy_wasm32.sh` then lints the libs of the members
that build for wasm32 on that target, where `#[cfg(target_arch = "wasm32")]`
code compiles and wgpu's handles are not `Send`/`Sync` (GUP-412; the script says
which members and why). The gallery sync check keeps
`scripts/gallery_config.toml` and `examples/INDEX.md` in step with the Cargo
examples (the Gallery workflow fails on drift). `actionlint` checks every
workflow file in `.github/workflows/`, including shellcheck on their `run:`
scripts: GitHub does not report a workflow it cannot parse, it just never starts
it (GUP-409). The files are named explicitly because, with no arguments,
actionlint looks for the workflows next to the nearest `.git` above it, which
from the hook's snapshot is the real checkout. `test_pre_commit.sh` tests the
pre-commit hook's scoping rules and its staged snapshot, and
`test_shared_build_dir.sh` that checkouts sharing a build directory get their
own workspace-member artifacts (GUP-411).

The whitespace check passes only when `git grep` finds nothing (exit 1): a
`git grep` error must fail it, not pass it.

This task checks the tree it runs in: run by hand, your working tree, including
unstaged edits and untracked files; in CI, the pushed commit. The pre-commit
hook runs `mask pre-commit`, which runs this task only when the staged change
needs it, and then inside its staged snapshot when the working tree differs from
the index. CI runs its own full checks on every push regardless.

```bash
shopt -qs globstar
concurrently --group --names '\s,rs,nix,md,gha,marks,gallery,hook' \
   'git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" -- "*.rs"; test $? -eq 1' \
   'cargo fmt --all -- --check && cargo fmt --manifest-path dogfood/Cargo.toml -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo clippy --workspace --all-targets --all-features -- -D warnings && ./scripts/clippy_wasm32.sh' \
   'nixfmt --check flake.nix && statix check flake.nix' \
   'prettier --cache --log-level warn --check "**/*.md" && mdl --git-recurse .' \
   'actionlint .github/workflows/*.y*ml' \
   'mask validate-marks' \
   './scripts/check_gallery_sync.sh' \
   './scripts/test_pre_commit.sh && ./scripts/test_shared_build_dir.sh'
```

## pre-commit

The git pre-commit hook (installed by `flake.nix`): checks proportional to the
staged files. Every commit gets the trailing-whitespace and gallery sync checks,
and prettier and mdl on the changed Markdown files; a docs-only commit gets
nothing else. A commit inside workspace crates also runs rustfmt on those
crates, and strict clippy on them and on every member that depends on them.
Anything workspace-level or unrecognised runs `mask all-check`. The rules and
their reasons are in `scripts/pre_commit.sh`; preview them with
`scripts/pre_commit.sh --plan [PATH...]`.

The scoping is local only. CI never scopes, so anything the hook skipped is
still checked before it merges.

The hook checks exactly what is being committed, not the working tree (GUP-409).
When the working tree matches the index it checks in place; otherwise it checks
out the index into a per-worktree snapshot under the git directory and runs
there, so unstaged edits and untracked files can neither fail a clean commit nor
pass a broken one. Running `mask pre-commit` by hand does the same.
`mask all-check` run by hand checks your working tree.

```bash
./scripts/pre_commit.sh
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
(`mask pre-commit`) is scoped to the change and does not render. Expect 30-60
minutes from cold; `mask ci <workflow>` runs one. Not covered: the weekly
Comprehensive Benchmarking job (316 Criterion benchmarks; `--list` checks every
bench target accepts Criterion's flags), the Gallery deploy, and the
manual-dispatch mobile workflows.

```bash
set -euo pipefail
mask ci lint
mask ci gallery
mask ci wasm
mask ci visual-regression
mask ci tests
mask ci dogfood
mask ci performance
echo "mask ci: all workflows passed locally"
```

### ci lint

> Lint workflow: the full local gate, `mask all-check`

```bash
set -euo pipefail
export VK_ICD_FILENAMES="${GUP_LAVAPIPE_ICD:-$LIBGL_DRIVERS_PATH/../../share/vulkan/icd.d/lvp_icd.x86_64.json}"
export WGPU_BACKEND=vulkan
mask all-check
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

> Visual regression workflow: goldens, gup-core, culling/LOD, examples smoke,
> browser

```bash
set -euo pipefail
export VK_ICD_FILENAMES="${GUP_LAVAPIPE_ICD:-$LIBGL_DRIVERS_PATH/../../share/vulkan/icd.d/lvp_icd.x86_64.json}"
export WGPU_BACKEND=vulkan
cargo test -p gup-visual-regression
cargo test --lib visual_regression -- --test-threads=1
cargo check -p gup-culling-lod --all-targets --all-features
cargo test -p gup-culling-lod -- --test-threads=1
cargo test -p gup-text -- --test-threads=1
cargo test -p gup-core --lib --test scatter_png --test targets \
  --test scene_items --test msaa --test svg -- --test-threads=1
cargo test -p gup-core --doc
cargo test -p gup-core --test compile_fail
mask smoke-examples
# The browser job (GUP-408), on SwiftShader as on CI
./scripts/wasm_browser.sh
# and its WASM size budget (GUP-417)
./scripts/wasm_size.sh
```

### ci tests

> Tests workflow: every workspace member's tests, then the `debug` feature

The only CI job that runs the root crate's full suite. Built without debug info
(as on CI) so the 111 integration-test binaries fit on disk; even so, check that
the build directory (`CARGO_BUILD_BUILD_DIR`, else `CARGO_TARGET_DIR`) has about
10 GB free.

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
