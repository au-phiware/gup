// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-487: the macros' expansions name the crate the caller can name. A
//! detached workspace holds a stand-in `gup` facade that re-exports gup-core
//! and a user crate depending only on it, renamed to `charts`; it must
//! compile.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn wgsl_function_expands_through_a_renamed_facade() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reexport");
    // Start from the main workspace's lockfile so the detached workspace
    // resolves the same versions offline. Resolving afresh can pick a release
    // the registry has since yanked (CI hit `slab` 0.4.10); cargo prunes the
    // entries it does not need and adds the two local crates.
    let lock = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    std::fs::copy(&lock, root.join("Cargo.lock")).expect("copy the workspace Cargo.lock");
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
        .join("reexport");
    let out = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .args(["-p", "reexport-user"])
        .env("CARGO_TARGET_DIR", target)
        .output()
        .expect("run cargo");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
