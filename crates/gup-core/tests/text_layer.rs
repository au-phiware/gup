// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-542: text layers. Labels at data or band positions, offsets read at
//! resolve, the rows that draw nothing, `within` spans, zooming, the plot
//! clip, SVG, the cache, and the `gup_core/text_annotations` golden.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::scene::TextRole;
use gup_core::{ImageTarget, Layout, PlacedText, SvgTarget, VectorTarget};
use gup_visual_regression::RgbaImage;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const W: f32 = 480.0;
const H: f32 = 320.0;

/// The annotation labels of a layout.
fn labels(layout: &Layout) -> Vec<&PlacedText> {
    layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::Annotation)
        .collect()
}

fn label<'a>(layout: &'a Layout, text: &str) -> Option<&'a PlacedText> {
    labels(layout).into_iter().find(|t| &*t.run.text == text)
}

/// A plot over x 0..10 and y 0..10 with a disc at every row.
fn plot_with_discs(rows: &[(f64, f64)]) -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut plot = Plot::new();
    plot.legends(false);
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    plot.circles(rows.to_vec())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1);
    (plot, x, y)
}

/// AC2: a label at a numeric `(x, y)` is anchored at the mapped position
/// plus `(dx, dy)`.
#[test]
fn a_label_is_anchored_at_its_mapped_position_plus_the_offset() {
    let cx = Context::new_blocking().unwrap();
    let rows = [(2.0, 3.0), (5.0, 5.0), (7.5, 8.0)];
    let (mut plot, x, y) = plot_with_discs(&rows);
    plot.text(rows)
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|r: &(f64, f64)| format!("({}, {})", r.0, r.1))
        .anchor(Anchor::BOTTOM_LEFT)
        .dx(4.0)
        .dy(-6.0);
    let layout = plot.resolve(&cx, W, H).unwrap().layout.clone();
    assert_eq!(labels(&layout).len(), 3);
    for (vx, vy) in rows {
        let t = label(&layout, &format!("({vx}, {vy})")).expect("each row is labelled");
        let want = (x.read().eval(vx) + 4.0, y.read().eval(vy) - 6.0);
        assert!(
            (f64::from(t.run.at.x) - want.0).abs() <= 0.5
                && (f64::from(t.run.at.y) - want.1).abs() <= 0.5,
            "{:?} at {:?}, want {want:?}",
            t.run.text,
            t.run.at
        );
        assert_eq!(t.run.anchor, Anchor::BOTTOM_LEFT);
        // Above and to the right of its point.
        assert!(f64::from(t.bounds.bottom()) <= want.1 + 0.5, "{t:?}");
        assert!(f64::from(t.bounds.left()) >= want.0 - 1.5, "{t:?}");
    }
}

/// AC2: `x_key` places a label at the centre of its key's band.
#[test]
fn x_key_places_a_label_at_its_bands_centre() {
    #[derive(Clone)]
    struct Day {
        name: &'static str,
        high: f64,
    }
    let days = vec![
        Day {
            name: "Mon",
            high: 21.0,
        },
        Day {
            name: "Tue",
            high: 25.0,
        },
        Day {
            name: "Wed",
            high: 18.0,
        },
    ];
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new().domain(0.0, 30.0)));
    plot.add(Selection::<Day, Circle>::new(days.clone()))
        .attr(Circle::X, x.encode_key(|d: &Day| d.name))
        .attr(Circle::Y, y.encode(|d: &Day| d.high));
    plot.text(days)
        .x_key(|d: &Day| d.name)
        .y(|d: &Day| d.high)
        .text(|d: &Day| d.high)
        .anchor(Anchor::BOTTOM);
    let layout = plot.resolve(&cx, W, H).unwrap().layout.clone();
    let band = x.read().clone();
    for (code, (name, high)) in [("Mon", 21.0), ("Tue", 25.0), ("Wed", 18.0)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(&*band.keys()[code], name);
        let t = label(&layout, &high.to_string()).expect("each day is labelled");
        let centre = band.eval(code as f64);
        assert!(
            (f64::from(t.run.at.x) - centre).abs() <= 0.5,
            "{name}: label at {}, band centre {centre}",
            t.run.at.x
        );
    }
}

/// AC2: `dx_with` follows a value read at resolve: here one data unit in
/// pixels, which a resize changes.
#[test]
fn dx_with_follows_a_value_read_at_resolve() {
    let cx = Context::new_blocking().unwrap();
    let rows = [(5.0, 5.0)];
    let (mut plot, x, _) = plot_with_discs(&rows);
    let unit = {
        let x = x.clone();
        Derived::new(move || {
            let s = x.read();
            Px((s.eval(1.0) - s.eval(0.0)) as f32)
        })
    };
    plot.text(rows)
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|_: &(f64, f64)| "five")
        .dx_with(unit);
    for width in [480.0, 720.0] {
        let layout = plot.resolve(&cx, width, H).unwrap().layout.clone();
        let t = label(&layout, "five").unwrap();
        let s = x.read();
        let want = s.eval(5.0) + (s.eval(1.0) - s.eval(0.0));
        assert!(
            (f64::from(t.run.at.x) - want).abs() <= 0.5,
            "width {width}: at {}, want {want}",
            t.run.at.x
        );
    }
}

/// AC2: a row with a non-finite position, or a key not in the axis's
/// domain, draws nothing; the other rows draw.
#[test]
fn non_finite_positions_and_unknown_keys_draw_nothing() {
    let cx = Context::new_blocking().unwrap();
    let rows = [(2.0, 2.0), (f64::NAN, 5.0), (8.0, f64::INFINITY)];
    let (mut plot, ..) = plot_with_discs(&rows[..1]);
    plot.text(rows)
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|r: &(f64, f64)| format!("{}", r.0));
    let layout = plot.resolve(&cx, W, H).unwrap().layout.clone();
    let texts: Vec<&str> = labels(&layout).iter().map(|t| &*t.run.text).collect();
    assert_eq!(texts, ["2"]);

    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new().domain(0.0, 1.0)));
    plot.add(Selection::<&str, Circle>::new(vec!["a", "b"]))
        .attr(Circle::X, x.encode_key(|k: &&str| *k))
        .attr(Circle::Y, y.encode(|_: &&str| 0.5));
    plot.text(["a", "zzz", "b"])
        .x_key(|k: &&str| *k)
        .y(|_: &&str| 0.5)
        .text(|k: &&str| *k);
    let layout = plot.resolve(&cx, W, H).unwrap().layout.clone();
    let texts: Vec<&str> = labels(&layout).iter().map(|t| &*t.run.text).collect();
    assert_eq!(texts, ["a", "b"], "a label never adds a key");
    assert_eq!(x.read().keys().len(), 2);
}

/// A value on a band axis, or a key on a numeric one, is a configuration
/// error that names the setter to use.
#[test]
fn a_position_that_does_not_suit_its_axis_is_an_error() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
    plot.add(Selection::<&str, Circle>::new(vec!["a", "b"]))
        .attr(Circle::X, x.encode_key(|k: &&str| *k))
        .attr(Circle::Y, y.encode(|_: &&str| 0.5));
    plot.text(["a"])
        .x(|_: &&str| 1.0)
        .y(|_: &&str| 0.5)
        .text(|k: &&str| *k);
    let err = plot.resolve(&cx, W, H).expect_err("a value on a band axis");
    assert!(err.to_string().contains("x_key"), "{err}");

    let (mut plot, ..) = plot_with_discs(&[(1.0, 1.0)]);
    plot.text(["a"])
        .x(|_: &&str| 1.0)
        .y_key(|k: &&str| *k)
        .text(|k: &&str| *k);
    let err = plot
        .resolve(&cx, W, H)
        .expect_err("a key on a numeric axis");
    assert!(err.to_string().contains("`y(..)`"), "{err}");
}

/// AC2: a `within_y` span shorter than the label's ink height draws
/// nothing; a taller one draws it.
#[test]
fn within_y_draws_only_labels_that_fit() {
    let cx = Context::new_blocking().unwrap();
    // Rows: (x, bar height in data units). The plot is 320 px tall over
    // 0..10, so a unit is under 32 px and 0.1 of one is about 2 px.
    let rows = [(3.0, 0.1), (7.0, 5.0)];
    let (mut plot, ..) = plot_with_discs(&rows);
    plot.text(rows)
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1 / 2.0)
        .text(|r: &(f64, f64)| format!("h{}", r.1))
        .within_y(|r: &(f64, f64)| (0.0, r.1));
    let layout = plot.resolve(&cx, W, H).unwrap().layout.clone();
    let texts: Vec<&str> = labels(&layout).iter().map(|t| &*t.run.text).collect();
    assert_eq!(texts, ["h5"]);
}

/// AC2: after a zoom, labels move with the data (their positions mapped
/// again, their text not evaluated again), and the zoom writes 0 column
/// bytes.
#[test]
fn zooming_moves_labels_and_writes_no_column_bytes() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = (0..20)
        .map(|i| (0.5 + 0.45 * f64::from(i), f64::from(i % 7) + 1.5))
        .collect();
    let (mut plot, x, y) = plot_with_discs(&rows);
    let evaluated = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&evaluated);
    plot.text(rows.clone())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(move |r: &(f64, f64)| {
            count.fetch_add(1, Ordering::Relaxed);
            format!("{:.2}", r.0)
        })
        .anchor(Anchor::TOP)
        .dy(5.0);
    let before = plot.resolve(&cx, W, H).unwrap().layout.clone();
    assert_eq!(evaluated.load(Ordering::Relaxed), rows.len());
    let uploads = cx.upload_stats();
    let at = gup_core::geom::Point::new(before.plot.x + 100.0, before.plot.y + 100.0);
    plot.zoom(at, 0.5).unwrap();
    let after = plot.resolve(&cx, W, H).unwrap().layout.clone();
    let written = cx.upload_stats() - uploads;
    assert_eq!(written.columns.bytes, 0, "{written:?}");
    assert_eq!(written.columns.writes, 0, "{written:?}");
    assert_eq!(evaluated.load(Ordering::Relaxed), rows.len(), "cached text");

    let (xs, ys) = (x.read().clone(), y.read().clone());
    let mut moved = 0;
    for t in labels(&after) {
        let row = rows
            .iter()
            .find(|r| format!("{:.2}", r.0) == *t.run.text)
            .unwrap();
        assert!((f64::from(t.run.at.x) - xs.eval(row.0)).abs() <= 0.5);
        assert!((f64::from(t.run.at.y) - (ys.eval(row.1) + 5.0)).abs() <= 0.5);
        let old = label(&before, &t.run.text).unwrap();
        if (old.run.at.x - t.run.at.x).abs() > 1.0 {
            moved += 1;
        }
    }
    assert!(!labels(&after).is_empty());
    assert!(moved > 0, "the zoom moved the labels");
    assert!(labels(&after).len() < labels(&before).len(), "zoomed in");
}

/// AC4: a cached resolve does no text-layer work: the offset (read on
/// every full resolve) is not read again.
#[test]
fn a_cached_resolve_does_no_text_layer_work() {
    let cx = Context::new_blocking().unwrap();
    let rows = [(5.0, 5.0)];
    let (mut plot, ..) = plot_with_discs(&rows);
    let reads = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&reads);
    plot.text(rows)
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|_: &(f64, f64)| "five")
        .dy_with(Derived::new(move || {
            count.fetch_add(1, Ordering::Relaxed);
            Px(-3.0)
        }));
    plot.resolve(&cx, W, H).unwrap();
    let (n, generation) = (reads.load(Ordering::Relaxed), plot.resolve_generation());
    assert!(n > 0);
    for _ in 0..3 {
        plot.resolve(&cx, W, H).unwrap();
    }
    assert_eq!(reads.load(Ordering::Relaxed), n);
    assert_eq!(plot.resolve_generation(), generation);
    // A full resolve reads it again.
    plot.resolve(&cx, W + 1.0, H).unwrap();
    assert!(reads.load(Ordering::Relaxed) > n);
}

/// Whether `c` is the red the edge labels are drawn in (or a blend of it
/// over white).
fn reddish(c: [u8; 4]) -> bool {
    let [r, g, b, _] = c.map(i32::from);
    r > 160 && r - g > 60 && r - b > 60
}

/// AC2: labels are clipped to the plot rect. Labels centred on the right
/// and top edges of the plot rect draw ink inside it and none outside.
#[test]
fn labels_on_the_plot_edge_are_clipped_to_the_plot_rect() {
    let cx = Context::new_blocking().unwrap();
    let make = |right: f64, top: f64| {
        let (mut plot, x, y) = plot_with_discs(&[(5.0, 5.0)]);
        plot.text([(right, 5.0, "Right edge"), (5.0, top, "Top edge")])
            .x(|r: &(f64, f64, &str)| r.0)
            .y(|r: &(f64, f64, &str)| r.1)
            .text(|r: &(f64, f64, &str)| r.2)
            .size(16.0)
            .color(Color::hex(0xff0000));
        (plot, x, y)
    };
    // Labels do not change the layout, so the probe's rect is the plot's.
    let (mut probe, x, y) = make(5.0, 5.0);
    let plot_rect = probe.resolve(&cx, W, H).unwrap().layout.plot;
    let (right, top) = (
        x.read().invert(f64::from(plot_rect.right())),
        y.read().invert(f64::from(plot_rect.top())),
    );
    let (mut plot, ..) = make(right, top);
    let resolved = plot.resolve(&cx, W, H).unwrap().clone();
    assert_eq!(resolved.layout.plot, plot_rect);
    for name in ["Right edge", "Top edge"] {
        let t = label(&resolved.layout, name).expect("half of each label is inside");
        assert!(resolved.layout.plot.contains_rect(&t.bounds), "{t:?}");
    }
    let image = ImageTarget::new(&cx, W as u32, H as u32)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let p = resolved.layout.plot;
    let (mut inside, mut outside) = (0, Vec::new());
    for (px, py, c) in image.enumerate_pixels() {
        if !reddish(c.0) {
            continue;
        }
        let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
        if fx >= p.left() && fx <= p.right() && fy >= p.top() && fy <= p.bottom() {
            inside += 1;
        } else {
            outside.push((px, py));
        }
    }
    assert!(inside > 40, "{inside} red pixels inside the plot rect");
    assert!(outside.is_empty(), "red ink outside {p:?}: {outside:?}");
}

/// Six Australian cities and one off the chart.
#[derive(Clone)]
struct City {
    name: &'static str,
    lon: f64,
    lat: f64,
}

fn cities() -> Vec<City> {
    let city = |name, lon, lat| City { name, lon, lat };
    vec![
        city("Perth", 115.86, -31.95),
        city("Darwin", 130.84, -12.46),
        city("Alice Springs", 133.88, -23.70),
        city("Adelaide", 138.60, -34.93),
        city("Hobart", 147.33, -42.88),
        // On the right end of the x domain: its label is clipped.
        city("Sydney", 151.21, -33.87),
        // Off the chart: neither its disc nor its label is drawn.
        city("Auckland", 174.76, -36.85),
    ]
}

const THRESHOLD: f64 = -18.0;
const VISIBLE: [&str; 7] = [
    "Perth",
    "Darwin",
    "Alice Springs",
    "Adelaide",
    "Hobart",
    "Sydney",
    "threshold",
];

/// The annotated scatter of `gup_core/text_annotations`.
fn annotated() -> Plot {
    let mut plot = Plot::new();
    plot.legends(false);
    plot.title("Cities, labelled");
    plot.x(Linear::new().domain(112.0, 151.21));
    plot.y(Linear::new().domain(-46.0, -8.0));
    plot.line([(112.0, THRESHOLD), (151.21, THRESHOLD)])
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .color(Color::hex(0xd55e00))
        .width(Px(1.0));
    plot.circles(cities())
        .x(|c: &City| c.lon)
        .y(|c: &City| c.lat)
        .radius(Px(4.0));
    plot.text(cities())
        .x(|c: &City| c.lon)
        .y(|c: &City| c.lat)
        .text(|c: &City| c.name)
        .anchor(Anchor::BOTTOM)
        .dy(-6.0);
    plot.text(["threshold"])
        .x(|_: &&str| 151.21)
        .y(|_: &&str| THRESHOLD)
        .text(|t: &&str| *t)
        .anchor(Anchor::BOTTOM_RIGHT)
        .dx(-4.0)
        .dy(-3.0)
        .color(Color::hex(0xd55e00));
    plot
}

/// AC3: the `gup_core/text_annotations` golden: labels above their discs,
/// Sydney's clipped at the right edge, a threshold annotated at its right
/// end, none over a tick label.
#[test]
fn text_annotations_golden() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (480u32, 320u32);
    let mut plot = annotated();
    let resolved = plot.resolve(&cx, w as f32, h as f32).unwrap().clone();
    let layout = &resolved.layout;
    let names: Vec<&str> = labels(layout).iter().map(|t| &*t.run.text).collect();
    assert_eq!(names, VISIBLE);
    // Every label is inside the plot rect and clear of every tick label.
    for t in labels(layout) {
        assert!(layout.plot.contains_rect(&t.bounds), "{t:?}");
        for tick in layout
            .texts
            .iter()
            .filter(|t| t.run.role == TextRole::TickLabel)
        {
            let (a, b) = (t.bounds, tick.bounds);
            let overlap = a.left() < b.right()
                && b.left() < a.right()
                && a.top() < b.bottom()
                && b.top() < a.bottom();
            assert!(
                !overlap,
                "{:?} overlaps tick {:?}",
                t.run.text, tick.run.text
            );
        }
    }
    let sydney = label(layout, "Sydney").unwrap();
    assert!(
        (sydney.bounds.right() - layout.plot.right()).abs() < 0.01,
        "Sydney is cut at the right edge: {sydney:?}"
    );
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    harness()
        .run(
            "gup_core/text_annotations",
            Ok((vr, layout_metadata(&resolved.layout))),
        )
        .assert_ok();
}

/// AC3: `SvgTarget` writes every visible label as a `<text>` inside the
/// plot's clip group, once, and no hidden one.
#[test]
fn svg_writes_every_visible_label_inside_the_plot_clip() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = annotated();
    let scene = plot.resolve(&cx, W, H).unwrap().scene.clone();
    let mut svg = SvgTarget::new();
    svg.render(&scene).unwrap();
    let svg = svg.into_string();
    let mut clipped = false;
    let mut found = Vec::new();
    for line in svg.lines() {
        if line.starts_with("<g clip-path=") {
            clipped = true;
        } else if line == "</g>" {
            clipped = false;
        } else if line.starts_with("<text") {
            for c in cities().iter().map(|c| c.name).chain(["threshold"]) {
                if line.ends_with(&format!(">{c}</text>")) {
                    assert!(clipped, "{c} is outside the clip group: {line}");
                    found.push(c);
                }
            }
        }
    }
    found.sort_unstable();
    let mut want = VISIBLE.to_vec();
    want.sort_unstable();
    assert_eq!(found, want, "each visible label once, and no Auckland");
}

/// AC4: the cost of 1,000 labels on a 1,000-point scatter, as the median
/// full resolve (a resize each time) with and without the text layer, and
/// a render of every label (the glyph atlas holds them). The numbers mean
/// something in release: `cargo test -p gup-core --release --test
/// text_layer resolve_cost -- --nocapture`.
#[test]
fn resolve_cost_of_a_thousand_labels() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = (0..1000)
        .map(|i| {
            let t = f64::from(i);
            (
                (t * 0.618_034).fract() * 100.0,
                (t * 0.414_214).fract() * 100.0,
            )
        })
        .collect();
    let make = |labelled: bool| {
        let mut plot = Plot::new();
        plot.circles(rows.clone())
            .x(|r: &(f64, f64)| r.0)
            .y(|r: &(f64, f64)| r.1);
        if labelled {
            plot.text(rows.clone())
                .x(|r: &(f64, f64)| r.0)
                .y(|r: &(f64, f64)| r.1)
                .text(|r: &(f64, f64)| format!("{:.1}", r.0))
                .anchor(Anchor::BOTTOM)
                .dy(-4.0);
        }
        plot
    };
    let median = |plot: &mut Plot| {
        plot.resolve(&cx, 800.0, 600.0).unwrap();
        let mut times: Vec<f64> = (0..21)
            .map(|i| {
                let start = std::time::Instant::now();
                plot.resolve(&cx, 800.0 + (i % 2) as f32, 600.0).unwrap();
                start.elapsed().as_secs_f64() * 1e3
            })
            .collect();
        times.sort_by(f64::total_cmp);
        times[times.len() / 2]
    };
    let (without, with) = (median(&mut make(false)), median(&mut make(true)));
    println!(
        "metric text_layer.resolve_ms without={without:.3} with={with:.3} delta={:.3}",
        with - without
    );
    let mut plot = make(true);
    let resolved = plot.resolve(&cx, 800.0, 600.0).unwrap().clone();
    let drawn = labels(&resolved.layout).len();
    assert!(drawn > 900, "{drawn} of 1000 labels drawn");
    let image = ImageTarget::new(&cx, 800, 600)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    assert_eq!(image.dimensions(), (800, 600));
}
