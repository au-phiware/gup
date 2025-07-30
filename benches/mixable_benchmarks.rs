//! Performance benchmarks for the Mixable trait system.
//!
//! These benchmarks validate that composition adds <1% runtime overhead
//! compared to direct rendering as specified in the story requirements.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::{
    CrossFadeComposition, CustomCompositionBehavior, GupResult, Mixable, MixableExt, RenderContext,
};
use std::{hint::black_box, time::Duration};
use tokio::runtime::Builder;

/// Benchmark visualization that tracks render calls for performance measurement
#[derive(Debug, Clone)]
struct BenchmarkVisualization {
    id: usize,
    data_size: usize,
}

impl BenchmarkVisualization {
    fn new(id: usize, data_size: usize) -> Self {
        Self { id, data_size }
    }
}

impl Mixable for BenchmarkVisualization {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        // Simulate some work proportional to data size
        for i in 0..self.data_size {
            black_box(i * self.id);
        }
        Ok(())
    }
}

/// Direct rendering function for comparison baseline
fn render_direct(
    visualizations: &mut [BenchmarkVisualization],
    context: &mut RenderContext,
) -> GupResult<()> {
    for viz in visualizations {
        viz.render(context)?;
    }
    Ok(())
}

/// Benchmark single visualization rendering
fn bench_single_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_render");
    group.measurement_time(Duration::from_secs(10));

    for data_size in [100, 1000, 10000].iter() {
        let mut viz = BenchmarkVisualization::new(1, *data_size);
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        let mut context = rt.block_on(RenderContext::new()).unwrap();

        group.bench_with_input(BenchmarkId::new("direct", data_size), data_size, |b, _| {
            b.iter(|| {
                black_box(viz.render(&mut context)).unwrap();
            });
        });
    }
    group.finish();
}

/// Benchmark two-component composition vs direct rendering
fn bench_two_component_composition(c: &mut Criterion) {
    let mut group = c.benchmark_group("two_component");
    group.measurement_time(Duration::from_secs(10));

    for data_size in [100, 1000, 10000].iter() {
        let viz1 = BenchmarkVisualization::new(1, *data_size);
        let viz2 = BenchmarkVisualization::new(2, *data_size);
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        let mut context = rt.block_on(RenderContext::new()).unwrap();

        // Direct rendering baseline
        group.bench_with_input(BenchmarkId::new("direct", data_size), data_size, |b, _| {
            b.iter(|| {
                black_box(render_direct(
                    &mut [viz1.clone(), viz2.clone()],
                    &mut context,
                ))
                .unwrap();
            });
        });

        // Composed rendering
        let mut composed = viz1.clone().mix(viz2.clone());
        group.bench_with_input(
            BenchmarkId::new("composed", data_size),
            data_size,
            |b, _| {
                b.iter(|| {
                    black_box(composed.render(&mut context)).unwrap();
                });
            },
        );
    }
    group.finish();
}

/// Benchmark three-component composition vs direct rendering
fn bench_three_component_composition(c: &mut Criterion) {
    let mut group = c.benchmark_group("three_component");
    group.measurement_time(Duration::from_secs(10));

    for data_size in [100, 1000, 10000].iter() {
        let viz1 = BenchmarkVisualization::new(1, *data_size);
        let viz2 = BenchmarkVisualization::new(2, *data_size);
        let viz3 = BenchmarkVisualization::new(3, *data_size);
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        let mut context = rt.block_on(RenderContext::new()).unwrap();

        // Direct rendering baseline
        group.bench_with_input(BenchmarkId::new("direct", data_size), data_size, |b, _| {
            b.iter(|| {
                black_box(render_direct(
                    &mut [viz1.clone(), viz2.clone(), viz3.clone()],
                    &mut context,
                ))
                .unwrap();
            });
        });

        // Composed rendering
        let mut composed = viz1.clone().mix(viz2.clone()).mix(viz3.clone());
        group.bench_with_input(
            BenchmarkId::new("composed", data_size),
            data_size,
            |b, _| {
                b.iter(|| {
                    black_box(composed.render(&mut context)).unwrap();
                });
            },
        );
    }
    group.finish();
}

/// Benchmark deep composition chains
fn bench_deep_composition(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_composition");
    group.measurement_time(Duration::from_secs(15));

    let data_size = 1000;
    let rt = Builder::new_current_thread().enable_all().build().unwrap();
    let mut context = rt.block_on(RenderContext::new()).unwrap();

    for chain_length in [2, 4, 8].iter() {
        // Create visualizations for direct rendering
        let vizs: Vec<_> = (0..*chain_length)
            .map(|i| BenchmarkVisualization::new(i, data_size))
            .collect();

        // Direct rendering baseline
        group.bench_with_input(
            BenchmarkId::new("direct", chain_length),
            chain_length,
            |b, _| {
                b.iter(|| {
                    black_box(render_direct(&mut vizs.clone(), &mut context)).unwrap();
                });
            },
        );

        // Build composition chain manually for different lengths
        match *chain_length {
            2 => {
                let mut composed = vizs[0].clone().mix(vizs[1].clone());
                group.bench_with_input(
                    BenchmarkId::new("composed", chain_length),
                    chain_length,
                    |b, _| {
                        b.iter(|| {
                            black_box(composed.render(&mut context)).unwrap();
                        });
                    },
                );
            }
            4 => {
                let mut composed = vizs[0]
                    .clone()
                    .mix(vizs[1].clone())
                    .mix(vizs[2].clone())
                    .mix(vizs[3].clone());
                group.bench_with_input(
                    BenchmarkId::new("composed", chain_length),
                    chain_length,
                    |b, _| {
                        b.iter(|| {
                            black_box(composed.render(&mut context)).unwrap();
                        });
                    },
                );
            }
            8 => {
                let mut composed = vizs[0]
                    .clone()
                    .mix(vizs[1].clone())
                    .mix(vizs[2].clone())
                    .mix(vizs[3].clone())
                    .mix(vizs[4].clone())
                    .mix(vizs[5].clone())
                    .mix(vizs[6].clone())
                    .mix(vizs[7].clone());
                group.bench_with_input(
                    BenchmarkId::new("composed", chain_length),
                    chain_length,
                    |b, _| {
                        b.iter(|| {
                            black_box(composed.render(&mut context)).unwrap();
                        });
                    },
                );
            }
            _ => {}
        }
    }
    group.finish();
}

/// Benchmark composition creation overhead
fn bench_composition_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("composition_creation");
    group.measurement_time(Duration::from_secs(5));

    let viz1 = BenchmarkVisualization::new(1, 1000);
    let viz2 = BenchmarkVisualization::new(2, 1000);

    group.bench_function("create_composition", |b| {
        b.iter(|| {
            black_box(viz1.clone().mix(viz2.clone()));
        });
    });

    group.bench_function("create_vector", |b| {
        b.iter(|| {
            black_box(vec![viz1.clone(), viz2.clone()]);
        });
    });

    group.finish();
}

/// Benchmark different composition modes
fn bench_composition_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("composition_modes");
    group.measurement_time(Duration::from_secs(10));

    let viz1 = BenchmarkVisualization::new(1, 1000);
    let viz2 = BenchmarkVisualization::new(2, 1000);
    let rt = Builder::new_current_thread().enable_all().build().unwrap();
    let mut context = rt.block_on(RenderContext::new()).unwrap();

    let mut overlay = viz1.clone().overlay(viz2.clone());
    let mut merge = viz1.clone().merge(viz2.clone());
    let mut beside = viz1.clone().beside(viz2.clone());
    let custom_behavior =
        CustomCompositionBehavior::CrossFade(CrossFadeComposition { fade_factor: 0.5 });
    let mut custom = viz1.custom_compose(viz2, custom_behavior);

    group.bench_function("overlay", |b| {
        b.iter(|| {
            black_box(overlay.render(&mut context)).unwrap();
        });
    });

    group.bench_function("merge", |b| {
        b.iter(|| {
            black_box(merge.render(&mut context)).unwrap();
        });
    });

    group.bench_function("beside", |b| {
        b.iter(|| {
            black_box(beside.render(&mut context)).unwrap();
        });
    });

    group.bench_function("custom", |b| {
        b.iter(|| {
            black_box(custom.render(&mut context)).unwrap();
        });
    });

    group.finish();
}

/// Benchmark validation overhead
fn bench_validation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");
    group.measurement_time(Duration::from_secs(5));

    let viz1 = BenchmarkVisualization::new(1, 1000);
    let viz2 = BenchmarkVisualization::new(2, 1000);
    let composed = viz1.mix(viz2);

    group.bench_function("is_valid", |b| {
        b.iter(|| {
            black_box(composed.is_valid());
        });
    });

    group.bench_function("description", |b| {
        b.iter(|| {
            black_box(composed.description());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_render,
    bench_two_component_composition,
    bench_three_component_composition,
    bench_deep_composition,
    bench_composition_creation,
    bench_composition_modes,
    bench_validation_overhead
);
criterion_main!(benches);
