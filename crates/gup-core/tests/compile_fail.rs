// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed channels turn the three classic encoding mistakes into compile
//! errors (RFC-001 §4, §12 risk 11). The `.stderr` snapshots are reviewed
//! for message quality and are exact rustc output, so they match only the
//! toolchain pinned in `rust-toolchain.toml` (used by the dev shell and CI).
//! Regenerate them with `TRYBUILD=overwrite` when that pin moves.

#[test]
fn channel_mistakes_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
