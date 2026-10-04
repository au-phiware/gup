// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Examples smoke test.
//!
//! Two tests live here:
//!
//! - `skip_list_matches_examples` (always runs, no GPU): every windowed
//!   example without a headless path is on the skip list with a reason, every
//!   skip-list entry names an example that still exists, and every
//!   `examples/` expected-failure entry names a real example.
//! - `examples_run_headless` (ignored by default; needs built example
//!   binaries): runs every non-skipped example. Examples that call
//!   `gup::export::gallery::screenshot_request()` run with
//!   `GUP_SCREENSHOT_PATH` set (they render one frame offscreen and exit) and
//!   must write a non-blank PNG; console examples must exit zero. Each result
//!   is reconciled with `tests/visual_regression/expected_failures.toml`.
//!
//! Run the full smoke test with `mask smoke-examples`, or:
//!
//! ```text
//! cargo build --examples --all-features
//! cargo test --all-features --test examples_smoke -- --ignored --test-threads=1
//! ```
//!
//! Environment knobs:
//!
//! - `GUP_SMOKE_FILTER=a,b`: only run examples whose name contains `a` or `b`.
//! - `GUP_SMOKE_TIMEOUT_SECS` (default 60): per-example timeout.
//! - `GUP_SMOKE_WINDOWED=1`: also run the skip-listed *windowed* examples
//!   (needs a display) for `GUP_SMOKE_WINDOW_SECS` (default 5) seconds each;
//!   an example passes if it is still running (or exited zero) at the
//!   deadline and fails if it panicked or exited non-zero first.

use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{
    Check, CheckFailure, ExpectedFailures, GoldenStore, Harness, LayoutMetadata, PxRect, RgbaImage,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SKIP_LIST: &str = "tests/visual_regression/examples_skip_list.toml";
const EXPECTED_FAILURES: &str = "tests/visual_regression/expected_failures.toml";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Kind {
    /// Supports `GUP_SCREENSHOT_PATH`: renders one frame offscreen and exits.
    Screenshot,
    /// Opens a window (winit `EventLoop` / `ApplicationHandler`, or the
    /// `GupApp` shell) and has no headless path.
    Windowed,
    /// Neither: prints to the console and exits.
    Console,
}

#[derive(Debug)]
struct Example {
    name: String,
    source: PathBuf,
    required_features: Vec<String>,
    kind: Kind,
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Example targets of the `gup` package, from `cargo metadata`.
fn examples() -> Vec<Example> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(cargo)
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(root())
        .output()
        .expect("failed to run cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).expect("metadata JSON");
    let package = meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|p| p["name"] == "gup")
        .expect("gup package");
    let mut examples: Vec<Example> = package["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .filter(|t| {
            t["kind"]
                .as_array()
                .is_some_and(|k| k.iter().any(|k| k == "example"))
        })
        .map(|t| {
            let source = PathBuf::from(t["src_path"].as_str().expect("src_path"));
            let text = std::fs::read_to_string(&source).expect("example source");
            let kind = if text.contains("screenshot_request") {
                Kind::Screenshot
            } else if text.contains("EventLoop")
                || text.contains("ApplicationHandler")
                || text.contains("GupApp::new")
            {
                Kind::Windowed
            } else {
                Kind::Console
            };
            Example {
                name: t["name"].as_str().expect("name").to_string(),
                source,
                required_features: t["required-features"]
                    .as_array()
                    .map(|f| {
                        f.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                kind,
            }
        })
        .collect();
    examples.sort_by(|a, b| a.name.cmp(&b.name));
    examples
}

/// The skip list: example name → reason.
fn skip_list() -> BTreeMap<String, String> {
    let path = root().join(SKIP_LIST);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let doc: toml_edit::DocumentMut = text
        .parse()
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut skips = BTreeMap::new();
    let tables = doc
        .get("skip")
        .and_then(|i| i.as_array_of_tables())
        .unwrap_or_else(|| panic!("{}: expected [[skip]] tables", path.display()));
    for (i, t) in tables.iter().enumerate() {
        let get = |k: &str| {
            t.get(k)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    panic!(
                        "{}: entry {} needs a non-empty `{k}`",
                        path.display(),
                        i + 1
                    )
                })
                .to_string()
        };
        let name = get("name");
        assert!(
            skips.insert(name.clone(), get("reason")).is_none(),
            "{}: duplicate entry {name}",
            path.display()
        );
    }
    skips
}

fn expected_failures() -> ExpectedFailures {
    ExpectedFailures::load(root().join(EXPECTED_FAILURES))
        .unwrap_or_else(|e| panic!("invalid expected-failure list: {e}"))
}

#[test]
fn skip_list_matches_examples() {
    let examples = examples();
    let skips = skip_list();
    let names: Vec<&str> = examples.iter().map(|e| e.name.as_str()).collect();

    let stale: Vec<_> = skips
        .keys()
        .filter(|s| !names.contains(&s.as_str()))
        .collect();
    assert!(
        stale.is_empty(),
        "{SKIP_LIST} lists examples that no longer exist (remove them): {stale:?}"
    );

    let unlisted: Vec<_> = examples
        .iter()
        .filter(|e| e.kind == Kind::Windowed && !skips.contains_key(&e.name))
        .map(|e| format!("{} ({})", e.name, e.source.display()))
        .collect();
    assert!(
        unlisted.is_empty(),
        "windowed examples without a GUP_SCREENSHOT_PATH path must be on {SKIP_LIST} \
         (or gain a headless path): {unlisted:?}"
    );

    let needless: Vec<_> = examples
        .iter()
        .filter(|e| e.kind == Kind::Screenshot && skips.contains_key(&e.name))
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        needless.is_empty(),
        "these examples support GUP_SCREENSHOT_PATH and must be smoke-tested, not skipped: {needless:?}"
    );

    for entry in expected_failures().entries() {
        if let Some(name) = entry.case.strip_prefix("examples/") {
            assert!(
                examples
                    .iter()
                    .any(|e| entry.matches_case(&format!("examples/{}", e.name))),
                "expected-failure entry {:?} matches no example ({name})",
                entry.case
            );
        }
    }
}

fn env_secs(var: &str, default: u64) -> Duration {
    Duration::from_secs(
        std::env::var(var)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default),
    )
}

/// `target/<profile>/examples`, derived from this test binary's location
/// (`target/<profile>/deps/examples_smoke-<hash>`).
fn examples_bin_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    exe.parent()
        .and_then(Path::parent)
        .expect("test binary lives in target/<profile>/deps")
        .join("examples")
}

enum RunEnd {
    Exited(std::process::ExitStatus),
    StillRunning,
}

/// Run `bin` in `cwd` until it exits or `limit` elapses; output goes to
/// `log`. Examples run in a scratch directory because several of them write
/// files into their working directory.
fn run_with_limit(
    bin: &Path,
    cwd: &Path,
    envs: &[(&str, &Path)],
    limit: Duration,
    log: &Path,
) -> std::io::Result<RunEnd> {
    let file = std::fs::File::create(log)?;
    let mut child = Command::new(bin)
        .current_dir(cwd)
        .envs(envs.iter().map(|(k, v)| (*k, v.as_os_str())))
        .env("RUST_BACKTRACE", "0")
        .stdin(Stdio::null())
        .stdout(file.try_clone()?)
        .stderr(file)
        .spawn()?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(RunEnd::Exited(status));
        }
        if start.elapsed() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(RunEnd::StillRunning);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn log_tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(6);
    lines[start..].join(" | ")
}

#[test]
#[ignore = "needs built example binaries; run `mask smoke-examples`"]
fn examples_run_headless() {
    let skips = skip_list();
    let expected = expected_failures();
    let artifacts = default_artifact_dir(root()).join("examples");
    std::fs::create_dir_all(&artifacts).expect("artifact dir");
    let scratch = artifacts.join("cwd");
    std::fs::create_dir_all(&scratch).expect("scratch cwd");
    // Examples are not golden-compared here; the harness is used for its
    // NotBlank check and expected-failure reconciliation.
    let harness = Harness::new(
        GoldenStore::new(root().join("tests/golden"), &artifacts).with_bless(false),
        expected.clone(),
    );
    let bin_dir = examples_bin_dir();
    let timeout = env_secs("GUP_SMOKE_TIMEOUT_SECS", 60);
    let window_secs = env_secs("GUP_SMOKE_WINDOW_SECS", 5);
    let run_windowed = std::env::var("GUP_SMOKE_WINDOWED").is_ok_and(|v| v == "1");
    let filters: Vec<String> = std::env::var("GUP_SMOKE_FILTER")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let mut lines = Vec::new();
    let mut problems = Vec::new();
    let mut counts = BTreeMap::<&str, usize>::new();

    for ex in examples() {
        if !filters.is_empty() && !filters.iter().any(|f| ex.name.contains(f.as_str())) {
            continue;
        }
        let case = format!("examples/{}", ex.name);
        let windowed_run = run_windowed && ex.kind == Kind::Windowed;
        if let Some(reason) = skips.get(&ex.name)
            && !windowed_run
        {
            *counts.entry("skipped").or_default() += 1;
            lines.push(format!("SKIP  {:<40} {reason}", ex.name));
            continue;
        }
        let bin = bin_dir.join(&ex.name);
        if !bin.exists() {
            if !ex.required_features.is_empty() {
                *counts.entry("not built").or_default() += 1;
                lines.push(format!(
                    "SKIP  {:<40} not built (requires --features {})",
                    ex.name,
                    ex.required_features.join(",")
                ));
            } else {
                problems.push(format!(
                    "{}: binary {} missing; run `cargo build --examples --all-features` first",
                    ex.name,
                    bin.display()
                ));
            }
            continue;
        }

        let log = artifacts.join(format!("{}.log", ex.name));
        let png = artifacts.join(format!("{}.png", ex.name));
        let _ = std::fs::remove_file(&png);
        let started = Instant::now();
        let (envs, limit): (Vec<(&str, &Path)>, Duration) = match ex.kind {
            Kind::Screenshot => (vec![("GUP_SCREENSHOT_PATH", png.as_path())], timeout),
            Kind::Console => (vec![], timeout),
            Kind::Windowed => (vec![], window_secs),
        };
        let end = run_with_limit(&bin, &scratch, &envs, limit, &log);
        let elapsed = started.elapsed().as_secs_f32();

        let render: Result<(), String> = match (ex.kind, end) {
            (_, Err(e)) => Err(format!("failed to spawn: {e}")),
            (Kind::Windowed, Ok(RunEnd::StillRunning)) => Ok(()),
            (_, Ok(RunEnd::StillRunning)) => Err(format!("timed out after {}s", limit.as_secs())),
            (_, Ok(RunEnd::Exited(status))) if !status.success() => {
                Err(format!("exited with {status}: {}", log_tail(&log)))
            }
            (Kind::Screenshot, Ok(RunEnd::Exited(_))) if !png.exists() => Err(format!(
                "exited zero but wrote no PNG to GUP_SCREENSHOT_PATH: {}",
                log_tail(&log)
            )),
            _ => Ok(()),
        };

        let report = match (render, ex.kind) {
            (Err(msg), _) => harness.run_checks(&case, Err(msg), &[]),
            (Ok(()), Kind::Screenshot) => {
                let capture = RgbaImage::load_png(&png)
                    .map_err(|e| format!("screenshot unreadable: {e}"))
                    .map(|img| {
                        let full = PxRect::new(0.0, 0.0, img.width() as f32, img.height() as f32);
                        (img, LayoutMetadata::new(full))
                    });
                harness.run_checks(&case, capture, &[Check::NotBlank])
            }
            (Ok(()), _) => {
                let verdict = expected.reconcile(&case, &[Check::Render], &[] as &[CheckFailure]);
                if !verdict.is_ok() {
                    problems.push(format!(
                        "{case}: passes now; remove its expected-failure entries: {:?}",
                        verdict.unexpected_passes
                    ));
                }
                *counts.entry("passed").or_default() += 1;
                lines.push(format!("PASS  {:<40} {:?} {elapsed:.1}s", ex.name, ex.kind));
                continue;
            }
        };

        let status = if !report.is_ok() {
            problems.push(report.to_string());
            "FAIL "
        } else if !report.verdict.expected_failures.is_empty() {
            "XFAIL"
        } else {
            "PASS "
        };
        *counts
            .entry(match status {
                "FAIL " => "failed",
                "XFAIL" => "expected failures",
                _ => "passed",
            })
            .or_default() += 1;
        let detail = report
            .failures
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        lines.push(format!(
            "{status} {:<40} {:?} {elapsed:.1}s {detail}",
            ex.name, ex.kind
        ));
    }

    eprintln!(
        "examples smoke test (artifacts in {}):",
        artifacts.display()
    );
    for line in &lines {
        eprintln!("  {line}");
    }
    eprintln!("  totals: {counts:?}");
    assert!(
        problems.is_empty(),
        "examples smoke test found {} unexpected result(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}
