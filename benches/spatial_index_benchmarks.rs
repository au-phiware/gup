// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks comparing spatial index algorithms (GUP-078).
//!
//! Measures query performance across:
//! - Morton vs Hierarchical vs Linear Scan
//! - Uniform vs Clustered data distributions
//! - Point queries and region queries
//! - Various dataset sizes (1K to 100K elements)

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::spatial_index::{Aabb, ElementPosition, HierarchicalGrid, MortonIndex, SpatialQuery};
use std::hint::black_box;

/// Generate a uniform grid of elements.
fn uniform_elements(count: usize) -> (Vec<ElementPosition>, Aabb) {
    let side = (count as f32).sqrt().ceil() as usize;
    let spacing = 1000.0 / side as f32;
    let elements: Vec<ElementPosition> = (0..count)
        .map(|i| ElementPosition {
            position: [(i % side) as f32 * spacing, (i / side) as f32 * spacing],
            size: [5.0, 5.0],
            element_index: i as u32,
        })
        .collect();
    let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
    (elements, bounds)
}

/// Generate clustered elements (4 clusters in corners).
fn clustered_elements(count: usize) -> (Vec<ElementPosition>, Aabb) {
    let per_cluster = count / 4;
    let centres = [
        [100.0, 100.0],
        [900.0, 100.0],
        [100.0, 900.0],
        [900.0, 900.0],
    ];
    let mut elements = Vec::with_capacity(count);
    let mut id = 0u32;
    for centre in &centres {
        let side = (per_cluster as f32).sqrt().ceil() as usize;
        for j in 0..per_cluster {
            elements.push(ElementPosition {
                position: [
                    centre[0] + (j % side) as f32 * 0.5,
                    centre[1] + (j / side) as f32 * 0.5,
                ],
                size: [5.0, 5.0],
                element_index: id,
            });
            id += 1;
        }
    }
    let bounds = Aabb::new([0.0, 0.0], [1000.0, 1000.0]);
    (elements, bounds)
}

/// Linear scan baseline for point queries.
fn linear_scan_point(elements: &[ElementPosition], point: [f32; 2]) -> Vec<u32> {
    elements
        .iter()
        .filter(|e| {
            let b = Aabb::from_center_size(e.position, e.size);
            b.contains_point(point)
        })
        .map(|e| e.element_index)
        .collect()
}

/// Linear scan baseline for region queries.
fn linear_scan_region(elements: &[ElementPosition], region: &Aabb) -> Vec<u32> {
    elements
        .iter()
        .filter(|e| {
            let b = Aabb::from_center_size(e.position, e.size);
            b.intersects(region)
        })
        .map(|e| e.element_index)
        .collect()
}

fn bench_point_queries(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("point_query_uniform");

    for &size in &sizes {
        let (elements, bounds) = uniform_elements(size);
        let morton = MortonIndex::build(&elements, bounds);
        let hier = HierarchicalGrid::build(&elements, bounds);
        let query_point = [500.0, 500.0];

        group.bench_with_input(BenchmarkId::new("linear", size), &size, |b, _| {
            b.iter(|| black_box(linear_scan_point(&elements, query_point)))
        });
        group.bench_with_input(BenchmarkId::new("morton", size), &size, |b, _| {
            b.iter(|| black_box(morton.query_point(query_point)))
        });
        group.bench_with_input(BenchmarkId::new("hierarchical", size), &size, |b, _| {
            b.iter(|| black_box(hier.query_point(query_point)))
        });
    }
    group.finish();
}

fn bench_point_queries_clustered(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("point_query_clustered");

    for &size in &sizes {
        let (elements, bounds) = clustered_elements(size);
        let morton = MortonIndex::build(&elements, bounds);
        let hier = HierarchicalGrid::build(&elements, bounds);
        let query_point = [100.0, 100.0]; // In a cluster

        group.bench_with_input(BenchmarkId::new("linear", size), &size, |b, _| {
            b.iter(|| black_box(linear_scan_point(&elements, query_point)))
        });
        group.bench_with_input(BenchmarkId::new("morton", size), &size, |b, _| {
            b.iter(|| black_box(morton.query_point(query_point)))
        });
        group.bench_with_input(BenchmarkId::new("hierarchical", size), &size, |b, _| {
            b.iter(|| black_box(hier.query_point(query_point)))
        });
    }
    group.finish();
}

fn bench_region_queries(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("region_query_uniform");
    let region = Aabb::new([200.0, 200.0], [400.0, 400.0]);

    for &size in &sizes {
        let (elements, bounds) = uniform_elements(size);
        let morton = MortonIndex::build(&elements, bounds);
        let hier = HierarchicalGrid::build(&elements, bounds);

        group.bench_with_input(BenchmarkId::new("linear", size), &size, |b, _| {
            b.iter(|| black_box(linear_scan_region(&elements, &region)))
        });
        group.bench_with_input(BenchmarkId::new("morton", size), &size, |b, _| {
            b.iter(|| black_box(morton.query_region(&region)))
        });
        group.bench_with_input(BenchmarkId::new("hierarchical", size), &size, |b, _| {
            b.iter(|| black_box(hier.query_region(&region)))
        });
    }
    group.finish();
}

fn bench_build_time(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("build_time");

    for &size in &sizes {
        let (elements, bounds) = uniform_elements(size);

        group.bench_with_input(BenchmarkId::new("morton", size), &size, |b, _| {
            b.iter(|| black_box(MortonIndex::build(&elements, bounds)))
        });
        group.bench_with_input(BenchmarkId::new("hierarchical", size), &size, |b, _| {
            b.iter(|| black_box(HierarchicalGrid::build(&elements, bounds)))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_point_queries,
    bench_point_queries_clustered,
    bench_region_queries,
    bench_build_time,
);
criterion_main!(benches);
