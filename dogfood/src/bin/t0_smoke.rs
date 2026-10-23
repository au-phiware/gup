// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Control: the simplest possible chart -> PNG, to check text/axes render.

use gup::prelude::*;

struct P {
    x: f32,
    y: f32,
}

fn main() -> gup::Result<()> {
    env_logger::init();
    let data: Vec<P> = (0..20)
        .map(|i| P {
            x: i as f32,
            y: (i * i) as f32,
        })
        .collect();

    let cx = Context::new_blocking()?;
    gup::scatter(data)
        .x(|p| p.x)
        .y(|p| p.y)
        .title("Smoke test")
        .save_png(&cx, "/tmp/gup-dogfood/t0_smoke_builder.png", 800, 500)
}
