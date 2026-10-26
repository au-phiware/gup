// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GUP-563: every method that takes a pixel position accepts a `(f32, f32)`
// as well as a `geom::Point`, from a crate that never names `geom::Point`.
use gup_core::Input;
use gup_core::prelude::*;

fn call_all(plot: &mut Plot) {
    // A `geom::Point`, without naming the type.
    let Input::PointerMoved { at: point } = Input::pointer_moved((1.0, 2.0)) else {
        unreachable!()
    };

    plot.zoom((1.0, 2.0), 2.0).unwrap();
    plot.zoom(point, 2.0).unwrap();
    let _ = plot.invert((1.0, 2.0));
    let _ = plot.invert(point);
    let _ = plot.invert([1.0, 2.0]);
    let _ = plot.pick_row((1.0, 2.0));
    let _ = plot.pick_row(point);
    let _ = plot.pick::<u32>((1.0, 2.0));
    let _ = plot.pick::<u32>(point);
}

#[allow(dead_code)]
fn builders() {
    let mut s = gup_core::scatter(Vec::<u32>::new());
    let _ = s.pick((1.0, 2.0));
    let mut l = gup_core::line(Vec::<u32>::new());
    let _ = l.pick((1.0, 2.0));
    let mut b = gup_core::bar(Vec::<u32>::new());
    let _ = b.pick((1.0, 2.0));
    let mut h = gup_core::histogram(Vec::<f64>::new());
    let _ = h.pick((1.0, 2.0));
    let mut m = gup_core::heatmap(Vec::<u32>::new());
    let _ = m.pick((1.0, 2.0));

    let _ = Input::pointer_down((1.0, 2.0), gup_core::Button::Primary);
    let _ = Input::pointer_up([1.0, 2.0], gup_core::Button::Primary);
    let _ = Input::wheel((1.0, 2.0), 1.0);
}

fn main() {
    let _ = (call_all as fn(&mut Plot), builders as fn());
}
