// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Level-of-detail tiers of one chunk (GUP-631's decimation pyramid,
//! GUP-714): which rows stand for which cells at every zoom level.
//!
//! A chunk's finite rows each get a 32-bit Morton key on a grid of
//! `65,536 × 65,536` cells (tier 16) anchored in data units: a cell is
//! `2^ex` by `2^ey` data units, at multiples of those sizes, and the keys
//! count cells from the chunk's own minimum cell. The grid depends on the
//! chunk's rows alone, so a layer whose extent grows (appends) leaves the
//! tiers of built chunks valid. Tier `t` has `2^t × 2^t` cells over the
//! same span: a tier-`t` cell is the run of keys equal in their top `2t`
//! bits.
//!
//! A cell's representative at a tier is its lowest row, so tiers nest: a
//! cell's representative is one of its four children's. One stable radix
//! sort of `(key, row)` pairs puts every tier's cells in runs of keys, and
//! a row represents its cell at every tier from its `first_tier` on (see
//! [`first_tiers`]). Rows are kept in **progressive order**, by
//! `first_tier` first: tier `t` is the prefix `order[..counts[t]]`.
//!
//! Each tier's block of the prefix, `order[counts[t - 1]..counts[t]]`, is
//! cut into [`Run`]s: consecutive stretches of at most [`RUN_LEN`] of the
//! block's representatives in Morton order of their keys, each with the
//! box of their positions (GUP-720). A stretch of a Morton curve is
//! compact, so a frame draws, of the prefix, only the runs whose boxes it
//! can see ([`ranges`]): a zoom into a chunk that spans the whole extent
//! (unsorted rows) skips most of it. Within a run the representatives are
//! **spread**: drawn in bit-reversed order of their Morton positions, so
//! consecutive instances land far apart. Drawn in Morton order, neighbours
//! overlap and the GPU serialises their blending: a shuffled 10M-row full
//! view took 20.3 ms against 13.9 in row order (GUP-720), as a sorted x
//! costs more than a shuffled one (RFC-001 "GUP-631 findings").
//!
//! Tiers are stored while they hold at most half the chunk's rows; past
//! the finest stored tier a chunk draws every row ([`Tiers::prefix`] is
//! `None`). Tier 0 is always stored.
//!
//! Each frame, a visible chunk draws the prefix of the tier [`tier`]
//! chooses for the view (GUP-716) through the indexed-rows glue (GUP-715).

/// Rows an appended chunk holds before it builds tiers (GUP-719): below
/// this a full draw is cheap. It rebuilds each time its rows have doubled.
pub(crate) const TAIL_MIN_ROWS: u32 = 65_536;

/// The finest tier: `2^16` cells a side, 16 bits of each axis in a key.
pub(crate) const TMAX: u32 = 16;

/// Cells a side at [`TMAX`].
const SIDE: f64 = 65_536.0;

/// A row's `first_tier` when it never represents a cell: a later row of
/// its tier-16 cell, or a row with a non-finite position.
const NEVER: u8 = TMAX as u8 + 1;

/// Bits of a key per radix-sort pass: three passes cover 32 bits.
const DIGIT_BITS: u32 = 11;

/// Buckets of a radix-sort pass.
const BUCKETS: usize = 1 << DIGIT_BITS;

/// Representatives a [`Run`] holds at most: small enough that a zoom view
/// skips most of a block, large enough that a 476,000-representative chunk
/// is about 120 runs (GUP-720).
pub(crate) const RUN_LEN: u32 = 4_096;

/// Ranges of representatives one chunk draws in a frame at most: further
/// runs merge across the smallest gaps between them, drawing a few
/// instances more instead of a draw more (GUP-720, after GUP-492's finding
/// on draw counts).
pub(crate) const MAX_RANGES: usize = 64;

/// A stretch of a tier's block of representatives, `order[start..end]`,
/// and the box of their positions in data units (each axis `(min, max)`;
/// a constant axis reads 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Run {
    pub start: u32,
    pub end: u32,
    pub x: (f64, f64),
    pub y: (f64, f64),
}

/// The tiers of one chunk: see the [module docs](self).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tiers {
    /// The rows of the chunk when the tiers were built: `order` holds rows
    /// below it only (an appended chunk's later rows draw in full, GUP-719).
    pub rows: u32,
    /// Cell width at the finest tier, as a power of two: `2^ex` data units.
    pub ex: i32,
    /// Cell height at the finest tier, as a power of two: `2^ey` data units.
    pub ey: i32,
    /// The finest stored tier (`0..=16`).
    pub stored: u32,
    /// Chunk-local rows in progressive order: tier `t` is
    /// `order[..counts[t]]`. Its length is `counts[stored]`.
    pub order: Vec<u32>,
    /// `counts[t]`, the representatives of tier `t`, for `t` in
    /// `0..=stored`: non-decreasing, `counts[0]` is 1 (0 for a chunk
    /// without a finite row).
    pub counts: Vec<u32>,
    /// Every tier block cut into runs of at most [`RUN_LEN`], in `order`'s
    /// order: they tile `order`, and none crosses a block's end.
    pub runs: Vec<Run>,
}

impl Tiers {
    /// The tiers of a chunk of `rows` rows whose row `r` (chunk-local) is
    /// at `xy(r)`. Rows with a non-finite x or y are skipped: they
    /// represent nothing. Reads every row twice (once for the extent, once
    /// for the keys); the peak scratch is 16 B a finite row.
    pub fn build(rows: u32, xy: impl Fn(u32) -> (f64, f64)) -> Tiers {
        // The extent of the finite rows.
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        let mut finite = 0usize;
        for r in 0..rows {
            let (x, y) = xy(r);
            if x.is_finite() && y.is_finite() {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
                finite += 1;
            }
        }
        if finite == 0 {
            return Tiers {
                rows,
                ex: 0,
                ey: 0,
                stored: 0,
                order: Vec::new(),
                counts: vec![0],
                runs: Vec::new(),
            };
        }
        let (ex, ey) = match (exponent(x0, x1), exponent(y0, y1)) {
            (Some(ex), Some(ey)) => (ex, ey),
            (Some(e), None) | (None, Some(e)) => (e, e),
            (None, None) => (0, 0),
        };
        let (gx, gy) = (Axis::new(ex, x0, x1), Axis::new(ey, y0, y1));

        // `key << 32 | row` per finite row, with each pass's histogram.
        let mut hist = [[0u32; BUCKETS]; 3];
        let mut pairs = Vec::with_capacity(finite);
        for r in 0..rows {
            let (x, y) = xy(r);
            if x.is_finite() && y.is_finite() {
                let key = part1by1(gx.cell(x)) | part1by1(gy.cell(y)) << 1;
                for (pass, h) in hist.iter_mut().enumerate() {
                    h[digit(key, pass)] += 1;
                }
                pairs.push(u64::from(key) << 32 | u64::from(r));
            }
        }
        let mut sorted = radix_sort(pairs, &mut hist);
        // The rows in key order, for the blocks' Morton order (`first_tiers`
        // consumes the pairs).
        let morton: Vec<u32> = sorted.iter().map(|&p| p as u32).collect();

        // Each row's first tier, by row, and how many rows start at each.
        let mut first = vec![NEVER; rows as usize];
        first_tiers(&mut sorted, &mut first);
        drop(sorted);
        let mut starts = [0u32; NEVER as usize + 1];
        for &t in &first {
            starts[usize::from(t)] += 1;
        }

        // The storage rule: tier 0, then every tier of at most half the rows.
        let half = rows / 2;
        let mut counts = Vec::with_capacity(TMAX as usize + 1);
        let mut sum = 0u32;
        for (t, &s) in starts[..=TMAX as usize].iter().enumerate() {
            sum += s;
            if t > 0 && sum > half {
                break;
            }
            counts.push(sum);
        }
        let stored = counts.len() as u32 - 1;

        // The progressive order: a stable counting sort by first tier of
        // the rows in key order, so each block is in Morton order.
        let mut next: Vec<u32> = std::iter::once(0).chain(counts.iter().copied()).collect();
        let mut order = vec![0u32; counts[stored as usize] as usize];
        for r in morton {
            let t = first[r as usize];
            if u32::from(t) <= stored {
                let at = &mut next[usize::from(t)];
                order[*at as usize] = r;
                *at += 1;
            }
        }
        drop(first);
        let runs = runs(&order, &counts, &xy);
        spread(&mut order, &runs);
        Tiers {
            rows,
            ex,
            ey,
            stored,
            order,
            counts,
            runs,
        }
    }

    /// The representatives of tier `t`, the length of the prefix of
    /// [`order`](Self::order) to draw, or `None` past the stored tiers
    /// (draw every row).
    pub fn prefix(&self, t: u32) -> Option<u32> {
        self.counts.get(t as usize).copied()
    }

    /// For tier `t` one past the stored tiers, the finest stored tier's
    /// prefix, which a frame may draw in its place (at most one tier coarser
    /// than asked) instead of every row; `None` for any other `t`. GUP-720:
    /// a chunk spanning the extent of 10M unsorted rows stores tier 13 and a
    /// 10x zoom asks 14, so it drew every row, 135 ms, against 15.9 ms and
    /// 0.31% of the plot rect over ΔE00 10 for tier 13. The frame takes it
    /// only when drawing every row would cost more than the threshold
    /// ([`in_view`]).
    pub fn cap(&self, t: u32) -> Option<u32> {
        (t == self.stored + 1).then(|| self.counts[self.stored as usize])
    }
}

/// Each tier block of `order` (blocks end at `counts`) cut into runs of at
/// most [`RUN_LEN`], with the boxes of their rows' positions `xy`.
fn runs(order: &[u32], counts: &[u32], xy: &impl Fn(u32) -> (f64, f64)) -> Vec<Run> {
    let mut runs = Vec::with_capacity(order.len().div_ceil(RUN_LEN as usize) + counts.len());
    let mut start = 0;
    for &end in counts {
        for s in (start..end).step_by(RUN_LEN as usize) {
            let e = end.min(s + RUN_LEN);
            let mut run = Run {
                start: s,
                end: e,
                x: (f64::INFINITY, f64::NEG_INFINITY),
                y: (f64::INFINITY, f64::NEG_INFINITY),
            };
            for &r in &order[s as usize..e as usize] {
                let (x, y) = xy(r);
                run.x = (run.x.0.min(x), run.x.1.max(x));
                run.y = (run.y.0.min(y), run.y.1.max(y));
            }
            runs.push(run);
        }
        start = end;
    }
    runs
}

/// Reorder each run of `order` from Morton order to its bit-reversal: the
/// representative at position `i` of a run of `n` is the one of Morton rank
/// `reverse(i)`, over `ceil(log2 n)` bits, skipping ranks past the run's
/// end. The run's set, and so its box, is unchanged.
fn spread(order: &mut [u32], runs: &[Run]) {
    let mut morton = Vec::with_capacity(RUN_LEN as usize);
    for run in runs {
        let s = &mut order[run.start as usize..run.end as usize];
        morton.clear();
        morton.extend_from_slice(s);
        let n = s.len();
        let bits = n.next_power_of_two().trailing_zeros();
        let mut at = 0;
        for i in 0..n.next_power_of_two() {
            let j = if bits == 0 {
                0
            } else {
                i.reverse_bits() >> (usize::BITS - bits)
            };
            if j < n {
                s[at] = morton[j];
                at += 1;
            }
        }
    }
}

/// An estimate of the rows of a chunk inside a view, from the runs of its
/// prefix `order[..count]` (`count` one of the tiers' `counts`): each run's
/// representatives weighted by `share`, the share of its box the view holds
/// (as if its representatives were spread evenly over it), scaled from
/// representatives to the `rows` they stand for.
pub(crate) fn in_view(
    runs: &[Run],
    count: u32,
    rows: u32,
    share: &mut dyn FnMut(&Run) -> f64,
) -> f64 {
    if count == 0 {
        return 0.0;
    }
    let reps: f64 = runs
        .iter()
        .take_while(|r| r.start < count)
        .map(|r| f64::from(r.end - r.start) * share(r))
        .sum();
    reps * f64::from(rows) / f64::from(count)
}

/// The ranges of `order[..count]` (a tier's prefix: `count` is one of the
/// tiers' `counts`) a frame draws: the `runs` that `seen` keeps, adjacent
/// ones merged into one range; past [`MAX_RANGES`] ranges, the ranges
/// either side of the narrowest gaps merged too (drawing the gaps'
/// instances, which `seen` rejected): every gap narrower than the
/// narrowest kept one is merged, so ties may leave fewer ranges. In order;
/// empty when no run is seen. `seen` is a trait object so one copy serves
/// every caller (WASM size).
pub(crate) fn ranges(
    runs: &[Run],
    count: u32,
    seen: &mut dyn FnMut(&Run) -> bool,
) -> Vec<std::ops::Range<u32>> {
    let mut out: Vec<std::ops::Range<u32>> = Vec::new();
    for run in runs.iter().take_while(|r| r.start < count) {
        debug_assert!(run.end <= count, "a run crosses the prefix's end");
        if !seen(run) {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.end == run.start => last.end = run.end,
            _ => out.push(run.start..run.end),
        }
    }
    if out.len() > MAX_RANGES {
        // The smallest gap width `g` that leaves at most `MAX_RANGES`
        // ranges when every gap narrower than `g` is merged: a binary
        // search over widths (no sort, a few dozen passes over the gaps).
        let gaps = || out.windows(2).map(|w| w[1].start - w[0].end);
        let kept = |g: u32| 1 + gaps().filter(|&w| w >= g).count();
        let (mut lo, mut hi) = (0u32, gaps().max().unwrap_or(0) + 1);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if kept(mid) <= MAX_RANGES {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        let mut merged: Vec<std::ops::Range<u32>> = Vec::with_capacity(MAX_RANGES);
        for r in &out {
            match merged.last_mut() {
                Some(last) if r.start - last.end < lo => last.end = r.end,
                _ => merged.push(r.clone()),
            }
        }
        out = merged;
    }
    out
}

/// How many tiers finer than "at most one representative per pixel of
/// area" a chunk draws: one, so at most four a pixel. GUP-631 measured the
/// share of pixels more than ΔE00 10 from the full draw of a 10M-row
/// scatter at 1920 × 1080: at the unbiased tier 0.994% of the image but
/// 1.076% of the plot rect (over the 1% rule); one tier finer 0.580% /
/// 0.628%, for 1.0 ms more GPU time (RFC-001 "GUP-631 findings",
/// "Fidelity").
const LOD_TIER_BIAS: u32 = 1;

/// The tier a chunk draws, from its tiers' cell exponents `ex`, `ey` and
/// the view's pixels per data unit along x and y over the chunk, `kx` and
/// `ky` (0: the chunk has no span on that axis): the finest tier whose
/// cells are at least one pixel in area, plus [`LOD_TIER_BIAS`],
///
/// ```text
/// t = floor(16 + (ex + ey) / 2 - 0.5 * log2(A)) + LOD_TIER_BIAS
/// ```
///
/// with `A = 1 / (kx * ky)` the data area of one pixel, at least 0. Past
/// tier 16 the finest cells are coarser than the view needs, so `None`:
/// the chunk draws every row (as the GUP-631 spike did past its finest
/// tier; a chunk spanning a million seconds seen through a 1 ms window
/// must not draw one row per 16 s). A chunk with no span on one axis is a
/// line of cells: the finest tier whose cells are at least one pixel long
/// on the other axis (`floor(16 + e + log2(k))`), with the same bias. With
/// no span on either axis every row is at one place and one representative
/// is drawn (tier 0). `None` (draw every row) for a scale that is not
/// finite.
pub(crate) fn tier(ex: i32, ey: i32, kx: f64, ky: f64) -> Option<u32> {
    if !(kx.is_finite() && ky.is_finite() && kx >= 0.0 && ky >= 0.0) {
        return None;
    }
    let t = match (kx > 0.0, ky > 0.0) {
        (true, true) => f64::from(TMAX) + f64::from(ex + ey) / 2.0 + 0.5 * (kx * ky).log2(),
        (true, false) => f64::from(TMAX) + f64::from(ex) + kx.log2(),
        (false, true) => f64::from(TMAX) + f64::from(ey) + ky.log2(),
        (false, false) => 0.0,
    };
    let t = t.floor() + f64::from(LOD_TIER_BIAS);
    (t <= f64::from(TMAX)).then(|| t.max(0.0) as u32)
}

/// What one prepare's level of detail may do: the plot's side of it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Frame<'a> {
    /// Whether the plot shows each of the layer's draw entries
    /// (`cull::visible_entries`; empty: every entry).
    pub visible: &'a [bool],
    /// Whether to spread lazy tier builds over frames, one chunk a
    /// prepare: the plot has a host that draws again when asked
    /// ([`Chart::needs_frame`](crate::Chart::needs_frame)). Otherwise (a
    /// headless render, a host that never set a waker) every missing chunk
    /// builds in the one prepare, so a single frame is complete.
    pub spread: bool,
    /// The plot rect when the plot culls (as `visible` is computed): a
    /// chunk drawing a tier draws only the [`Run`]s that can reach it
    /// (GUP-720). `None`: every run.
    pub plot: Option<crate::geom::Rect>,
}

/// One axis of the tier-16 grid: `cell(v) = floor(v / 2^e) - origin`, the
/// division as one or two multiplications by powers of two (exact unless
/// the product is subnormal, where rounding is still monotonic).
#[derive(Debug, Clone, Copy)]
struct Axis {
    a: f64,
    b: f64,
    origin: f64,
    /// `origin` as an integer, when every cell of the extent is well
    /// inside `i64`: the floor is then a truncation and a compare (no
    /// libm call).
    fast: Option<i64>,
}

/// Cells (as `v / 2^e`) at most this far from zero take the integer path.
const FAST: f64 = (1u64 << 62) as f64;

impl Axis {
    /// The axis of cells `2^e` wide counted from the cell of `min`, for
    /// values in `min..=max`.
    fn new(e: i32, min: f64, max: f64) -> Self {
        let (a, b) = inverse_pow2(e);
        let mut axis = Self {
            a,
            b,
            origin: 0.0,
            fast: None,
        };
        axis.origin = axis.floor(min);
        if axis.origin.abs() < FAST && axis.floor(max).abs() < FAST {
            axis.fast = Some(axis.origin as i64);
        }
        axis
    }

    /// `floor(v / 2^e)`.
    fn floor(&self, v: f64) -> f64 {
        (v * self.a * self.b).floor()
    }

    /// The cell of `v`, in `0..65_536` for `v` in the extent the axis was
    /// made for: `floor(v / 2^e) - origin`, on either path.
    #[inline]
    fn cell(&self, v: f64) -> u32 {
        let s = v * self.a * self.b;
        match self.fast {
            Some(origin) => {
                // Truncation is exact here, and so is `t as f64`: past
                // 2^53 every `s` is an integer.
                let t = s as i64;
                (t - i64::from(t as f64 > s) - origin) as u32
            }
            None => (s.floor() - self.origin) as u32,
        }
    }

    /// Whether `min..=max` spans fewer than 65,536 cells.
    fn fits(e: i32, min: f64, max: f64) -> bool {
        let axis = Self::new(e, min, max);
        axis.floor(max) - axis.origin < SIDE
    }
}

/// `2^-e` as a product of two normal `f64`s (the second 1 when `2^-e` is
/// normal): `e` ranges far enough below -1,022 for a chunk of subnormals.
fn inverse_pow2(e: i32) -> (f64, f64) {
    let k = -e;
    if (-1022..=1023).contains(&k) {
        (pow2(k), 1.0)
    } else {
        (pow2(k / 2), pow2(k - k / 2))
    }
}

/// `2^k` for `k` in `-1022..=1023`.
fn pow2(k: i32) -> f64 {
    debug_assert!((-1022..=1023).contains(&k), "2^{k} is not a normal f64");
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// `floor(log2(v))` of a positive finite `v`, subnormals included.
fn ilog2(v: f64) -> i32 {
    let bits = v.to_bits();
    let biased = (bits >> 52) as i32 & 0x7ff;
    if biased == 0 {
        // Subnormal: the mantissa's highest bit, at 2^-1074 for bit 0.
        -1074 + (63 - (bits & ((1 << 52) - 1)).leading_zeros() as i32)
    } else {
        biased - 1023
    }
}

/// The smallest `e` with `floor(max / 2^e) - floor(min / 2^e) < 65_536`,
/// or `None` when `min == max` (every `e` fits).
fn exponent(min: f64, max: f64) -> Option<i32> {
    if min >= max {
        return None;
    }
    // Half the span, which cannot overflow: the span is about 2^(e + 16).
    let mut e = ilog2((max / 2.0 - min / 2.0).max(f64::from_bits(1))) + 1 - 16;
    while !Axis::fits(e, min, max) {
        e += 1;
    }
    while Axis::fits(e - 1, min, max) {
        e -= 1;
    }
    Some(e)
}

/// The low 16 bits of `v` spread to the even bits.
fn part1by1(v: u32) -> u32 {
    let mut x = v & 0xffff;
    x = (x | (x << 8)) & 0x00ff_00ff;
    x = (x | (x << 4)) & 0x0f0f_0f0f;
    x = (x | (x << 2)) & 0x3333_3333;
    x = (x | (x << 1)) & 0x5555_5555;
    x
}

/// Radix-sort pass `pass`'s digit of `key`.
fn digit(key: u32, pass: usize) -> usize {
    (key >> (DIGIT_BITS * pass as u32)) as usize & (BUCKETS - 1)
}

/// Sort `key << 32 | row` pairs by key, stably (equal keys stay in row
/// order): least significant digit first, one pass per digit of `hist`
/// (each pass's histogram, consumed as its bucket offsets). A pass whose
/// digit is the same for every key is skipped.
fn radix_sort(pairs: Vec<u64>, hist: &mut [[u32; BUCKETS]; 3]) -> Vec<u64> {
    let n = pairs.len() as u32;
    let mut src = pairs;
    let mut dst = vec![0u64; src.len()];
    for (pass, offsets) in hist.iter_mut().enumerate() {
        if offsets.contains(&n) {
            continue;
        }
        let mut sum = 0;
        for o in offsets.iter_mut() {
            (*o, sum) = (sum, sum + *o);
        }
        for &pair in &src {
            let at = &mut offsets[digit((pair >> 32) as u32, pass)];
            dst[*at as usize] = pair;
            *at += 1;
        }
        std::mem::swap(&mut src, &mut dst);
    }
    src
}

/// The first tier at which keys `a` and `b` (distinct) are in different
/// cells: `16 - floor(h / 2)`, `h` their highest differing bit.
fn split_tier(a: u32, b: u32) -> u8 {
    (TMAX - (31 - (a ^ b).leading_zeros()) / 2) as u8
}

/// Write `first[row]`, the coarsest tier at which `row` is the lowest row
/// of its cell, for every row that represents a cell (the rest stay
/// [`NEVER`]), from the sorted `key << 32 | row` pairs (consumed).
///
/// A tier-`t` cell is a run of the sorted keys, and only the first row of
/// a run of equal keys (its lowest, the sort being stable) can represent
/// anything; call those rows heads. A head is the lowest row of its
/// tier-`t` cell unless the nearest head with a lower row on its left, or
/// on its right, shares the cell (any lower head in the run is at least
/// that far). So its first tier is the larger of its split tiers with
/// those two neighbours (0 for a missing one): one pass with a stack of
/// heads in increasing row order. The stack lives in the pairs already
/// read (it never holds more heads than were read), and a head's split
/// tier with its left neighbour waits in `first`.
fn first_tiers(sorted: &mut [u64], first: &mut [u8]) {
    let mut top = 0;
    let mut prev = None;
    for i in 0..sorted.len() {
        let pair = sorted[i];
        let (key, row) = ((pair >> 32) as u32, pair as u32);
        if prev == Some(key) {
            continue;
        }
        prev = Some(key);
        // Heads of higher rows on the stack have their right neighbour.
        let mut left = 0;
        while let Some(&other) = sorted[..top].last() {
            let split = split_tier(key, (other >> 32) as u32);
            if (other as u32) < row {
                left = split;
                break;
            }
            let t = &mut first[other as u32 as usize];
            *t = (*t).max(split);
            top -= 1;
        }
        first[row as usize] = left;
        sorted[top] = pair;
        top += 1;
    }
}

#[cfg(test)]
mod draw_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A 64-bit LCG (Knuth's MMIX constants), as the benches use.
    struct Lcg(u64);

    impl Lcg {
        fn unit(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// `scale_bench`'s walk: x sorted over `x0..x0 + 1000`, y a random walk.
    fn walk(n: usize, seed: u64) -> Vec<(f64, f64)> {
        let mut rng = Lcg(seed);
        let mut y = 0.0;
        (0..n)
            .map(|i| {
                y += rng.unit() - 0.5;
                (i as f64 * 1000.0 / n as f64, y)
            })
            .collect()
    }

    fn uniform(n: usize, seed: u64) -> Vec<(f64, f64)> {
        let mut rng = Lcg(seed);
        (0..n)
            .map(|_| (rng.unit() * 2.0 - 1.0, rng.unit() * 50.0))
            .collect()
    }

    /// A few tight clusters in a wide, mostly empty extent.
    fn clustered(n: usize, seed: u64) -> Vec<(f64, f64)> {
        let mut rng = Lcg(seed);
        let centres = [(-1e4, 3.0), (0.0, 0.0), (2.5e4, -7e3), (1e4, 1e4)];
        (0..n)
            .map(|i| {
                let (cx, cy) = centres[i % centres.len()];
                (cx + rng.unit() * 1e-3, cy + rng.unit() * 1e-2)
            })
            .collect()
    }

    fn build(rows: &[(f64, f64)]) -> Tiers {
        Tiers::build(rows.len() as u32, |r| rows[r as usize])
    }

    /// The cell of every finite row at tier 16, computed independently of
    /// the build's key path.
    fn cells(rows: &[(f64, f64)], t: &Tiers) -> Vec<Option<(u32, u32)>> {
        let finite = |&&(x, y): &&(f64, f64)| x.is_finite() && y.is_finite();
        let min =
            |f: fn(&(f64, f64)) -> f64| rows.iter().filter(finite).map(f).fold(f64::MAX, f64::min);
        let (x0, y0) = (min(|p| p.0), min(|p| p.1));
        let q = |v: f64, v0: f64, e: i32| {
            // 2^-e, split in two where it is not a normal f64.
            let (a, b) = inverse_pow2(e);
            let c = (v * a * b).floor() - (v0 * a * b).floor();
            assert!((0.0..SIDE).contains(&c), "cell {c} of {v} out of range");
            c as u32
        };
        rows.iter()
            .map(|p| finite(&p).then(|| (q(p.0, x0, t.ex), q(p.1, y0, t.ey))))
            .collect()
    }

    /// Brute force: the lowest row of each occupied cell of tier `t`.
    fn representatives(cells: &[Option<(u32, u32)>], t: u32) -> Vec<u32> {
        let mut lowest: HashMap<(u32, u32), u32> = HashMap::new();
        for (r, c) in cells.iter().enumerate() {
            if let Some((cx, cy)) = *c {
                let s = TMAX - t;
                lowest.entry((cx >> s, cy >> s)).or_insert(r as u32);
            }
        }
        let mut reps: Vec<u32> = lowest.into_values().collect();
        reps.sort_unstable();
        reps
    }

    /// AC1 and AC2 on `rows`: counts, nesting, brute-force equality at
    /// every tier, and the storage rule.
    fn check(name: &str, rows: &[(f64, f64)]) -> Tiers {
        let tiers = build(rows);
        let n = rows.len() as u32;
        assert_eq!(tiers.counts.len() as u32, tiers.stored + 1, "{name}");
        assert!(
            tiers.counts.is_sorted(),
            "{name}: counts {:?}",
            tiers.counts
        );
        assert_eq!(
            tiers.order.len() as u32,
            tiers.counts[tiers.stored as usize]
        );
        let mut distinct = tiers.order.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), tiers.order.len(), "{name}: rows repeat");
        let cells = cells(rows, &tiers);
        for t in 0..=TMAX {
            let reps = representatives(&cells, t);
            if t <= tiers.stored {
                let mut prefix = tiers.order[..tiers.counts[t as usize] as usize].to_vec();
                // Each tier block's runs are consecutive stretches of its
                // Morton order of the finest key (distinct: one
                // representative a tier-`t` cell), each spread.
                let start = if t == 0 {
                    0
                } else {
                    tiers.counts[t as usize - 1]
                };
                let key = |r: u32| {
                    let (cx, cy) = cells[r as usize].expect("a finite representative");
                    part1by1(cx) | part1by1(cy) << 1
                };
                let mut keys = Vec::new();
                for run in tiers
                    .runs
                    .iter()
                    .filter(|r| r.start >= start && r.end <= tiers.counts[t as usize])
                {
                    let drawn: Vec<u32> = tiers.order[run.start as usize..run.end as usize]
                        .iter()
                        .map(|&r| key(r))
                        .collect();
                    let mut sorted = drawn.clone();
                    sorted.sort_unstable();
                    // Position `i` holds Morton rank `reverse(i)`.
                    let n = sorted.len();
                    let bits = n.next_power_of_two().trailing_zeros();
                    let ranks: Vec<usize> = (0..n.next_power_of_two())
                        .map(|i| {
                            if bits == 0 {
                                0
                            } else {
                                i.reverse_bits() >> (usize::BITS - bits)
                            }
                        })
                        .filter(|&j| j < n)
                        .collect();
                    let want: Vec<u32> = ranks.iter().map(|&j| sorted[j]).collect();
                    assert_eq!(
                        drawn, want,
                        "{name}: tier {t} run {}.. is not spread",
                        run.start
                    );
                    keys.extend(sorted);
                }
                assert_eq!(
                    keys.len(),
                    prefix.len() - start as usize,
                    "{name}: tier {t} runs"
                );
                assert!(
                    keys.windows(2).all(|w| w[0] < w[1]),
                    "{name}: tier {t} block's runs are not Morton stretches"
                );
                prefix.sort_unstable();
                assert_eq!(prefix, reps, "{name}: tier {t}");
                assert!(
                    reps.len() as u32 <= n / 2 || t == 0,
                    "{name}: tier {t} stored"
                );
                assert_eq!(tiers.prefix(t), Some(reps.len() as u32));
            } else {
                assert_eq!(tiers.prefix(t), None, "{name}: tier {t}");
                if t == tiers.stored + 1 {
                    assert!(reps.len() as u32 > n / 2, "{name}: tier {t} not stored");
                }
            }
        }
        check_runs(name, rows, &tiers);
        tiers
    }

    /// The runs tile `order`, each within one tier block and at most
    /// `RUN_LEN` long, and each box is its representatives' exact box.
    fn check_runs(name: &str, rows: &[(f64, f64)], tiers: &Tiers) {
        let mut at = 0;
        for run in &tiers.runs {
            assert_eq!(run.start, at, "{name}: runs tile the order");
            assert!(run.end > run.start && run.end - run.start <= RUN_LEN);
            let block = tiers.counts.partition_point(|&c| c <= run.start);
            assert!(
                run.end <= tiers.counts[block],
                "{name}: a run crosses a block"
            );
            let reps = &tiers.order[run.start as usize..run.end as usize];
            let along = |f: fn(&(f64, f64)) -> f64| {
                let v = reps.iter().map(|&r| f(&rows[r as usize]));
                (
                    v.clone().fold(f64::INFINITY, f64::min),
                    v.fold(f64::NEG_INFINITY, f64::max),
                )
            };
            assert_eq!((run.x, run.y), (along(|p| p.0), along(|p| p.1)), "{name}");
            at = run.end;
        }
        assert_eq!(
            at as usize,
            tiers.order.len(),
            "{name}: runs cover the order"
        );
    }

    #[test]
    fn tiers_match_brute_force() {
        for (name, rows) in [
            ("walk", walk(100_000, 1)),
            ("uniform", uniform(100_000, 2)),
            ("clustered", clustered(100_000, 3)),
        ] {
            let tiers = check(name, &rows);
            assert_eq!(tiers.counts[0], 1, "{name}");
            assert!(tiers.stored > 0, "{name}: {:?}", tiers.counts);
        }
    }

    #[test]
    fn non_finite_rows_are_skipped() {
        let mut rows = walk(100_000, 4);
        for (i, row) in rows.iter_mut().enumerate() {
            match i % 7 {
                0 => row.0 = f64::NAN,
                3 => row.1 = f64::INFINITY,
                5 if i % 2 == 0 => row.0 = f64::NEG_INFINITY,
                _ => {}
            }
        }
        // The first row is not finite: the first representative is not 0.
        let tiers = check("walk with NaNs", &rows);
        assert_eq!(tiers.counts[0], 1);
        for &r in &tiers.order {
            let (x, y) = rows[r as usize];
            assert!(x.is_finite() && y.is_finite(), "row {r} is not finite");
        }
        let none = build(&[(f64::NAN, 0.0), (1.0, f64::NAN)]);
        assert_eq!(
            (none.stored, &none.counts[..], none.order.len()),
            (0, &[0][..], 0)
        );
        let empty = Tiers::build(0, |_| unreachable!());
        assert_eq!((empty.stored, &empty.counts[..]), (0, &[0][..]));
    }

    #[test]
    fn storage_rule() {
        // One row per cell of a 16 × 16 grid: tier 4 is every row, tier
        // 3 a quarter of them, so tiers 0-3 are stored.
        let grid: Vec<(f64, f64)> = (0..256)
            .map(|i| (f64::from(i % 16), f64::from(i / 16)))
            .collect();
        let tiers = check("16x16 grid", &grid);
        assert_eq!((tiers.ex, tiers.ey), (-12, -12));
        assert_eq!(tiers.counts, [1, 4, 16, 64]);
        assert_eq!(tiers.prefix(4), None);
        // One tier past the stored tiers may draw the finest stored tier;
        // no other tier (GUP-720).
        assert_eq!(tiers.cap(2), None);
        assert_eq!(tiers.cap(4), Some(64));
        assert_eq!(tiers.cap(5), None);
        // The estimate scales the representatives seen to the rows.
        let seen = in_view(&tiers.runs, 64, 256, &mut |_| 0.5);
        assert!((seen - 128.0).abs() < 1e-9, "{seen}");
        assert_eq!(in_view(&tiers.runs, 0, 256, &mut |_| 1.0), 0.0);
        // 1,000 rows, one per cell of a 32 × 32 grid: tier 4 (256) is the
        // finest at most half the rows.
        let grid: Vec<(f64, f64)> = (0..1000)
            .map(|i| (f64::from(i % 32), f64::from(i / 32)))
            .collect();
        let tiers = check("32x32 grid", &grid);
        assert_eq!(tiers.counts, [1, 4, 16, 64, 256]);
        // Coincident rows: one representative at every tier, so every
        // tier holds at most half the rows and all are stored.
        let same = vec![(3.5, -2.0); 1000];
        let tiers = check("coincident", &same);
        assert_eq!((tiers.stored, tiers.order.as_slice()), (TMAX, &[0][..]));
        assert_eq!(tiers.counts, [1; TMAX as usize + 1]);
        // A single row is tier 0 alone, more than half of one row.
        let one = check("one row", &[(1.0, 2.0)]);
        assert_eq!((one.stored, one.order.as_slice()), (0, &[0][..]));
        // Rows on one vertical line: the degenerate x axis takes y's cells.
        let line: Vec<(f64, f64)> = (0..1000).map(|i| (7.0, f64::from(i))).collect();
        let tiers = check("vertical line", &line);
        assert_eq!(tiers.ex, tiers.ey);
    }

    #[test]
    fn grid_is_anchored_in_data_units() {
        // x over 0..1000 (span 999.99): 2^-6 data units a cell is the
        // smallest power of two with fewer than 65,536 cells.
        let tiers = build(&walk(100_000, 5));
        assert_eq!(tiers.ex, -6);
        for (min, max) in [(0.0, 1.0), (-3.0, 5e9), (1e-9, 2e-9), (-1.0, 1.0)] {
            let e = exponent(min, max).unwrap();
            let span = |e: i32| {
                let s = 2f64.powi(-e);
                (max * s).floor() - (min * s).floor()
            };
            assert!(span(e) < SIDE && span(e - 1) >= SIDE, "{min}..{max}: {e}");
        }
        assert_eq!(exponent(2.0, 2.0), None);
    }

    #[test]
    fn grid_depends_on_the_chunk_alone() {
        // Chunk 0 of a layer of three chunks whose later chunks reach far
        // beyond chunk 0's extent, and of a layer of chunk 0 alone.
        let chunk = 40_000;
        let mut layer = walk(chunk, 6);
        layer.extend(
            uniform(chunk, 7)
                .into_iter()
                .map(|(x, y)| (x * 1e6, y - 1e5)),
        );
        layer.extend(clustered(chunk, 8));
        let alone = build(&layer[..chunk]);
        let first = Tiers::build(chunk as u32, |r| layer[r as usize]);
        assert_eq!(first, alone);
        let first_of_three = check("chunk 0 of 3", &layer[..chunk]);
        assert_eq!(first_of_three, alone);
        // Shifting the chunk by whole cells moves its origin (and its runs'
        // boxes), not its grid.
        let shift = 4096.0 * 2f64.powi(alone.ex);
        let shifted: Vec<(f64, f64)> = layer[..chunk]
            .iter()
            .map(|&(x, y)| (x + shift, y))
            .collect();
        let mut moved = alone.clone();
        for run in &mut moved.runs {
            run.x = (run.x.0 + shift, run.x.1 + shift);
        }
        assert_eq!(build(&shifted), moved);
    }

    #[test]
    fn extreme_magnitudes_fit() {
        // Subnormals (cells smaller than any normal f64), values near
        // f64::MAX (a span that overflows), and huge values a few ulps
        // apart: every extent fits 16 bits.
        let sub = f64::from_bits(1);
        let cases: [(&str, Vec<(f64, f64)>); 5] = [
            (
                "subnormal",
                (0..1000)
                    .map(|i| (sub * f64::from(i), -sub * f64::from(i % 17)))
                    .collect(),
            ),
            (
                "near MAX",
                (0..1000)
                    .map(|i| {
                        (
                            f64::MAX * (f64::from(i) / 500.0 - 1.0),
                            -f64::MAX + f64::from(i),
                        )
                    })
                    .collect(),
            ),
            (
                "1e300 ulps",
                (0..1000)
                    .map(|i| (f64::from_bits(1e300f64.to_bits() + i), 1e300))
                    .collect(),
            ),
            (
                // A few ulps apart: `v / 2^e` is past 2^62, the f64 path.
                "4 ulps of 1/7",
                (0..1000)
                    .map(|i| (f64::from_bits((1.0f64 / 7.0).to_bits() + i % 4), -0.5))
                    .collect(),
            ),
            (
                "mixed",
                (0..1000)
                    .map(|i| (if i % 2 == 0 { -sub } else { 1e300 }, f64::from(i)))
                    .collect(),
            ),
        ];
        let (lo, hi) = (1.0 / 7.0, f64::from_bits((1.0f64 / 7.0).to_bits() + 3));
        let slow = Axis::new(exponent(lo, hi).unwrap(), lo, hi);
        assert_eq!(slow.fast, None, "the f64 path");
        for (name, rows) in cases {
            let tiers = check(name, &rows);
            assert!(
                tiers.ex > -1200 && tiers.ex < 1100,
                "{name}: ex {}",
                tiers.ex
            );
        }
    }

    /// A sorted walk of `n` rows, shuffled (Fisher-Yates), as `lod_spike
    /// --shuffle` draws it: the chunk then spans the whole extent.
    fn shuffled(n: usize, seed: u64) -> Vec<(f64, f64)> {
        let mut rows = walk(n, seed);
        let mut rng = Lcg(seed ^ 0x9e37_79b9_7f4a_7c15);
        for i in (1..n).rev() {
            let j = (rng.unit() * (i + 1) as f64) as usize;
            rows.swap(i, j.min(i));
        }
        rows
    }

    /// A Morton run is compact: the median run's box is a small share of
    /// the chunk's extent, even when the chunk's rows are unsorted (AC1).
    #[test]
    fn runs_are_compact_in_a_shuffled_chunk() {
        let rows = shuffled(1_000_000, 9);
        let tiers = check("shuffled walk", &rows);
        let extent = |f: fn(&(f64, f64)) -> f64| {
            let v = rows.iter().map(f);
            v.clone().fold(f64::NEG_INFINITY, f64::max) - v.fold(f64::INFINITY, f64::min)
        };
        let area = extent(|p| p.0) * extent(|p| p.1);
        let mut shares: Vec<f64> = tiers
            .runs
            .iter()
            .map(|r| (r.x.1 - r.x.0) * (r.y.1 - r.y.0) / area)
            .collect();
        shares.sort_by(f64::total_cmp);
        let median = shares[shares.len() / 2];
        eprintln!(
            "{} runs over {} representatives; box share of the extent: median {:.4}%, \
             max {:.2}%",
            tiers.runs.len(),
            tiers.order.len(),
            100.0 * median,
            100.0 * shares[shares.len() - 1]
        );
        assert!(tiers.runs.len() > 50, "{} runs", tiers.runs.len());
        assert!(
            median < 0.03,
            "median run box {:.3}% of the extent",
            100.0 * median
        );
    }

    /// `ranges` keeps the runs `seen` keeps and nothing past the prefix,
    /// merges adjacent ones, and past `MAX_RANGES` merges across the
    /// narrowest gaps: checked against brute force.
    #[test]
    fn ranges_merge_the_seen_runs() {
        let runs: Vec<Run> = (0..1000u32)
            .map(|i| Run {
                start: 10 * i,
                end: 10 * i + 10,
                x: (f64::from(i), f64::from(i)),
                y: (0.0, 0.0),
            })
            .collect();
        let mut rng = Lcg(11);
        for (count, p) in [
            (10_000, 0.5),
            (5_000, 0.05),
            (10_000, 0.01),
            (2_000, 1.0),
            (10_000, 0.0),
        ] {
            let seen: Vec<bool> = (0..1000).map(|_| rng.unit() < p).collect();
            let got = ranges(&runs, count, &mut |r: &Run| seen[(r.start / 10) as usize]);
            // Brute force: the instances of the seen runs, merged.
            let want: Vec<u32> = (0..count).filter(|&i| seen[(i / 10) as usize]).collect();
            let mut exact: Vec<std::ops::Range<u32>> = Vec::new();
            for &i in &want {
                match exact.last_mut() {
                    Some(r) if r.end == i => r.end = i + 1,
                    _ => exact.push(i..i + 1),
                }
            }
            assert!(got.len() <= MAX_RANGES);
            assert!(got.iter().all(|r| r.end <= count && r.start < r.end));
            assert!(got.windows(2).all(|w| w[0].end < w[1].start), "{got:?}");
            if exact.len() <= MAX_RANGES {
                assert_eq!(got, exact, "count {count}, p {p}");
            } else {
                // Every seen instance is drawn, the ranges start and end on
                // seen runs, and every gap kept is wider than every gap
                // merged.
                for &i in &want {
                    assert!(got.iter().any(|r| r.contains(&i)), "{i} not drawn");
                }
                for r in &got {
                    assert!(seen[(r.start / 10) as usize] && seen[((r.end - 1) / 10) as usize]);
                }
                let gap = |w: &[std::ops::Range<u32>]| w[1].start - w[0].end;
                let kept_min = got.windows(2).map(gap).min().unwrap();
                let merged_max = exact
                    .windows(2)
                    .filter(|w| got.iter().any(|r| r.contains(&w[0].end)))
                    .map(gap)
                    .max()
                    .unwrap();
                assert!(kept_min > merged_max, "{kept_min} {merged_max}");
                assert!(got.len() > MAX_RANGES / 2, "{} ranges", got.len());
            }
        }
        assert!(ranges(&runs, 0, &mut |_: &Run| true).is_empty());
        let all = ranges(&runs, 10_000, &mut |_: &Run| true);
        assert_eq!((all.len(), &all[0]), (1, &(0..10_000)));
    }

    /// The GUP-631 spike's `View::tier` (`examples/lod_spike.rs`): the
    /// finest tier of a grid of `2^t × 2^t` cells over the layer's extent
    /// `ext` whose cells are at least one pixel in area, for domains `dom`
    /// drawn into a `w × h` plot rect.
    fn spike_tier(ext: [(f64, f64); 2], dom: [(f64, f64); 2], w: f64, h: f64) -> u32 {
        let ([(x0, x1), (y0, y1)], [(d0, d1), (e0, e1)]) = (ext, dom);
        let a0 = w * (x1 - x0) / (d1 - d0) * h * (y1 - y0) / (e1 - e0);
        (0.5 * a0.log2()).floor().clamp(0.0, 31.0) as u32
    }

    /// Pixels per data unit of `dom` drawn over `px` pixels.
    fn k(dom: (f64, f64), px: f64) -> f64 {
        px / (dom.1 - dom.0)
    }

    /// Where a chunk's grid is the spike's (its extent is exactly
    /// `65,536` cells of `2^ex × 2^ey`), [`tier`] is the spike's tier plus
    /// the bias, whatever the view.
    #[test]
    fn the_tier_is_the_spikes_plus_the_bias_on_the_same_grid() {
        let (w, h) = (1850.0, 1035.0);
        for (ex, ey) in [(-6, -5), (-9, -7), (0, 0), (-20, 3)] {
            let ext = [
                (0.0, 65_536.0 * 2f64.powi(ex)),
                (-1.0, -1.0 + 65_536.0 * 2f64.powi(ey)),
            ];
            for zoom in [1.04, 1.0, 0.5, 0.1, 1e-3, 3.0, 1e3] {
                let pad = |(a, b): (f64, f64)| {
                    let (m, r) = ((a + b) / 2.0, (b - a) / 2.0 * zoom);
                    (m - r, m + r)
                };
                let dom = [pad(ext[0]), pad(ext[1])];
                let spike = spike_tier(ext, dom, w, h);
                let ours = tier(ex, ey, k(dom[0], w), k(dom[1], h));
                let want = Some(spike + LOD_TIER_BIAS).filter(|&t| t <= TMAX);
                assert_eq!(ours, want, "{ex} {ey} {zoom}");
            }
        }
    }

    /// The spike's own example: the full view of its 10M-row walk at
    /// 1920 × 1080 (domains the extent padded 2%, the plot rect 1850 ×
    /// 1035, as `lod_spike` prints) is its tier 10. One chunk spanning that
    /// extent has cells of `2^-6 × 2^-5` data units (a grid of 1,024 ×
    /// 2,048 over 1,000 × 1,450.6: 1.45 times the area, a quarter tier
    /// coarser): its tier is also 10 before the bias, so it draws 11, the
    /// spike's "one tier finer".
    #[test]
    fn the_spikes_full_view_of_10m_rows_is_its_tier() {
        let ext = [
            (0.0, 999.9999),
            (-884.399_506_026_744_9, 566.157_731_420_119),
        ];
        let dom = [
            (-19.999_998, 1_019.999_898),
            (-913.410_650_775_682_1, 595.168_876_169_056_3),
        ];
        let (w, h) = (1850.0, 1035.0);
        assert_eq!(spike_tier(ext, dom, w, h), 10);
        let rows: Vec<(f64, f64)> = vec![(ext[0].0, ext[1].0), (ext[0].1, ext[1].1)];
        let tiers = Tiers::build(2, |r| rows[r as usize]);
        assert_eq!((tiers.ex, tiers.ey), (-6, -5));
        let (kx, ky) = (k(dom[0], w), k(dom[1], h));
        assert_eq!(tier(tiers.ex, tiers.ey, kx, ky), Some(11));
        assert_eq!(LOD_TIER_BIAS, 1);
    }

    /// A chunk without a span on one axis is a line of cells, and one
    /// without a span on either a point; a scale that is not finite draws
    /// every row.
    #[test]
    fn degenerate_chunks_and_scales() {
        // Cells of 2^-6 units at tier 16, 4 px a unit: 2^-4 px. Tier 12's
        // cells are one pixel long, so 13 with the bias.
        assert_eq!(tier(-6, -6, 4.0, 0.0), Some(13));
        assert_eq!(tier(-6, -6, 0.0, 4.0), Some(13));
        assert_eq!(tier(-6, -6, 0.0, 0.0), Some(LOD_TIER_BIAS));
        assert_eq!(tier(-6, -6, f64::INFINITY, 1.0), None);
        assert_eq!(tier(-6, -6, f64::NAN, 1.0), None);
        // Zoomed in past the finest tier: every row; out: tier 0.
        assert_eq!(tier(0, 0, 1e9, 1e9), None);
        assert_eq!(tier(0, 0, 1e-12, 1e-12), Some(0));
    }
}
