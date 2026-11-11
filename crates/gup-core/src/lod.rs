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
//! `first_tier` and then by row: tier `t` is the prefix
//! `order[..counts[t]]`.
//!
//! Tiers are stored while they hold at most half the chunk's rows; past
//! the finest stored tier a chunk draws every row ([`Tiers::prefix`] is
//! `None`). Tier 0 is always stored.
//!
//! Nothing draws from the tiers yet: uploads are GUP-715's, the per-frame
//! tier choice GUP-716's.

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

/// The tiers of one chunk: see the [module docs](self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tiers {
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
                ex: 0,
                ey: 0,
                stored: 0,
                order: Vec::new(),
                counts: vec![0],
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

        // The progressive order: a stable counting sort by first tier.
        let mut next: Vec<u32> = std::iter::once(0).chain(counts.iter().copied()).collect();
        let mut order = vec![0u32; counts[stored as usize] as usize];
        for (r, &t) in (0..rows).zip(&first) {
            if u32::from(t) <= stored {
                let at = &mut next[t as usize];
                order[*at as usize] = r;
                *at += 1;
            }
        }
        Tiers {
            ex,
            ey,
            stored,
            order,
            counts,
        }
    }

    /// The representatives of tier `t`, the length of the prefix of
    /// [`order`](Self::order) to draw, or `None` past the stored tiers
    /// (draw every row).
    pub fn prefix(&self, t: u32) -> Option<u32> {
        self.counts.get(t as usize).copied()
    }
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
                // Each tier block is in row order.
                let start = if t == 0 {
                    0
                } else {
                    tiers.counts[t as usize - 1]
                };
                assert!(
                    prefix[start as usize..].is_sorted(),
                    "{name}: tier {t} block"
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
        tiers
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
        // Shifting the chunk by whole cells moves its origin, not its grid.
        let cell = 2f64.powi(alone.ex);
        let shifted: Vec<(f64, f64)> = layer[..chunk]
            .iter()
            .map(|&(x, y)| (x + 4096.0 * cell, y))
            .collect();
        assert_eq!(build(&shifted), alone);
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
}
