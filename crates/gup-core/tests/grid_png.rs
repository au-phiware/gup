// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S10a (GUP-460): `Plot::grid`. The reference scatter with grid
//! lines at the ticks: drawn behind the marks, inside the plot rect, under
//! the axes; counted per `Axes`; followed through a zoom; written as SVG.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use common::svg_marks;
use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::scene::{ItemKind, Z_GRID};
use gup_core::{ImageTarget, Renderer, SvgTarget, VectorTarget};
use gup_visual_regression::RgbaImage;

fn close(got: image::Rgba<u8>, want: Color) -> bool {
    let want = want.to_rgba8();
    got.0.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2)
}

fn grid_plot(axes: Axes) -> Plot {
    let mut plot = scatter::plot();
    plot.grid(axes);
    plot
}

fn grid_rules(resolved: &gup_core::Resolved) -> usize {
    resolved
        .scene
        .items
        .iter()
        .filter(|i| i.z == Z_GRID)
        .map(|i| match &i.kind {
            ItemKind::Rules(r) => r.len(),
            _ => panic!("the grid item is not a Rules item"),
        })
        .sum()
}

#[test]
fn grid_lines_sit_at_the_ticks_behind_the_marks() {
    let cx = Context::new_blocking().unwrap();
    let theme = Theme::default();
    let (plain, _) = scatter::plot().render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let mut grid = grid_plot(Axes::Both);
    let (image, layout) = grid.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let p = layout.plot;
    assert!(layout.x_ticks.len() >= 4 && layout.y_ticks.len() >= 4);

    // Halfway between two x ticks, on a y tick's row: the grid colour.
    let row = |py: f32| (py.floor() + 0.5) as u32;
    let mut found = false;
    for w in layout.x_ticks.windows(2) {
        let x = ((w[0].1 + w[1].1) / 2.0) as u32;
        for &(_, py) in &layout.y_ticks {
            let y = row(py);
            if (p.top() as u32..p.bottom() as u32).contains(&y)
                && *plain.get_pixel(x, y) == image::Rgba([255, 255, 255, 255])
            {
                assert!(close(*image.get_pixel(x, y), theme.grid), "({x},{y})");
                found = true;
            }
        }
    }
    assert!(found, "no empty pixel between ticks on a grid row");

    // Where a dot covers a grid row, the dot wins.
    let mut covered = 0;
    for &(_, py) in &layout.y_ticks {
        let y = row(py);
        for x in p.left() as u32 + 2..p.right() as u32 - 2 {
            let bg = plain.get_pixel(x, y).0;
            if bg[..3].iter().map(|c| 255 - *c as u32).sum::<u32>() > 300 {
                // Dots are slightly translucent, so the grid shows through
                // by a few levels (a fraction of the 26-level grid step).
                let (a, b) = (image.get_pixel(x, y).0, plain.get_pixel(x, y).0);
                assert!(
                    a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 16),
                    "({x},{y}): {a:?} vs {b:?}"
                );
                covered += 1;
            }
        }
    }
    assert!(covered > 0, "no dot on a grid row");

    // Outside the plot rect, on a tick row: no grid colour.
    for &(_, py) in &layout.y_ticks {
        let y = row(py);
        for x in [p.left() - 3.0, p.right() + 3.0] {
            assert!(!close(*image.get_pixel(x as u32, y), theme.grid));
        }
    }
    // The axis lines stay dark where grid lines meet them.
    let axis_x = p.left() as u32 - 1;
    for &(_, py) in &layout.y_ticks {
        let y = row(py);
        assert_eq!(image.get_pixel(axis_x, y), plain.get_pixel(axis_x, y));
    }
    let mid = (p.top() + p.height / 2.0) as u32;
    assert!(close(*image.get_pixel(axis_x, mid), theme.axis));

    let meta = layout_metadata(&layout);
    let vr = RgbaImage::new(WIDTH, HEIGHT, image.into_raw()).unwrap();
    harness()
        .run("gup_core/grid", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    // GUP-467: the SVG draws the same marks (and grid lines).
    let scene = grid
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .scene
        .clone();
    svg_marks::assert_matches("gup_core/grid", &scene, &vr, &meta);
}

#[test]
fn grid_rules_follow_the_axes() {
    let cx = Context::new_blocking().unwrap();
    for (axes, x, y) in [
        (Axes::Neither, false, false),
        (Axes::X, true, false),
        (Axes::Y, false, true),
        (Axes::Both, true, true),
    ] {
        let mut plot = grid_plot(axes);
        let resolved = plot.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
        let (nx, ny) = (resolved.layout.x_ticks.len(), resolved.layout.y_ticks.len());
        assert_eq!(
            grid_rules(resolved),
            if x { nx } else { 0 } + if y { ny } else { 0 },
            "{axes:?}"
        );
    }
}

#[test]
fn zooming_a_grid_writes_no_columns_and_only_grid_rules() {
    const FRAMES: usize = 12;
    let cx = Context::new_blocking().unwrap();
    let run = |grid: Axes| {
        let (mut plot, x, _) = scatter::plot_rows(scatter::countries_n(2_000), Px(3.0));
        plot.grid(grid);
        let mut target = ImageTarget::new(&cx, 640, 400).unwrap();
        let mut renderer = Renderer::new();
        let first = plot.resolve(&cx, 640.0, 400.0).unwrap();
        renderer.render(&cx, &first.scene, &mut target).unwrap();
        let (x0, x1) = x.read().current_domain().unwrap();
        let before = cx.upload_stats();
        let mut rules = 0;
        for i in 1..=FRAMES {
            let k = 0.7f64.powi(i as i32);
            x.write().set_domain(x0, x0 + (x1 - x0) * k).unwrap();
            let resolved = plot.resolve(&cx, 640.0, 400.0).unwrap();
            rules += grid_rules(resolved);
            renderer.render(&cx, &resolved.scene, &mut target).unwrap();
        }
        let written = cx.upload_stats() - before;
        assert_eq!(written.columns.bytes, 0, "{written:?}");
        (
            written.instances.bytes,
            rules as u64,
            written.instances.writes,
        )
    };
    let (plain, none, plain_writes) = run(Axes::Neither);
    let (both, rules, writes) = run(Axes::Both);
    let (x_only, x_rules, _) = run(Axes::X);
    assert_eq!(none, 0);
    assert!(rules > 0 && x_rules > 0 && x_rules < rules);
    // A rule is a fixed number of bytes: the extra bytes are exactly the
    // grid's rules, however many the zoom produced. The grid is one more
    // draw per frame, so one more write.
    let rule_bytes = (x_only - plain) / x_rules;
    assert!(rule_bytes > 0);
    assert_eq!(x_only - plain, rule_bytes * x_rules);
    assert_eq!(both - plain, rule_bytes * rules);
    assert_eq!(writes, plain_writes + FRAMES as u64);
}

#[test]
fn svg_writes_one_clipped_line_per_grid_line() {
    let cx = Context::new_blocking().unwrap();
    let svg = |mut plot: Plot| {
        let guides = plot
            .resolve(&cx, WIDTH as f32, HEIGHT as f32)
            .unwrap()
            .scene
            .guides();
        let mut target = SvgTarget::new();
        target.render(&guides).unwrap();
        (target.svg().to_owned(), guides.rules().count())
    };
    let (a, a_rules) = svg(grid_plot(Axes::Both));
    let (b, b_rules) = svg(scatter::plot());
    let grid = a_rules - b_rules;
    assert!(grid >= 8);
    assert_eq!(a.matches("<line ").count(), a_rules);
    assert_eq!(b.matches("<line ").count(), b_rules);
    let start = a.find(r#"<g clip-path="#).expect("a clipped group");
    let group = &a[start..start + a[start..].find("</g>").unwrap()];
    assert_eq!(group.matches("<line ").count(), grid, "{group}");
}
