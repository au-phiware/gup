// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native benchmark runner that outputs JSON results.
//!
//! This binary runs the same interaction benchmarks that the WASM runner
//! executes in-browser, producing a JSON file in the [`BenchSuite`](gup::wasm_bench::BenchSuite) format.
//! The output can then be compared with WASM benchmark results using the
//! `benchmark_comparison.sh` script.
//!
//! # Usage
//!
//! ```bash
//! cargo run --release --bin wasm_bench_native > native_results.json
//! ```

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let json = gup::wasm_bench_interaction::run_native_benchmarks();
        println!("{json}");
    }
    #[cfg(target_arch = "wasm32")]
    {
        eprintln!("This binary is not supported on wasm32 targets.");
        std::process::exit(1);
    }
}
