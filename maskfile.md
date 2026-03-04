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
   'sed -i "/[[:space:]]\+$/s///" **/*.rs && cargo fmt --all' \
   'nixfmt flake.nix' \
   'prettier --cache --log-level warn --write "**/*.md"'
```

## fmt-check

Check if code is formatted

```bash
shopt -qs globstar
concurrently --group --names '\s,rs,nix,md' \
   '! git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" **/*.rs' \
   'cargo fmt --all -- --check' \
   'nixfmt --check flake.nix' \
   'prettier --cache --log-level warn --check "**/*.md"'
```

## all-fix

Run Rust check, linters and formatters' checks.

```bash
shopt -qs globstar
concurrently --group --names rs,nix,md \
   'sed -i "/[[:space:]]\+$/s///" **/*.rs && cargo fmt --all && cargo clippy --allow-no-vcs --fix --all-targets --all-features -- -D warnings && cargo check' \
   'nixfmt flake.nix && statix fix flake.nix' \
   'prettier --cache --log-level warn --write "**/*.md" && mdl --git-recurse .'
```

## all-check

Run Rust check, linters and formatters' checks.

```bash
shopt -qs globstar
concurrently --group --names '\s,rs,nix,md,marks' \
   '! git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" **/*.rs' \
   'mask check && cargo fmt --all -- --check && cargo clippy --allow-no-vcs --fix --all-targets --all-features -- -D warnings' \
   'nixfmt --check flake.nix && statix check flake.nix' \
   'prettier --cache --log-level warn --check "**/*.md" && mdl --git-recurse .' \
   'mask validate-marks'
```

## clean

Clean build artifacts

```bash
cargo clean
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
