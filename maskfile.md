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
cargo check --workspace --exclude gup-bevy --all-targets
```

## test

Run tests (single-threaded to avoid GPU resource conflicts)

```bash
cargo test -- --test-threads=1
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
(`GUP_WASM_OVER_WGPU_MAX_GZ` overrides the budget), or when the scatter is more
than 2% over its recorded size in `crates/gup-core/PERF_BUDGETS.md` (GUP-438,
through `perf_budget.sh --wasm-only`). The Visual regression workflow's browser
job runs the same script. Needs `wasm-bindgen` at the harnesses' `Cargo.lock`
version. Delete `$CARGO_TARGET_DIR/wasm32-unknown-unknown/release` afterwards if
disk is tight. See `scripts/wasm_size.sh` and `crates/gup-core/PERF_BUDGETS.md`.

```bash
./scripts/wasm_size.sh
```

## wasm-attrib

Where the reference scatter's WASM bytes go (GUP-450). Builds the scatter
harness exactly as `wasm-size` does, runs `wasm-bindgen --target web` keeping
the name section (the file a user ships, minus its names; cargo's own output
still holds wasm-bindgen's describe functions), and prints three sections with
`twiggy` (pinned in the dev shell): `== top`, the 30 largest functions and data
segments; `== monos`, generic functions grouped by path with the instantiation
hash removed, the 20 largest groups, each with its instantiation count and
total; and `== crates`, raw bytes summed by crate (the first path segment of the
demangled name), with data segments as `data` and everything else as `other`.
The name section itself is listed apart, since it is not shipped. Sizes are
**raw** bytes: find targets with this, and judge a cut by the gz totals of
`mask wasm-size`. Local only, not a CI job. See `scripts/wasm_attrib.sh`.

`--row-type` (GUP-459) builds the harness with and without its `extra-row-type`
feature instead, and prints one section, `== row type`: the per-group raw byte
and instantiation-count differences (hashes and `.NNNN` suffixes stripped), the
30 largest by absolute size, and the total. It is the item-by-item account of
`wasm.per_row_type.gz_bytes`; the instantiations for the extra row type of
`bind_axes`, `tooltip_text` and `dispatch` show that the vtable was linked.

OPTIONS

- row_type
  - flags: --row-type
  - desc: Diff the build with and without the extra row type

```bash
./scripts/wasm_attrib.sh ${row_type:+--row-type}
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
directly for its options (`--skip-wasm`, `--wasm-only`, `--budgets FILE`,
`--metrics FILE`).

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

## env-doctor

Report the headless environment `scripts/headless_env.sh` resolves (lavapipe
ICD, `xvfb-run`, backend, screen); exits 1 if the ICD or `xvfb-run` is missing

```bash
./scripts/headless_env.sh --check
```

## egui-screenshot

Run the `gup-egui` example headless, save its screenshot, and compare it with
the chart's own render (`screenshot_parity`, ignored by default: it needs the
screenshot)

```bash
set -euo pipefail
shot="${CARGO_TARGET_DIR:-target}/visual-regression/gup_egui/egui_chart.png"
mkdir -p "$(dirname "$shot")"
GUP_SCREENSHOT_PATH="$shot" timeout 300 ./scripts/headless_env.sh --run \
  cargo run -p gup-egui --example egui_chart
GUP_EGUI_SCREENSHOT="$shot" ./scripts/headless_env.sh --run \
  cargo test -p gup-egui --test screenshot_parity -- --ignored --test-threads=1
```

## subset-inter

Regenerate `gup-text`'s bundled font (GUP-407): subset the full Inter Regular
and SemiBold (GUP-436) to the characters listed in
`crates/gup-text/fonts/inter-subset.txt` and `inter-semibold-subset.txt`
(GUP-451: ASCII and a few spaces only) respectively. Keeps GPOS `kern` and GSUB
`tnum` (for shaping, GUP-405) and every other OpenType feature is dropped, as is
hinting (fontdue and resvg don't hint). The missing-glyph box keeps its outline,
so a character outside the subset draws as a visible box, not as nothing. The
copyright, trademark and licence names (IDs 0, 7, 13 and 14) are kept, so an
embedded copy carries its OFL notice. The output is byte-for-byte reproducible
with the dev shell's fonttools (pinned by `flake.lock`). Commit the result with
the character list.

```bash
set -euo pipefail
fonts=crates/gup-text/fonts
for w in Regular SemiBold; do
  list="$fonts/inter-subset.txt"
  [[ $w == SemiBold ]] && list="$fonts/inter-semibold-subset.txt"
  pyftsubset "$fonts/Inter-$w.ttf" \
    --unicodes-file="$list" \
    --layout-features=kern,tnum \
    --no-hinting \
    --notdef-outline \
    --name-IDs=0,1,2,3,4,5,6,7,13,14 \
    --output-file="$fonts/Inter-$w-Subset.ttf"
  for f in "$fonts/Inter-$w.ttf" "$fonts/Inter-$w-Subset.ttf"; do
    printf '%-44s %7d B raw %6d B gz\n' "$f" "$(stat -c %s "$f")" \
      "$(gzip -9 -c "$f" | wc -c)"
  done
done
```

## smoke-examples

Build every example and run the headless ones, under Xvfb when available
(`gup::show` examples capture from a window; see `tests/examples_smoke.rs` for
the `GUP_SMOKE_*` knobs)

Examples are built without debug info, stripped, into a separate target
directory: with debug info the example binaries take many GB.

```bash
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}/smoke"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_STRIP=true CARGO_INCREMENTAL=0
cargo build --examples --all-features -p gup
smoke=(cargo test --all-features --test examples_smoke -- --include-ignored --test-threads=1)
# `gup::show` examples capture a frame in a real (Xvfb) window, so they
# need a display. Windowed runs (GUP_SMOKE_WINDOWED=1) use the real one.
if [ "${GUP_SMOKE_WINDOWED:-}" != 1 ] && command -v xvfb-run >/dev/null; then
  # Xvfb has no GPU: lavapipe, no WAYLAND_DISPLAY, as on CI.
  eval "$(./scripts/headless_env.sh)"
  xvfb-run -a "${smoke[@]}"
else
  "${smoke[@]}"
fi
```

## lint

Apply clippy's automatic fixes, then lint strictly. `cargo clippy --fix` exits 0
whatever it cannot fix, so only the strict runs after it can fail.

```bash
concurrently --group --names clippy,statix,mdl,gha \
   'cargo clippy --allow-no-vcs --fix --workspace --exclude gup-bevy --all-targets --all-features && cargo clippy --allow-no-vcs --fix --manifest-path dogfood/Cargo.toml --all-targets && cargo fmt --all && cargo clippy --workspace --exclude gup-bevy --all-targets -- -D warnings && cargo clippy --workspace --exclude gup-bevy --all-targets --all-features -- -D warnings && cargo clippy --manifest-path dogfood/Cargo.toml --all-targets -- -D warnings && cargo clippy -p gup-core --lib --no-default-features -- -D warnings && ./scripts/clippy_wasm32.sh' \
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
   'cargo clippy --workspace --exclude gup-bevy --all-targets -- -D warnings && cargo clippy --workspace --exclude gup-bevy --all-targets --all-features -- -D warnings && cargo clippy --manifest-path dogfood/Cargo.toml --all-targets -- -D warnings && cargo clippy -p gup-core --lib --no-default-features -- -D warnings && ./scripts/clippy_wasm32.sh' \
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
cannot fix, so only the strict clippy runs after it can fail. rustfmt runs again
after it, because its fixes are not formatted, and the pre-commit hook would
otherwise refuse the commit (found in GUP-446). `actionlint` has no fixer, so it
only checks.

```bash
shopt -qs globstar
concurrently --group --names rs,nix,md,gha \
   'git grep -lz --untracked "[[:space:]]\+$" -- "*.rs" | xargs -0 -r sed -i "/[[:space:]]\+$/s///" && cargo fmt --all && cargo fmt --manifest-path dogfood/Cargo.toml && cargo clippy --allow-no-vcs --fix --workspace --exclude gup-bevy --all-targets --all-features && cargo clippy --workspace --exclude gup-bevy --all-targets -- -D warnings && cargo clippy --workspace --exclude gup-bevy --all-targets --all-features -- -D warnings && cargo clippy --manifest-path dogfood/Cargo.toml --all-targets -- -D warnings && cargo clippy -p gup-core --lib --no-default-features -- -D warnings && ./scripts/clippy_wasm32.sh' \
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
pre-commit hook's scoping rules and its staged snapshot,
`test_shared_build_dir.sh` that checkouts sharing a build directory get their
own workspace-member artifacts (GUP-411), and `test_prune_build_dir.sh` that the
prune bounds the build directory's growth per commit without touching a live
checkout's artifacts (GUP-427), and `test_checkpoint.sh` that
`scripts/checkpoint.sh` snapshots, restores and drops work-in-progress in a
throwaway repository, without hooks or remote refs (GUP-496).

The whitespace and `ignore` checks pass only when `git grep` finds nothing (exit
1): a `git grep` error must fail them, not pass them. The `ignore` check
(GUP-501) rejects a bare `#[ignore]`: every ignored test says why, as
`#[ignore = "reason"]` (`crates/gup-culling-lod` is frozen and exempt).

This task checks the tree it runs in: run by hand, your working tree, including
unstaged edits and untracked files; in CI, the pushed commit. The pre-commit
hook runs `mask pre-commit`, which runs this task only when the staged change
needs it, and then inside its staged snapshot when the working tree differs from
the index. CI runs its own full checks on every push regardless.

```bash
shopt -qs globstar
concurrently --group --names '\s,ignore,rs,nix,md,gha,gallery,hook' \
   'git --no-pager grep --untracked --name-only --full-name "[[:space:]]\+$" -- "*.rs"; test $? -eq 1' \
   'git --no-pager grep -n -E "#\[ignore\]" -- "*.rs" ":!crates/gup-culling-lod"; test $? -eq 1' \
   'cargo fmt --all -- --check && cargo fmt --manifest-path dogfood/Cargo.toml -- --check && cargo clippy --workspace --exclude gup-bevy --all-targets -- -D warnings && cargo clippy --workspace --exclude gup-bevy --all-targets --all-features -- -D warnings && cargo clippy --manifest-path dogfood/Cargo.toml --all-targets -- -D warnings && cargo clippy -p gup-core --lib --no-default-features -- -D warnings && ./scripts/clippy_wasm32.sh' \
   'nixfmt --check flake.nix && statix check flake.nix' \
   'prettier --cache --log-level warn --check "**/*.md" && mdl --git-recurse .' \
   'actionlint .github/workflows/*.y*ml' \
   './scripts/check_gallery_sync.sh' \
   './scripts/test_pre_commit.sh && ./scripts/test_shared_build_dir.sh && ./scripts/test_prune_build_dir.sh && ./scripts/test_checkpoint.sh'
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
realistic-usage tasks with pixel checks (see `dogfood/README.md`). Headless by
default (`scripts/headless_env.sh --run`: lavapipe and Xvfb);
`DOGFOOD_DISPLAY=1` uses the real display, `DOGFOOD_SKIP_WINDOWED=1` skips the
windowed tasks.

```bash
./dogfood/run_all.sh
```

## merge-check

> After merging story branches, before pushing: the checks textual merges miss

Run it after every merge of story branches, before pushing. It needs a GPU or
lavapipe, Chromium and `wasm-bindgen` like its parts, and takes as long as its
parts (it adds no new checks).

Runs gup-core's tests with all features, `mask wasm-size` and
`mask wasm-browser` one after another, even when one fails, then prints one
table with a PASS, FAIL or SKIP per step. `--dogfood` adds the dogfood suite,
which is off by default. Needs a GPU or lavapipe, Chromium and `wasm-bindgen`
like its parts, and takes as long as its parts (it adds no new checks). Logs go
to `$CARGO_TARGET_DIR/merge-check/`. See `scripts/merge_check.sh`.

OPTIONS

- dogfood
  - flags: --dogfood
  - desc: Also run the dogfood suite (`./dogfood/run_all.sh`)

```bash
./scripts/merge_check.sh ${dogfood:+--dogfood}
```

## ci

> Run the push-to-main CI workflows locally, on software Vulkan (GUP-403)

GitHub's runners have no GPU, so every subcommand forces Mesa's lavapipe, as the
workflows do (`WGPU_BACKEND=vulkan` also keeps `gup-core`, which accepts every
backend, off a hardware GL adapter). Each subcommand mirrors one file in
`.github/workflows/`; change both together. Run this before pushing: the hook
(`mask pre-commit`) is scoped to the change and does not render. Expect 30-60
minutes from cold; `mask ci <workflow>` runs one. Not covered: the Gallery
deploy.

```bash
set -euo pipefail
mask ci lint
mask ci gallery
mask ci wasm
mask ci visual-regression
mask ci tests
mask ci dogfood
echo "mask ci: all workflows passed locally"
```

### ci lint

> Lint workflow: the full local gate, `mask all-check`

```bash
set -euo pipefail
eval "$(./scripts/headless_env.sh)"
mask all-check
```

### ci gallery

> Gallery workflow: config sync, headless thumbnails, HTML and link checks

```bash
set -euo pipefail
eval "$(./scripts/headless_env.sh)"
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
```

### ci visual-regression

> Visual regression workflow: goldens, gup-core, examples smoke, browser

```bash
set -euo pipefail
eval "$(./scripts/headless_env.sh)"
cargo test -p gup-visual-regression
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

> Tests workflow: every workspace member's tests

Built without debug info (as on CI) so the test binaries fit on disk; check that
the build directory (`CARGO_BUILD_BUILD_DIR`, else `CARGO_TARGET_DIR`) has about
10 GB free.

```bash
set -euo pipefail
eval "$(./scripts/headless_env.sh)"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_STRIP=debuginfo CARGO_INCREMENTAL=0
xvfb-run -a cargo test --workspace --exclude gup-bevy -- --test-threads=1
# The egui screenshot comparison (GUP-501), as the workflow's next step
shot="$(mktemp -d)/egui_chart.png"
GUP_SCREENSHOT_PATH="$shot" timeout 300 ./scripts/headless_env.sh --run \
  cargo run -p gup-egui --example egui_chart
GUP_EGUI_SCREENSHOT="$shot" ./scripts/headless_env.sh --run \
  cargo test -p gup-egui --test screenshot_parity -- --ignored --test-threads=1
```

### ci dogfood

> Dogfood workflow, with the windowed tasks under Xvfb as on CI

The windowed tasks' pixel checks assume the window keeps its requested size. A
tiling Wayland compositor resizes it and the checks fail, so this always uses
Xvfb (no window manager), like the workflow.

```bash
set -euo pipefail
eval "$(./scripts/headless_env.sh)"
export GUP_HEADLESS=1 # the xvfb-run below is the headless wrapper
(cd dogfood && cargo fmt -- --check && cargo test --release --lib --bin dogfood_check)
xvfb-run -a -s "-screen 0 1920x1080x24" ./dogfood/run_all.sh
```

## clean

Delete this checkout's build artifacts. With a build directory shared between
checkouts (`scripts/cargo_env.sh`), `cargo clean` would delete every checkout's,
dependencies included, so this deletes only what this checkout and its
pre-commit snapshot built: their workspace-member units, incremental caches and
target directory (GUP-427). Without one, it is `cargo clean`.

```bash
if [[ -n ${CARGO_BUILD_BUILD_DIR:-} ]]; then
  ./scripts/prune_build_dir.sh \
    --forget "$(git rev-parse --show-toplevel)" \
    --forget "$(git rev-parse --absolute-git-dir)/gup-pre-commit/tree"
else
  cargo clean
fi
```

## prune-build

Prune the build directory (GUP-427): delete what checkouts that no longer exist
built, superseded and idle incremental caches, and the least recently compiled
caches over the size cap. The pre-commit hook runs it after its Rust checks. The
rules, the knobs (`GUP_PRUNE_IDLE_DAYS`, `GUP_PRUNE_INCREMENTAL_MB`,
`GUP_PRUNE_WAIT`) and why they are safe are in `scripts/prune_build_dir.sh`.

OPTIONS

- dry_run
  - flags: -n --dry-run
  - desc: List what would be deleted, and delete nothing

```bash
./scripts/prune_build_dir.sh ${dry_run:+--dry-run}
```

## checkpoint

Snapshot the whole working tree to `refs/checkpoints/<branch>` (GUP-496):
tracked edits, staged changes, deletions and untracked files that `.gitignore`
does not ignore. It uses git plumbing, so no hook runs, and HEAD, the branch,
the index and the stash are untouched; nothing is sent to a remote. It never
replaces a real commit: commit whenever the code builds.

OPTIONS

- message
  - flags: -m --message
  - type: string
  - desc: Append to the checkpoint commit message

```bash
if [ -n "$message" ]; then
  ./scripts/checkpoint.sh save -m "$message"
else
  ./scripts/checkpoint.sh save
fi
```

## checkpoint-restore

Make the working tree match the last checkpoint of a branch (GUP-496): write the
checkpoint's files and delete tracked or untracked files (not ignored ones) that
it lacks. HEAD, the branch and the index are untouched, so the staging area is
not restored. It refuses when the checkpoint is not based on HEAD or the working
tree has changes the checkpoint lacks; `--force` first saves the current state
to `refs/checkpoints-backup/<branch>`. It never replaces a real commit.

OPTIONS

- force
  - flags: --force
  - desc: Back up the current state first, then restore even if HEAD moved
- branch
  - flags: --branch
  - type: string
  - desc: Restore this branch's checkpoint (default: the current branch)

```bash
./scripts/checkpoint.sh restore ${force:+--force} $branch
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
