// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chunk culling (RFC-001 S9f): which draw entries can put a pixel inside
//! the plot rect. Draws, CPU picks and GPU picks all use one result, so
//! they keep agreeing.

use crate::geom::Rect;
use crate::order::DrawEntry;
use crate::render::Span;

/// An interval of pixels.
pub(crate) type Interval = (f64, f64);

/// The pixel box of the positions of a chunk's rows.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct PixelBox {
    pub x: Interval,
    pub y: Interval,
}

impl PixelBox {
    /// The box that covers every pixel.
    #[cfg(test)]
    pub const ALL: Self = Self {
        x: (f64::NEG_INFINITY, f64::INFINITY),
        y: (f64::NEG_INFINITY, f64::INFINITY),
    };

    /// The box of no pixels: the identity of [`grow`](Self::grow).
    pub const EMPTY: Self = Self {
        x: (f64::INFINITY, f64::NEG_INFINITY),
        y: (f64::INFINITY, f64::NEG_INFINITY),
    };

    /// Extend the box on `axis` (0: x, 1: y) to cover `v`.
    pub fn grow(&mut self, axis: usize, v: Interval) {
        let i = if axis == 0 { &mut self.x } else { &mut self.y };
        *i = (i.0.min(v.0), i.1.max(v.1));
    }

    fn union(self, other: Self) -> Self {
        Self {
            x: (self.x.0.min(other.x.0), self.x.1.max(other.x.1)),
            y: (self.y.0.min(other.y.0), self.y.1.max(other.y.1)),
        }
    }

    /// Whether the box, grown by `margin` on every side, meets `plot`.
    fn visible(self, margin: f64, plot: Rect) -> bool {
        let (l, r) = (f64::from(plot.left()), f64::from(plot.right()));
        let (t, b) = (f64::from(plot.top()), f64::from(plot.bottom()));
        self.x.1 + margin >= l
            && self.x.0 - margin <= r
            && self.y.1 + margin >= t
            && self.y.0 - margin <= b
    }
}

/// Which of a layer's draw `entries` are visible: `boxes` holds each
/// chunk's position box (`None` when it has no finite position, so it
/// draws nothing); an entry's box is the union of the boxes of the chunk
/// of its own row and of its next row, which for a polyline's bridge are
/// the two chunks it joins. `margin` is [`Layer::cull_margin`] plus the
/// pick tolerance.
///
/// [`Layer::cull_margin`]: crate::selection::Layer::cull_margin
pub(crate) fn visible_entries(
    boxes: &[Option<PixelBox>],
    entries: &[DrawEntry],
    margin: f32,
    plot: Rect,
) -> Vec<bool> {
    let margin = f64::from(margin);
    entries
        .iter()
        .map(|e| {
            let (a, b) = (boxes[e.this], boxes[e.next]);
            match (a, b) {
                (None, None) => false,
                (Some(a), None) => a.visible(margin, plot),
                (None, Some(b)) => b.visible(margin, plot),
                (Some(a), Some(b)) => a.union(b).visible(margin, plot),
            }
        })
        .collect()
}

/// The stored-row ranges the CPU pick index must build items from, for a
/// layer whose chunks are `chunks` (`(first row, rows)`): a quad layer's
/// visible chunks; for a polyline, the vertices of each visible chunk's own
/// segments and of each visible bridge. A polyline range is read in pairs
/// of consecutive rows. `visible` is [`visible_entries`]'s result.
pub(crate) fn pick_ranges(
    polyline: bool,
    chunks: &[(usize, usize)],
    visible: &[bool],
) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    for (k, &(base, rows)) in chunks.iter().enumerate() {
        if visible[k] && rows > 0 {
            out.push(base..base + rows);
        }
        if polyline && k + 1 < chunks.len() && visible[chunks.len() + k] && rows > 0 {
            let at = base + rows - 1;
            out.push(at..at + 2);
        }
    }
    out
}

/// The `spans` whose entry is visible, in order.
pub(crate) fn visible_spans(spans: &[Span], visible: &[bool]) -> Vec<Span> {
    spans
        .iter()
        .filter(|s| visible[s.chunk as usize])
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLOT: Rect = Rect {
        x: 50.0,
        y: 10.0,
        width: 400.0,
        height: 300.0,
    };

    fn bx(x: Interval, y: Interval) -> Option<PixelBox> {
        Some(PixelBox { x, y })
    }

    fn quad(chunks: usize) -> Vec<DrawEntry> {
        (0..chunks)
            .map(|k| DrawEntry {
                this: k,
                first: 0,
                next: k,
                next_first: 1,
                instances: 10,
            })
            .collect()
    }

    #[test]
    fn a_chunk_wholly_left_of_the_plot_is_culled() {
        let boxes = [bx((0.0, 30.0), (20.0, 200.0))];
        assert_eq!(visible_entries(&boxes, &quad(1), 4.0, PLOT), [false]);
    }

    #[test]
    fn a_chunk_ending_within_the_margin_is_kept() {
        let boxes = [
            bx((0.0, 47.0), (20.0, 200.0)),
            bx((0.0, 45.9), (20.0, 200.0)),
        ];
        assert_eq!(visible_entries(&boxes, &quad(2), 4.0, PLOT), [true, false]);
    }

    #[test]
    fn a_wide_margin_keeps_marks_whose_half_width_reaches_inside() {
        // Centres 20 px right of the plot; bars 60 px wide reach inside.
        let boxes = [bx((470.0, 500.0), (20.0, 200.0))];
        assert_eq!(visible_entries(&boxes, &quad(1), 4.0, PLOT), [false]);
        assert_eq!(visible_entries(&boxes, &quad(1), 64.0, PLOT), [true]);
    }

    #[test]
    fn a_segment_with_ends_on_opposite_sides_is_kept() {
        // x spans the union of x and x_end: one end left, one right.
        let boxes = [bx((0.0, 600.0), (20.0, 30.0))];
        assert_eq!(visible_entries(&boxes, &quad(1), 4.0, PLOT), [true]);
    }

    #[test]
    fn a_bridge_between_culled_chunks_on_opposite_sides_is_kept() {
        let boxes = [
            bx((0.0, 30.0), (20.0, 30.0)),
            bx((500.0, 600.0), (40.0, 50.0)),
            bx((700.0, 800.0), (40.0, 50.0)),
        ];
        let entry = |this, next| DrawEntry {
            this,
            first: 0,
            next,
            next_first: 0,
            instances: 1,
        };
        let entries = [
            entry(0, 0),
            entry(1, 1),
            entry(2, 2),
            entry(0, 1),
            entry(1, 2),
        ];
        assert_eq!(
            visible_entries(&boxes, &entries, 4.0, PLOT),
            [false, false, false, true, false]
        );
    }

    #[test]
    fn a_chunk_without_a_finite_position_is_culled() {
        let boxes = [None];
        assert_eq!(visible_entries(&boxes, &quad(1), 4.0, PLOT), [false]);
    }

    #[test]
    fn constant_positions_keep_or_cull_by_the_one_point() {
        let inside = bx((100.0, 100.0), (50.0, 50.0));
        let outside = bx((10.0, 10.0), (50.0, 50.0));
        assert_eq!(visible_entries(&[inside], &quad(1), 4.0, PLOT), [true]);
        assert_eq!(visible_entries(&[outside], &quad(1), 4.0, PLOT), [false]);
    }

    #[test]
    fn unknown_images_cover_every_pixel() {
        assert_eq!(
            visible_entries(&[Some(PixelBox::ALL)], &quad(1), 0.0, PLOT),
            [true]
        );
    }

    #[test]
    fn pick_ranges_follow_the_visible_entries() {
        let chunks = [(0, 4), (4, 4), (8, 2)];
        assert_eq!(
            pick_ranges(false, &chunks, &[true, false, true]),
            [0..4, 8..10]
        );
        // Three own entries then two bridges.
        let seen = [false, true, false, true, false];
        assert_eq!(pick_ranges(true, &chunks, &seen), [3..5, 4..8]);
    }

    #[test]
    fn spans_keep_their_order() {
        let span = |chunk| Span { chunk, rows: 0..3 };
        let spans = [span(0), span(2), span(1), span(3)];
        let kept = visible_spans(&spans, &[true, false, true, true]);
        assert_eq!(kept, [span(0), span(2), span(3)]);
    }
}
