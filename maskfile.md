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
```

## test

Run tests

```bash
cargo test
```

## lint

Run clippy linter

```bash
cargo clippy --allow-no-vcs --fix --all-targets --all-features -- -D warnings
```

## fmt

Format all code

```bash
cargo fmt --all
```

## lint-check

Run clippy linter without writing fixes

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## fmt-check

Check if code is formatted

```bash
cargo fmt --all -- --check
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

## bench

Run performance benchmarks

```bash
cargo bench
```
