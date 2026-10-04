// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Suite runner: runs every task binary from the manifest (`suite.rs`),
//! applies the pixel checks to what each produced, and classifies results:
//!
//! * `PASS`  — the check met its bar;
//! * `XFAIL` — a tracked known gap still fails, as expected;
//! * `FAIL`  — an untracked failure (a regression, or a new gap);
//! * `XPASS` — a tracked known gap now passes: remove the gap entry;
//! * `SKIP`  — windowed task skipped via `DOGFOOD_SKIP_WINDOWED=1`.
//!
//! Exits non-zero on any `FAIL` or `XPASS`.
//!
//! Environment:
//! * `DOGFOOD_TIMEOUT_SECS` — per-task timeout (default 300).
//! * `DOGFOOD_SKIP_WINDOWED=1` — skip tasks that need a display.
//! * `DOGFOOD_ONLY=name[,name]` — run only the named tasks.
//! * `GITHUB_STEP_SUMMARY` — if set, a Markdown summary is appended to it.

use gup_dogfood::suite::{self, Exit, Kind, OUT_DIR, Task};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    Skip,
    Pass,
    XFail,
    XPass,
    Fail,
}

impl Status {
    fn is_failure(self) -> bool {
        matches!(self, Status::Fail | Status::XPass)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Status::Skip => "SKIP",
            Status::Pass => "PASS",
            Status::XFail => "XFAIL",
            Status::XPass => "XPASS",
            Status::Fail => "FAIL",
        };
        f.pad(s)
    }
}

struct Line {
    status: Status,
    what: String,
    detail: String,
}

struct Report {
    task: Task,
    elapsed: Duration,
    lines: Vec<Line>,
    log_tail: Vec<String>,
}

impl Report {
    /// Worst line status; a task with no lines (skipped) is `Skip`.
    fn status(&self) -> Status {
        self.lines
            .iter()
            .map(|l| l.status)
            .max()
            .unwrap_or(Status::Skip)
    }
}

/// Classify a pass/fail measurement against whether it is a tracked gap.
fn classify(passed: bool, gap: Option<suite::Gap>) -> Status {
    match (passed, gap.is_some()) {
        (true, false) => Status::Pass,
        (false, true) => Status::XFail,
        (true, true) => Status::XPass,
        (false, false) => Status::Fail,
    }
}

enum RunResult {
    Exited { code: Option<i32>, stderr: String },
    TimedOut,
    SpawnError(String),
}

fn run_bin(bin_dir: &Path, task: &Task, timeout: Duration) -> RunResult {
    let logs = Path::new(OUT_DIR).join("logs");
    let _ = fs::create_dir_all(&logs);
    let out_log = logs.join(format!("{}.stdout.log", task.name));
    let err_log = logs.join(format!("{}.stderr.log", task.name));
    let (Ok(out), Ok(err)) = (fs::File::create(&out_log), fs::File::create(&err_log)) else {
        return RunResult::SpawnError(format!("cannot create logs in {}", logs.display()));
    };
    let mut cmd = Command::new(bin_dir.join(task.bin));
    cmd.envs(task.env.iter().copied())
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err);
    if task.kind == Kind::Windowed {
        cmd.env("DOGFOOD_AUTO", "1");
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return RunResult::SpawnError(format!("{}: {e}", bin_dir.join(task.bin).display()));
        }
    };
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return RunResult::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return RunResult::SpawnError(e.to_string()),
        }
    };
    RunResult::Exited {
        code: status.code(),
        stderr: fs::read_to_string(&err_log).unwrap_or_default(),
    }
}

/// Last few meaningful stderr lines, skipping GPU driver chatter.
fn tail(stderr: &str, n: usize) -> Vec<String> {
    let lines: Vec<&str> = stderr
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.contains("wgpu_hal") && !l.contains("vkCreate"))
        .collect();
    lines[lines.len().saturating_sub(n)..]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn run_task(bin_dir: &Path, task: Task, timeout: Duration, skip_windowed: bool) -> Report {
    let mut report = Report {
        task,
        elapsed: Duration::ZERO,
        lines: vec![],
        log_tail: vec![],
    };
    let task = &report.task;
    if skip_windowed && task.kind == Kind::Windowed {
        return report;
    }
    // Remove stale outputs so a previous run can never satisfy a check.
    for c in &task.checks {
        let _ = fs::remove_file(Path::new(OUT_DIR).join(c.file));
    }
    let start = Instant::now();
    let result = run_bin(bin_dir, task, timeout);
    report.elapsed = start.elapsed();

    let mut lines = Vec::new();
    let mut run_checks = false;
    match (&result, task.exit) {
        (RunResult::SpawnError(e), _) => {
            lines.push(Line {
                status: Status::Fail,
                what: "run".into(),
                detail: e.clone(),
            });
        }
        (RunResult::TimedOut, _) => lines.push(Line {
            status: Status::Fail,
            what: "run".into(),
            detail: format!("timed out after {}s", timeout.as_secs()),
        }),
        (RunResult::Exited { code: Some(0), .. }, Exit::Success) => {
            lines.push(Line {
                status: Status::Pass,
                what: "exits 0".into(),
                detail: String::new(),
            });
            run_checks = true;
        }
        (RunResult::Exited { code, .. }, Exit::Success) => lines.push(Line {
            status: Status::Fail,
            what: "exits 0".into(),
            detail: format!("exit status {code:?}"),
        }),
        (RunResult::Exited { code: Some(0), .. }, Exit::Fails { gap, .. }) => lines.push(Line {
            status: Status::XPass,
            what: "expected failure".into(),
            detail: format!("now exits 0; remove the tracked gap: {}", gap.0),
        }),
        (
            RunResult::Exited { code, stderr },
            Exit::Fails {
                stderr_contains,
                gap,
            },
        ) => {
            if stderr.contains(stderr_contains) {
                lines.push(Line {
                    status: Status::XFail,
                    what: "expected failure".into(),
                    detail: format!(
                        "exit {code:?}, stderr contains {stderr_contains:?} — {}",
                        gap.0
                    ),
                });
            } else {
                lines.push(Line {
                    status: Status::Fail,
                    what: "expected failure".into(),
                    detail: format!(
                        "exit {code:?} but stderr lacks {stderr_contains:?}: failure mode changed"
                    ),
                });
            }
        }
    }
    if let RunResult::Exited { stderr, .. } = &result {
        if lines.iter().any(|l| l.status.is_failure()) {
            report.log_tail = tail(stderr, 8);
        }
    }

    if run_checks {
        for c in &task.checks {
            let m = c.measure.evaluate_file(&Path::new(OUT_DIR).join(c.file));
            let (passed, detail) = (m.passed, m.detail);
            let status = classify(passed, c.gap);
            let detail = match c.gap {
                Some(g) if status == Status::XPass => {
                    format!("{detail}; now passes, remove the tracked gap: {}", g.0)
                }
                Some(g) => format!("{detail} — known gap: {}", g.0),
                None => detail,
            };
            lines.push(Line {
                status,
                what: format!("{}: {}", c.file, c.what),
                detail,
            });
        }
    }
    report.lines = lines;
    report
}

fn print_report(r: &Report) {
    println!(
        "\n[{:<5}] {} ({}, {:.1}s) — {}",
        r.status(),
        r.task.name,
        r.task.bin,
        r.elapsed.as_secs_f32(),
        r.task.intent
    );
    if r.status() == Status::Skip {
        println!("    SKIP   windowed task (DOGFOOD_SKIP_WINDOWED)");
    }
    for l in &r.lines {
        println!("    {:<6} {}  [{}]", l.status, l.what, l.detail);
    }
    for t in &r.log_tail {
        println!("      | {t}");
    }
}

fn write_step_summary(reports: &[Report]) {
    let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") else {
        return;
    };
    let Ok(mut f) = fs::OpenOptions::new().append(true).create(true).open(path) else {
        return;
    };
    let mut s = String::from(
        "## Dogfood suite\n\n| Task | Status | Pass | XFail | Fail | XPass |\n|---|---|---|---|---|---|\n",
    );
    for r in reports {
        let count = |st| r.lines.iter().filter(|l| l.status == st).count();
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            r.task.name,
            r.status(),
            count(Status::Pass),
            count(Status::XFail),
            count(Status::Fail),
            count(Status::XPass)
        ));
    }
    let gaps: Vec<_> = reports
        .iter()
        .flat_map(|r| {
            r.lines
                .iter()
                .filter(|l| l.status != Status::Pass)
                .map(move |l| (r, l))
        })
        .collect();
    if !gaps.is_empty() {
        s.push_str("\n### Non-passing checks\n\n");
        for (r, l) in gaps {
            s.push_str(&format!(
                "- **{}** `{}` {} — {}\n",
                l.status, r.task.name, l.what, l.detail
            ));
        }
    }
    let _ = f.write_all(s.as_bytes());
}

fn main() -> ExitCode {
    let bin_dir: PathBuf = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .expect("locate sibling task binaries");
    let timeout = Duration::from_secs(
        std::env::var("DOGFOOD_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300),
    );
    let skip_windowed = std::env::var("DOGFOOD_SKIP_WINDOWED").is_ok_and(|v| v == "1");
    let only: Option<Vec<String>> = std::env::var("DOGFOOD_ONLY")
        .ok()
        .map(|s| s.split(',').map(|t| t.trim().to_string()).collect());
    if let Err(e) = fs::create_dir_all(OUT_DIR) {
        eprintln!("cannot create {OUT_DIR}: {e}");
        return ExitCode::FAILURE;
    }

    println!(
        "gup dogfood suite: binaries in {}, outputs in {OUT_DIR}",
        bin_dir.display()
    );
    let setup = run_task(&bin_dir, suite::setup(), timeout, false);
    print_report(&setup);
    if setup.status().is_failure() {
        println!("\nfixture generation failed; not running tasks");
        return ExitCode::FAILURE;
    }

    let mut reports = Vec::new();
    for task in suite::tasks() {
        if only
            .as_ref()
            .is_some_and(|o| !o.iter().any(|n| n == task.name))
        {
            continue;
        }
        let r = run_task(&bin_dir, task, timeout, skip_windowed);
        print_report(&r);
        reports.push(r);
    }

    let tally = |st: Status| reports.iter().filter(|r| r.status() == st).count();
    println!(
        "\nTasks: {} pass, {} pass with tracked gaps (XFAIL), {} FAIL, {} XPASS (stale gap tracking), {} skipped",
        tally(Status::Pass),
        tally(Status::XFail),
        tally(Status::Fail),
        tally(Status::XPass),
        tally(Status::Skip)
    );
    write_step_summary(&reports);
    if reports.iter().any(|r| r.status().is_failure()) {
        println!("dogfood: FAILED (unexpected failures or stale gap tracking above)");
        ExitCode::FAILURE
    } else {
        println!("dogfood: OK");
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_distinguishes_all_four_outcomes() {
        let gap = Some(suite::Gap("tracked"));
        assert_eq!(classify(true, None), Status::Pass);
        assert_eq!(classify(false, gap), Status::XFail);
        assert_eq!(classify(true, gap), Status::XPass);
        assert_eq!(classify(false, None), Status::Fail);
    }

    #[test]
    fn only_untracked_failures_and_stale_gaps_fail_the_build() {
        assert!(Status::Fail.is_failure());
        assert!(Status::XPass.is_failure());
        assert!(!Status::XFail.is_failure());
        assert!(!Status::Pass.is_failure());
        assert!(!Status::Skip.is_failure());
    }

    #[test]
    fn tail_drops_gpu_driver_noise() {
        let t = tail("a\n\nwgpu_hal noise\nb\nvkCreateInstance x\nc\n", 2);
        assert_eq!(t, vec!["b".to_string(), "c".to_string()]);
    }
}
