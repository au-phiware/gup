// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Rust and wgpu versions the docs state must match the manifests (GUP-704).
//! This checks strings, not prose: it needs no GPU.

use std::{fs, path::Path};

use toml_edit::DocumentMut;

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// `workspace.package.rust-version` in `Cargo.toml`.
fn workspace_rust_version() -> String {
    let doc: DocumentMut = read("Cargo.toml").parse().expect("Cargo.toml parses");
    doc["workspace"]["package"]["rust-version"]
        .as_str()
        .expect("workspace.package.rust-version is a string")
        .to_owned()
}

/// The major version of the single `wgpu` package in `Cargo.lock`.
fn locked_wgpu_major() -> String {
    let doc: DocumentMut = read("Cargo.lock").parse().expect("Cargo.lock parses");
    let packages = doc["package"]
        .as_array_of_tables()
        .expect("Cargo.lock has [[package]] tables");
    let wgpu: Vec<_> = packages
        .iter()
        .filter(|p| p["name"].as_str() == Some("wgpu"))
        .collect();
    assert_eq!(wgpu.len(), 1, "Cargo.lock must hold exactly one wgpu");
    let version = wgpu[0]["version"]
        .as_str()
        .expect("the wgpu package has a version");
    version
        .split('.')
        .next()
        .expect("a version has a major number")
        .to_owned()
}

#[test]
fn docs_state_the_workspace_rust_version() {
    let version = workspace_rust_version();
    for file in [
        "README.md",
        "docs/tutorials/README.md",
        "docs/tutorials/01_getting_started.md",
        "CHANGELOG.md",
    ] {
        assert!(
            read(file).contains(&version),
            "{file} must state Rust {version} (workspace rust-version in Cargo.toml)"
        );
    }
    for file in [
        "README.md",
        "docs/tutorials/README.md",
        "docs/tutorials/01_getting_started.md",
    ] {
        assert!(
            !read(file).contains("1.85"),
            "{file} still states Rust 1.85; it must say {version}"
        );
    }
}

#[test]
fn readme_states_the_locked_wgpu_major() {
    let major = locked_wgpu_major();
    let expected = format!("wgpu {major}");
    assert!(
        read("README.md").contains(&expected),
        "README.md must state `{expected}` (the wgpu major in Cargo.lock)"
    );
}
