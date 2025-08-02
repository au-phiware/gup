// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Performance benchmarks for GPU buffer management system.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::{BufferPool, BufferType, GpuBuffer, RenderContext};
use std::hint::black_box;
use std::sync::Arc;
use tokio::runtime::Runtime;

// Note: Buffer upload benchmarks disabled due to GPU driver race conditions in tight benchmark loops.
// GPU data upload operations in high-frequency criterion benchmarks cause segfaults even with
// synchronization attempts. Upload functionality is thoroughly tested in integration tests.

fn bench_buffer_pool(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { RenderContext::new().await.unwrap() });
    let device = Arc::new(context.device().clone());

    let mut group = c.benchmark_group("buffer_pool");

    for size in [100, 1_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::new("pool_allocation", size),
            size,
            |b, &_size| {
                let mut pool = BufferPool::new(device.clone());
                b.iter(|| {
                    let buffer = black_box(pool.allocate::<f32>(BufferType::Vertex, *size));
                    pool.deallocate(buffer);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("direct_allocation", size),
            size,
            |b, &_size| {
                b.iter(|| {
                    let _buffer = black_box(GpuBuffer::<f32>::new(
                        context.device(),
                        BufferType::Vertex,
                        *size,
                    ));
                });
            },
        );
    }

    group.finish();
}

fn bench_buffer_memory_efficiency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { RenderContext::new().await.unwrap() });
    let device = Arc::new(context.device().clone());

    c.bench_function("buffer_pool_efficiency", |b| {
        b.iter(|| {
            let mut pool = BufferPool::new(device.clone());

            // Allocate and deallocate many buffers
            let mut buffers = Vec::new();
            for _ in 0..100 {
                buffers.push(pool.allocate::<f32>(BufferType::Vertex, 1000));
            }

            for buffer in buffers {
                pool.deallocate(buffer);
            }

            // Check pool efficiency
            let stats = pool.get_stats();
            black_box(stats.pool_efficiency());
        });
    });
}

criterion_group!(benches, bench_buffer_pool, bench_buffer_memory_efficiency);
criterion_main!(benches);
