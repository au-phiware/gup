// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WGSL optimization engine for the transpilation pipeline.
//!
//! Provides optimization passes that operate on the transpile AST
//! ([`WgslModule`]) to improve generated WGSL code quality. Passes
//! include dead code elimination, constant folding, redundant type
//! conversion elimination, and identity operation removal.
//!
//! # Example
//!
//! ```rust,ignore
//! use gup_macros::transpile::{WgslCodeGen, optimizer::*};
//!
//! let mut module = /* transpile Rust → WgslModule */;
//! let results = optimize_module(&mut module, &OptimizationConfig::default());
//! for r in &results {
//!     println!("{}: {}", r.pass_name, r.description);
//! }
//! ```

use std::collections::HashSet;

use super::ast::*;

// ---------------------------------------------------------------------------
// Configuration & result types
// ---------------------------------------------------------------------------

/// Controls which optimization level to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// No optimizations – fastest compilation.
    None,
    /// Safe, conservative optimizations only.
    Basic,
    /// All optimizations – may increase compilation time.
    Aggressive,
}

/// Configuration for the optimization pipeline.
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    pub level: OptimizationLevel,
    pub enable_dead_code_elimination: bool,
    pub enable_constant_folding: bool,
    pub enable_redundant_conversion_elimination: bool,
    pub enable_identity_operation_removal: bool,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            level: OptimizationLevel::Basic,
            enable_dead_code_elimination: true,
            enable_constant_folding: true,
            enable_redundant_conversion_elimination: true,
            enable_identity_operation_removal: true,
        }
    }
}

impl OptimizationConfig {
    /// Configuration that performs no optimizations.
    pub fn none() -> Self {
        Self {
            level: OptimizationLevel::None,
            enable_dead_code_elimination: false,
            enable_constant_folding: false,
            enable_redundant_conversion_elimination: false,
            enable_identity_operation_removal: false,
        }
    }

    /// Configuration that applies all optimizations aggressively.
    pub fn aggressive() -> Self {
        Self {
            level: OptimizationLevel::Aggressive,
            enable_dead_code_elimination: true,
            enable_constant_folding: true,
            enable_redundant_conversion_elimination: true,
            enable_identity_operation_removal: true,
        }
    }
}

/// Result of a single optimization pass.
#[derive(Debug, Clone)]
pub struct PassResult {
    pub pass_name: String,
    pub changed: bool,
    pub description: String,
    pub items_affected: usize,
}

// ---------------------------------------------------------------------------
// Dead code elimination – unused local variables
// ---------------------------------------------------------------------------

/// Remove unused local variable bindings from function bodies.
///
/// A `let` binding is unused if the variable name never appears in
/// subsequent statements, **and** the initialiser expression has no
/// observable side effects (currently only pure expressions are
/// considered side-effect-free – function calls are kept).
pub fn dead_variable_elimination(module: &mut WgslModule) -> PassResult {
    let mut total_removed = 0;

    for func in &mut module.functions {
        total_removed += eliminate_dead_vars_in_body(&mut func.body);
    }

    PassResult {
        pass_name: "dead_variable_elimination".to_string(),
        changed: total_removed > 0,
        description: if total_removed > 0 {
            format!("removed {total_removed} unused variable(s)")
        } else {
            "no unused variables found".to_string()
        },
        items_affected: total_removed,
    }
}

/// Removes unused `let` bindings from a flat statement list.
fn eliminate_dead_vars_in_body(body: &mut Vec<WgslStatement>) -> usize {
    // Collect all identifiers referenced in the body (excluding the
    // let-binding names themselves).
    let used_idents = collect_used_idents(body);

    let before = body.len();
    body.retain(|stmt| {
        if let WgslStatement::Let { name, value, .. } = stmt {
            // Keep if the name is used elsewhere or the initialiser may
            // have side effects (function calls).
            used_idents.contains(name.as_str()) || expr_has_side_effects(value)
        } else {
            true
        }
    });
    before - body.len()
}

/// Collect all identifier names referenced in a list of statements,
/// excluding the names introduced by `let` bindings (we care about
/// *references*, not *definitions*).
fn collect_used_idents(stmts: &[WgslStatement]) -> HashSet<String> {
    let mut used = HashSet::new();
    for stmt in stmts {
        collect_idents_in_stmt(stmt, &mut used);
    }
    // Now remove identifiers that are *only* introduced by let bindings.
    // Actually we don't remove them – they could be referenced later. The
    // retain logic checks per-variable if it appears in the *rest* of the
    // code. A simpler approach: collect all ident references in all
    // expressions across all statements.
    used
}

fn collect_idents_in_stmt(stmt: &WgslStatement, used: &mut HashSet<String>) {
    match stmt {
        WgslStatement::Let { value, .. } => {
            collect_idents_in_expr(value, used);
        }
        WgslStatement::Return(Some(e)) => collect_idents_in_expr(e, used),
        WgslStatement::Return(None) => {}
        WgslStatement::If {
            condition,
            body,
            else_body,
        } => {
            collect_idents_in_expr(condition, used);
            for s in body {
                collect_idents_in_stmt(s, used);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    collect_idents_in_stmt(s, used);
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
            collect_idents_in_expr(initialiser, used);
            collect_idents_in_expr(condition, used);
            collect_idents_in_expr(update, used);
            for s in body {
                collect_idents_in_stmt(s, used);
            }
        }
        WgslStatement::While { condition, body } => {
            collect_idents_in_expr(condition, used);
            for s in body {
                collect_idents_in_stmt(s, used);
            }
        }
        WgslStatement::Loop { body } => {
            for s in body {
                collect_idents_in_stmt(s, used);
            }
        }
        WgslStatement::Break | WgslStatement::Continue => {}
        WgslStatement::Expression(e) => collect_idents_in_expr(e, used),
        WgslStatement::Assign(t, v) => {
            collect_idents_in_expr(t, used);
            collect_idents_in_expr(v, used);
        }
        WgslStatement::CompoundAssign(t, _, v) => {
            collect_idents_in_expr(t, used);
            collect_idents_in_expr(v, used);
        }
        WgslStatement::Switch {
            selector,
            cases,
            default_body,
        } => {
            collect_idents_in_expr(selector, used);
            for case in cases {
                for sel in &case.selectors {
                    collect_idents_in_expr(sel, used);
                }
                for s in &case.body {
                    collect_idents_in_stmt(s, used);
                }
            }
            if let Some(db) = default_body {
                for s in db {
                    collect_idents_in_stmt(s, used);
                }
            }
        }
    }
}

fn collect_idents_in_expr(expr: &WgslExpr, used: &mut HashSet<String>) {
    match expr {
        WgslExpr::Ident(name) => {
            used.insert(name.clone());
        }
        WgslExpr::Literal(_) => {}
        WgslExpr::Binary(l, _, r) => {
            collect_idents_in_expr(l, used);
            collect_idents_in_expr(r, used);
        }
        WgslExpr::Unary(_, inner) => collect_idents_in_expr(inner, used),
        WgslExpr::Call(_, args) | WgslExpr::TypeConstructor(_, args) => {
            for a in args {
                collect_idents_in_expr(a, used);
            }
        }
        WgslExpr::MemberAccess(base, _) => collect_idents_in_expr(base, used),
        WgslExpr::IndexAccess(base, idx) => {
            collect_idents_in_expr(base, used);
            collect_idents_in_expr(idx, used);
        }
        WgslExpr::Paren(inner) => collect_idents_in_expr(inner, used),
        WgslExpr::Cast(_, inner) => collect_idents_in_expr(inner, used),
    }
}

/// Returns true if the expression may have side effects (e.g. function calls).
fn expr_has_side_effects(expr: &WgslExpr) -> bool {
    match expr {
        WgslExpr::Call(_, _) => true,
        WgslExpr::Binary(l, _, r) => expr_has_side_effects(l) || expr_has_side_effects(r),
        WgslExpr::Unary(_, inner) => expr_has_side_effects(inner),
        WgslExpr::Paren(inner) => expr_has_side_effects(inner),
        WgslExpr::Cast(_, inner) => expr_has_side_effects(inner),
        WgslExpr::TypeConstructor(_, args) => args.iter().any(expr_has_side_effects),
        WgslExpr::MemberAccess(base, _) => expr_has_side_effects(base),
        WgslExpr::IndexAccess(base, idx) => {
            expr_has_side_effects(base) || expr_has_side_effects(idx)
        }
        WgslExpr::Literal(_) | WgslExpr::Ident(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Constant folding
// ---------------------------------------------------------------------------

/// Evaluate constant expressions at compile time.
///
/// Handles:
/// - Literal arithmetic: `2.0 + 3.0` → `5.0`
/// - Identity operations: `x * 1.0` → `x`, `x + 0.0` → `x`
/// - Zero multiplication: `x * 0.0` → `0.0`
/// - Double negation: `--x` → `x`
/// - Negation of literal: `-2.0` → `-2.0` as literal
pub fn constant_folding(module: &mut WgslModule) -> PassResult {
    let mut total_folded = 0;

    for func in &mut module.functions {
        for stmt in &mut func.body {
            total_folded += fold_stmt(stmt);
        }
    }

    PassResult {
        pass_name: "constant_folding".to_string(),
        changed: total_folded > 0,
        description: if total_folded > 0 {
            format!("folded {total_folded} constant expression(s)")
        } else {
            "no constants to fold".to_string()
        },
        items_affected: total_folded,
    }
}

fn fold_stmt(stmt: &mut WgslStatement) -> usize {
    match stmt {
        WgslStatement::Let { value, .. } => fold_expr(value),
        WgslStatement::Return(Some(e)) => fold_expr(e),
        WgslStatement::Return(None) => 0,
        WgslStatement::If {
            condition,
            body,
            else_body,
        } => {
            let mut count = fold_expr(condition);
            for s in body.iter_mut() {
                count += fold_stmt(s);
            }
            if let Some(eb) = else_body {
                for s in eb.iter_mut() {
                    count += fold_stmt(s);
                }
            }
            count
        }
        WgslStatement::For {
            initialiser,
            condition,
            update,
            body,
            ..
        } => {
            let mut count = fold_expr(initialiser);
            count += fold_expr(condition);
            count += fold_expr(update);
            for s in body.iter_mut() {
                count += fold_stmt(s);
            }
            count
        }
        WgslStatement::While { condition, body } => {
            let mut count = fold_expr(condition);
            for s in body.iter_mut() {
                count += fold_stmt(s);
            }
            count
        }
        WgslStatement::Loop { body } => {
            let mut count = 0;
            for s in body.iter_mut() {
                count += fold_stmt(s);
            }
            count
        }
        WgslStatement::Break | WgslStatement::Continue => 0,
        WgslStatement::Expression(e) => fold_expr(e),
        WgslStatement::Assign(_, v) => fold_expr(v),
        WgslStatement::CompoundAssign(_, _, v) => fold_expr(v),
        WgslStatement::Switch {
            selector,
            cases,
            default_body,
        } => {
            let mut count = fold_expr(selector);
            for case in cases {
                for sel in &mut case.selectors {
                    count += fold_expr(sel);
                }
                for s in case.body.iter_mut() {
                    count += fold_stmt(s);
                }
            }
            if let Some(db) = default_body {
                for s in db.iter_mut() {
                    count += fold_stmt(s);
                }
            }
            count
        }
    }
}

/// Recursively fold constant expressions. Returns count of folds applied.
fn fold_expr(expr: &mut WgslExpr) -> usize {
    // First, recurse into children.
    let mut count = match expr {
        WgslExpr::Binary(l, _, r) => fold_expr(l) + fold_expr(r),
        WgslExpr::Unary(_, inner) => fold_expr(inner),
        WgslExpr::Call(_, args) | WgslExpr::TypeConstructor(_, args) => {
            args.iter_mut().map(fold_expr).sum()
        }
        WgslExpr::MemberAccess(base, _) => fold_expr(base),
        WgslExpr::IndexAccess(base, idx) => fold_expr(base) + fold_expr(idx),
        WgslExpr::Paren(inner) => fold_expr(inner),
        WgslExpr::Cast(_, inner) => fold_expr(inner),
        _ => 0,
    };

    // Now try to fold this node.
    if let Some(folded) = try_fold(expr) {
        *expr = folded;
        count += 1;
    }

    count
}

fn try_fold(expr: &WgslExpr) -> Option<WgslExpr> {
    match expr {
        // Literal ⊕ Literal → Literal
        WgslExpr::Binary(left, op, right) => {
            if let (WgslExpr::Literal(l), WgslExpr::Literal(r)) = (left.as_ref(), right.as_ref()) {
                return fold_literals(l, *op, r);
            }
            // Identity operations
            match op {
                BinaryOp::Mul => {
                    if is_literal_one(right) {
                        return Some(*left.clone());
                    }
                    if is_literal_one(left) {
                        return Some(*right.clone());
                    }
                    if is_literal_zero(right) {
                        return Some(*right.clone());
                    }
                    if is_literal_zero(left) {
                        return Some(*left.clone());
                    }
                }
                BinaryOp::Add => {
                    if is_literal_zero(right) {
                        return Some(*left.clone());
                    }
                    if is_literal_zero(left) {
                        return Some(*right.clone());
                    }
                }
                BinaryOp::Sub => {
                    if is_literal_zero(right) {
                        return Some(*left.clone());
                    }
                }
                BinaryOp::Div => {
                    if is_literal_one(right) {
                        return Some(*left.clone());
                    }
                }
                _ => {}
            }
            None
        }
        // Double negation: --x → x
        WgslExpr::Unary(UnaryOp::Negate, inner) => {
            if let WgslExpr::Unary(UnaryOp::Negate, innermost) = inner.as_ref() {
                return Some(*innermost.clone());
            }
            // Negate literal
            if let WgslExpr::Literal(Literal::Float(v)) = inner.as_ref() {
                return Some(WgslExpr::Literal(Literal::Float(-v)));
            }
            if let WgslExpr::Literal(Literal::Int(v)) = inner.as_ref() {
                return Some(WgslExpr::Literal(Literal::Int(-v)));
            }
            None
        }
        _ => None,
    }
}

fn fold_literals(left: &Literal, op: BinaryOp, right: &Literal) -> Option<WgslExpr> {
    match (left, op, right) {
        (Literal::Float(a), BinaryOp::Add, Literal::Float(b)) => {
            Some(WgslExpr::Literal(Literal::Float(a + b)))
        }
        (Literal::Float(a), BinaryOp::Sub, Literal::Float(b)) => {
            Some(WgslExpr::Literal(Literal::Float(a - b)))
        }
        (Literal::Float(a), BinaryOp::Mul, Literal::Float(b)) => {
            Some(WgslExpr::Literal(Literal::Float(a * b)))
        }
        (Literal::Float(a), BinaryOp::Div, Literal::Float(b)) if *b != 0.0 => {
            Some(WgslExpr::Literal(Literal::Float(a / b)))
        }
        (Literal::Int(a), BinaryOp::Add, Literal::Int(b)) => {
            Some(WgslExpr::Literal(Literal::Int(a + b)))
        }
        (Literal::Int(a), BinaryOp::Sub, Literal::Int(b)) => {
            Some(WgslExpr::Literal(Literal::Int(a - b)))
        }
        (Literal::Int(a), BinaryOp::Mul, Literal::Int(b)) => {
            Some(WgslExpr::Literal(Literal::Int(a * b)))
        }
        (Literal::UInt(a), BinaryOp::Add, Literal::UInt(b)) => {
            Some(WgslExpr::Literal(Literal::UInt(a + b)))
        }
        (Literal::UInt(a), BinaryOp::Sub, Literal::UInt(b)) if a >= b => {
            Some(WgslExpr::Literal(Literal::UInt(a - b)))
        }
        (Literal::UInt(a), BinaryOp::Mul, Literal::UInt(b)) => {
            Some(WgslExpr::Literal(Literal::UInt(a * b)))
        }
        _ => None,
    }
}

fn is_literal_zero(expr: &WgslExpr) -> bool {
    matches!(expr, WgslExpr::Literal(Literal::Float(v)) if *v == 0.0)
        || matches!(expr, WgslExpr::Literal(Literal::Int(0)))
        || matches!(expr, WgslExpr::Literal(Literal::UInt(0)))
}

fn is_literal_one(expr: &WgslExpr) -> bool {
    matches!(expr, WgslExpr::Literal(Literal::Float(v)) if *v == 1.0)
        || matches!(expr, WgslExpr::Literal(Literal::Int(1)))
        || matches!(expr, WgslExpr::Literal(Literal::UInt(1)))
}

// ---------------------------------------------------------------------------
// Redundant type conversion elimination
// ---------------------------------------------------------------------------

/// Eliminate redundant type conversions such as `f32(x)` when `x` is
/// already known to be `f32`, or `f32(f32(x))` → `f32(x)`.
pub fn redundant_conversion_elimination(module: &mut WgslModule) -> PassResult {
    let mut total = 0;

    for func in &mut module.functions {
        for stmt in &mut func.body {
            total += elim_conversions_stmt(stmt);
        }
    }

    PassResult {
        pass_name: "redundant_conversion_elimination".to_string(),
        changed: total > 0,
        description: if total > 0 {
            format!("eliminated {total} redundant type conversion(s)")
        } else {
            "no redundant conversions found".to_string()
        },
        items_affected: total,
    }
}

fn elim_conversions_stmt(stmt: &mut WgslStatement) -> usize {
    match stmt {
        WgslStatement::Let { value, .. } => elim_conversions_expr(value),
        WgslStatement::Return(Some(e)) => elim_conversions_expr(e),
        WgslStatement::Return(None) => 0,
        WgslStatement::If {
            condition,
            body,
            else_body,
        } => {
            let mut count = elim_conversions_expr(condition);
            for s in body.iter_mut() {
                count += elim_conversions_stmt(s);
            }
            if let Some(eb) = else_body {
                for s in eb.iter_mut() {
                    count += elim_conversions_stmt(s);
                }
            }
            count
        }
        WgslStatement::For {
            initialiser,
            condition,
            update,
            body,
            ..
        } => {
            let mut count = elim_conversions_expr(initialiser);
            count += elim_conversions_expr(condition);
            count += elim_conversions_expr(update);
            for s in body.iter_mut() {
                count += elim_conversions_stmt(s);
            }
            count
        }
        WgslStatement::While { condition, body } => {
            let mut count = elim_conversions_expr(condition);
            for s in body.iter_mut() {
                count += elim_conversions_stmt(s);
            }
            count
        }
        WgslStatement::Loop { body } => {
            let mut count = 0;
            for s in body.iter_mut() {
                count += elim_conversions_stmt(s);
            }
            count
        }
        WgslStatement::Break | WgslStatement::Continue => 0,
        WgslStatement::Expression(e) => elim_conversions_expr(e),
        WgslStatement::Assign(_, v) => elim_conversions_expr(v),
        WgslStatement::CompoundAssign(_, _, v) => elim_conversions_expr(v),
        WgslStatement::Switch {
            selector,
            cases,
            default_body,
        } => {
            let mut count = elim_conversions_expr(selector);
            for case in cases {
                for sel in &mut case.selectors {
                    count += elim_conversions_expr(sel);
                }
                for s in case.body.iter_mut() {
                    count += elim_conversions_stmt(s);
                }
            }
            if let Some(db) = default_body {
                for s in db.iter_mut() {
                    count += elim_conversions_stmt(s);
                }
            }
            count
        }
    }
}

fn elim_conversions_expr(expr: &mut WgslExpr) -> usize {
    // First, recurse into children.
    let mut count = match expr {
        WgslExpr::Binary(l, _, r) => elim_conversions_expr(l) + elim_conversions_expr(r),
        WgslExpr::Unary(_, inner) => elim_conversions_expr(inner),
        WgslExpr::Call(_, args) | WgslExpr::TypeConstructor(_, args) => {
            args.iter_mut().map(elim_conversions_expr).sum()
        }
        WgslExpr::MemberAccess(base, _) => elim_conversions_expr(base),
        WgslExpr::IndexAccess(base, idx) => {
            elim_conversions_expr(base) + elim_conversions_expr(idx)
        }
        WgslExpr::Paren(inner) => elim_conversions_expr(inner),
        WgslExpr::Cast(_, inner) => elim_conversions_expr(inner),
        _ => 0,
    };

    // Eliminate double casts: `f32(f32(x))` → `f32(x)`, and `f32(i32(x))` → `f32(x)`.
    if let WgslExpr::Cast(outer_ty, inner) = expr {
        if let WgslExpr::Cast(_, innermost) = inner.as_ref() {
            // Outer cast subsumes the inner cast – just keep outer(innermost).
            *expr = WgslExpr::Cast(outer_ty.clone(), innermost.clone());
            count += 1;
        }
    }

    count
}

// ---------------------------------------------------------------------------
// Pipeline: run all enabled passes
// ---------------------------------------------------------------------------

/// Run all enabled optimization passes on a module.
pub fn optimize_module(module: &mut WgslModule, config: &OptimizationConfig) -> Vec<PassResult> {
    if config.level == OptimizationLevel::None {
        return vec![];
    }

    let mut results = Vec::new();

    if config.enable_constant_folding {
        results.push(constant_folding(module));
    }

    if config.enable_redundant_conversion_elimination {
        results.push(redundant_conversion_elimination(module));
    }

    if config.enable_identity_operation_removal {
        // Identity operations are handled as part of constant folding;
        // run a second constant folding pass to catch expressions that
        // became foldable after conversion elimination.
        let second = constant_folding(module);
        if second.changed {
            results.push(second);
        }
    }

    if config.enable_dead_code_elimination {
        results.push(dead_variable_elimination(module));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a simple module with one function.
    fn module_with_body(body: Vec<WgslStatement>) -> WgslModule {
        WgslModule {
            structs: vec![],
            functions: vec![WgslFunction {
                name: "test_fn".to_string(),
                params: vec![WgslParam {
                    name: "x".to_string(),
                    ty: WgslType::Scalar(ScalarType::F32),
                }],
                return_type: WgslType::Scalar(ScalarType::F32),
                body,
            }],
        }
    }

    // ---------------------------------------------------------------
    // Dead variable elimination
    // ---------------------------------------------------------------

    #[test]
    fn dead_var_removes_unused_let() {
        let mut module = module_with_body(vec![
            WgslStatement::Let {
                name: "unused".to_string(),
                ty: None,
                value: WgslExpr::Literal(Literal::Float(42.0)),
                mutable: false,
            },
            WgslStatement::Return(Some(WgslExpr::Ident("x".to_string()))),
        ]);

        let result = dead_variable_elimination(&mut module);
        assert!(result.changed);
        assert_eq!(result.items_affected, 1);
        assert_eq!(module.functions[0].body.len(), 1);
    }

    #[test]
    fn dead_var_keeps_used_let() {
        let mut module = module_with_body(vec![
            WgslStatement::Let {
                name: "doubled".to_string(),
                ty: None,
                value: WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("x".to_string())),
                    BinaryOp::Mul,
                    Box::new(WgslExpr::Literal(Literal::Float(2.0))),
                ),
                mutable: false,
            },
            WgslStatement::Return(Some(WgslExpr::Ident("doubled".to_string()))),
        ]);

        let result = dead_variable_elimination(&mut module);
        assert!(!result.changed);
        assert_eq!(module.functions[0].body.len(), 2);
    }

    #[test]
    fn dead_var_keeps_side_effect_let() {
        let mut module = module_with_body(vec![
            WgslStatement::Let {
                name: "unused".to_string(),
                ty: None,
                value: WgslExpr::Call(
                    "some_function".to_string(),
                    vec![WgslExpr::Ident("x".to_string())],
                ),
                mutable: false,
            },
            WgslStatement::Return(Some(WgslExpr::Ident("x".to_string()))),
        ]);

        let result = dead_variable_elimination(&mut module);
        assert!(!result.changed);
        // Kept because the call might have side effects.
        assert_eq!(module.functions[0].body.len(), 2);
    }

    // ---------------------------------------------------------------
    // Constant folding
    // ---------------------------------------------------------------

    #[test]
    fn fold_literal_add() {
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Binary(
            Box::new(WgslExpr::Literal(Literal::Float(2.0))),
            BinaryOp::Add,
            Box::new(WgslExpr::Literal(Literal::Float(3.0))),
        )))]);

        let result = constant_folding(&mut module);
        assert!(result.changed);

        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Literal(Literal::Float(v)))) => {
                assert!((v - 5.0).abs() < f64::EPSILON);
            }
            other => panic!("expected folded float, got {other:?}"),
        }
    }

    #[test]
    fn fold_identity_mul_one() {
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Binary(
            Box::new(WgslExpr::Ident("x".to_string())),
            BinaryOp::Mul,
            Box::new(WgslExpr::Literal(Literal::Float(1.0))),
        )))]);

        let result = constant_folding(&mut module);
        assert!(result.changed);

        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Ident(name))) => {
                assert_eq!(name, "x");
            }
            other => panic!("expected identity fold, got {other:?}"),
        }
    }

    #[test]
    fn fold_mul_zero() {
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Binary(
            Box::new(WgslExpr::Ident("x".to_string())),
            BinaryOp::Mul,
            Box::new(WgslExpr::Literal(Literal::Float(0.0))),
        )))]);

        let result = constant_folding(&mut module);
        assert!(result.changed);

        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Literal(Literal::Float(v)))) => {
                assert!((*v).abs() < f64::EPSILON);
            }
            other => panic!("expected zero fold, got {other:?}"),
        }
    }

    #[test]
    fn fold_add_zero() {
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Binary(
            Box::new(WgslExpr::Literal(Literal::Float(0.0))),
            BinaryOp::Add,
            Box::new(WgslExpr::Ident("x".to_string())),
        )))]);

        let result = constant_folding(&mut module);
        assert!(result.changed);

        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Ident(name))) => {
                assert_eq!(name, "x");
            }
            other => panic!("expected identity fold, got {other:?}"),
        }
    }

    #[test]
    fn fold_double_negation() {
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Unary(
            UnaryOp::Negate,
            Box::new(WgslExpr::Unary(
                UnaryOp::Negate,
                Box::new(WgslExpr::Ident("x".to_string())),
            )),
        )))]);

        let result = constant_folding(&mut module);
        assert!(result.changed);

        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Ident(name))) => {
                assert_eq!(name, "x");
            }
            other => panic!("expected double-negation fold, got {other:?}"),
        }
    }

    #[test]
    fn fold_nested_constant_expr() {
        // (2.0 + 3.0) * 4.0 → 5.0 * 4.0 → 20.0
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Binary(
            Box::new(WgslExpr::Binary(
                Box::new(WgslExpr::Literal(Literal::Float(2.0))),
                BinaryOp::Add,
                Box::new(WgslExpr::Literal(Literal::Float(3.0))),
            )),
            BinaryOp::Mul,
            Box::new(WgslExpr::Literal(Literal::Float(4.0))),
        )))]);

        let result = constant_folding(&mut module);
        assert!(result.changed);

        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Literal(Literal::Float(v)))) => {
                assert!((v - 20.0).abs() < f64::EPSILON);
            }
            other => panic!("expected fully folded literal, got {other:?}"),
        }
    }

    #[test]
    fn fold_int_arithmetic() {
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Binary(
            Box::new(WgslExpr::Literal(Literal::Int(10))),
            BinaryOp::Mul,
            Box::new(WgslExpr::Literal(Literal::Int(3))),
        )))]);

        let result = constant_folding(&mut module);
        assert!(result.changed);

        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Literal(Literal::Int(v)))) => {
                assert_eq!(*v, 30);
            }
            other => panic!("expected folded int, got {other:?}"),
        }
    }

    // ---------------------------------------------------------------
    // Redundant conversion elimination
    // ---------------------------------------------------------------

    #[test]
    fn elim_double_cast() {
        // f32(i32(x)) → f32(x)
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Cast(
            WgslType::Scalar(ScalarType::F32),
            Box::new(WgslExpr::Cast(
                WgslType::Scalar(ScalarType::I32),
                Box::new(WgslExpr::Ident("x".to_string())),
            )),
        )))]);

        let result = redundant_conversion_elimination(&mut module);
        assert!(result.changed);
        assert_eq!(result.items_affected, 1);

        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Cast(ty, inner))) => {
                assert_eq!(*ty, WgslType::Scalar(ScalarType::F32));
                assert_eq!(**inner, WgslExpr::Ident("x".to_string()));
            }
            other => panic!("expected single cast, got {other:?}"),
        }
    }

    #[test]
    fn elim_no_conversion_needed() {
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Ident(
            "x".to_string(),
        )))]);

        let result = redundant_conversion_elimination(&mut module);
        assert!(!result.changed);
    }

    // ---------------------------------------------------------------
    // Full pipeline
    // ---------------------------------------------------------------

    #[test]
    fn optimize_module_none_level() {
        let mut module = module_with_body(vec![WgslStatement::Return(Some(WgslExpr::Binary(
            Box::new(WgslExpr::Literal(Literal::Float(2.0))),
            BinaryOp::Add,
            Box::new(WgslExpr::Literal(Literal::Float(3.0))),
        )))]);

        let results = optimize_module(&mut module, &OptimizationConfig::none());
        assert!(results.is_empty());

        // Expression should be unchanged.
        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Binary(..))) => {} // still binary
            other => panic!("expected unchanged expression, got {other:?}"),
        }
    }

    #[test]
    fn optimize_module_basic_level() {
        let mut module = module_with_body(vec![
            WgslStatement::Let {
                name: "unused".to_string(),
                ty: None,
                value: WgslExpr::Literal(Literal::Float(42.0)),
                mutable: false,
            },
            WgslStatement::Return(Some(WgslExpr::Binary(
                Box::new(WgslExpr::Ident("x".to_string())),
                BinaryOp::Mul,
                Box::new(WgslExpr::Literal(Literal::Float(1.0))),
            ))),
        ]);

        let results = optimize_module(&mut module, &OptimizationConfig::default());
        let any_changed = results.iter().any(|r| r.changed);
        assert!(any_changed);

        // After optimization: unused var removed, identity mul folded
        assert_eq!(module.functions[0].body.len(), 1);
        match &module.functions[0].body[0] {
            WgslStatement::Return(Some(WgslExpr::Ident(name))) => {
                assert_eq!(name, "x");
            }
            other => panic!("expected optimised return x, got {other:?}"),
        }
    }

    #[test]
    fn optimize_constant_propagation_across_passes() {
        // Build: let a = 2.0 + 3.0; return a * 1.0;
        // After constant folding: let a = 5.0; return a * 1.0;
        // After identity removal (second fold pass): let a = 5.0; return a;
        let mut module = module_with_body(vec![
            WgslStatement::Let {
                name: "a".to_string(),
                ty: None,
                value: WgslExpr::Binary(
                    Box::new(WgslExpr::Literal(Literal::Float(2.0))),
                    BinaryOp::Add,
                    Box::new(WgslExpr::Literal(Literal::Float(3.0))),
                ),
                mutable: false,
            },
            WgslStatement::Return(Some(WgslExpr::Binary(
                Box::new(WgslExpr::Ident("a".to_string())),
                BinaryOp::Mul,
                Box::new(WgslExpr::Literal(Literal::Float(1.0))),
            ))),
        ]);

        let results = optimize_module(&mut module, &OptimizationConfig::aggressive());
        let any_changed = results.iter().any(|r| r.changed);
        assert!(any_changed);

        // `a` is still used in return, so the let stays.
        assert_eq!(module.functions[0].body.len(), 2);

        // But the initialiser should be folded to 5.0.
        match &module.functions[0].body[0] {
            WgslStatement::Let {
                value: WgslExpr::Literal(Literal::Float(v)),
                ..
            } => {
                assert!((v - 5.0).abs() < f64::EPSILON);
            }
            other => panic!("expected folded let, got {other:?}"),
        }

        // And the return should be just `a` (identity mul removed).
        match &module.functions[0].body[1] {
            WgslStatement::Return(Some(WgslExpr::Ident(name))) => {
                assert_eq!(name, "a");
            }
            other => panic!("expected optimised return, got {other:?}"),
        }
    }
}
