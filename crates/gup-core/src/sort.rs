// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crate's few sorts (GUP-744). Each `sort*` call on another element
//! type, or with another comparator, compiles another whole sort (about
//! 4 KB of WASM, 1.5 KB gzipped), so the crate sorts through these: one
//! unstable sort of `u64`s, which `f64`s and small keys reach through
//! order-preserving bits, and one stable sort of indices.
//!
//! Each gives exactly the order of the sort it replaces: an unstable sort
//! only where equal elements are indistinguishable, and otherwise the
//! stable order, which any stable sort agrees on.

use std::cmp::Ordering;

/// `x`'s bits as a `u64` whose order is [`f64::total_cmp`]'s.
fn f64_key(x: f64) -> u64 {
    let bits = x.to_bits();
    if bits >> 63 == 1 {
        !bits
    } else {
        bits | 1 << 63
    }
}

/// The `f64` whose [`f64_key`] is `k`.
fn f64_of_key(k: u64) -> f64 {
    f64::from_bits(if k >> 63 == 1 { k & !(1 << 63) } else { !k })
}

/// Sort `v` ascending: the crate's one unstable sort.
#[inline(never)]
pub(crate) fn sort_u64(v: &mut [u64]) {
    v.sort_unstable();
}

/// Sort `v` by [`f64::total_cmp`]. Equal keys are equal bits, so this is
/// the order of a stable or an unstable sort by `total_cmp`.
pub(crate) fn sort_f64(v: &mut [f64]) {
    let mut keys: Vec<u64> = v.iter().map(|&x| f64_key(x)).collect();
    sort_u64(&mut keys);
    for (x, k) in v.iter_mut().zip(keys) {
        *x = f64_of_key(k);
    }
}

/// Sort `v` ascending and drop repeats.
pub(crate) fn sort_dedup_usize(v: &mut Vec<usize>) {
    let mut keys: Vec<u64> = v.iter().map(|&x| x as u64).collect();
    sort_u64(&mut keys);
    keys.dedup();
    v.clear();
    v.extend(keys.into_iter().map(|k| k as usize));
}

/// Sort indices `v` by `cmp`, keeping equal ones in their order: the
/// crate's one stable sort.
#[inline(never)]
pub(crate) fn sort_indices_by(v: &mut [usize], cmp: &mut dyn FnMut(usize, usize) -> Ordering) {
    v.sort_by(|&a, &b| cmp(a, b));
}

/// Sort `v` by `cmp`, keeping equal elements in their order, as
/// [`slice::sort_by`] does.
pub(crate) fn sort_by<T>(v: &mut Vec<T>, mut cmp: impl FnMut(&T, &T) -> Ordering) {
    let mut order: Vec<usize> = (0..v.len()).collect();
    sort_indices_by(&mut order, &mut |a, b| cmp(&v[a], &v[b]));
    let mut old: Vec<Option<T>> = v.drain(..).map(Some).collect();
    v.extend(order.into_iter().filter_map(|i| old[i].take()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_keys_order_as_total_cmp() {
        let values = [
            f64::NEG_INFINITY,
            -1.5,
            -0.0,
            0.0,
            f64::MIN_POSITIVE,
            2.0,
            f64::INFINITY,
            f64::NAN,
            -f64::NAN,
        ];
        for &a in &values {
            assert_eq!(f64_of_key(f64_key(a)).to_bits(), a.to_bits());
            for &b in &values {
                assert_eq!(f64_key(a).cmp(&f64_key(b)), a.total_cmp(&b), "{a} {b}");
            }
        }
        let mut v = values.to_vec();
        let mut want = v.clone();
        want.sort_by(f64::total_cmp);
        sort_f64(&mut v);
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&v), bits(&want));
    }

    #[test]
    fn sort_by_is_stable() {
        let mut v: Vec<(u8, usize)> = [3u8, 1, 3, 0, 1, 3].into_iter().zip(0..).collect();
        let mut want = v.clone();
        want.sort_by(|a, b| a.0.cmp(&b.0));
        sort_by(&mut v, |a, b| a.0.cmp(&b.0));
        assert_eq!(v, want);
    }

    #[test]
    fn sort_dedup_usize_sorts_and_drops_repeats() {
        let mut v = vec![5, 1, 5, usize::MAX, 0, 1];
        sort_dedup_usize(&mut v);
        assert_eq!(v, [0, 1, 5, usize::MAX]);
    }
}
