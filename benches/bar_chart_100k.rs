// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmark: 100 000-bar chart construction via `BarChartBuilder`.
//!
//! Measures the CPU-side cost of `build_with_data` for a dataset with
//! 100 000 rows spread across 1 000 categories and 1 series (simple),
//! 10 series (grouped), and 10 series (stacked).

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::AccessorFunction;
use gup::chart_builder::builders::bar::bar;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[derive(Debug, Clone)]
struct Row {
    category: String,
    value: f32,
    series: String,
}

/// Generate `n` rows spread across `cats` categories and `series` series.
fn generate_data(n: usize, cats: usize, series_count: usize) -> Vec<Row> {
    (0..n)
        .map(|i| Row {
            category: format!("cat_{}", i % cats),
            value: (i as f32 * 0.1) % 1000.0,
            series: format!("s_{}", i % series_count),
        })
        .collect()
}

fn x_acc() -> AccessorFunction<Row> {
    AccessorFunction::new(|d: &Row| AccessorValue::String(d.category.clone()))
}

fn y_acc() -> AccessorFunction<Row> {
    AccessorFunction::new(|d: &Row| AccessorValue::Float(d.value))
}

fn series_acc() -> AccessorFunction<Row> {
    AccessorFunction::new(|d: &Row| AccessorValue::String(d.series.clone()))
}

fn bench_bar_chart_build(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("bar_chart_build");

    // Simple — 100 000 rows, 1 000 categories, 1 series.
    let data_simple = generate_data(100_000, 1_000, 1);
    group.bench_with_input(
        BenchmarkId::new("simple", "100k"),
        &data_simple,
        |b, data| {
            b.iter(|| {
                let builder = bar().x(x_acc()).y(y_acc());
                let _ = builder.build_with_data(data.clone(), context.clone());
            })
        },
    );

    // Grouped — 100 000 rows, 1 000 categories, 10 series.
    let data_grouped = generate_data(100_000, 1_000, 10);
    group.bench_with_input(
        BenchmarkId::new("grouped", "100k"),
        &data_grouped,
        |b, data| {
            b.iter(|| {
                let builder = bar().x(x_acc()).y(y_acc()).group_by(series_acc());
                let _ = builder.build_with_data(data.clone(), context.clone());
            })
        },
    );

    // Stacked — 100 000 rows, 1 000 categories, 10 series.
    let data_stacked = generate_data(100_000, 1_000, 10);
    group.bench_with_input(
        BenchmarkId::new("stacked", "100k"),
        &data_stacked,
        |b, data| {
            b.iter(|| {
                let builder = bar().x(x_acc()).y(y_acc()).stack_by(series_acc());
                let _ = builder.build_with_data(data.clone(), context.clone());
            })
        },
    );

    group.finish();
}

criterion_group!(benches, bench_bar_chart_build);
criterion_main!(benches);
