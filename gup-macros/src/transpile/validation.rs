// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WGSL validation for the transpilation pipeline.
//!
//! Validates transpiled WGSL modules for correctness and best practices
//! beyond what the parser checks. Issues are reported as diagnostics.

use std::collections::HashSet;

use super::ast::*;
use super::diagnostics::{DiagnosticBuilder, TranspilationDiagnostic};

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate a transpiled WGSL module and return any diagnostics.
///
/// Checks for:
/// - Functions with empty bodies
/// - Unused function parameters
/// - Missing return statements
/// - Unreachable code after return/break/continue
pub fn validate_module(module: &WgslModule) -> Vec<TranspilationDiagnostic> {
    let mut diagnostics = Vec::new();

    for func in &module.functions {
        validate_function(func, &mut diagnostics);
    }

    diagnostics
}

fn validate_function(func: &WgslFunction, diagnostics: &mut Vec<TranspilationDiagnostic>) {
    // Check for empty body.
    if func.body.is_empty() {
        diagnostics.push(
            DiagnosticBuilder::warning(format!("function `{}` has an empty body", func.name))
                .note("Empty functions generate valid but useless WGSL".to_string())
                .build(),
        );
    }

    // Check for unused parameters.
    let used_idents = collect_all_idents(&func.body);
    for param in &func.params {
        if !used_idents.contains(&param.name) {
            diagnostics.push(
                DiagnosticBuilder::hint(format!(
                    "parameter `{}` is unused in function `{}`",
                    param.name, func.name
                ))
                .help(format!(
                    "consider prefixing with underscore: `_{}`",
                    param.name
                ))
                .build(),
            );
        }
    }

    // Check for missing return in non-void functions.
    if func.return_type != WgslType::Void && !func.body.is_empty() {
        if !ends_with_return(&func.body) {
            diagnostics.push(
                DiagnosticBuilder::warning(format!(
                    "function `{}` may not return a value on all paths",
                    func.name
                ))
                .note("WGSL requires explicit return statements".to_string())
                .build(),
            );
        }
    }

    // Check for unreachable code after return/break/continue.
    check_unreachable(&func.body, &func.name, diagnostics);
}

/// Collect all identifiers referenced in statements.
fn collect_all_idents(stmts: &[WgslStatement]) -> HashSet<String> {
    let mut idents = HashSet::new();
    for stmt in stmts {
        collect_idents_stmt(stmt, &mut idents);
    }
    idents
}

fn collect_idents_stmt(stmt: &WgslStatement, idents: &mut HashSet<String>) {
    match stmt {
        WgslStatement::Let { value, .. } => collect_idents_expr(value, idents),
        WgslStatement::Return(Some(e)) => collect_idents_expr(e, idents),
        WgslStatement::Return(None) => {}
        WgslStatement::If {
            condition,
            body,
            else_body,
        } => {
            collect_idents_expr(condition, idents);
            for s in body {
                collect_idents_stmt(s, idents);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    collect_idents_stmt(s, idents);
                }
            }
        }
        WgslStatement::For {
            initialiser,
            condition,
            update,
            body,
            ..
        } => {
            collect_idents_expr(initialiser, idents);
            collect_idents_expr(condition, idents);
            collect_idents_expr(update, idents);
            for s in body {
                collect_idents_stmt(s, idents);
            }
        }
        WgslStatement::While { condition, body } => {
            collect_idents_expr(condition, idents);
            for s in body {
                collect_idents_stmt(s, idents);
            }
        }
        WgslStatement::Loop { body } => {
            for s in body {
                collect_idents_stmt(s, idents);
            }
        }
        WgslStatement::Break | WgslStatement::Continue => {}
        WgslStatement::Expression(e) => collect_idents_expr(e, idents),
        WgslStatement::Assign(t, v) => {
            collect_idents_expr(t, idents);
            collect_idents_expr(v, idents);
        }
        WgslStatement::CompoundAssign(t, _, v) => {
            collect_idents_expr(t, idents);
            collect_idents_expr(v, idents);
        }
    }
}

fn collect_idents_expr(expr: &WgslExpr, idents: &mut HashSet<String>) {
    match expr {
        WgslExpr::Ident(name) => {
            idents.insert(name.clone());
        }
        WgslExpr::Literal(_) => {}
        WgslExpr::Binary(l, _, r) => {
            collect_idents_expr(l, idents);
            collect_idents_expr(r, idents);
        }
        WgslExpr::Unary(_, inner) => collect_idents_expr(inner, idents),
        WgslExpr::Call(_, args) | WgslExpr::TypeConstructor(_, args) => {
            for a in args {
                collect_idents_expr(a, idents);
            }
        }
        WgslExpr::MemberAccess(base, _) => collect_idents_expr(base, idents),
        WgslExpr::IndexAccess(base, idx) => {
            collect_idents_expr(base, idents);
            collect_idents_expr(idx, idents);
        }
        WgslExpr::Paren(inner) => collect_idents_expr(inner, idents),
        WgslExpr::Cast(_, inner) => collect_idents_expr(inner, idents),
    }
}

/// Check if a statement list ends with a return on all paths.
fn ends_with_return(stmts: &[WgslStatement]) -> bool {
    if let Some(last) = stmts.last() {
        match last {
            WgslStatement::Return(_) => true,
            WgslStatement::If {
                body, else_body, ..
            } => {
                // Both branches must return for the if to count.
                ends_with_return(body) && else_body.as_ref().is_some_and(|eb| ends_with_return(eb))
            }
            _ => false,
        }
    } else {
        false
    }
}

/// Check for unreachable code after return/break/continue.
fn check_unreachable(
    stmts: &[WgslStatement],
    func_name: &str,
    diagnostics: &mut Vec<TranspilationDiagnostic>,
) {
    for (i, stmt) in stmts.iter().enumerate() {
        let is_terminator = matches!(
            stmt,
            WgslStatement::Return(_) | WgslStatement::Break | WgslStatement::Continue
        );

        if is_terminator && i + 1 < stmts.len() {
            diagnostics.push(
                DiagnosticBuilder::warning(format!(
                    "unreachable code after {} in function `{func_name}`",
                    match stmt {
                        WgslStatement::Return(_) => "return",
                        WgslStatement::Break => "break",
                        WgslStatement::Continue => "continue",
                        _ => "terminator",
                    }
                ))
                .help("remove the unreachable statements".to_string())
                .build(),
            );
            break; // Only report once per block.
        }

        // Recurse into nested blocks.
        match stmt {
            WgslStatement::If {
                body, else_body, ..
            } => {
                check_unreachable(body, func_name, diagnostics);
                if let Some(eb) = else_body {
                    check_unreachable(eb, func_name, diagnostics);
                }
            }
            WgslStatement::For { body, .. }
            | WgslStatement::While { body, .. }
            | WgslStatement::Loop { body } => {
                check_unreachable(body, func_name, diagnostics);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_with_func(
        name: &str,
        params: Vec<WgslParam>,
        return_type: WgslType,
        body: Vec<WgslStatement>,
    ) -> WgslModule {
        WgslModule {
            structs: vec![],
            functions: vec![WgslFunction {
                name: name.to_string(),
                params,
                return_type,
                body,
            }],
        }
    }

    #[test]
    fn warns_empty_function_body() {
        let module = module_with_func("empty", vec![], WgslType::Void, vec![]);
        let diags = validate_module(&module);
        assert!(diags.iter().any(|d| d.message.contains("empty body")));
    }

    #[test]
    fn hints_unused_parameter() {
        let module = module_with_func(
            "unused_param",
            vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            WgslType::Scalar(ScalarType::F32),
            vec![WgslStatement::Return(Some(WgslExpr::Literal(
                Literal::Float(1.0),
            )))],
        );
        let diags = validate_module(&module);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("parameter `x` is unused"))
        );
    }

    #[test]
    fn no_hint_for_used_parameter() {
        let module = module_with_func(
            "used_param",
            vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            WgslType::Scalar(ScalarType::F32),
            vec![WgslStatement::Return(Some(WgslExpr::Ident(
                "x".to_string(),
            )))],
        );
        let diags = validate_module(&module);
        assert!(diags.is_empty());
    }

    #[test]
    fn warns_missing_return() {
        let module = module_with_func(
            "no_return",
            vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            WgslType::Scalar(ScalarType::F32),
            vec![WgslStatement::Expression(WgslExpr::Ident("x".to_string()))],
        );
        let diags = validate_module(&module);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("may not return a value"))
        );
    }

    #[test]
    fn no_warning_for_void_function_without_return() {
        let module = module_with_func(
            "void_fn",
            vec![],
            WgslType::Void,
            vec![WgslStatement::Expression(WgslExpr::Call(
                "do_something".to_string(),
                vec![],
            ))],
        );
        let diags = validate_module(&module);
        // No "may not return" warning for void functions.
        assert!(diags.iter().all(|d| !d.message.contains("may not return")));
    }

    #[test]
    fn warns_unreachable_code_after_return() {
        let module = module_with_func(
            "unreachable",
            vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            WgslType::Scalar(ScalarType::F32),
            vec![
                WgslStatement::Return(Some(WgslExpr::Ident("x".to_string()))),
                WgslStatement::Expression(WgslExpr::Call("unreachable_call".to_string(), vec![])),
            ],
        );
        let diags = validate_module(&module);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unreachable code after return"))
        );
    }

    #[test]
    fn no_warning_for_reachable_code() {
        let module = module_with_func(
            "reachable",
            vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            WgslType::Scalar(ScalarType::F32),
            vec![
                WgslStatement::Let {
                    name: "a".to_string(),
                    ty: None,
                    value: WgslExpr::Binary(
                        Box::new(WgslExpr::Ident("x".to_string())),
                        BinaryOp::Mul,
                        Box::new(WgslExpr::Literal(Literal::Float(2.0))),
                    ),
                    mutable: false,
                },
                WgslStatement::Return(Some(WgslExpr::Ident("a".to_string()))),
            ],
        );
        let diags = validate_module(&module);
        assert!(
            diags.is_empty(),
            "expected no diagnostics, got: {:?}",
            diags
        );
    }

    #[test]
    fn if_else_both_return_counts_as_return() {
        let module = module_with_func(
            "if_return",
            vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            WgslType::Scalar(ScalarType::F32),
            vec![WgslStatement::If {
                condition: WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("x".to_string())),
                    BinaryOp::Greater,
                    Box::new(WgslExpr::Literal(Literal::Float(0.0))),
                ),
                body: vec![WgslStatement::Return(Some(WgslExpr::Ident(
                    "x".to_string(),
                )))],
                else_body: Some(vec![WgslStatement::Return(Some(WgslExpr::Literal(
                    Literal::Float(0.0),
                )))]),
            }],
        );
        let diags = validate_module(&module);
        // Should have no "may not return" warning since both branches return.
        assert!(
            diags.iter().all(|d| !d.message.contains("may not return")),
            "got: {:?}",
            diags
        );
    }
}
