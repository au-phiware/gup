# Releasing Gup

The checklist for release 0.1.0 and the releases after it. Steps marked
**Owner** need a credential or a decision only the maintainer has; an agent
never does them. Publishing is irreversible: crates.io can yank a version, not
delete it.

All seven crates (`gup`, `gup-core`, `gup-core-macros`, `gup-wgsl`, `gup-text`,
`gup-egui`, `gup-bevy`) share one version and are published together.

## Checklist

- [ ] 1. `mask release-check` passes on `main` (agent or Owner). It runs
     `mask all-check`, the root and gup-core tests,
     `mask merge-check --dogfood`, `mask publish-check` (a publish dry run),
     `mask docs-rs`, and checks that one version is used everywhere, that
     exactly the seven crates are publishable, that their categories are
     crates.io slugs and that the tree is clean and at or above `origin/main`.
     It takes tens of minutes and never publishes. Its `ci` row reads the latest
     run of every workflow on `origin/main` and every job of it, including the
     Platforms workflow's Windows and macOS (Metal) jobs (GUP-591), and is
     `SKIP` without an authenticated `gh`: then read the runs yourself
     (`gh run list --commit <sha>`, `gh run view <id> --json jobs`). If the
     macOS job is red, neither `CHANGELOG.md` nor `README.md` may claim macOS
     support.
- [ ] 2. **Owner**: `cargo login` with a crates.io API token scoped to
     `publish-new` and `publish-update`.
- [ ] 3. **Owner**: confirm the seven names are still free
     (`https://crates.io/api/v1/crates/<name>` returns 404 for each). They were
     free on 2026-11-02.
- [ ] 4. **Owner**: enable GitHub Pages (source: GitHub Actions) and set the
     repository variable `DEPLOY_GALLERY=true`, so the gallery deploys.
- [ ] 5. **Owner**: date the CHANGELOG heading (`## [0.1.0] - YYYY-MM-DD`; it
     says `Unreleased` until now) and commit. Re-run `mask release-check`'s
     `one-version` and `clean-tree` checks
     (`scripts/release_check.sh --only one-version,clean-tree`).
- [ ] 6. **Owner**: `cargo publish --workspace`. Cargo publishes in dependency
     order and waits for each crate to appear in the index.
- [ ] 7. **Owner**: tag `v0.1.0`, push the tag, and create a GitHub release with
     the CHANGELOG section as its notes.
- [ ] 8. GUP-599: switch the docs to the crates.io dependency (the README's
     Quick Start, the tutorials).
- [ ] 9. Check docs.rs built all seven (`https://docs.rs/crate/<name>/0.1.0`).
     If one failed, read its build log, then fix it in a 0.1.1.

## If a publish fails midway

The crates already published stay. Fix the cause in a commit that does not
change the published crates. If re-running `cargo publish --workspace` stops at
a crate that is already on crates.io, publish the rest by name
(`cargo publish -p <crate>`), in the order of step 6's output.
