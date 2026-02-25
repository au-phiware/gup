// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Z-order curve (Morton order) spatial index.
//!
//! Morton encoding interleaves the bits of the x and y coordinates to produce
//! a single key that preserves 2D spatial locality. Elements are sorted by
//! Morton key, enabling efficient range queries via binary search.
//!
//! This implementation normalises world coordinates into a 16-bit grid
//! (65 536 × 65 536), yielding 32-bit Morton keys. This is more than
//! sufficient for sub-pixel precision in visualisation contexts while keeping
//! keys compact and sort-friendly.
//!
//! The index returns *candidate* element indices; the caller is responsible
//! for precise hit testing against element bounds. This keeps memory usage
//! minimal (8 bytes per element: key + index).

use super::{Aabb, ElementPosition, SpatialQuery};

/// A single entry in the sorted Morton key array.
///
/// This struct is GPU-compatible (`repr(C)` + `bytemuck::Pod`) so it can be
/// uploaded directly to a storage buffer for GPU-side binary search.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MortonEntry {
    /// 32-bit Z-order key.
    pub key: MortonKey,
    /// Original element index.
    pub element_index: u32,
}

/// A 32-bit Morton key encoding 2D position.
#[repr(C)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck::Pod, bytemuck::Zeroable,
)]
pub struct MortonKey(pub u32);

impl MortonKey {
    /// Encode 16-bit x, y coordinates into a 32-bit Morton key.
    pub fn encode(x: u16, y: u16) -> Self {
        Self(interleave(x as u32) | (interleave(y as u32) << 1))
    }

    /// Decode a Morton key back to 16-bit x, y coordinates.
    pub fn decode(self) -> (u16, u16) {
        let x = deinterleave(self.0) as u16;
        let y = deinterleave(self.0 >> 1) as u16;
        (x, y)
    }
}

/// Spread the lower 16 bits of `v` into even bit positions of a 32-bit value.
fn interleave(mut v: u32) -> u32 {
    // Magic-number bit-spreading from Sean Eron Anderson's Bit Twiddling Hacks
    v &= 0x0000_FFFF;
    v = (v | (v << 8)) & 0x00FF_00FF;
    v = (v | (v << 4)) & 0x0F0F_0F0F;
    v = (v | (v << 2)) & 0x3333_3333;
    v = (v | (v << 1)) & 0x5555_5555;
    v
}

/// Compact every other bit of `v` into the lower 16 bits.
fn deinterleave(mut v: u32) -> u32 {
    v &= 0x5555_5555;
    v = (v | (v >> 1)) & 0x3333_3333;
    v = (v | (v >> 2)) & 0x0F0F_0F0F;
    v = (v | (v >> 4)) & 0x00FF_00FF;
    v = (v | (v >> 8)) & 0x0000_FFFF;
    v
}

/// Spatial index based on Z-order curve sorting.
///
/// Entries are sorted by Morton key. Queries use binary search to find a
/// key range and return *candidate* element indices. The caller must perform
/// precise hit testing. This keeps index memory at 8 bytes per element.
pub struct MortonIndex {
    /// Sorted array of (Morton key, element index).
    entries: Vec<MortonEntry>,
    /// World-space bounds used for normalisation.
    bounds: Aabb,
    /// Number of indexed elements.
    count: usize,
    /// Inverse world dimensions (precomputed for fast normalisation).
    inv_width: f32,
    inv_height: f32,
}

impl MortonIndex {
    /// Build a Morton index from a set of elements.
    pub fn build(elements: &[ElementPosition], bounds: Aabb) -> Self {
        let w = bounds.width().max(f32::EPSILON);
        let h = bounds.height().max(f32::EPSILON);
        let inv_width = 1.0 / w;
        let inv_height = 1.0 / h;

        let mut entries: Vec<MortonEntry> = elements
            .iter()
            .map(|elem| {
                let key = world_to_morton(elem.position, &bounds, inv_width, inv_height);
                MortonEntry {
                    key,
                    element_index: elem.element_index,
                }
            })
            .collect();

        entries.sort_unstable_by_key(|e| e.key);

        let count = entries.len();
        Self {
            entries,
            bounds,
            count,
            inv_width,
            inv_height,
        }
    }

    /// Find the range of entries whose Morton keys fall within `[lo, hi]`.
    ///
    /// Returns `(start, end)` indices into `self.entries` (exclusive end).
    fn key_range(&self, lo: MortonKey, hi: MortonKey) -> (usize, usize) {
        let start = self.entries.partition_point(|e| e.key < lo);
        let end = self.entries.partition_point(|e| e.key <= hi);
        (start, end)
    }

    /// Get the sorted entries (for benchmarking / debugging).
    pub fn entries(&self) -> &[MortonEntry] {
        &self.entries
    }

    /// Get the world-space bounds used for normalisation.
    pub fn bounds(&self) -> &Aabb {
        &self.bounds
    }

    /// Get the inverse width used for normalisation.
    pub fn inv_width(&self) -> f32 {
        self.inv_width
    }

    /// Get the inverse height used for normalisation.
    pub fn inv_height(&self) -> f32 {
        self.inv_height
    }
}

/// Convert a world-space position to a Morton key.
pub fn world_to_morton(point: [f32; 2], bounds: &Aabb, inv_w: f32, inv_h: f32) -> MortonKey {
    let nx = ((point[0] - bounds.min[0]) * inv_w).clamp(0.0, 1.0 - f32::EPSILON);
    let ny = ((point[1] - bounds.min[1]) * inv_h).clamp(0.0, 1.0 - f32::EPSILON);
    let gx = (nx * 65536.0) as u16;
    let gy = (ny * 65536.0) as u16;
    MortonKey::encode(gx, gy)
}

impl SpatialQuery for MortonIndex {
    fn query_point(&self, point: [f32; 2]) -> Vec<u32> {
        // For a point query, search a neighbourhood around the point key.
        // We use a generous radius in grid units to account for element sizes.
        let key = world_to_morton(point, &self.bounds, self.inv_width, self.inv_height);
        let (px, py) = key.decode();

        // Radius of 512 grid cells ≈ 0.8% of world extent, generous enough
        // for most element sizes relative to the visualisation area.
        let radius = 512u16;
        let lo = MortonKey::encode(px.saturating_sub(radius), py.saturating_sub(radius));
        let hi = MortonKey::encode(px.saturating_add(radius), py.saturating_add(radius));

        let (start, end) = self.key_range(lo, hi);
        self.entries[start..end]
            .iter()
            .map(|e| e.element_index)
            .collect()
    }

    fn query_region(&self, region: &Aabb) -> Vec<u32> {
        // Convert region bounds to Morton keys and search the range.
        // The Z-curve is space-filling but not axis-aligned, so this range
        // may include false positives. The caller must filter precisely.
        let lo = world_to_morton(region.min, &self.bounds, self.inv_width, self.inv_height);
        let hi = world_to_morton(region.max, &self.bounds, self.inv_width, self.inv_height);

        let (start, end) = self.key_range(lo, hi);
        self.entries[start..end]
            .iter()
            .map(|e| e.element_index)
            .collect()
    }

    fn element_count(&self) -> usize {
        self.count
    }

    fn memory_usage_bytes(&self) -> usize {
        self.entries.capacity() * std::mem::size_of::<MortonEntry>() + std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morton_encode_decode_roundtrip() {
        for x in [0u16, 1, 100, 255, 1000, 65535] {
            for y in [0u16, 1, 100, 255, 1000, 65535] {
                let key = MortonKey::encode(x, y);
                let (dx, dy) = key.decode();
                assert_eq!((dx, dy), (x, y), "Roundtrip failed for ({x}, {y})");
            }
        }
    }

    #[test]
    fn test_morton_spatial_locality() {
        // Points close in 2D should have Morton keys close in value
        let a = MortonKey::encode(100, 100);
        let b = MortonKey::encode(101, 100);
        let c = MortonKey::encode(200, 200);

        let diff_ab = (a.0 as i64 - b.0 as i64).unsigned_abs();
        let diff_ac = (a.0 as i64 - c.0 as i64).unsigned_abs();
        assert!(diff_ab < diff_ac, "Near points should have closer keys");
    }

    #[test]
    fn test_morton_ordering() {
        // The Z-order curve should place (0,0) < (1,0) < (0,1) < (1,1)
        let k00 = MortonKey::encode(0, 0);
        let k10 = MortonKey::encode(1, 0);
        let k01 = MortonKey::encode(0, 1);
        let k11 = MortonKey::encode(1, 1);
        assert!(k00 < k10);
        assert!(k10 < k01);
        assert!(k01 < k11);
    }

    #[test]
    fn test_morton_index_build_and_query_point() {
        let elements: Vec<ElementPosition> = (0..100)
            .map(|i| {
                let t = i as f32 / 100.0;
                ElementPosition {
                    position: [t * 100.0, t * 100.0],
                    size: [10.0, 10.0],
                    element_index: i as u32,
                }
            })
            .collect();

        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let index = MortonIndex::build(&elements, bounds);

        // Query at (50,50) should return candidates near that position
        let candidates = index.query_point([50.0, 50.0]);
        assert!(
            !candidates.is_empty(),
            "Should find candidates near (50,50)"
        );

        // Query far from any element should return empty
        let far_candidates = index.query_point([500.0, 500.0]);
        assert!(
            far_candidates.is_empty(),
            "Should find no candidates far from data"
        );
    }

    #[test]
    fn test_morton_index_region_query() {
        let elements: Vec<ElementPosition> = (0..100)
            .map(|i| {
                let t = i as f32 / 100.0;
                ElementPosition {
                    position: [t * 100.0, t * 100.0],
                    size: [2.0, 2.0],
                    element_index: i as u32,
                }
            })
            .collect();

        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let index = MortonIndex::build(&elements, bounds);

        let region = Aabb::new([20.0, 20.0], [40.0, 40.0]);
        let candidates = index.query_region(&region);
        assert!(
            !candidates.is_empty(),
            "Region query should find candidates"
        );
    }

    #[test]
    fn test_morton_index_sorted() {
        let elements: Vec<ElementPosition> = (0..50)
            .map(|i| ElementPosition {
                position: [i as f32 * 20.0, i as f32 * 10.0],
                size: [5.0, 5.0],
                element_index: i as u32,
            })
            .collect();

        let bounds = Aabb::new([0.0, 0.0], [1000.0, 500.0]);
        let index = MortonIndex::build(&elements, bounds);

        for window in index.entries().windows(2) {
            assert!(
                window[0].key <= window[1].key,
                "Entries should be sorted by Morton key"
            );
        }
    }

    #[test]
    fn test_morton_index_empty() {
        let bounds = Aabb::new([0.0, 0.0], [100.0, 100.0]);
        let index = MortonIndex::build(&[], bounds);
        assert_eq!(index.element_count(), 0);
        assert!(index.query_point([50.0, 50.0]).is_empty());
        assert!(index.query_region(&bounds).is_empty());
    }

    #[test]
    fn test_morton_index_memory_overhead() {
        let elements: Vec<ElementPosition> = (0..10_000)
            .map(|i| ElementPosition {
                position: [(i % 100) as f32 * 10.0, (i / 100) as f32 * 10.0],
                size: [5.0, 5.0],
                element_index: i as u32,
            })
            .collect();

        let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
        let index = MortonIndex::build(&elements, bounds);

        // MortonEntry is 8 bytes (key + index). For 10K elements: 80 KB.
        // Source ElementData is 32 bytes × 10K = 320 KB.
        // Index overhead ratio: 80/320 = 25%.
        let source_data_bytes = elements.len() * 32; // ElementData size
        let overhead_pct = index.memory_usage_bytes() as f64 / source_data_bytes as f64 * 100.0;
        println!(
            "Morton: {:.0} KB for {} elements, {:.1}% of source data",
            index.memory_usage_bytes() as f64 / 1024.0,
            elements.len(),
            overhead_pct,
        );
        // Morton stores 8 bytes/element, which is 25% of 32-byte ElementData.
        assert!(overhead_pct < 50.0, "Morton overhead should be < 50%");
    }
}
