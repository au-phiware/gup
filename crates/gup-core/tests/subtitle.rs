// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-552: the chart subtitle: its place in the title block, the plot
//! rect it pushes down, truncation, and the light and dark goldens.

mod common;

use common::vr::{harness, layout_metadata, rgba8};
use gup_core::prelude::*;
use gup_core::scene::TextRole;
use gup_core::{Chart as _, Layout, PlacedText};
use gup_visual_regression::RgbaImage;

const W: u32 = 640;
const H: u32 = 400;
const TITLE: &str = "Rain fell most in June";
const SUBTITLE: &str = "Monthly rainfall, mm, 2025";

const RAIN: [f64; 12] = [
    42.0, 38.0, 51.0, 63.0, 80.0, 112.0, 97.0, 74.0, 66.0, 58.0, 49.0, 44.0,
];

type Chart = gup_core::LineChart<(f64, f64)>;

fn rain() -> Chart {
    gup_core::line(RAIN.iter().enumerate().map(|(m, r)| (m as f64 + 1.0, *r)))
        .x(|d| d.0)
        .y(|d| d.1)
        .width(3.0)
        .x_title("Month")
        .y_title("mm")
}

fn layout(chart: &mut Chart, cx: &Context, w: f32, h: f32) -> Layout {
    chart.resolve(cx, w, h).unwrap().layout.clone()
}

fn run(layout: &Layout, role: TextRole) -> Option<&PlacedText> {
    layout.texts.iter().find(|t| t.run.role == role)
}

/// AC2: the subtitle sits a label gap under the title, on the same centre,
/// and the plot starts below it.
#[test]
fn the_subtitle_goes_under_the_title_and_pushes_the_plot_down() {
    let cx = Context::new_blocking().unwrap();
    let plain = layout(&mut rain().title(TITLE), &cx, W as f32, H as f32);
    let both = layout(
        &mut rain().title(TITLE).subtitle(SUBTITLE),
        &cx,
        W as f32,
        H as f32,
    );
    let title = run(&both, TextRole::Title).unwrap().bounds;
    let sub = run(&both, TextRole::Subtitle).unwrap().bounds;
    let gap = Theme::default().label_gap.0;
    assert!(
        (sub.top() - title.bottom() - gap).abs() <= 1.0,
        "title {title:?} subtitle {sub:?}"
    );
    let centre = |r: gup_core::geom::Rect| r.x + r.width / 2.0;
    assert!((centre(title) - centre(sub)).abs() <= 1.0);
    assert!((centre(sub) - (both.plot.x + both.plot.width / 2.0)).abs() <= 1.0);
    // The title did not move; the plot top moved by about the subtitle's
    // height plus the gap.
    assert_eq!(title, run(&plain, TextRole::Title).unwrap().bounds);
    let shift = both.plot.top() - plain.plot.top();
    assert!(
        (shift - (sub.height + gap)).abs() <= 3.0,
        "shift {shift}, subtitle height {}",
        sub.height
    );
    assert!(sub.bottom() < both.plot.top());
}

/// AC2: with no title, the subtitle takes the title's place.
#[test]
fn a_subtitle_without_a_title_takes_the_titles_position() {
    let cx = Context::new_blocking().unwrap();
    let titled = layout(&mut rain().title(TITLE), &cx, W as f32, H as f32);
    let sub_only = layout(&mut rain().subtitle(SUBTITLE), &cx, W as f32, H as f32);
    assert!(run(&sub_only, TextRole::Title).is_none());
    let sub = run(&sub_only, TextRole::Subtitle).unwrap().bounds;
    let title = run(&titled, TextRole::Title).unwrap().bounds;
    // Same top edge to within the two fonts' ascent difference.
    assert!((sub.top() - title.top()).abs() <= 4.0, "{sub:?} {title:?}");
    assert!(sub_only.plot.top() < titled.plot.top());
    assert!(sub.bottom() < sub_only.plot.top());
}

/// AC2: a subtitle wider than the plot is shortened with an ellipsis.
#[test]
fn a_long_subtitle_is_shortened() {
    let cx = Context::new_blocking().unwrap();
    let long =
        "Monthly rainfall in millimetres at the station, averaged over every year of the record";
    let l = layout(&mut rain().title(TITLE).subtitle(long), &cx, 320.0, 300.0);
    let sub = run(&l, TextRole::Subtitle).unwrap();
    assert!(sub.run.text.ends_with('…'), "{:?}", sub.run.text);
    assert!(sub.run.text.len() < long.len());
    assert!(sub.bounds.width <= l.plot.width + 1.0);
}

/// AC2: without a subtitle nothing changes, and the window title is the
/// title alone.
#[test]
fn no_subtitle_no_change() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = rain().title(TITLE);
    let l = layout(&mut chart, &cx, W as f32, H as f32);
    assert!(run(&l, TextRole::Subtitle).is_none());
    let mut with = rain().title(TITLE).subtitle(SUBTITLE);
    with.resolve(&cx, W as f32, H as f32).unwrap();
    assert_eq!(with.title_text(), Some(TITLE));
}

fn golden(name: &str, theme: Theme) {
    let cx = Context::new_blocking().unwrap();
    let mut chart = rain()
        .title(TITLE)
        .subtitle(SUBTITLE)
        .stroke(theme.palette[0])
        .theme(theme.clone());
    let (image, layout) = chart.render_resolved(&cx, W, H).unwrap();
    let vr = RgbaImage::new(W, H, image.into_raw()).unwrap();
    let meta = layout_metadata(&layout)
        .with_background(rgba8(theme.background))
        .with_expected_color("series", rgba8(theme.palette[0]));
    harness().run(name, Ok((vr, meta))).assert_ok();
}

/// AC3: the light golden.
#[test]
fn subtitle_golden() {
    golden("gup_core/subtitle", Theme::default());
}

/// The same chart in the dark theme.
#[test]
fn subtitle_dark_golden() {
    golden("gup_core/subtitle_dark", Theme::dark());
}
