// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Size-descending draw order (RFC-001 S6e, GUP-431).
//!
//! A layer whose size channel is a column draws its largest marks first,
//! so no mark hides under a larger one drawn after it. Each batch of rows
//! (the first evaluation, then each append) is sorted on the CPU before it
//! goes into the column store, so a batch's rows are stored largest
//! first and upload as before: one batch is one run of stored rows. The
//! draw order merges the runs: a list of segments, each a range of
//! consecutive stored rows, drawn as one instanced draw per segment (and
//! chunk). A store built at once is one segment per chunk, exactly the
//! draws of a layer in data order; an append of `m` rows adds at most
//! `2m` segments and uploads only its own rows. No ordering data goes to
//! the GPU: a draw's `first_instance` picks its rows.

use std::ops::Range;

use crate::channel::Geometry;
use crate::column::{Chunk, ColumnData, NULL_CODE};
use crate::render::Span;

/// Merged runs above this many segments (beyond one per 1024 rows) are
/// re-sorted: [`DrawOrder::needs_compaction`].
const COMPACT_SEGMENTS: usize = 64;

/// The draw order of a layer whose size channel is a column.
#[derive(Clone, Debug)]
pub(crate) struct DrawOrder {
    /// Which of the layer's columns is the size.
    column: usize,
    /// Whether a larger value draws a smaller mark (a decreasing scale).
    reversed: bool,
    /// Each stored row's sort key: its size value (negated when
    /// `reversed`) as `f32`, which rounds monotonically; nulls are -∞.
    keys: Vec<f32>,
    /// The first stored row of each batch.
    batches: Vec<u64>,
    /// Each stored row's index among its batch's data rows.
    local: Vec<u32>,
    /// The stored rows in draw order, as ranges of consecutive rows.
    segments: Vec<Range<u64>>,
}

impl DrawOrder {
    /// An empty order over size column `column`.
    pub(crate) fn new(column: usize, reversed: bool) -> Self {
        Self {
            column,
            reversed,
            keys: Vec::new(),
            batches: Vec::new(),
            local: Vec::new(),
            segments: Vec::new(),
        }
    }

    /// Sort a batch of rows about to be appended to the store, largest
    /// first (stable, so equal sizes keep data order), permuting every
    /// column alike, and merge it into the draw order after the stored
    /// rows: a new row draws after every stored row at least its size.
    pub(crate) fn push_batch(&mut self, columns: &mut [ColumnData]) {
        let keys: Vec<f32> = match &columns[self.column] {
            ColumnData::Values(v) => v.iter().map(|&v| self.key(v)).collect(),
            ColumnData::Codes(c) => c
                .iter()
                .map(|&c| {
                    if c == NULL_CODE {
                        f32::NEG_INFINITY
                    } else {
                        self.key(f64::from(c))
                    }
                })
                .collect(),
        };
        let n = keys.len();
        // Largest first and, for equal keys, data order: sort each row's
        // inverted, order-preserving key bits above its index (an unstable
        // sort of `u64`s is several times faster than a stable sort by
        // `f32`).
        let mut packed: Vec<u64> = keys
            .iter()
            .enumerate()
            .map(|(i, &k)| (u64::from(!ordered_bits(k)) << 32) | i as u64)
            .collect();
        packed.sort_unstable();
        let perm: Vec<u32> = packed.into_iter().map(|p| p as u32).collect();
        if perm.iter().enumerate().any(|(i, &p)| i as u32 != p) {
            for column in columns.iter_mut() {
                permute(column, &perm);
            }
        }
        let start = self.keys.len() as u64;
        self.keys.extend(perm.iter().map(|&i| keys[i as usize]));
        self.local.extend(perm);
        self.batches.push(start);
        self.merge(start..start + n as u64);
    }

    fn key(&self, v: f64) -> f32 {
        if v.is_finite() {
            // `+ 0.0` makes -0 equal to 0 under `total_cmp`.
            (if self.reversed { -v } else { v }) as f32 + 0.0
        } else {
            f32::NEG_INFINITY
        }
    }

    /// Merge the sorted run of stored rows `run` (the newest) into the
    /// segments: `O(segments + run · log rows)`.
    fn merge(&mut self, run: Range<u64>) {
        let keys = &self.keys;
        let at = |r: &Range<u64>| &keys[r.start as usize..r.end as usize];
        let mut out: Vec<Range<u64>> = Vec::with_capacity(self.segments.len() + 1);
        let mut push = |r: Range<u64>| {
            if r.is_empty() {
                return;
            }
            match out.last_mut() {
                Some(last) if last.end == r.start => last.end = r.end,
                _ => out.push(r),
            }
        };
        let mut next = run.start;
        for seg in std::mem::take(&mut self.segments) {
            let mut s = seg.start;
            while s < seg.end && next < run.end {
                // Stored rows at least as large as the next new row draw
                // before it.
                let k = keys[next as usize];
                let p = s + at(&(s..seg.end)).partition_point(|&e| e >= k) as u64;
                push(s..p);
                s = p;
                if s == seg.end {
                    break;
                }
                // New rows larger than the stored row at `s` draw here.
                let e = keys[s as usize];
                let q = next + at(&(next..run.end)).partition_point(|&n| n > e) as u64;
                push(next..q);
                next = q;
            }
            push(s..seg.end);
        }
        push(next..run.end);
        self.segments = out;
    }

    /// The stored rows in draw order, as ranges of consecutive rows.
    #[cfg(test)]
    pub(crate) fn segments(&self) -> &[Range<u64>] {
        &self.segments
    }

    /// Whether the runs are fragmented enough (appends interleaving with
    /// stored rows) that the layer should evaluate its rows again into
    /// one sorted run: more than 64 segments, plus one per 1024 rows, so
    /// re-uploading costs `O(1)` bytes per appended row on average.
    pub(crate) fn needs_compaction(&self) -> bool {
        self.segments.len() > COMPACT_SEGMENTS + self.keys.len() / 1024
    }

    /// The data row (index into the selection's rows) stored at `stored`:
    /// how picking maps a drawn instance (`row_base + instance_index`)
    /// back to its row (RFC-001 S9).
    #[cfg_attr(not(test), expect(dead_code, reason = "picking is RFC-001 S9"))]
    pub(crate) fn row_of(&self, stored: u64) -> u64 {
        let batch = self.batches.partition_point(|&b| b <= stored) - 1;
        self.batches[batch] + u64::from(self.local[stored as usize])
    }

    /// The instanced draws over `chunks`, in draw order: each segment
    /// split at chunk boundaries.
    pub(crate) fn spans(&self, chunks: &[Chunk]) -> Vec<Span> {
        let mut spans = Vec::with_capacity(self.segments.len() + chunks.len());
        for seg in &self.segments {
            let mut s = seg.start;
            while s < seg.end {
                let k = chunks.partition_point(|c| c.row_base() + u64::from(c.rows()) <= s);
                let chunk = &chunks[k];
                let end = seg.end.min(chunk.row_base() + u64::from(chunk.rows()));
                spans.push(Span {
                    chunk: k as u32,
                    rows: (s - chunk.row_base()) as u32..(end - chunk.row_base()) as u32,
                });
                s = end;
            }
        }
        spans
    }
}

/// One span per chunk, every row in upload order: a layer's draws
/// without a size order.
pub(crate) fn chunk_spans(chunks: &[Chunk]) -> Vec<Span> {
    chunks
        .iter()
        .enumerate()
        .map(|(k, c)| Span {
            chunk: k as u32,
            rows: 0..c.rows(),
        })
        .collect()
}

/// One of a layer's instanced draw entries: what one `Chunk` uniform entry
/// and one set of bound columns describe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DrawEntry {
    /// The chunk the instance's own row is in.
    pub this: usize,
    /// That row for instance 0, within the chunk.
    pub first: u32,
    /// The chunk a polyline's next row is in.
    pub next: usize,
    /// The next row for instance 0, within that chunk.
    pub next_first: u32,
    /// Instances the entry can draw.
    pub instances: u32,
}

/// A layer's draw entries: one per chunk, its rows from the first; for a
/// [`Geometry::Polyline`], one segment per row but the chunk's last (whose
/// successor is in the next chunk), then one **bridge** per chunk boundary,
/// the segment from chunk `k`'s last row to chunk `k + 1`'s first. A
/// bridge binds the two rows' columns from their two buffers and takes
/// each chunk's own origin, so the column store needs no overlap rows
/// (RFC-001 "GUP-432 findings").
pub(crate) fn draw_entries(geometry: Geometry, chunks: &[Chunk]) -> Vec<DrawEntry> {
    let own = chunks.iter().enumerate().map(|(k, c)| DrawEntry {
        this: k,
        first: 0,
        next: k,
        next_first: 1,
        instances: match geometry {
            Geometry::Quad => c.rows(),
            Geometry::Polyline => c.rows().saturating_sub(1),
        },
    });
    match geometry {
        Geometry::Quad => own.collect(),
        Geometry::Polyline => own
            .chain(chunks.windows(2).enumerate().map(|(k, pair)| DrawEntry {
                this: k,
                first: pair[0].rows() - 1,
                next: k + 1,
                next_first: 0,
                instances: 1,
            }))
            .collect(),
    }
}

/// A polyline's spans over [`draw_entries`], in data order: each chunk's
/// own segments, then the bridge to the next chunk.
pub(crate) fn polyline_spans(chunks: &[Chunk]) -> Vec<Span> {
    let n = chunks.len();
    let mut spans = Vec::with_capacity(2 * n);
    for (k, c) in chunks.iter().enumerate() {
        if c.rows() > 1 {
            spans.push(Span {
                chunk: k as u32,
                rows: 0..c.rows() - 1,
            });
        }
        if k + 1 < n {
            spans.push(Span {
                chunk: (n + k) as u32,
                rows: 0..1,
            });
        }
    }
    spans
}

/// `k`'s bits as a `u32` whose order is `k`'s (`total_cmp`'s) order.
fn ordered_bits(k: f32) -> u32 {
    let bits = k.to_bits();
    if bits >> 31 == 1 {
        !bits
    } else {
        bits | 1 << 31
    }
}

fn permute(column: &mut ColumnData, perm: &[u32]) {
    match column {
        ColumnData::Values(v) => *v = perm.iter().map(|&i| v[i as usize]).collect(),
        ColumnData::Codes(c) => *c = perm.iter().map(|&i| c[i as usize]).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stored rows in draw order.
    fn drawn(order: &DrawOrder) -> Vec<u64> {
        order.segments().iter().flat_map(Clone::clone).collect()
    }

    fn sizes(order: &DrawOrder) -> Vec<f32> {
        drawn(order)
            .into_iter()
            .map(|r| order.keys[r as usize])
            .collect()
    }

    #[test]
    fn a_batch_is_stored_largest_first_with_every_column_alike() {
        let mut order = DrawOrder::new(1, false);
        let mut columns = vec![
            ColumnData::Values(vec![0.0, 1.0, 2.0, 3.0]),
            ColumnData::Values(vec![1.0, 4.0, f64::NAN, 4.0]),
            ColumnData::Codes(vec![10, 11, 12, 13]),
        ];
        order.push_batch(&mut columns);
        // Equal sizes keep data order; the null draws first.
        assert_eq!(columns[0], ColumnData::Values(vec![1.0, 3.0, 0.0, 2.0]));
        assert_eq!(columns[2], ColumnData::Codes(vec![11, 13, 10, 12]));
        assert_eq!(
            (order.segments().len(), order.segments()[0].clone()),
            (1, 0..4)
        );
        assert_eq!(
            (0..4).map(|r| order.row_of(r)).collect::<Vec<_>>(),
            [1, 3, 0, 2]
        );
    }

    #[test]
    fn a_reversed_scale_stores_the_smallest_value_first() {
        let mut order = DrawOrder::new(0, true);
        let mut columns = vec![ColumnData::Values(vec![1.0, 3.0, 2.0])];
        order.push_batch(&mut columns);
        assert_eq!(columns[0], ColumnData::Values(vec![1.0, 2.0, 3.0]));
    }

    #[test]
    fn appended_batches_merge_into_one_descending_order() {
        let mut order = DrawOrder::new(0, false);
        order.push_batch(&mut [ColumnData::Values(vec![1.0, 5.0, 3.0, 7.0])]);
        // Larger, between and smaller, and equal to a stored row.
        order.push_batch(&mut [ColumnData::Values(vec![0.0, 8.0, 4.0, 5.0])]);
        assert_eq!(sizes(&order), [8.0, 7.0, 5.0, 5.0, 4.0, 3.0, 1.0, 0.0]);
        // The stored 5 (row 1) draws before the appended 5 (row 5).
        assert_eq!(drawn(&order), [4, 0, 1, 5, 6, 2, 3, 7]);
        assert_eq!(order.segments(), &[4..5, 0..2, 5..7, 2..4, 7..8]);
        // Rows map back to data rows: the appended 5 is data row 7.
        assert_eq!(order.row_of(5), 7);
        assert_eq!(order.row_of(4), 5);
        assert_eq!(order.row_of(0), 3);
        // A batch wholly smaller extends the last segment.
        order.push_batch(&mut [ColumnData::Values(vec![-1.0, -2.0])]);
        assert_eq!(order.segments(), &[4..5, 0..2, 5..7, 2..4, 7..10]);
    }

    #[test]
    fn random_batches_merge_to_the_sorted_order() {
        // A small LCG, so the test is deterministic.
        let mut state = 0x2545_f491_u64;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            (state >> 33) as f64 / f64::from(1u32 << 31)
        };
        let mut order = DrawOrder::new(0, false);
        let mut all = Vec::new();
        for batch in [100, 1, 7, 0, 30, 3] {
            let v: Vec<f64> = (0..batch).map(|_| (next() * 20.0).floor()).collect();
            all.extend(v.iter().map(|&v| v as f32));
            order.push_batch(&mut [ColumnData::Values(v)]);
        }
        all.sort_by(|a, b| b.total_cmp(a));
        assert_eq!(sizes(&order), all);
        let mut rows = drawn(&order);
        rows.sort_unstable();
        assert_eq!(rows, (0..all.len() as u64).collect::<Vec<_>>());
        // Ties draw stored (earlier) rows first.
        let drawn = drawn(&order);
        for w in drawn.windows(2) {
            let (a, b) = (order.keys[w[0] as usize], order.keys[w[1] as usize]);
            assert!(a > b || (a == b && order.row_of(w[0]) < order.row_of(w[1])));
        }
    }

    #[test]
    fn fragmented_runs_ask_for_compaction() {
        let mut order = DrawOrder::new(0, false);
        order.push_batch(&mut [ColumnData::Values((0..200).map(f64::from).collect())]);
        assert!(!order.needs_compaction());
        // Each value lands between two stored rows: two segments each.
        for i in 0..40 {
            order.push_batch(&mut [ColumnData::Values(vec![f64::from(i) * 4.0 + 0.5])]);
        }
        assert!(order.segments().len() > COMPACT_SEGMENTS);
        assert!(order.needs_compaction());
    }

    /// A disc of the adversarial chain: its centre `x`, radius `r` (both
    /// in pixels) and colour key.
    #[derive(Clone, Copy, Debug)]
    struct Disc {
        x: f64,
        y: f64,
        r: f64,
        key: &'static str,
    }

    /// Discs tangent inside one another at `x = 70.5`, each radius three
    /// times the last, so a larger disc covers each smaller disc's centre
    /// and no smaller disc reaches a larger one's: drawn in data order
    /// (smallest first) only the largest centre would show its own fill.
    fn chain(radii: &[f64], keys: &[&'static str]) -> Vec<Disc> {
        radii
            .iter()
            .zip(keys)
            .map(|(&r, &key)| Disc {
                x: 70.5 + r,
                y: 215.5,
                r,
                key,
            })
            .collect()
    }

    /// `fill` at `opacity` over `under`, in sRGB, per channel in 0..=255.
    fn over(fill: crate::Color, opacity: f32, under: [f32; 3]) -> [f32; 3] {
        let f = [fill.r, fill.g, fill.b];
        std::array::from_fn(|k| f[k] * opacity * 255.0 + under[k] * (1.0 - opacity))
    }

    /// Check every chain disc's centre pixel: the discs that clearly cover
    /// it (none has an edge within 1.5 px), composited largest first at
    /// `opacity` over white. Returns how many centres two discs cover.
    fn check_centres(image: &image::RgbaImage, discs: &[Disc], opacity: f32) -> usize {
        let mut two = 0;
        for d in discs.iter().filter(|d| d.key != "filler") {
            let (px, py) = (d.x.floor(), d.y.floor());
            let (cx, cy) = (px + 0.5, py + 0.5);
            let mut covering: Vec<&Disc> = Vec::new();
            for e in discs.iter().filter(|e| e.key != "filler") {
                let dist = (e.x - cx).hypot(e.y - cy);
                assert!(
                    (dist - e.r).abs() > 1.5,
                    "disc {e:?}'s edge is near {d:?}'s centre"
                );
                if dist < e.r {
                    covering.push(e);
                }
            }
            // Adversarial: a larger disc covers every centre but the
            // largest's; no smaller disc does.
            assert!(
                covering.iter().all(|e| e.r >= d.r),
                "{d:?} under {covering:?}"
            );
            let largest = discs.iter().map(|e| e.r).fold(0.0, f64::max);
            assert!(
                d.r == largest || covering.len() > 1,
                "{d:?} overlaps nothing"
            );
            covering.sort_by(|a, b| b.r.total_cmp(&a.r));
            let want = covering
                .iter()
                .fold([255.0; 3], |under, e| over(colour(e.key), opacity, under));
            let got = image.get_pixel(px as u32, py as u32).0;
            let tolerance = if opacity < 1.0 { 3.0 } else { 2.0 };
            assert!(
                (0..3).all(|k| (f32::from(got[k]) - want[k]).abs() <= tolerance),
                "{d:?} at ({px}, {py}): {got:?}, wanted {want:?} ({} discs)",
                covering.len()
            );
            two += usize::from(covering.len() == 2);
        }
        two
    }

    const KEYS: [&str; 6] = ["filler", "a", "b", "c", "d", "e"];

    fn colour(key: &str) -> crate::Color {
        let code = KEYS.iter().position(|k| *k == key).unwrap();
        crate::OKABE_ITO[code]
    }

    /// AC2 at `opacity`, with `filler` (at least one) tiny discs first (out of
    /// the chain's way) and `chunk_rows` rows per chunk: rows inserted
    /// smallest first draw largest first, before and after appending a
    /// smaller and a larger disc, and the append uploads only the new
    /// rows' column bytes.
    fn size_order_case(filler: usize, chunk_rows: Option<u32>, opacity: f32) {
        use crate::prelude::*;
        use crate::target::ImageTarget;

        let cx = Context::new_blocking().unwrap();
        let mut rows: Vec<Disc> = (0..filler)
            .map(|_| Disc {
                x: 470.0,
                y: 60.0,
                r: 1.0,
                key: "filler",
            })
            .collect();
        rows.extend(chain(&[6.0, 18.0, 54.0], &["a", "b", "c"]));
        let appended = chain(&[2.0, 162.0], &["d", "e"]);
        let pixels = || Linear::new().domain(0.0, 1.0).range(Px(0.0), Px(1.0));
        // Keys take codes in first-seen order: KEYS order, as the filler
        // comes first.
        let palette = Categorical::okabe_ito();

        let mut plot = Plot::new();
        // Legends are on by default (GUP-426); their text atlas would upload on
        // an append, and they are not this test's subject.
        plot.legends(false);
        plot.x(Linear::new().domain(0.0, 1.0));
        plot.y(Linear::new().domain(0.0, 1.0));
        let layer = plot.add(Selection::<Disc, Circle>::new(rows.clone()));
        if let Some(n) = chunk_rows {
            layer.max_chunk_rows(n);
        }
        layer
            .attr(Circle::X, pixels().encode(|d: &Disc| d.x))
            .attr(Circle::Y, pixels().encode(|d: &Disc| d.y))
            .attr(Circle::RADIUS, pixels().encode(|d: &Disc| d.r))
            .attr(Circle::FILL, palette.encode_key(|d: &Disc| d.key))
            .attr(Circle::FILL_OPACITY, Unit(opacity));
        let render = |plot: &mut Plot| {
            let resolved = plot.resolve(&cx, 560.0, 450.0).unwrap().clone();
            let image = ImageTarget::new(&cx, 560, 450)
                .unwrap()
                .render_blocking(&cx, &resolved.scene)
                .unwrap();
            (resolved, image)
        };
        let (resolved, image) = render(&mut plot);
        let p = resolved.layout.plot;
        for d in rows.iter().chain(&appended) {
            assert!(
                d.x - d.r > f64::from(p.left()) && d.x + d.r < f64::from(p.right()),
                "{d:?} leaves {p:?}"
            );
            assert!(d.y - d.r > f64::from(p.top()) && d.y + d.r < f64::from(p.bottom()));
        }
        check_centres(&image, &rows, opacity);

        let layer = plot.layer_mut::<Disc, Circle>().unwrap();
        let stride: u64 = layer
            .store()
            .unwrap()
            .formats()
            .iter()
            .map(|f| f.stride())
            .sum();
        let start = cx.upload_stats();
        layer.append(appended.clone()).unwrap();
        let (resolved, image) = render(&mut plot);
        let uploads = cx.upload_stats() - start;
        assert_eq!(
            uploads.columns.bytes,
            2 * stride,
            "only the new rows' columns"
        );
        assert_eq!(uploads.validity.bytes, 0);
        // No ordering data at all (the draws' instance ranges carry it):
        // besides the columns, only uniforms and the guides' instances.
        assert_eq!(uploads.textures.bytes, 0);
        // The new largest disc draws first and the new smallest after the
        // chain, ahead of the filler.
        let spans = resolved
            .scene
            .items
            .iter()
            .find_map(|i| match &i.kind {
                crate::scene::ItemKind::Marks(b) => Some(b.gpu.spans.clone()),
                _ => None,
            })
            .unwrap();
        assert!(spans.len() > 1, "{spans:?}");
        rows.extend(appended);
        let two = check_centres(&image, &rows, opacity);
        assert!(two > 0, "no centre is covered by exactly two discs");
    }

    #[test]
    fn rows_draw_largest_first_before_and_after_an_append() {
        size_order_case(1, None, 1.0);
    }

    #[test]
    fn translucent_rows_composite_largest_first() {
        size_order_case(1, None, 0.5);
    }

    #[test]
    fn size_order_spans_chunks() {
        size_order_case(120, Some(64), 1.0);
        size_order_case(120, Some(64), 0.5);
    }

    /// Appends that interleave with the stored rows add draws until the
    /// layer evaluates its rows again into one sorted run.
    #[test]
    fn fragmented_layers_compact_into_one_run() {
        use crate::prelude::*;

        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
        let size = Linear::new().domain(0.0, 400.0).range(Px(1.0), Px(5.0));
        plot.add(Selection::<f64, Circle>::new(
            (0..200).map(|i| f64::from(i) * 2.0).collect::<Vec<_>>(),
        ))
        .attr(Circle::X, x.encode(|v: &f64| *v))
        .attr(Circle::Y, y.encode(|v: &f64| *v))
        .attr(Circle::RADIUS, size.encode(|v: &f64| *v));
        let draws = |plot: &mut Plot| {
            let resolved = plot.resolve(&cx, 300.0, 200.0).unwrap().clone();
            let layer = plot.layer_mut::<f64, Circle>().unwrap();
            let segments = layer.draw_order().unwrap().segments().len();
            (resolved, segments)
        };
        assert_eq!(draws(&mut plot).1, 1);
        let mut most = 0;
        for i in 0..40 {
            let layer = plot.layer_mut::<f64, Circle>().unwrap();
            layer.append([f64::from(i) * 8.0 + 1.0]).unwrap();
            let (_, segments) = draws(&mut plot);
            most = most.max(segments);
        }
        // Each append split a segment until the 65th segment re-sorted.
        assert!(most > 60, "{most} segments at most");
        let (_, segments) = draws(&mut plot);
        assert!(segments < 30, "{segments} segments after compaction");
    }
}
