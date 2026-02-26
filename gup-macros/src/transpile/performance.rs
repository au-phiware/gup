// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance analysis framework for transpiled WGSL shaders.
//!
//! Analyses WGSL AST nodes for patterns known to cause GPU performance
//! issues: control flow divergence, expensive fragment-shader loops,
//! and redundant operations.

use super::ast::*;
use super::diagnostics::{
    DiagnosticBuilder, DiagnosticLevel, Position, SourceSpan, TranspilationDiagnostic,
};

// ---------------------------------------------------------------------------
// Performance warning types
// ---------------------------------------------------------------------------

/// Category of a performance warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningCategory {
    /// Control flow that may cause thread divergence.
    ControlFlowDivergence,
    /// Expensive operations in performance-critical sections.
    ExpensiveOperation,
    /// Large loops that may stall execution.
    LargeLoop,
    /// Redundant operations that waste compute.
    RedundantOperation,
}

/// Estimated impact of a performance issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
}

/// A performance warning for a specific piece of shader code.
#[derive(Debug, Clone)]
pub struct PerformanceWarning {
    pub category: WarningCategory,
    pub impact: ImpactLevel,
    pub message: String,
    pub suggestion: Option<String>,
    pub function_name: String,
}

impl PerformanceWarning {
    /// Convert to a diagnostic for unified error reporting.
    pub fn to_diagnostic(&self) -> TranspilationDiagnostic {
        let level = match self.impact {
            ImpactLevel::High => DiagnosticLevel::Warning,
            ImpactLevel::Medium => DiagnosticLevel::Warning,
            ImpactLevel::Low => DiagnosticLevel::Hint,
        };

        let mut builder = if level == DiagnosticLevel::Warning {
            DiagnosticBuilder::warning(&self.message)
        } else {
            DiagnosticBuilder::hint(&self.message)
        };

        builder = builder.note(format!("in function `{}`", self.function_name));

        if let Some(ref s) = self.suggestion {
            builder = builder.help(s.clone());
        }

        builder.build()
    }
}

// ---------------------------------------------------------------------------
// Analysis configuration
// ---------------------------------------------------------------------------

/// Configuration for performance analysis.
#[derive(Debug, Clone)]
pub struct PerformanceAnalysisConfig {
    /// Maximum loop iterations before flagging as expensive.
    pub max_loop_iterations: u64,
    /// Whether to warn about control flow divergence.
    pub warn_divergence: bool,
    /// Whether to detect expensive fragment shader patterns.
    pub warn_expensive_ops: bool,
}

impl Default for PerformanceAnalysisConfig {
    fn default() -> Self {
        Self {
            max_loop_iterations: 64,
            warn_divergence: true,
            warn_expensive_ops: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Analysis engine
// ---------------------------------------------------------------------------

/// Analyse a module for performance issues.
pub fn analyse_performance(
    module: &WgslModule,
    config: &PerformanceAnalysisConfig,
) -> Vec<PerformanceWarning> {
    let mut warnings = Vec::new();

    for func in &module.functions {
        analyse_function(func, config, &mut warnings);
    }

    warnings
}

fn analyse_function(
    func: &WgslFunction,
    config: &PerformanceAnalysisConfig,
    warnings: &mut Vec<PerformanceWarning>,
) {
    for stmt in &func.body {
        analyse_stmt(stmt, &func.name, config, warnings, 0);
    }
}

fn analyse_stmt(
    stmt: &WgslStatement,
    func_name: &str,
    config: &PerformanceAnalysisConfig,
    warnings: &mut Vec<PerformanceWarning>,
    loop_depth: u32,
) {
    match stmt {
        WgslStatement::If {
            condition,
            body,
            else_body,
        } => {
            // Warn about complex conditions that may cause divergence.
            if config.warn_divergence && contains_complex_condition(condition) {
                warnings.push(PerformanceWarning {
                    category: WarningCategory::ControlFlowDivergence,
                    impact: ImpactLevel::Medium,
                    message: "complex conditional expression may cause thread divergence"
                        .to_string(),
                    suggestion: Some(
                        "consider using select() for simple conditional assignments".to_string(),
                    ),
                    function_name: func_name.to_string(),
                });
            }

            for s in body {
                analyse_stmt(s, func_name, config, warnings, loop_depth);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    analyse_stmt(s, func_name, config, warnings, loop_depth);
                }
            }
        }
        WgslStatement::For {
            condition, body, ..
        } => {
            // Check for large constant-bound loops.
            if let Some(limit) = extract_loop_limit(condition) {
                if limit > config.max_loop_iterations {
                    warnings.push(PerformanceWarning {
                        category: WarningCategory::LargeLoop,
                        impact: if limit > 256 {
                            ImpactLevel::High
                        } else {
                            ImpactLevel::Medium
                        },
                        message: format!(
                            "loop iterates up to {limit} times, which may be expensive on GPU"
                        ),
                        suggestion: Some(
                            "consider reducing iteration count or using a compute shader"
                                .to_string(),
                        ),
                        function_name: func_name.to_string(),
                    });
                }
            }

            for s in body {
                analyse_stmt(s, func_name, config, warnings, loop_depth + 1);
            }
        }
        WgslStatement::While { body, .. } | WgslStatement::Loop { body } => {
            if config.warn_divergence && loop_depth > 0 {
                warnings.push(PerformanceWarning {
                    category: WarningCategory::ControlFlowDivergence,
                    impact: ImpactLevel::Medium,
                    message: "nested dynamic loop may cause significant thread divergence"
                        .to_string(),
                    suggestion: Some(
                        "consider restructuring to avoid nested loops on GPU".to_string(),
                    ),
                    function_name: func_name.to_string(),
                });
            }

            for s in body {
                analyse_stmt(s, func_name, config, warnings, loop_depth + 1);
            }
        }
        _ => {}
    }
}

/// Check if a condition expression is complex enough to potentially
/// cause divergence (involves function calls or nested logical operators).
fn contains_complex_condition(expr: &WgslExpr) -> bool {
    match expr {
        WgslExpr::Call(_, _) => true,
        WgslExpr::Binary(l, BinaryOp::And, r) | WgslExpr::Binary(l, BinaryOp::Or, r) => {
            // Nested logical ops are more likely to diverge.
            contains_complex_condition(l) || contains_complex_condition(r)
        }
        WgslExpr::Binary(l, _, r) => contains_complex_condition(l) || contains_complex_condition(r),
        WgslExpr::Unary(_, inner) => contains_complex_condition(inner),
        WgslExpr::Paren(inner) => contains_complex_condition(inner),
        _ => false,
    }
}

/// Try to extract a numeric loop limit from a `i < N` condition.
fn extract_loop_limit(condition: &WgslExpr) -> Option<u64> {
    if let WgslExpr::Binary(_, BinaryOp::Less | BinaryOp::LessEqual, right) = condition {
        match right.as_ref() {
            WgslExpr::Literal(Literal::Int(v)) => Some(*v as u64),
            WgslExpr::Literal(Literal::UInt(v)) => Some(*v),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_with_body(func_name: &str, body: Vec<WgslStatement>) -> WgslModule {
        WgslModule {
            structs: vec![],
            functions: vec![WgslFunction {
                name: func_name.to_string(),
                params: vec![WgslParam {
                    name: "x".to_string(),
                    ty: WgslType::Scalar(ScalarType::F32),
                }],
                return_type: WgslType::Scalar(ScalarType::F32),
                body,
            }],
        }
    }

    #[test]
    fn large_loop_triggers_warning() {
        let module = module_with_body(
            "expensive_fn",
            vec![WgslStatement::For {
                var_name: "i".to_string(),
                initialiser: WgslExpr::Literal(Literal::Int(0)),
                condition: WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("i".to_string())),
                    BinaryOp::Less,
                    Box::new(WgslExpr::Literal(Literal::Int(100))),
                ),
                update: WgslExpr::Ident("i".to_string()),
                body: vec![WgslStatement::Expression(WgslExpr::Ident("x".to_string()))],
            }],
        );

        let warnings = analyse_performance(&module, &PerformanceAnalysisConfig::default());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].category, WarningCategory::LargeLoop);
        assert!(warnings[0].message.contains("100"));
    }

    #[test]
    fn small_loop_no_warning() {
        let module = module_with_body(
            "cheap_fn",
            vec![WgslStatement::For {
                var_name: "i".to_string(),
                initialiser: WgslExpr::Literal(Literal::Int(0)),
                condition: WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("i".to_string())),
                    BinaryOp::Less,
                    Box::new(WgslExpr::Literal(Literal::Int(4))),
                ),
                update: WgslExpr::Ident("i".to_string()),
                body: vec![WgslStatement::Expression(WgslExpr::Ident("x".to_string()))],
            }],
        );

        let warnings = analyse_performance(&module, &PerformanceAnalysisConfig::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn complex_condition_triggers_divergence_warning() {
        let module = module_with_body(
            "divergent_fn",
            vec![WgslStatement::If {
                condition: WgslExpr::Call(
                    "some_check".to_string(),
                    vec![WgslExpr::Ident("x".to_string())],
                ),
                body: vec![WgslStatement::Return(Some(WgslExpr::Literal(
                    Literal::Float(1.0),
                )))],
                else_body: None,
            }],
        );

        let warnings = analyse_performance(&module, &PerformanceAnalysisConfig::default());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].category, WarningCategory::ControlFlowDivergence);
    }

    #[test]
    fn simple_condition_no_divergence_warning() {
        let module = module_with_body(
            "simple_fn",
            vec![WgslStatement::If {
                condition: WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("x".to_string())),
                    BinaryOp::Greater,
                    Box::new(WgslExpr::Literal(Literal::Float(0.0))),
                ),
                body: vec![WgslStatement::Return(Some(WgslExpr::Literal(
                    Literal::Float(1.0),
                )))],
                else_body: None,
            }],
        );

        let warnings = analyse_performance(&module, &PerformanceAnalysisConfig::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn nested_loop_triggers_divergence() {
        let module = module_with_body(
            "nested_fn",
            vec![WgslStatement::For {
                var_name: "i".to_string(),
                initialiser: WgslExpr::Literal(Literal::Int(0)),
                condition: WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("i".to_string())),
                    BinaryOp::Less,
                    Box::new(WgslExpr::Literal(Literal::Int(4))),
                ),
                update: WgslExpr::Ident("i".to_string()),
                body: vec![WgslStatement::While {
                    condition: WgslExpr::Binary(
                        Box::new(WgslExpr::Ident("x".to_string())),
                        BinaryOp::Greater,
                        Box::new(WgslExpr::Literal(Literal::Float(0.0))),
                    ),
                    body: vec![WgslStatement::Break],
                }],
            }],
        );

        let warnings = analyse_performance(&module, &PerformanceAnalysisConfig::default());
        assert!(
            warnings
                .iter()
                .any(|w| w.category == WarningCategory::ControlFlowDivergence)
        );
    }

    #[test]
    fn performance_warning_to_diagnostic() {
        let warning = PerformanceWarning {
            category: WarningCategory::LargeLoop,
            impact: ImpactLevel::High,
            message: "loop iterates 1000 times".to_string(),
            suggestion: Some("reduce iterations".to_string()),
            function_name: "my_func".to_string(),
        };

        let diag = warning.to_diagnostic();
        assert_eq!(diag.level, DiagnosticLevel::Warning);
        assert!(diag.message.contains("1000"));
        assert_eq!(diag.notes.len(), 1);
        assert!(diag.notes[0].contains("my_func"));
    }

    #[test]
    fn high_iteration_count_gets_high_impact() {
        let module = module_with_body(
            "very_expensive",
            vec![WgslStatement::For {
                var_name: "i".to_string(),
                initialiser: WgslExpr::Literal(Literal::Int(0)),
                condition: WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("i".to_string())),
                    BinaryOp::Less,
                    Box::new(WgslExpr::Literal(Literal::Int(512))),
                ),
                update: WgslExpr::Ident("i".to_string()),
                body: vec![WgslStatement::Expression(WgslExpr::Ident("x".to_string()))],
            }],
        );

        let warnings = analyse_performance(&module, &PerformanceAnalysisConfig::default());
        assert_eq!(warnings[0].impact, ImpactLevel::High);
    }
}
