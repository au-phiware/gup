# GUP-408: Run gup-core's Browser Smoke Test in CI

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-09; AC1's
GitHub run and AC4's timing await the first push) **Created**: 2026-10-06

## Context

[GUP-401](GUP-401_RFC_001_S3_Scene_Renderer_RenderTarget.md) ran gup-core in a
real browser for the first time (headless Chromium, WebGPU). That run found two
bugs no native test could:

- `std::time::Instant::now` panics on wasm32;
- the glue's uniform layout differed because naga's WGSL writer drops `@align`.

Both are fixed. A native unit test now guards the second, but the general class,
behaviour that differs only in a browser, is caught only by `mask wasm-browser`,
which needs a local GPU and runs only by hand.

## User Story

> "As a Gup maintainer, I want every push to render a chart in a browser, so
> that browser-only regressions fail CI instead of reaching users."

## Acceptance Criteria

- [ ] A CI job builds the `wasm-size/scatter` harness and runs the
      `mask wasm-browser` page in headless Chromium on a GitHub runner, using a
      software WebGPU adapter (SwiftShader/Dawn) if no GPU is available. The job
      records which adapter ran. _The `browser` job in `visual-regression.yml`
      does this, forcing SwiftShader, and writes the browser and adapter to the
      step summary. Verified locally with the CI's exact Chrome for Testing
      build (155.0.8059.39), `/dev/dri` hidden and no Vulkan driver:
      `adapter vendor=google architecture=swiftshader     fallback=true`,
      `GUP PASS`. Left unticked until it passes on GitHub._
- [x] The job fails on `GUP FAIL`, on a timeout, or on any console error. It
      uploads the browser PNG as an artifact. _Also on an uncaught exception, a
      failed load and a "rendering" warning (Chrome's channel for WGSL and
      uncaptured WebGPU errors); each path was run locally. The PNG is the
      `browser-smoke` artifact._
- [x] A seeded regression (for example reintroducing `std::time::Instant` in a
      render path) makes the job fail. This is shown once and noted in the
      retrospective. _Two, both on Chrome for Testing 155 without a GPU: the
      `Instant` regression, and a WGSL derivative in non-uniform control flow
      that naga and the native suite accept._
- [ ] The job takes under 10 minutes and caches the wasm build. _It caches
      `target` and the Cargo registry, keyed on the harness's `Cargo.lock` and
      `rust-toolchain.toml`, and `timeout-minutes: 10` enforces the budget.
      Locally the cold release build takes about 40 s and the browser run 2 s;
      the runner's timing is for GitHub to confirm._

## Technical Tasks

- [x] Find Chromium flags for software WebGPU on Linux runners
      (`--enable-unsafe-swiftshader` or Dawn's fallback adapter).
- [x] Add the job to the visual-regression workflow, with Nix or a pinned
      Chromium.
- [x] Make `mask wasm-browser` exit non-zero on console errors.

## Dependencies

### Prerequisite Stories

- GUP-401 ✅ (`mask wasm-browser`)

### Enables Stories

- RFC-001 S8's wasm entry point, tested on every push.

## Testing Strategy

- Run the job on a branch with a seeded failure, then without it.

## Success Metrics

- Browser regressions fail CI.

## Risk Assessment

- **Medium**: software WebGPU in headless Chromium may be unavailable or flaky
  on GitHub runners. Mitigation: record the outcome. If it is infeasible,
  document why and fall back to a nightly self-hosted run.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked (AC1 and AC4 await the
      first GitHub run)
- [x] Story status is updated in the story file and INDEX.md
- [x] A retrospective is added

## Implementation Summary

`mask wasm-browser` is now `scripts/wasm_browser.sh`, and the Visual regression
workflow's new `browser` job runs the same script on every push and pull
request.

- **`scripts/browser_smoke.mjs`** (new, no npm dependencies): serves the
  wasm-bindgen output with Node's `http` module (correct `application/wasm`
  type, ephemeral port), starts Chromium (`$GUP_CHROMIUM`, default `chromium`)
  in its own process group, and drives it over the DevTools protocol with Node's
  built-in `WebSocket`. It records `Runtime.consoleAPICalled`,
  `Runtime.exceptionThrown` and `Log.entryAdded`. It fails on `GUP FAIL`, no
  result within `$GUP_BROWSER_TIMEOUT` (90 s), `console.error`/`assert`, an
  uncaught exception, a log entry at error level, or a `rendering` warning. It
  stops a second after the first page error, because a panic inside a spawned
  future leaves the page waiting forever. It prints the browser version and
  adapter, appends them to `$GITHUB_STEP_SUMMARY` and writes the PNG.
- **Flags**: `--headless=new`, `--no-sandbox`, `--enable-unsafe-webgpu`,
  `--enable-features=Vulkan`, `--use-vulkan=swiftshader`,
  `--use-webgpu-adapter=swiftshader`, `--use-angle=swiftshader`,
  `--disable-vulkan-surface`.
- **`scripts/wasm_browser.sh`** (new): checks that `wasm-bindgen` matches the
  harness's `Cargo.lock`, builds the harness in release for wasm32, binds it and
  runs the driver. It exports `CARGO_TARGET_DIR`, because the harness is its own
  workspace and without it the build landed in the harness directory, not where
  the old task read it. `mask wasm-size` had the same bug and is fixed too.
- **`crates/gup-core/wasm-size/scatter/index.html`**: logs
  `GUP ADAPTER vendor=… architecture=… fallback=…` (wgpu's web backend reports
  no adapter details). The PNG comes before the result line, which is always
  last. A `data:` favicon avoids a 404 that would now fail the run.
- **`.github/workflows/visual-regression.yml`**: the `browser` job runs
  `rustup toolchain install` (rust-toolchain.toml), caches the Cargo registry
  and `target`, takes wasm-bindgen from its release tarball at the `Cargo.lock`
  version, and uses Node 24 (`actions/setup-node@v4`) and Chrome for Testing
  155.0.8059.39 (`browser-actions/setup-chrome@v2`,
  `install-dependencies: true`). It uploads the `browser-smoke` artifact and has
  `timeout-minutes: 10`. actionlint is clean.
- **`maskfile.md`**: `wasm-browser` calls the script. `ci visual-regression`
  runs it, and now runs the same gup-core test targets as the workflow (it had
  drifted to `--lib --test scatter_png` only).
- **Docs**: `.github/workflows/README.md` gains a Visual regression section, and
  RFC-001's "gup-core in a browser" notes the CI job and the SwiftShader
  finding.

### Evidence (local)

- **Clean, no GPU** (Chromium 154 from Nix; `/dev/dri` hidden with bwrap, no
  Vulkan ICDs): PASS on SwiftShader in 1.4 s.
- **Clean, as CI** (Chrome for Testing 155, the pin; fresh worktree, no
  `CARGO_TARGET_DIR`, wasm-bindgen from the release tarball, no GPU): PASS,
  `adapter vendor=google architecture=swiftshader fallback=true`. The cold wasm
  build took about 40 s.
- **Seed 1**, `use std::time::Instant` in `render.rs` (CfT 155): FAIL in 2 s
  with
  `console.error: panicked at library/std/src/sys/pal/wasm/../unsupported/time.rs:13:9: time not implemented on this platform`,
  `uncaught: RuntimeError: unreachable` and `the page reported no result`.
  Before the fail-fast change, the same seed waited out the whole timeout: the
  panic happens in a spawned future, so the page never reports.
- **Seed 2**, `return v.color + vec4<f32>(dpdx(v.color.r) * 0.0);` under
  `if v.color.a > 0.5` in `rect.wgsl` (CfT 155): FAIL with
  `log.warning (rendering): Error while parsing WGSL: :43:19 error: 'dpdx' must only be called from uniform control flow`,
  three follow-on invalid-object warnings and
  `GUP FAIL white=0 grey=64000 coloured=0`. Natively,
  `cargo test -p gup-core --test scene_items` passes with the seed (2 passed):
  naga accepts it.
- **Page never reports** (`GUP_BROWSER_TIMEOUT=3`): FAIL,
  `no GUP PASS/FAIL within 3 s`, with the browser's last log lines.
- **Missing module**: FAIL,
  `log.error (network): Failed to load resource: the server responded with a status of 404 (Not Found)`.
- **No browser** (`GUP_CHROMIUM=/nonexistent`): FAIL,
  `cannot start /nonexistent: spawn /nonexistent ENOENT`.

The PNG from the CfT 155 run (320×200) shows the title "Wealth, population and
life expectanc…" (clipped at the right edge, the known S7 layout issue), log y
ticks 100k–1G, x ticks 0–60000, about 200 viridis circles on the `#eef1f6` plot
background, and the vertical legend bar (purple at the bottom, yellow at the
top). The page reported `white=30884 grey=16219 coloured=16897`, the same counts
as GUP-406's run.

### For GitHub to confirm

- The `browser` job passes on `ubuntu-latest`: `setup-chrome` installs CfT
  155.0.8059.39 with its dependencies, `--no-sandbox` starts under Ubuntu's
  AppArmor, and the step summary shows `architecture=swiftshader`.
- Wall-clock time under 10 minutes cold, and the cache saved and restored.
- The `browser-smoke` artifact contains `browser_scatter.png`.
- Optionally, a branch with seed 1 or 2 fails the job.

## Retrospective

**Completed**: 2026-10-09

### Key Technical Learnings

#### Headless Chromium was on SwiftShader all along

- **Challenge**: The story treated "software WebGPU on a GPU-less runner" as the
  main unknown, and `mask wasm-browser` said it needed a GPU.
- **Solution**: A five-line page logging `adapter.info` showed that headless
  Chromium with `--enable-unsafe-webgpu --enable-features=Vulkan` hands out
  `vendor=google architecture=swiftshader fallback=true` even on a machine with
  an Intel GPU. The real GPU appears only with
  `--ignore-gpu-blocklist --use-angle=vulkan`. So GUP-401's two browser bugs
  were found on SwiftShader, and the GPU was never needed. The script now forces
  SwiftShader (`--use-vulkan=swiftshader --use-webgpu-adapter=swiftshader`) and
  logs it. Local runs and CI therefore test the same adapter. The run also
  passed with `/dev/dri` hidden (bwrap) and no Vulkan ICDs, on Chromium 154 and
  on the pinned Chrome for Testing 155.
- **Pattern**: Before solving "how do we get X in CI", check what local runs
  were actually using.

#### Chrome's stderr log loses console levels, and WGSL errors are warnings

- **Challenge**: With `--enable-logging=stderr`, every console message prints as
  `INFO:CONSOLE`, whatever its level. WebGPU diagnostics do not appear there at
  all.
- **Solution**: Drive Chrome over the DevTools protocol (Node's built-in
  `WebSocket`, no npm packages). Use `Runtime.consoleAPICalled` for levels,
  `Runtime.exceptionThrown` for uncaught errors, and `Log.entryAdded` for
  browser messages. Chrome reports a WGSL compilation error, and every
  uncaptured WebGPU validation error, as a `warning` from source `rendering`, so
  those fail the run too. Failing on `error` alone would have missed seed 2's
  root cause. The pixel check still caught that seed, because the whole command
  buffer was invalid.
- **Pattern**: Probe how the failure you want to catch is actually reported
  before writing the rule that catches it.

#### wgpu on the web has no default uncaptured-error handler

- **Challenge**: With seed 2, `ImageTarget::render` returned `Ok` and a blank
  image. Natively, wgpu-core would have panicked.
- **Solution**: Here the browser smoke test catches it through Chrome's console.
  A user's page cannot, so this became GUP-410 (error scopes in gup-core).
- **Pattern**: The same wgpu call can fail loudly natively and silently on the
  web. A browser test should watch the browser's own diagnostics, not only the
  return values.

#### A panic in a spawned future never settles the page's promise

- **Challenge**: Seed 1 (`std::time::Instant`) panicked inside a
  wasm-bindgen-futures task. `render_scatter`'s promise never settled, so the
  page's `try/catch` never ran and the run waited out the timeout.
- **Solution**: The driver stops one second after the first page error. Seed 1
  now fails in 2 s, naming the panic and the `unreachable` trap.

#### Naga and Chrome's WGSL compiler disagree on uniformity

- **Challenge**: Finding a "WGSL error only the browser rejects" for the seeded
  proof.
- **Solution**: A small naga 27 probe (parse and validate with all flags)
  against candidate snippets. naga rejects reserved words, division by zero in
  constants, redefinitions and duplicate switch cases, as Chrome does. It
  accepts `textureSample` or `dpdx` in non-uniform control flow, which Chrome
  rejects. A derivative times zero in `rect.wgsl`'s fragment shader changes no
  pixels natively, so the native suite passes, and it fails in Chrome.

### Architectural Decisions

#### A shell script plus a dependency-free Node driver, not Nix in CI

- **Decision**: `scripts/wasm_browser.sh` (build, bind, run) and
  `scripts/browser_smoke.mjs` (serve, drive, judge). `mask wasm-browser` and the
  workflow both call the script. The CI job installs the pinned toolchain,
  wasm-bindgen from its release tarball at the `Cargo.lock` version, Node 24 and
  Chrome for Testing 155.0.8059.39.
- **Reasoning**: The locked nixpkgs has neither wasm-bindgen 0.2.113 (newest
  0.2.108) nor a Chromium on the dev shell's PATH (the flake's `chromium-webgpu`
  calls the system's). Putting nixpkgs' Chromium 145 in the dev shell would have
  replaced the developer's newer system Chromium. The driver also replaces
  miniserve, the fixed port and `sleep 1`.
- **Trade-off**: Two Chrome versions in play: the developer's Chromium locally,
  and CfT 155 in CI. Both run SwiftShader, and the summary line names the
  version.
- **Future**: The Chrome pin moves by hand, like `rust-toolchain.toml`. Bump it
  when the Chrome that users run changes enough to matter.

#### `--no-sandbox`

- **Decision**: Always pass it.
- **Reasoning**: The page is our own build, served from localhost. Ubuntu
  24.04's AppArmor userns restriction, and Nix's Chromium outside NixOS (no
  setuid helper), cannot start the sandbox.

### Development Workflow Insights

- `builtins.getFlake (toString ./.)` in a dirty checkout copied the working tree
  into the store and ran the pool out of space. Two tool calls failed with
  ENOSPC before the process was found and killed. Read the locked nixpkgs path
  from `nix flake archive --json` instead.
- `pkill -f gup-browser-smoke-` killed the tool's own shell again, as GUP-401's
  retrospective warned. The driver now starts Chromium in its own process group
  and kills the group, so it leaves nothing behind. That also fixed a hang when
  Chromium ran under a wrapper (bwrap) that `kill` on the wrapper's PID did not
  stop.
- Chrome for Testing runs on NixOS under `steam-run` with `LD_LIBRARY_PATH`
  built from the Nix Chromium's runtime closure (`nix-store -qR`). It needs a
  working directory outside `/tmp`. That made it possible to test the exact CI
  browser build locally.
- The orchestrator's actionlint check (`nix run nixpkgs#actionlint`) passed on
  the edited workflow.

### Follow-up Stories

1. **GUP-410: Surface WebGPU errors from gup-core on every target**. Error
   scopes around pipeline creation and rendering, so a validation failure is an
   `Err`, not a native panic or a silent blank image on wasm.

Noted without stories:

- `wasm.yml`'s best-effort `wasm-pack test --headless --chrome` steps say "GPU
  tiers gracefully degrade". With these flags, SwiftShader could run them for
  real. They test the legacy `gup` crate, so this waits for the strategic
  review's T2 replacement rather than reviving them.
- A hardware-GPU browser run (`--ignore-gpu-blocklist --use-angle=vulkan`) would
  catch driver-specific browser bugs. Only a self-hosted runner could run it in
  CI.
