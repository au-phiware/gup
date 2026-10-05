// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed channels turn the three classic encoding mistakes into compile
//! errors (RFC-001 §4, §12 risk 11). The `.stderr` snapshots are reviewed
//! for message quality and are exact rustc output: CI runs this suite on
//! the dev shell's rustc (1.93.1). Regenerate them with `TRYBUILD=overwrite`
//! when that toolchain changes.

#[test]
fn channel_mistakes_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
