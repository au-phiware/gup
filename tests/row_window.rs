// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-701: the row window is `RowWindow`, so a host can import its own
//! `Window` beside `gup::prelude::*`.

use gup::prelude::*;

#[derive(Debug, PartialEq)]
struct Window; // the host's own `Window` does not clash with the prelude

#[test]
fn row_window_is_the_same_type_at_the_root_and_in_the_prelude() {
    let from_prelude: RowWindow = RowWindow::LastRows(2_000);
    let from_root: gup::RowWindow = gup::RowWindow::LastRows(2_000);
    assert_eq!(from_prelude, from_root);
    assert_ne!(RowWindow::All, from_root);
    assert_eq!(Window, Window);
}

#[test]
fn the_window_method_takes_a_row_window() {
    let _ = gup::line(vec![(0.0, 0.0)])
        .x(|p| p.0)
        .y(|p| p.1)
        .window(RowWindow::LastRows(10));
}
