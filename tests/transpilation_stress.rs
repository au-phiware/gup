// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stress tests for the Rust-to-WGSL transpilation system.
//!
//! These tests validate the transpiler's robustness with complex,
//! edge-case, and high-complexity shader functions that push the
//! boundaries of what the system supports.

use gup::shader_function::{self, ComposableShaderFunction};
use gup_macros::shader_fn;

// ---------------------------------------------------------------------------
// Deep nesting and complex control flow
// ---------------------------------------------------------------------------

/// Deeply nested conditional logic.
#[shader_fn]
fn deep_conditionals(value: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    if value > a {
        if value > b {
            if value > c {
                if value > d {
                    return 4.0;
                } else {
                    return 3.0;
                }
            } else {
                return 2.0;
            }
        } else {
            return 1.0;
        }
    } else {
        return 0.0;
    }
}

/// Multi-step loop with accumulation.
#[shader_fn]
fn loop_accumulation(steps: i32) -> f32 {
    let mut result = 0.0;
    for i in 0..steps {
        let fi = f32(i);
        let contribution = 1.0 / (fi + 1.0);
        result += contribution;
    }
    return result;
}

/// Nested loop pattern.
#[shader_fn]
fn nested_loops(outer: i32, inner: i32) -> i32 {
    let mut total = 0;
    for i in 0..outer {
        for j in 0..inner {
            total += i * inner + j;
        }
    }
    return total;
}

/// Complex mathematical chain using many builtins.
#[shader_fn]
fn math_chain(value: f32, frequency: f32, phase: f32, amplitude: f32, offset: f32) -> f32 {
    let s1 = sin(value * frequency + phase);
    let s2 = cos(value * frequency * 0.5 + phase);
    let combined = s1 * 0.6 + s2 * 0.4;
    let scaled = combined * amplitude;
    let shifted = scaled + offset;
    let clamped = clamp(shifted, -1.0, 1.0);
    let smoothed = clamped * clamped * (3.0 - 2.0 * abs(clamped));
    return smoothed;
}

/// Compound assignment operators.
#[shader_fn]
fn compound_ops(value: f32, factor: f32) -> f32 {
    let mut result = value;
    result += factor;
    result *= 2.0;
    result -= factor * 0.5;
    return result;
}

/// Multiple return paths with complex conditions.
#[shader_fn]
fn multi_return(value: f32, lo: f32, hi: f32) -> f32 {
    if value < lo {
        return 0.0;
    }
    if value > hi {
        return 1.0;
    }
    let normalised = (value - lo) / (hi - lo);
    if normalised < 0.5 {
        return normalised * normalised * 2.0;
    }
    let t = 1.0 - normalised;
    return 1.0 - t * t * 2.0;
}

// ---------------------------------------------------------------------------
// Large function body
// ---------------------------------------------------------------------------

/// A function with many let bindings and operations.
#[shader_fn]
fn many_bindings(x: f32, scale: f32) -> f32 {
    let a = x * scale;
    let b = a + 1.0;
    let c = b * 2.0;
    let d = c - 0.5;
    let e = abs(d);
    let f = sqrt(e);
    let g = clamp(f, 0.0, 10.0);
    let h = g / scale;
    let i = sin(h);
    let j = cos(h);
    let k = i * i + j * j;
    let l = sqrt(k);
    let m = max(l, 0.001);
    let n = 1.0 / m;
    let o = min(n, 100.0);
    let p = floor(o);
    let q = o - p;
    let r = q * q;
    let s = 1.0 - r;
    let t = max(s, 0.0);
    return t;
}

/// Many uniform parameters.
#[allow(clippy::too_many_arguments)]
#[shader_fn]
fn many_uniforms(value: f32, p1: f32, p2: f32, p3: f32, p4: f32, p5: f32, p6: f32) -> f32 {
    let step1 = value * p1 + p2;
    let step2 = step1 * p3 - p4;
    let step3 = clamp(step2, p5, p6);
    return step3;
}

// ---------------------------------------------------------------------------
// While loops and loop/break patterns
// ---------------------------------------------------------------------------

/// While loop with convergence check.
#[shader_fn]
fn converge(start: f32, goal: f32) -> f32 {
    let mut current = start;
    let mut steps = 0;
    while steps < 100 {
        let diff = goal - current;
        if abs(diff) < 0.001 {
            break;
        }
        current += diff * 0.1;
        steps += 1;
    }
    return current;
}

/// Loop with break for iterative refinement.
#[shader_fn]
fn iterative_refine(initial: f32, factor: f32) -> f32 {
    let mut value = initial;
    for _i in 0..50 {
        let next = value * factor;
        if abs(next - value) < 0.0001 {
            break;
        }
        value = next;
    }
    return value;
}

// ===========================================================================
// Tests
// ===========================================================================

#[test]
fn stress_deep_conditionals_generates_valid_wgsl() {
    let wgsl = DeepConditionals::wgsl_function();
    assert!(wgsl.contains("fn deep_conditionals"));
    // Count if/else occurrences — should have at least 4 ifs
    let if_count = wgsl.matches("if").count();
    assert!(
        if_count >= 4,
        "Expected at least 4 if statements, found {if_count} in:\n{wgsl}"
    );
}

#[test]
fn stress_loop_accumulation_generates_valid_wgsl() {
    let wgsl = LoopAccumulation::wgsl_function();
    assert!(wgsl.contains("fn loop_accumulation"));
    assert!(wgsl.contains("for"));
    assert!(wgsl.contains("f32("));
}

#[test]
fn stress_nested_loops_generates_valid_wgsl() {
    let wgsl = NestedLoops::wgsl_function();
    assert!(wgsl.contains("fn nested_loops"));
    let for_count = wgsl.matches("for").count();
    assert!(
        for_count >= 2,
        "Expected at least 2 for loops, found {for_count} in:\n{wgsl}"
    );
}

#[test]
fn stress_math_chain_generates_valid_wgsl() {
    let wgsl = MathChain::wgsl_function();
    assert!(wgsl.contains("fn math_chain"));
    assert!(wgsl.contains("sin("));
    assert!(wgsl.contains("cos("));
    assert!(wgsl.contains("clamp("));
    assert!(wgsl.contains("abs("));
}

#[test]
fn stress_compound_ops_generates_valid_wgsl() {
    let wgsl = CompoundOps::wgsl_function();
    assert!(wgsl.contains("fn compound_ops"));
    assert!(wgsl.contains("+="));
    assert!(wgsl.contains("*="));
    assert!(wgsl.contains("-="));
}

#[test]
fn stress_multi_return_generates_valid_wgsl() {
    let wgsl = MultiReturn::wgsl_function();
    assert!(wgsl.contains("fn multi_return"));
    let return_count = wgsl.matches("return").count();
    assert!(
        return_count >= 3,
        "Expected at least 3 return statements, found {return_count} in:\n{wgsl}"
    );
}

#[test]
fn stress_many_bindings_generates_valid_wgsl() {
    let wgsl = ManyBindings::wgsl_function();
    assert!(wgsl.contains("fn many_bindings"));
    let let_count = wgsl.matches("let ").count();
    assert!(
        let_count >= 15,
        "Expected at least 15 let bindings, found {let_count} in:\n{wgsl}"
    );
}

#[test]
fn stress_many_uniforms_generates_valid_wgsl() {
    let wgsl = ManyUniforms::wgsl_function();
    assert!(wgsl.contains("fn many_uniforms"));
    assert!(wgsl.contains("ManyUniformsUniforms"));
    assert!(wgsl.contains("p1"));
    assert!(wgsl.contains("p6"));
}

#[test]
fn stress_converge_generates_valid_wgsl() {
    let wgsl = Converge::wgsl_function();
    assert!(wgsl.contains("fn converge"));
    assert!(wgsl.contains("while") || wgsl.contains("for"));
}

#[test]
fn stress_iterative_refine_generates_valid_wgsl() {
    let wgsl = IterativeRefine::wgsl_function();
    assert!(wgsl.contains("fn iterative_refine"));
    assert!(wgsl.contains("break"));
}

// ---------------------------------------------------------------------------
// GPU compilation of all stress test functions
// ---------------------------------------------------------------------------

async fn validate_wgsl_compiles(label: &str, wgsl: &str) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("No GPU adapter available");
    let (device, _queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .expect("Failed to create device");

    let validation_wgsl = format!(
        "{wgsl}\n\n@compute @workgroup_size(1)\nfn main() {{\n    // validation entry point\n}}"
    );

    let _module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&validation_wgsl)),
    });
}

#[tokio::test]
async fn stress_deep_conditionals_compiles_on_gpu() {
    validate_wgsl_compiles("deep_conditionals", DeepConditionals::wgsl_function()).await;
}

#[tokio::test]
async fn stress_loop_accumulation_compiles_on_gpu() {
    validate_wgsl_compiles("loop_accumulation", LoopAccumulation::wgsl_function()).await;
}

#[tokio::test]
async fn stress_nested_loops_compiles_on_gpu() {
    validate_wgsl_compiles("nested_loops", NestedLoops::wgsl_function()).await;
}

#[tokio::test]
async fn stress_math_chain_compiles_on_gpu() {
    validate_wgsl_compiles("math_chain", MathChain::wgsl_function()).await;
}

#[tokio::test]
async fn stress_compound_ops_compiles_on_gpu() {
    validate_wgsl_compiles("compound_ops", CompoundOps::wgsl_function()).await;
}

#[tokio::test]
async fn stress_multi_return_compiles_on_gpu() {
    validate_wgsl_compiles("multi_return", MultiReturn::wgsl_function()).await;
}

#[tokio::test]
async fn stress_many_bindings_compiles_on_gpu() {
    validate_wgsl_compiles("many_bindings", ManyBindings::wgsl_function()).await;
}

#[tokio::test]
async fn stress_many_uniforms_compiles_on_gpu() {
    validate_wgsl_compiles("many_uniforms", ManyUniforms::wgsl_function()).await;
}

#[tokio::test]
async fn stress_converge_compiles_on_gpu() {
    validate_wgsl_compiles("converge", Converge::wgsl_function()).await;
}

#[tokio::test]
async fn stress_iterative_refine_compiles_on_gpu() {
    validate_wgsl_compiles("iterative_refine", IterativeRefine::wgsl_function()).await;
}

// ---------------------------------------------------------------------------
// Uniform struct correctness
// ---------------------------------------------------------------------------

#[test]
fn stress_many_uniforms_struct_has_all_fields() {
    let f = ManyUniforms::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    let u = f.create_uniforms().unwrap();
    assert_eq!(u.p1, 1.0);
    assert_eq!(u.p2, 2.0);
    assert_eq!(u.p3, 3.0);
    assert_eq!(u.p4, 4.0);
    assert_eq!(u.p5, 5.0);
    assert_eq!(u.p6, 6.0);
}

#[test]
fn stress_deep_conditionals_uniform_struct() {
    let f = DeepConditionals::new(1.0, 2.0, 3.0, 4.0);
    let u = f.create_uniforms().unwrap();
    assert_eq!(u.a, 1.0);
    assert_eq!(u.d, 4.0);
}

#[test]
fn stress_all_functions_implement_trait() {
    fn assert_composable<T: ComposableShaderFunction>() {}

    assert_composable::<DeepConditionals>();
    assert_composable::<LoopAccumulation>();
    assert_composable::<NestedLoops>();
    assert_composable::<MathChain>();
    assert_composable::<CompoundOps>();
    assert_composable::<MultiReturn>();
    assert_composable::<ManyBindings>();
    assert_composable::<ManyUniforms>();
    assert_composable::<Converge>();
    assert_composable::<IterativeRefine>();
}

// ---------------------------------------------------------------------------
// WGSL output quality checks
// ---------------------------------------------------------------------------

#[test]
fn stress_generated_wgsl_has_proper_indentation() {
    let wgsl = DeepConditionals::wgsl_function();
    // WGSL should have consistent 4-space indentation
    for line in wgsl.lines() {
        if !line.is_empty() {
            let leading = line.len() - line.trim_start().len();
            assert_eq!(
                leading % 4,
                0,
                "Line should have 4-space indentation: '{line}'"
            );
        }
    }
}

#[test]
fn stress_wgsl_uniform_struct_definition_format() {
    let wgsl = ManyUniforms::wgsl_function();
    // Should have a proper struct definition
    assert!(
        wgsl.contains("struct ManyUniformsUniforms"),
        "Should define uniform struct"
    );
    // Uniform fields should be present
    for field in &["p1", "p2", "p3", "p4", "p5", "p6"] {
        assert!(
            wgsl.contains(field),
            "Should contain uniform field {field} in:\n{wgsl}"
        );
    }
}
