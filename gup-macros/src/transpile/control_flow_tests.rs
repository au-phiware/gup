// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for control flow transpilation (GUP-058).
//!
//! Covers for loops, while loops, infinite loops, break/continue,
//! early returns, else-if chains, nested control flow, and variable
//! scoping across control paths.

#[cfg(test)]
mod tests {
    use crate::transpile::{RustToWgsl, WgslCodeGen};

    /// Helper to run the full transpile pipeline on a Rust function.
    fn transpile(func: &syn::ItemFn) -> String {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let wgsl_func = converter.convert_function(func).unwrap();
        let mut codegen = WgslCodeGen::new();
        codegen.generate_function(&wgsl_func)
    }

    /// Helper to transpile with uniform parameters.
    fn transpile_with_uniforms(
        func: &syn::ItemFn,
        uniform_params: impl IntoIterator<Item = String>,
    ) -> String {
        let mut converter = RustToWgsl::new(uniform_params);
        let wgsl_func = converter.convert_function(func).unwrap();
        let mut codegen = WgslCodeGen::new();
        codegen.generate_function(&wgsl_func)
    }

    // ===================================================================
    // AC1: Conditional Statements
    // ===================================================================

    #[test]
    fn ac1_simple_if() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                if x > 0.0 {
                    return x;
                }
                return 0.0;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("if (x > 0.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("return x;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac1_if_else() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                if x > 0.5 {
                    return 1.0;
                } else {
                    return 0.0;
                }
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("if (x > 0.5)"), "got:\n{wgsl}");
        assert!(wgsl.contains("} else {"), "got:\n{wgsl}");
        assert!(wgsl.contains("return 1.0;"), "got:\n{wgsl}");
        assert!(wgsl.contains("return 0.0;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac1_else_if_chain() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn classify(x: f32) -> i32 {
                if x > 1.0 {
                    return 2;
                } else if x > 0.0 {
                    return 1;
                } else {
                    return 0;
                }
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("if (x > 1.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("} else if (x > 0.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("} else {"), "got:\n{wgsl}");
    }

    #[test]
    fn ac1_if_with_variable_scoping() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut result = 0.0;
                if x > 0.0 {
                    let y = x * 2.0;
                    result = y;
                }
                return result;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("var result = 0.0;"), "got:\n{wgsl}");
        assert!(wgsl.contains("let y = x * 2.0;"), "got:\n{wgsl}");
        assert!(wgsl.contains("result = y;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac1_conditional_select_expression() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let result = if x > 0.0 { 1.0 } else { 0.0 };
                return result;
            }
        };
        let wgsl = transpile(&func);
        // If-else as expression → select()
        assert!(wgsl.contains("select("), "got:\n{wgsl}");
    }

    // ===================================================================
    // AC2: Loop Constructs
    // ===================================================================

    #[test]
    fn ac2_for_loop_simple() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(n: i32) -> i32 {
                let mut sum = 0;
                for i in 0..n {
                    sum += i;
                }
                return sum;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("for (var i = 0; i < n; i++)"), "got:\n{wgsl}");
        assert!(wgsl.contains("sum += i;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac2_for_loop_literal_bounds() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut total = 0.0;
                for i in 0..10 {
                    total += x;
                }
                return total;
            }
        };
        let wgsl = transpile(&func);
        assert!(
            wgsl.contains("for (var i = 0; i < 10; i++)"),
            "got:\n{wgsl}"
        );
    }

    #[test]
    fn ac2_while_loop() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut val = x;
                while val > 1.0 {
                    val = val / 2.0;
                }
                return val;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("while (val > 1.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("val = val / 2.0;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac2_infinite_loop_with_break() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut val = x;
                loop {
                    val = val * 0.5;
                    if val < 0.01 {
                        break;
                    }
                }
                return val;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("loop {"), "got:\n{wgsl}");
        assert!(wgsl.contains("break;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac2_continue_in_loop() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(n: i32) -> i32 {
                let mut sum = 0;
                for i in 0..n {
                    if i == 5 {
                        continue;
                    }
                    sum += i;
                }
                return sum;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("continue;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac2_for_loop_with_break() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(n: i32) -> i32 {
                let mut result = 0;
                for i in 0..100 {
                    if i > n {
                        break;
                    }
                    result = i;
                }
                return result;
            }
        };
        let wgsl = transpile(&func);
        assert!(
            wgsl.contains("for (var i = 0; i < 100; i++)"),
            "got:\n{wgsl}"
        );
        assert!(wgsl.contains("break;"), "got:\n{wgsl}");
    }

    // ===================================================================
    // AC3: Early Returns and Nested Control Flow
    // ===================================================================

    #[test]
    fn ac3_early_return_in_if() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                if x < 0.0 {
                    return 0.0;
                }
                return x;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("if (x < 0.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("return 0.0;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac3_early_return_in_loop() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn find_first(n: i32) -> i32 {
                for i in 0..n {
                    if i > 5 {
                        return i;
                    }
                }
                return -1;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("for (var i = 0; i < n; i++)"), "got:\n{wgsl}");
        assert!(wgsl.contains("return i;"), "got:\n{wgsl}");
        assert!(wgsl.contains("return -1;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac3_nested_if_in_loop() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut val = x;
                for i in 0..10 {
                    if val > 1.0 {
                        val = val - 1.0;
                    } else {
                        val = val * 2.0;
                    }
                }
                return val;
            }
        };
        let wgsl = transpile(&func);
        assert!(
            wgsl.contains("for (var i = 0; i < 10; i++)"),
            "got:\n{wgsl}"
        );
        assert!(wgsl.contains("if (val > 1.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("} else {"), "got:\n{wgsl}");
    }

    #[test]
    fn ac3_nested_loops() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(n: i32) -> i32 {
                let mut count = 0;
                for i in 0..n {
                    for j in 0..n {
                        count += 1;
                    }
                }
                return count;
            }
        };
        let wgsl = transpile(&func);
        // Should have two for loops
        let for_count = wgsl.matches("for (var").count();
        assert_eq!(for_count, 2, "Expected 2 for loops, got:\n{wgsl}");
    }

    #[test]
    fn ac3_while_with_nested_if() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut val = x;
                while val > 0.0 {
                    if val > 10.0 {
                        val = val - 10.0;
                    } else {
                        val = val - 1.0;
                    }
                }
                return val;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("while (val > 0.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("if (val > 10.0)"), "got:\n{wgsl}");
    }

    // ===================================================================
    // AC4: Variable Scoping and Mutable State
    // ===================================================================

    #[test]
    fn ac4_mutable_across_control_flow() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut result = 0.0;
                if x > 0.0 {
                    result = x;
                }
                return result;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("var result = 0.0;"), "got:\n{wgsl}");
        assert!(wgsl.contains("result = x;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac4_let_binding_in_loop() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(n: i32) -> f32 {
                let mut sum = 0.0;
                for i in 0..n {
                    let contrib = i as f32;
                    sum += contrib;
                }
                return sum;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("let contrib = f32(i);"), "got:\n{wgsl}");
        assert!(wgsl.contains("sum += contrib;"), "got:\n{wgsl}");
    }

    #[test]
    fn ac4_block_scoped_variable() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut result = 0.0;
                for i in 0..5 {
                    let local = x * i as f32;
                    result += local;
                }
                return result;
            }
        };
        let wgsl = transpile(&func);
        // `local` should be declared inside the for loop body
        assert!(wgsl.contains("let local ="), "got:\n{wgsl}");
    }

    // ===================================================================
    // Integration Tests — Complex Control Flow Patterns
    // ===================================================================

    #[test]
    fn integration_raymarching_pattern() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn raymarch(origin: f32, direction: f32) -> f32 {
                let mut distance = 0.0;
                for step in 0..64 {
                    let pos = origin + direction * distance;
                    let scene_dist = abs(pos);

                    if scene_dist < 0.001 {
                        return distance;
                    }

                    distance += scene_dist;

                    if distance > 100.0 {
                        break;
                    }
                }
                return -1.0;
            }
        };
        let wgsl = transpile(&func);
        assert!(
            wgsl.contains("for (var step = 0; step < 64; step++)"),
            "got:\n{wgsl}"
        );
        assert!(wgsl.contains("if (scene_dist < 0.001)"), "got:\n{wgsl}");
        assert!(wgsl.contains("return distance;"), "got:\n{wgsl}");
        assert!(wgsl.contains("break;"), "got:\n{wgsl}");
        assert!(wgsl.contains("return -1.0;"), "got:\n{wgsl}");
    }

    #[test]
    fn integration_accumulation_loop() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn accumulate(n: i32, scale: f32) -> f32 {
                let mut total = 0.0;
                for i in 0..n {
                    total += i as f32 * scale;
                }
                return total;
            }
        };
        let wgsl = transpile_with_uniforms(&func, ["scale".to_string()]);
        assert!(wgsl.contains("for (var i = 0; i < n; i++)"), "got:\n{wgsl}");
        assert!(
            wgsl.contains("total += f32(i) * uniforms.scale;"),
            "got:\n{wgsl}"
        );
    }

    #[test]
    fn integration_convergence_loop() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn converge(x: f32) -> f32 {
                let mut estimate = x;
                let mut prev = 0.0;
                while (estimate - prev).abs() > 0.001 {
                    prev = estimate;
                    estimate = (estimate + x / estimate) * 0.5;
                }
                return estimate;
            }
        };
        let wgsl = transpile(&func);
        assert!(
            wgsl.contains("while (abs((estimate - prev)) > 0.001)"),
            "got:\n{wgsl}"
        );
        assert!(wgsl.contains("prev = estimate;"), "got:\n{wgsl}");
    }

    #[test]
    fn integration_loop_with_multiple_breaks() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut val = x;
                loop {
                    if val < 0.0 {
                        break;
                    }
                    val = val - 1.0;
                    if val > 100.0 {
                        break;
                    }
                }
                return val;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("loop {"), "got:\n{wgsl}");
        let break_count = wgsl.matches("break;").count();
        assert_eq!(break_count, 2, "Expected 2 breaks, got:\n{wgsl}");
    }

    #[test]
    fn integration_for_loop_with_method_calls() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(data: f32) -> f32 {
                let mut sum = 0.0;
                for i in 0..8 {
                    let val = (data * i as f32).sin();
                    sum += val.abs();
                }
                return sum;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("for (var i = 0; i < 8; i++)"), "got:\n{wgsl}");
        assert!(wgsl.contains("sin((data * f32(i)))"), "got:\n{wgsl}");
        assert!(wgsl.contains("abs(val)"), "got:\n{wgsl}");
    }

    // ===================================================================
    // Error Cases
    // ===================================================================

    #[test]
    fn error_for_loop_without_range() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                for i in some_iter(x) {
                    return i;
                }
                return 0.0;
            }
        };
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let result = converter.convert_function(&func);
        assert!(result.is_err(), "Should error on non-range iterator");
        assert!(
            result.unwrap_err().message.contains("range"),
            "Error should mention range"
        );
    }

    // ===================================================================
    // Code Generation Tests
    // ===================================================================

    #[test]
    fn codegen_for_loop_indentation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(n: i32) -> i32 {
                let mut sum = 0;
                for i in 0..n {
                    sum += i;
                }
                return sum;
            }
        };
        let wgsl = transpile(&func);
        // Check proper indentation
        assert!(
            wgsl.contains("    for (var i = 0; i < n; i++) {"),
            "For loop should be indented, got:\n{wgsl}"
        );
        assert!(
            wgsl.contains("        sum += i;"),
            "Loop body should be double-indented, got:\n{wgsl}"
        );
    }

    #[test]
    fn codegen_while_loop_indentation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut val = x;
                while val > 1.0 {
                    val = val / 2.0;
                }
                return val;
            }
        };
        let wgsl = transpile(&func);
        assert!(
            wgsl.contains("    while (val > 1.0) {"),
            "While loop should be indented, got:\n{wgsl}"
        );
    }

    #[test]
    fn codegen_loop_indentation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut val = x;
                loop {
                    val = val - 1.0;
                    if val < 0.0 {
                        break;
                    }
                }
                return val;
            }
        };
        let wgsl = transpile(&func);
        assert!(
            wgsl.contains("    loop {"),
            "Loop should be indented, got:\n{wgsl}"
        );
        assert!(
            wgsl.contains("        val = val - 1.0;"),
            "Loop body should be double-indented, got:\n{wgsl}"
        );
    }

    #[test]
    fn codegen_else_if_chain() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn classify(x: f32) -> i32 {
                if x > 1.0 {
                    return 2;
                } else if x > 0.0 {
                    return 1;
                } else {
                    return 0;
                }
            }
        };
        let wgsl = transpile(&func);
        // Should generate proper else-if, not nested else { if }
        assert!(
            wgsl.contains("} else if (x > 0.0) {"),
            "Should generate else-if chain, got:\n{wgsl}"
        );
    }

    #[test]
    fn codegen_break_continue_placement() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(n: i32) -> i32 {
                let mut sum = 0;
                for i in 0..n {
                    if i == 3 {
                        continue;
                    }
                    if i == 7 {
                        break;
                    }
                    sum += i;
                }
                return sum;
            }
        };
        let wgsl = transpile(&func);
        assert!(wgsl.contains("continue;"), "got:\n{wgsl}");
        assert!(wgsl.contains("break;"), "got:\n{wgsl}");
    }
}
