// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Guides drawn beside the plot (RFC-001 S7): colour legends, stacked in
//! one column at the right. A legend is a title, then a body: a swatch row
//! per key, or a palette bar with ticks.

use crate::channel::Px;
use crate::geom::{Point, Rect};
use crate::scale::{Legend, Ramp};
use crate::scene::{
    Anchor, GradientBar, GradientDirection, HAlign, Item, ItemKind, RectPrim, Rule, TextRole,
    TextRun, TextStyle, VAlign, Weight, Z_TITLE,
};
use crate::theme::Theme;
use gup_text::TextSystem;

/// Between the plot and the legend column.
pub(crate) const COLUMN_GAP: f32 = 16.0;
const SWATCH: f32 = 12.0;
const SWATCH_ROW: f32 = 20.0;
const SWATCH_GAP: f32 = 6.0;
const TITLE_GAP: f32 = 6.0;
const BAR_WIDTH: f32 = 14.0;
const BAR_HEIGHT: f32 = 200.0;
const BAR_MIN: f32 = 60.0;
const TICK_LEN: f32 = 4.0;
const TICK_GAP: f32 = 4.0;

/// A legend and its title, as collected from the layers.
pub(crate) type Titled = (Option<String>, Legend);

/// `legends` in order without any equal to an earlier one; a repeat
/// lends its title to the first if that has none.
pub(crate) fn dedup(legends: impl IntoIterator<Item = Titled>) -> Vec<Titled> {
    let mut out: Vec<Titled> = Vec::new();
    for (title, legend) in legends {
        match out.iter_mut().find(|(_, l)| *l == legend) {
            Some((t, _)) => {
                if t.is_none() {
                    *t = title;
                }
            }
            None => out.push((title, legend)),
        }
    }
    out
}

/// What was drawn: scene items, text runs, and the regions the harness
/// treats as guides.
pub(crate) struct Placed {
    pub items: Vec<Item>,
    pub texts: Vec<TextRun>,
    pub guides: Vec<Rect>,
    /// The column's bounding box.
    pub bounds: Rect,
}

fn text_height(text: &TextSystem, size: f32) -> f32 {
    let m = text.measure("0", size);
    m.cap_height + m.descent
}

fn widest<'a>(text: &TextSystem, size: f32, labels: impl Iterator<Item = &'a str>) -> f32 {
    labels
        .map(|l| text.measure(l, size).width)
        .fold(0.0, f32::max)
}

fn body_width(text: &TextSystem, size: f32, legend: &Legend) -> f32 {
    match legend {
        Legend::Swatches(s) => {
            SWATCH + SWATCH_GAP + widest(text, size, s.iter().map(|s| &*s.label))
        }
        Legend::Ramp(r) => {
            BAR_WIDTH
                + TICK_LEN
                + TICK_GAP
                + widest(text, size, r.ticks().iter().map(|t| t.label.as_str()))
        }
    }
}

/// The width of the legend column: the widest legend, or title.
pub(crate) fn column_width(text: &TextSystem, th: &Theme, legends: &[Titled]) -> f32 {
    let size = th.tick_size.0;
    legends
        .iter()
        .map(|(title, legend)| {
            let title_w = title
                .as_deref()
                .map_or(0.0, |t| text.measure(t, size).width);
            body_width(text, size, legend).max(title_w)
        })
        .fold(0.0, f32::max)
}

/// How many swatch rows of `keys` to show in `space` pixels, and how many
/// keys the final "+N more" row stands for (0: none).
pub(crate) fn swatch_rows(keys: usize, space: f32) -> (usize, usize) {
    let fit = if space < SWATCH {
        0
    } else {
        ((space - SWATCH) / SWATCH_ROW) as usize + 1
    };
    if keys <= fit {
        (keys, 0)
    } else {
        let shown = fit.saturating_sub(1);
        (shown, keys - shown)
    }
}

/// Stack `legends` from `(x, plot.top())` down, within `plot`'s vertical
/// extent.
pub(crate) fn place(
    text: &TextSystem,
    th: &Theme,
    legends: &[Titled],
    x: f32,
    plot: Rect,
) -> Placed {
    let size = th.tick_size.0;
    let style = TextStyle {
        size: Px(size),
        color: th.label,
        weight: Weight::Regular,
    };
    let run = |s: &str, at: Point, anchor: Anchor| TextRun {
        text: s.into(),
        at,
        anchor,
        style,
        role: TextRole::Legend,
    };
    let row_h = text_height(text, size);
    let mut p = Placed {
        items: Vec::new(),
        texts: Vec::new(),
        guides: Vec::new(),
        bounds: Rect::new(x, plot.top(), 0.0, 0.0),
    };
    let mut y = plot.top();
    let mut rects = Vec::new();
    let mut rules = Vec::new();
    for (title, legend) in legends {
        if let Some(t) = title {
            p.texts.push(run(
                t,
                Point::new(x, y),
                Anchor::new(HAlign::Start, VAlign::Top),
            ));
            y += row_h + TITLE_GAP;
        }
        y = y.round(); // whole pixels: crisp swatch and bar edges
        let space = plot.bottom() - y;
        match legend {
            Legend::Swatches(swatches) => {
                let (shown, more) = swatch_rows(swatches.len(), space);
                for (i, s) in swatches[..shown].iter().enumerate() {
                    let top = y + i as f32 * SWATCH_ROW;
                    let r = Rect::new(x, top, SWATCH, SWATCH);
                    rects.push(RectPrim {
                        rect: r,
                        color: s.color,
                    });
                    p.guides.push(r);
                    p.texts.push(run(
                        &s.label,
                        Point::new(x + SWATCH + SWATCH_GAP, top + SWATCH / 2.0),
                        Anchor::new(HAlign::Start, VAlign::Middle),
                    ));
                }
                let mut rows = shown;
                if more > 0 && (shown > 0 || space >= SWATCH) {
                    p.texts.push(run(
                        &format!("+{more} more"),
                        Point::new(x, y + rows as f32 * SWATCH_ROW + SWATCH / 2.0),
                        Anchor::new(HAlign::Start, VAlign::Middle),
                    ));
                    rows += 1;
                }
                if rows > 0 {
                    y += (rows - 1) as f32 * SWATCH_ROW + SWATCH;
                }
            }
            Legend::Ramp(ramp) => {
                let (top, h) = (y, space.min(BAR_HEIGHT).floor());
                if h >= BAR_MIN {
                    let bar = Rect::new(x, top, BAR_WIDTH, h);
                    ramp_items(&mut p, &mut rules, th, ramp, bar, &run);
                    y = top + h;
                }
            }
        }
        y += th.pad.0;
    }
    if !rects.is_empty() {
        p.items.push(Item {
            z: Z_TITLE,
            clip: None,
            kind: ItemKind::Rects(rects),
        });
    }
    if !rules.is_empty() {
        p.guides.extend(rules.iter().map(Rule::bounds));
        p.items.push(Item {
            z: Z_TITLE,
            clip: None,
            kind: ItemKind::Rules(rules),
        });
    }
    p.items.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Text(p.texts.clone()),
    });
    let width = legends
        .iter()
        .map(|(t, l)| {
            let title_w = t.as_deref().map_or(0.0, |t| text.measure(t, size).width);
            body_width(text, size, l).max(title_w)
        })
        .fold(0.0, f32::max);
    let bottom = (y - th.pad.0).max(plot.top());
    p.bounds = Rect::new(x, plot.top(), width, bottom - plot.top());
    p
}

/// The bar, a tick at each of the ramp's ticks and its label.
fn ramp_items(
    p: &mut Placed,
    rules: &mut Vec<Rule>,
    th: &Theme,
    ramp: &Ramp,
    bar: Rect,
    run: &dyn Fn(&str, Point, Anchor) -> TextRun,
) {
    p.items.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Gradient(GradientBar::new(ramp, bar, GradientDirection::Vertical)),
    });
    p.guides.push(bar);
    for tick in ramp.ticks() {
        // On a pixel centre, and inside the bar even at its ends.
        let y = ((bar.bottom() - tick.t as f32 * bar.height).floor() + 0.5)
            .clamp(bar.top() + 0.5, bar.bottom() - 0.5);
        rules.push(Rule {
            p0: Point::new(bar.right(), y),
            p1: Point::new(bar.right() + TICK_LEN, y),
            width: Px(1.0),
            color: th.axis,
        });
        p.texts.push(run(
            &tick.label,
            Point::new(bar.right() + TICK_LEN + TICK_GAP, y),
            Anchor::new(HAlign::Start, VAlign::Middle),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swatch_rows_show_what_fits_and_count_the_rest() {
        // 12 px for the first row, 20 px for each further one.
        assert_eq!(swatch_rows(3, 52.0), (3, 0));
        assert_eq!(swatch_rows(3, 51.9), (1, 2));
        assert_eq!(swatch_rows(60, 112.0), (5, 55));
        assert_eq!(swatch_rows(60, 11.0), (0, 60));
        assert_eq!(swatch_rows(0, 0.0), (0, 0));
    }

    #[test]
    fn dedup_keeps_the_first_and_its_first_title() {
        let a = Legend::Swatches(vec![]);
        let b = Legend::Swatches(vec![crate::scale::Swatch {
            label: "k".into(),
            color: crate::channel::Color::BLACK,
        }]);
        let out = dedup([
            (None, a.clone()),
            (Some("B".into()), b.clone()),
            (Some("A".into()), a.clone()),
            (Some("other".into()), b.clone()),
        ]);
        assert_eq!(out, [(Some("A".into()), a), (Some("B".into()), b)]);
    }
}
