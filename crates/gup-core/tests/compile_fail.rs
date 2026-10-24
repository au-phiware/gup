// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed channels turn the three classic encoding mistakes into compile
//! errors (RFC-001 §4, §12 risk 11), and a borrowing key accessor compiles
//! only through `encode_key` (§12 risk 4). The `.stderr` snapshots are
//! reviewed for message quality and are exact rustc output, so they match
//! only the toolchain pinned in `rust-toolchain.toml` (used by the dev
//! shell and CI). Regenerate them with `TRYBUILD=overwrite` when that pin
//! moves.
//!
//! What changes a snapshot (GUP-500): a toolchain bump changes many; a new
//! `IntoEncoding` impl changes `bare_number`; a new `ColumnValue` type
//! changes none (its impls carry `#[diagnostic::do_not_recommend]`, add it to
//! a new one; `borrowing_accessor` fails the bound once, in `encode`, because
//! a second error through `attr` would print an unrelated `IntoEncoding`
//! hint). A new scale or WGSL module changes none: the library's
//! concrete `CpuMirror` impls carry `#[diagnostic::do_not_recommend]` (add
//! it to a new one), which keeps them out of rustc's "other types
//! implement" lists, and the unknown-import and unknown-module errors name
//! only the closest module.

#[test]
fn channel_mistakes_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
    t.pass("tests/compile_pass/*.rs");
}

/// `#[wgsl_function]` reports WGSL and calling-convention mistakes at the
/// user's code, saying what to do (GUP-420). naga_oil's reports point at
/// the WGSL line inside the Rust file.
#[test]
fn wgsl_function_mistakes_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/wgsl_function_fail/*.rs");
}

/// `#[derive(Mark)]` checks a mark's WGSL input struct against its channels
/// at compile time, and names the mark and the mismatch (GUP-422).
#[test]
fn derive_mark_mistakes_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/derive_mark_fail/*.rs");
}
