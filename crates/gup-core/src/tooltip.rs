// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tooltip overlay (RFC-001 §9): a box with text beside a hovered row,
//! built as plain scene items so it composes onto a cached scene.

use crate::geom::{Point, Rect};
use crate::scene::{
    Anchor, HAlign, Item, ItemKind, RectPrim, TextRole, TextRun, TextStyle, VAlign, Weight,
    Z_TOOLTIP,
};
use crate::theme::Theme;
use gup_text::TextSystem;

/// Distance between the anchor and the box.
const GAP: f32 = 8.0;
/// Space between the box edge and its text.
const PADDING: f32 = 6.0;
/// Keep the box this far inside the canvas.
const MARGIN: f32 = 1.0;

/// The top-left corner of a `w × h` box for anchor `a` on a `width × height`
/// canvas: up and right of the anchor, else flipped left and/or below, then
/// clamped inside the canvas (the low edge wins when the box is too big),
/// and rounded to whole pixels.
pub(crate) fn place(a: Point, w: f32, h: f32, width: f32, height: f32) -> Point {
    let mut x = a.x + GAP;
    if x + w > width - MARGIN {
        x = a.x - GAP - w;
    }
    let mut y = a.y - GAP - h;
    if y < MARGIN {
        y = a.y + GAP;
    }
    x = x.min(width - MARGIN - w).max(MARGIN);
    y = y.min(height - MARGIN - h).max(MARGIN);
    Point::new(x.round(), y.round())
}

/// The tooltip for `text` ('\n' separates lines) at `anchor`: its box and
/// the scene items (two rectangles, then the text runs).
pub(crate) fn overlay(
    system: &TextSystem,
    theme: &Theme,
    text: &str,
    anchor: Point,
    width: f32,
    height: f32,
) -> (Rect, [Item; 2]) {
    let size = theme.tooltip_size.0;
    let pitch = (1.4 * size).round();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut max_width = 0.0f32;
    for line in &lines {
        max_width = max_width.max(system.measure(line, size).width);
    }
    let m = system.measure(lines[0], size);
    let w = (2.0 * PADDING + max_width).ceil();
    let h = (2.0 * PADDING + (lines.len() - 1) as f32 * pitch + m.cap_height + m.descent).ceil();
    let at = place(anchor, w, h, width, height);
    let outer = Rect::new(at.x, at.y, w, h);
    let inner = Rect::new(at.x + 1.0, at.y + 1.0, w - 2.0, h - 2.0);
    let style = TextStyle {
        size: theme.tooltip_size,
        color: theme.tooltip_text,
        weight: Weight::Regular,
    };
    let runs = lines
        .iter()
        .enumerate()
        .map(|(i, line)| TextRun {
            text: (*line).into(),
            at: Point::new(at.x + PADDING, at.y + PADDING + i as f32 * pitch),
            anchor: Anchor::new(HAlign::Start, VAlign::Top),
            style,
            role: TextRole::Tooltip,
        })
        .collect();
    let rects = Item {
        z: Z_TOOLTIP,
        clip: None,
        kind: ItemKind::Rects(vec![
            RectPrim {
                rect: outer,
                color: theme.tooltip_border,
            },
            RectPrim {
                rect: inner,
                color: theme.tooltip_background,
            },
        ]),
    };
    let texts = Item {
        z: Z_TOOLTIP,
        clip: None,
        kind: ItemKind::Text(runs),
    };
    (outer, [rects, texts])
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: fn(f32, f32) -> Point = Point::new;

    #[test]
    fn up_and_right_of_the_anchor_by_default() {
        assert_eq!(
            place(P(100.0, 100.0), 50.0, 30.0, 480.0, 320.0),
            P(108.0, 62.0)
        );
    }

    #[test]
    fn flips_left_and_below_at_the_top_right_corner() {
        let at = place(P(475.0, 5.0), 60.0, 30.0, 480.0, 320.0);
        assert_eq!(at, P(407.0, 13.0));
        assert!(at.x + 60.0 <= 479.0 && at.y + 30.0 <= 319.0);
    }

    #[test]
    fn clamps_a_box_wider_than_the_canvas_to_x_one() {
        assert_eq!(place(P(240.0, 160.0), 600.0, 30.0, 480.0, 320.0).x, 1.0);
    }

    #[test]
    fn clamps_inside_at_the_bottom_and_rounds() {
        // Flipped below the anchor, then pulled up to fit.
        let at = place(P(10.4, 3.0), 40.0, 400.0, 480.0, 320.0);
        assert_eq!(at.y, 1.0);
        let at = place(P(100.5, 100.5), 50.0, 30.0, 480.0, 320.0);
        assert_eq!(at.x.fract(), 0.0);
        assert_eq!(at.y.fract(), 0.0);
    }
}
