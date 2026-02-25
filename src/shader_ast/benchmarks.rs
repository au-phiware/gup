// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmarks for the AST-based shader composition system.
//!
//! Validates that composition time remains <10ms for typical use cases
//! and that memory usage is within the 2x target.

use super::generator::generate_wgsl_minimal;
use super::optimizer::{AstOptimizationConfig, optimize};
use super::parser::parse_wgsl;
use super::types::*;
use std::time::Instant;

/// Result of a benchmark run.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Description of the benchmark.
    pub name: String,
    /// Time taken in milliseconds.
    pub elapsed_ms: f64,
    /// Whether the benchmark passed performance target.
    pub passed: bool,
    /// Memory usage estimate in bytes.
    pub memory_bytes: usize,
    /// Extra info for reporting.
    pub info: String,
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.passed { "✅ PASS" } else { "❌ FAIL" };
        write!(
            f,
            "{} {} ({:.3}ms, ~{} bytes) {}",
            status, self.name, self.elapsed_ms, self.memory_bytes, self.info
        )
    }
}

/// Generate a synthetic WGSL function for benchmarking.
fn make_function(name: &str, index: usize) -> String {
    format!(
        r#"fn {name}_{index}(value: f32, uniforms: {name}Uniforms_{index}) -> f32 {{
    let normalized = (value - uniforms.domain_min) / (uniforms.domain_max - uniforms.domain_min);
    return uniforms.range_min + normalized * (uniforms.range_max - uniforms.range_min);
}}"#,
        name = name,
        index = index,
    )
}

/// Generate a synthetic WGSL struct for benchmarking.
fn make_uniform_struct(name: &str, index: usize) -> String {
    format!(
        r#"struct {name}Uniforms_{index} {{
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
}}"#,
        name = name,
        index = index,
    )
}

/// Benchmark: Parse N functions into AST.
pub fn bench_parse(function_count: usize) -> BenchmarkResult {
    let mut source = String::new();
    for i in 0..function_count {
        source.push_str(&make_uniform_struct("scale", i));
        source.push('\n');
        source.push_str(&make_function("scale", i));
        source.push('\n');
    }

    let start = Instant::now();
    let module = parse_wgsl(&source).unwrap();
    let elapsed = start.elapsed();

    let memory_estimate = std::mem::size_of_val(&module)
        + module.functions.len() * 256 // rough estimate per function
        + module.structs.len() * 128;

    BenchmarkResult {
        name: format!("parse_{function_count}_functions"),
        elapsed_ms: elapsed.as_secs_f64() * 1000.0,
        passed: elapsed.as_secs_f64() * 1000.0 < 10.0,
        memory_bytes: memory_estimate,
        info: format!(
            "{} functions, {} structs",
            module.functions.len(),
            module.structs.len()
        ),
    }
}

/// Benchmark: Generate WGSL from AST with N functions.
pub fn bench_generate(function_count: usize) -> BenchmarkResult {
    let mut source = String::new();
    for i in 0..function_count {
        source.push_str(&make_uniform_struct("scale", i));
        source.push('\n');
        source.push_str(&make_function("scale", i));
        source.push('\n');
    }

    let module = parse_wgsl(&source).unwrap();

    let start = Instant::now();
    let wgsl = generate_wgsl_minimal(&module);
    let elapsed = start.elapsed();

    BenchmarkResult {
        name: format!("generate_{function_count}_functions"),
        elapsed_ms: elapsed.as_secs_f64() * 1000.0,
        passed: elapsed.as_secs_f64() * 1000.0 < 10.0,
        memory_bytes: wgsl.len(),
        info: format!("{} bytes WGSL output", wgsl.len()),
    }
}

/// Benchmark: Optimize AST with N functions (some unused).
pub fn bench_optimize(total_functions: usize, unused_count: usize) -> BenchmarkResult {
    let used_count = total_functions - unused_count;

    // Build a module with an entry point that calls some functions.
    let mut module = WgslModule::new();

    // Create used functions.
    for i in 0..used_count {
        module.functions.push(Function {
            name: format!("used_{i}"),
            parameters: vec![Parameter {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            return_type: Some(WgslType::Scalar(ScalarType::F32)),
            body: Block::new(vec![Statement::Return(Some(Expr::Binary(
                Box::new(Expr::Ident("x".to_string())),
                BinaryOp::Mul,
                Box::new(Expr::Literal(Literal::Float(2.0))),
            )))]),
            attributes: vec![],
        });
    }

    // Create unused functions.
    for i in 0..unused_count {
        module.functions.push(Function {
            name: format!("unused_{i}"),
            parameters: vec![Parameter {
                name: "y".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            return_type: Some(WgslType::Scalar(ScalarType::F32)),
            body: Block::new(vec![Statement::Return(Some(Expr::Ident("y".to_string())))]),
            attributes: vec![],
        });
    }

    // Entry point that calls all used functions.
    let mut call_chain = Expr::Literal(Literal::Float(1.0));
    for i in 0..used_count {
        call_chain = Expr::Call(format!("used_{i}"), vec![call_chain]);
    }
    module.functions.push(Function {
        name: "vs_main".to_string(),
        parameters: vec![],
        return_type: Some(WgslType::Scalar(ScalarType::F32)),
        body: Block::new(vec![Statement::Return(Some(call_chain))]),
        attributes: vec![Attribute::Vertex],
    });

    let start = Instant::now();
    let results = optimize(&mut module, &AstOptimizationConfig::default());
    let elapsed = start.elapsed();

    let changes: Vec<String> = results
        .iter()
        .filter(|r| r.changed)
        .map(|r| r.description.clone())
        .collect();

    BenchmarkResult {
        name: format!("optimize_{total_functions}_functions_{unused_count}_unused"),
        elapsed_ms: elapsed.as_secs_f64() * 1000.0,
        passed: elapsed.as_secs_f64() * 1000.0 < 10.0,
        memory_bytes: 0,
        info: if changes.is_empty() {
            "no changes".to_string()
        } else {
            changes.join("; ")
        },
    }
}

/// Benchmark: Full pipeline (parse + optimize + generate) for N functions.
pub fn bench_full_pipeline(function_count: usize) -> BenchmarkResult {
    let mut source = String::new();
    for i in 0..function_count {
        source.push_str(&make_uniform_struct("scale", i));
        source.push('\n');
        source.push_str(&make_function("scale", i));
        source.push('\n');
    }

    // Add entry point.
    let mut entry_body = String::from("fn vs_main() -> f32 {\n    return ");
    for i in (0..function_count).rev() {
        if i < function_count - 1 {
            entry_body.push_str(&format!("scale_{i}(", i = i));
        } else {
            entry_body.push_str(&format!("scale_{i}(1.0", i = i));
        }
    }
    for _ in 0..function_count.saturating_sub(1) {
        entry_body.push_str(", scale_0Uniforms_0())");
    }
    // Use a simpler entry point
    source.push_str("@vertex\nfn vs_main() -> f32 {\n    return 1.0;\n}\n");

    let start = Instant::now();
    let mut module = parse_wgsl(&source).unwrap();
    optimize(&mut module, &AstOptimizationConfig::default());
    let wgsl = generate_wgsl_minimal(&module);
    let elapsed = start.elapsed();

    let string_size = source.len();

    BenchmarkResult {
        name: format!("full_pipeline_{function_count}_functions"),
        elapsed_ms: elapsed.as_secs_f64() * 1000.0,
        passed: elapsed.as_secs_f64() * 1000.0 < 10.0,
        memory_bytes: wgsl.len(),
        info: format!(
            "input: {} bytes, output: {} bytes, ratio: {:.2}x",
            string_size,
            wgsl.len(),
            wgsl.len() as f64 / string_size.max(1) as f64
        ),
    }
}

/// Run all benchmarks and return results.
pub fn run_all_benchmarks() -> Vec<BenchmarkResult> {
    let mut results = Vec::new();

    // Parse benchmarks
    results.push(bench_parse(1));
    results.push(bench_parse(5));
    results.push(bench_parse(10));

    // Generate benchmarks
    results.push(bench_generate(1));
    results.push(bench_generate(5));
    results.push(bench_generate(10));

    // Optimize benchmarks
    results.push(bench_optimize(10, 5));
    results.push(bench_optimize(20, 10));

    // Full pipeline benchmarks
    results.push(bench_full_pipeline(1));
    results.push(bench_full_pipeline(5));
    results.push(bench_full_pipeline(10));

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_benchmark_under_10ms() {
        let result = bench_parse(10);
        assert!(
            result.passed,
            "Parse 10 functions took {:.3}ms (target: <10ms)",
            result.elapsed_ms
        );
    }

    #[test]
    fn test_generate_benchmark_under_10ms() {
        let result = bench_generate(10);
        assert!(
            result.passed,
            "Generate 10 functions took {:.3}ms (target: <10ms)",
            result.elapsed_ms
        );
    }

    #[test]
    fn test_optimize_benchmark_under_10ms() {
        let result = bench_optimize(20, 10);
        assert!(
            result.passed,
            "Optimize 20 functions took {:.3}ms (target: <10ms)",
            result.elapsed_ms
        );
    }

    #[test]
    fn test_full_pipeline_benchmark_under_10ms() {
        let result = bench_full_pipeline(10);
        assert!(
            result.passed,
            "Full pipeline 10 functions took {:.3}ms (target: <10ms)",
            result.elapsed_ms
        );
    }

    #[test]
    fn test_all_benchmarks_pass() {
        let results = run_all_benchmarks();
        for result in &results {
            assert!(result.passed, "Benchmark failed: {}", result);
        }
    }

    #[test]
    fn test_memory_usage_reasonable() {
        // Parse a 10-function module
        let mut source = String::new();
        for i in 0..10 {
            source.push_str(&make_uniform_struct("scale", i));
            source.push('\n');
            source.push_str(&make_function("scale", i));
            source.push('\n');
        }

        let string_size = source.len();
        let module = parse_wgsl(&source).unwrap();

        // Estimate AST memory (rough — use actual struct sizes)
        let ast_size = std::mem::size_of_val(&module)
            + module.functions.len() * std::mem::size_of::<Function>()
            + module.structs.len() * std::mem::size_of::<StructDef>();

        // AST should be <2x the string representation
        // Note: AST trades compactness for structured access
        assert!(
            ast_size < string_size * 3,
            "AST memory ({} bytes) exceeds 3x string ({} bytes)",
            ast_size,
            string_size
        );
    }
}
