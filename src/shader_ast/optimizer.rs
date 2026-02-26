// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! AST-based optimization passes for WGSL shader composition.
//!
//! Provides dead code elimination, constant folding, and function inlining
//! that operate on the AST rather than string manipulation.

use super::types::*;
use std::collections::HashSet;

/// Result of an optimization pass: whether it changed anything.
pub struct OptimizationResult {
    pub changed: bool,
    pub description: String,
}

// -----------------------------------------------------------------------
// Dead code elimination
// -----------------------------------------------------------------------

/// Removes functions from a module that are never called.
///
/// Entry points (functions with `@vertex`, `@fragment`, or `@compute` attributes)
/// are always kept. All functions reachable from entry points via calls are kept.
pub fn dead_code_elimination(module: &mut WgslModule) -> OptimizationResult {
    if module.functions.is_empty() {
        return OptimizationResult {
            changed: false,
            description: "no functions to optimize".to_string(),
        };
    }

    // Collect entry point names.
    let entry_points: HashSet<String> = module
        .functions
        .iter()
        .filter(|f| {
            f.attributes.iter().any(|a| {
                matches!(
                    a,
                    Attribute::Vertex | Attribute::Fragment | Attribute::Compute
                )
            })
        })
        .map(|f| f.name.clone())
        .collect();

    // If no entry points, keep all functions (they may all be library functions).
    if entry_points.is_empty() {
        return OptimizationResult {
            changed: false,
            description: "no entry points found; all functions kept".to_string(),
        };
    }

    // BFS from entry points to find all reachable functions.
    let mut reachable: HashSet<String> = entry_points.clone();
    let mut queue: Vec<String> = entry_points.into_iter().collect();

    while let Some(name) = queue.pop() {
        if let Some(func) = module.functions.iter().find(|f| f.name == name) {
            let called = collect_called_functions_block(&func.body);
            for callee in called {
                if reachable.insert(callee.clone()) {
                    queue.push(callee);
                }
            }
        }
    }

    let before = module.functions.len();
    module.functions.retain(|f| reachable.contains(&f.name));
    let removed = before - module.functions.len();

    // Also remove unused globals and structs.
    let used_types = collect_used_types(module);
    let before_structs = module.structs.len();
    module.structs.retain(|s| used_types.contains(&s.name));
    let removed_structs = before_structs - module.structs.len();

    let before_globals = module.globals.len();
    let used_globals = collect_used_globals(module);
    module.globals.retain(|g| used_globals.contains(&g.name));
    let removed_globals = before_globals - module.globals.len();

    let total_removed = removed + removed_structs + removed_globals;

    OptimizationResult {
        changed: total_removed > 0,
        description: format!(
            "removed {removed} function(s), {removed_structs} struct(s), {removed_globals} global(s)"
        ),
    }
}

/// Collect all function names called in a block.
fn collect_called_functions_block(block: &Block) -> HashSet<String> {
    let mut called = HashSet::new();
    for stmt in &block.statements {
        collect_called_functions_stmt(stmt, &mut called);
    }
    called
}

fn collect_called_functions_stmt(stmt: &Statement, called: &mut HashSet<String>) {
    match stmt {
        Statement::Let { value, .. } => collect_called_functions_expr(value, called),
        Statement::Assign(target, value) => {
            collect_called_functions_expr(target, called);
            collect_called_functions_expr(value, called);
        }
        Statement::Return(Some(expr)) => collect_called_functions_expr(expr, called),
        Statement::Return(None) => {}
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_called_functions_expr(condition, called);
            for s in &body.statements {
                collect_called_functions_stmt(s, called);
            }
            if let Some(eb) = else_body {
                for s in &eb.statements {
                    collect_called_functions_stmt(s, called);
                }
            }
        }
        Statement::For {
            init,
            condition,
            update,
            body,
        } => {
            if let Some(init_stmt) = init {
                collect_called_functions_stmt(init_stmt, called);
            }
            if let Some(cond) = condition {
                collect_called_functions_expr(cond, called);
            }
            if let Some(upd) = update {
                collect_called_functions_stmt(upd, called);
            }
            for s in &body.statements {
                collect_called_functions_stmt(s, called);
            }
        }
        Statement::Expression(expr) => collect_called_functions_expr(expr, called),
        Statement::Block(block) => {
            for s in &block.statements {
                collect_called_functions_stmt(s, called);
            }
        }
        Statement::CompoundAssign(target, _, value) => {
            collect_called_functions_expr(target, called);
            collect_called_functions_expr(value, called);
        }
        Statement::Loop { body } => {
            for s in &body.statements {
                collect_called_functions_stmt(s, called);
            }
        }
        Statement::Break | Statement::Continue => {}
        Statement::Switch { subject, cases } => {
            collect_called_functions_expr(subject, called);
            for case in cases {
                for s in &case.body.statements {
                    collect_called_functions_stmt(s, called);
                }
            }
        }
    }
}

fn collect_called_functions_expr(expr: &Expr, called: &mut HashSet<String>) {
    match expr {
        Expr::Call(name, args) => {
            called.insert(name.clone());
            for arg in args {
                collect_called_functions_expr(arg, called);
            }
        }
        Expr::TypeConstructor(_, args) => {
            for arg in args {
                collect_called_functions_expr(arg, called);
            }
        }
        Expr::Binary(left, _, right) => {
            collect_called_functions_expr(left, called);
            collect_called_functions_expr(right, called);
        }
        Expr::Unary(_, inner) => collect_called_functions_expr(inner, called),
        Expr::MemberAccess(base, _) => collect_called_functions_expr(base, called),
        Expr::IndexAccess(base, index) => {
            collect_called_functions_expr(base, called);
            collect_called_functions_expr(index, called);
        }
        Expr::Literal(_) | Expr::Ident(_) => {}
    }
}

/// Collect struct type names used in function parameters, return types, and globals.
fn collect_used_types(module: &WgslModule) -> HashSet<String> {
    let mut used = HashSet::new();
    for func in &module.functions {
        for param in &func.parameters {
            collect_type_names(&param.ty, &mut used);
        }
        if let Some(ref ret) = func.return_type {
            collect_type_names(ret, &mut used);
        }
    }
    for g in &module.globals {
        collect_type_names(&g.ty, &mut used);
    }
    used
}

fn collect_type_names(ty: &WgslType, names: &mut HashSet<String>) {
    match ty {
        WgslType::Struct(name) => {
            names.insert(name.clone());
        }
        WgslType::Array(elem, _) | WgslType::Atomic(elem) | WgslType::Pointer(_, elem) => {
            collect_type_names(elem, names);
        }
        _ => {}
    }
}

/// Collect identifiers used in any expression of the module's functions.
fn collect_used_globals(module: &WgslModule) -> HashSet<String> {
    let mut used = HashSet::new();
    for func in &module.functions {
        collect_idents_block(&func.body, &mut used);
    }
    used
}

fn collect_idents_block(block: &Block, idents: &mut HashSet<String>) {
    for stmt in &block.statements {
        collect_idents_stmt(stmt, idents);
    }
}

fn collect_idents_stmt(stmt: &Statement, idents: &mut HashSet<String>) {
    match stmt {
        Statement::Let { value, .. } => collect_idents_expr(value, idents),
        Statement::Assign(t, v) => {
            collect_idents_expr(t, idents);
            collect_idents_expr(v, idents);
        }
        Statement::Return(Some(e)) => collect_idents_expr(e, idents),
        Statement::Return(None) => {}
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_idents_expr(condition, idents);
            collect_idents_block(body, idents);
            if let Some(eb) = else_body {
                collect_idents_block(eb, idents);
            }
        }
        Statement::For {
            init,
            condition,
            update,
            body,
        } => {
            if let Some(i) = init {
                collect_idents_stmt(i, idents);
            }
            if let Some(c) = condition {
                collect_idents_expr(c, idents);
            }
            if let Some(u) = update {
                collect_idents_stmt(u, idents);
            }
            collect_idents_block(body, idents);
        }
        Statement::Expression(e) => collect_idents_expr(e, idents),
        Statement::Block(b) => collect_idents_block(b, idents),
        Statement::CompoundAssign(t, _, v) => {
            collect_idents_expr(t, idents);
            collect_idents_expr(v, idents);
        }
        Statement::Loop { body } => collect_idents_block(body, idents),
        Statement::Break | Statement::Continue => {}
        Statement::Switch { subject, cases } => {
            collect_idents_expr(subject, idents);
            for case in cases {
                collect_idents_block(&case.body, idents);
            }
        }
    }
}

fn collect_idents_expr(expr: &Expr, idents: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name) => {
            idents.insert(name.clone());
        }
        Expr::Call(_, args) | Expr::TypeConstructor(_, args) => {
            for a in args {
                collect_idents_expr(a, idents);
            }
        }
        Expr::Binary(l, _, r) => {
            collect_idents_expr(l, idents);
            collect_idents_expr(r, idents);
        }
        Expr::Unary(_, inner) => collect_idents_expr(inner, idents),
        Expr::MemberAccess(base, _) => collect_idents_expr(base, idents),
        Expr::IndexAccess(base, idx) => {
            collect_idents_expr(base, idents);
            collect_idents_expr(idx, idents);
        }
        Expr::Literal(_) => {}
    }
}

// -----------------------------------------------------------------------
// Constant folding
// -----------------------------------------------------------------------

/// Evaluate constant expressions at composition time.
///
/// Simplifies expressions such as:
/// - `1.0 * x` → `x`
/// - `x + 0.0` → `x`
/// - `0.0 * x` → `0.0`
/// - Literal arithmetic: `2.0 + 3.0` → `5.0`
pub fn constant_folding(module: &mut WgslModule) -> OptimizationResult {
    let mut changed = false;
    for func in &mut module.functions {
        if fold_block(&mut func.body) {
            changed = true;
        }
    }

    OptimizationResult {
        changed,
        description: if changed {
            "folded constant expressions".to_string()
        } else {
            "no constants to fold".to_string()
        },
    }
}

fn fold_block(block: &mut Block) -> bool {
    let mut changed = false;
    for stmt in &mut block.statements {
        if fold_statement(stmt) {
            changed = true;
        }
    }
    changed
}

fn fold_statement(stmt: &mut Statement) -> bool {
    match stmt {
        Statement::Let { value, .. } => fold_expr(value),
        Statement::Assign(_, value) => fold_expr(value),
        Statement::Return(Some(expr)) => fold_expr(expr),
        Statement::Return(None) => false,
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            let mut changed = fold_expr(condition);
            if fold_block(body) {
                changed = true;
            }
            if let Some(eb) = else_body
                && fold_block(eb)
            {
                changed = true;
            }
            changed
        }
        Statement::For {
            init,
            condition,
            update,
            body,
        } => {
            let mut changed = false;
            if let Some(i) = init
                && fold_statement(i)
            {
                changed = true;
            }
            if let Some(c) = condition
                && fold_expr(c)
            {
                changed = true;
            }
            if let Some(u) = update
                && fold_statement(u)
            {
                changed = true;
            }
            if fold_block(body) {
                changed = true;
            }
            changed
        }
        Statement::Expression(expr) => fold_expr(expr),
        Statement::Block(block) => fold_block(block),
        Statement::CompoundAssign(target, _, value) => {
            let mut changed = fold_expr(target);
            if fold_expr(value) {
                changed = true;
            }
            changed
        }
        Statement::Loop { body } => fold_block(body),
        Statement::Break | Statement::Continue => false,
        Statement::Switch { subject, cases } => {
            let mut changed = fold_expr(subject);
            for case in cases {
                if let Some(ref mut sel) = case.selector
                    && fold_expr(sel)
                {
                    changed = true;
                }
                if fold_block(&mut case.body) {
                    changed = true;
                }
            }
            changed
        }
    }
}

/// Recursively fold constant expressions. Returns true if any change was made.
fn fold_expr(expr: &mut Expr) -> bool {
    // First, recursively fold children.
    match expr {
        Expr::Binary(left, _, right) => {
            fold_expr(left);
            fold_expr(right);
        }
        Expr::Unary(_, inner) => {
            fold_expr(inner);
        }
        Expr::Call(_, args) | Expr::TypeConstructor(_, args) => {
            for arg in args.iter_mut() {
                fold_expr(arg);
            }
        }
        Expr::MemberAccess(base, _) => {
            fold_expr(base);
        }
        Expr::IndexAccess(base, index) => {
            fold_expr(base);
            fold_expr(index);
        }
        _ => {}
    }

    // Now try to fold this node.
    if let Some(folded) = try_fold(expr) {
        *expr = folded;
        return true;
    }

    false
}

/// Try to fold a single expression. Returns `Some(folded)` if foldable.
fn try_fold(expr: &Expr) -> Option<Expr> {
    match expr {
        // Literal + Literal → Literal
        Expr::Binary(left, op, right) => {
            if let (Expr::Literal(l), Expr::Literal(r)) = (left.as_ref(), right.as_ref()) {
                return fold_literals(l, *op, r);
            }
            // Identity operations
            match op {
                // x * 1.0 → x, 1.0 * x → x
                BinaryOp::Mul => {
                    if is_literal_one(right) {
                        return Some(*left.clone());
                    }
                    if is_literal_one(left) {
                        return Some(*right.clone());
                    }
                    // x * 0.0 → 0.0, 0.0 * x → 0.0
                    if is_literal_zero(right) {
                        return Some(*right.clone());
                    }
                    if is_literal_zero(left) {
                        return Some(*left.clone());
                    }
                }
                // x + 0.0 → x, 0.0 + x → x
                BinaryOp::Add => {
                    if is_literal_zero(right) {
                        return Some(*left.clone());
                    }
                    if is_literal_zero(left) {
                        return Some(*right.clone());
                    }
                }
                // x - 0.0 → x
                BinaryOp::Sub => {
                    if is_literal_zero(right) {
                        return Some(*left.clone());
                    }
                }
                // x / 1.0 → x
                BinaryOp::Div => {
                    if is_literal_one(right) {
                        return Some(*left.clone());
                    }
                }
                _ => {}
            }
            None
        }
        // -(-x) → x
        Expr::Unary(UnaryOp::Negate, inner) => {
            if let Expr::Unary(UnaryOp::Negate, innermost) = inner.as_ref() {
                return Some(*innermost.clone());
            }
            // Negate a literal
            if let Expr::Literal(Literal::Float(v)) = inner.as_ref() {
                return Some(Expr::Literal(Literal::Float(-v)));
            }
            if let Expr::Literal(Literal::Int(v)) = inner.as_ref() {
                return Some(Expr::Literal(Literal::Int(-v)));
            }
            None
        }
        _ => None,
    }
}

fn fold_literals(left: &Literal, op: BinaryOp, right: &Literal) -> Option<Expr> {
    match (left, op, right) {
        (Literal::Float(a), BinaryOp::Add, Literal::Float(b)) => {
            Some(Expr::Literal(Literal::Float(a + b)))
        }
        (Literal::Float(a), BinaryOp::Sub, Literal::Float(b)) => {
            Some(Expr::Literal(Literal::Float(a - b)))
        }
        (Literal::Float(a), BinaryOp::Mul, Literal::Float(b)) => {
            Some(Expr::Literal(Literal::Float(a * b)))
        }
        (Literal::Float(a), BinaryOp::Div, Literal::Float(b)) if *b != 0.0 => {
            Some(Expr::Literal(Literal::Float(a / b)))
        }
        (Literal::Int(a), BinaryOp::Add, Literal::Int(b)) => {
            Some(Expr::Literal(Literal::Int(a + b)))
        }
        (Literal::Int(a), BinaryOp::Sub, Literal::Int(b)) => {
            Some(Expr::Literal(Literal::Int(a - b)))
        }
        (Literal::Int(a), BinaryOp::Mul, Literal::Int(b)) => {
            Some(Expr::Literal(Literal::Int(a * b)))
        }
        _ => None,
    }
}

fn is_literal_zero(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Literal(Literal::Float(v)) if *v == 0.0
    ) || matches!(expr, Expr::Literal(Literal::Int(0)))
}

fn is_literal_one(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Literal(Literal::Float(v)) if *v == 1.0
    ) || matches!(expr, Expr::Literal(Literal::Int(1)))
}

// -----------------------------------------------------------------------
// Function inlining
// -----------------------------------------------------------------------

/// Inline small functions (fewer than `max_statements` statements) at call sites.
///
/// Only inlines functions that:
/// - Have a single `return expr;` body
/// - Are called at most `max_call_sites` times
/// - Are not entry points
pub fn function_inlining(
    module: &mut WgslModule,
    max_statements: usize,
    max_call_sites: usize,
) -> OptimizationResult {
    // Identify functions eligible for inlining.
    let inline_candidates: Vec<(String, Vec<String>, Expr)> = module
        .functions
        .iter()
        .filter(|f| {
            // Not an entry point.
            !f.attributes.iter().any(|a| {
                matches!(
                    a,
                    Attribute::Vertex | Attribute::Fragment | Attribute::Compute
                )
            })
            // Small enough.
            && f.body.statements.len() <= max_statements
            // Single return expression.
            && f.body.statements.len() == 1
            && matches!(&f.body.statements[0], Statement::Return(Some(_)))
        })
        .filter_map(|f| {
            if let Statement::Return(Some(expr)) = &f.body.statements[0] {
                let param_names: Vec<String> =
                    f.parameters.iter().map(|p| p.name.clone()).collect();
                Some((f.name.clone(), param_names, expr.clone()))
            } else {
                None
            }
        })
        .collect();

    if inline_candidates.is_empty() {
        return OptimizationResult {
            changed: false,
            description: "no functions eligible for inlining".to_string(),
        };
    }

    // Count call sites for each candidate.
    let mut call_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for func in &module.functions {
        let called = collect_called_functions_block(&func.body);
        for name in called {
            *call_counts.entry(name).or_default() += 1;
        }
    }

    // Filter by max call sites.
    let eligible: Vec<_> = inline_candidates
        .into_iter()
        .filter(|(name, _, _)| call_counts.get(name).copied().unwrap_or(0) <= max_call_sites)
        .collect();

    if eligible.is_empty() {
        return OptimizationResult {
            changed: false,
            description: "no functions within call-site threshold".to_string(),
        };
    }

    // Perform inlining: replace Call(name, args) with the body expression,
    // substituting parameters with arguments.
    let mut changed = false;
    let mut inlined_names = Vec::new();

    for (name, params, body_expr) in &eligible {
        for func in &mut module.functions {
            if func.name == *name {
                continue; // Don't inline into itself.
            }
            if inline_calls_in_block(&mut func.body, name, params, body_expr) {
                changed = true;
                if !inlined_names.contains(name) {
                    inlined_names.push(name.clone());
                }
            }
        }
    }

    OptimizationResult {
        changed,
        description: if inlined_names.is_empty() {
            "no call sites inlined".to_string()
        } else {
            format!("inlined function(s): {}", inlined_names.join(", "))
        },
    }
}

fn inline_calls_in_block(
    block: &mut Block,
    func_name: &str,
    params: &[String],
    body_expr: &Expr,
) -> bool {
    let mut changed = false;
    for stmt in &mut block.statements {
        if inline_calls_in_stmt(stmt, func_name, params, body_expr) {
            changed = true;
        }
    }
    changed
}

fn inline_calls_in_stmt(
    stmt: &mut Statement,
    func_name: &str,
    params: &[String],
    body_expr: &Expr,
) -> bool {
    match stmt {
        Statement::Let { value, .. } => inline_calls_in_expr(value, func_name, params, body_expr),
        Statement::Assign(_, value) => inline_calls_in_expr(value, func_name, params, body_expr),
        Statement::Return(Some(expr)) => inline_calls_in_expr(expr, func_name, params, body_expr),
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            let mut changed = inline_calls_in_expr(condition, func_name, params, body_expr);
            if inline_calls_in_block(body, func_name, params, body_expr) {
                changed = true;
            }
            if let Some(eb) = else_body
                && inline_calls_in_block(eb, func_name, params, body_expr)
            {
                changed = true;
            }
            changed
        }
        Statement::For {
            init,
            condition,
            update,
            body,
        } => {
            let mut changed = false;
            if let Some(i) = init
                && inline_calls_in_stmt(i, func_name, params, body_expr)
            {
                changed = true;
            }
            if let Some(c) = condition
                && inline_calls_in_expr(c, func_name, params, body_expr)
            {
                changed = true;
            }
            if let Some(u) = update
                && inline_calls_in_stmt(u, func_name, params, body_expr)
            {
                changed = true;
            }
            if inline_calls_in_block(body, func_name, params, body_expr) {
                changed = true;
            }
            changed
        }
        Statement::Expression(expr) => inline_calls_in_expr(expr, func_name, params, body_expr),
        Statement::Block(block) => inline_calls_in_block(block, func_name, params, body_expr),
        Statement::Return(None) => false,
        Statement::CompoundAssign(target, _, value) => {
            let t = inline_calls_in_expr(target, func_name, params, body_expr);
            let v = inline_calls_in_expr(value, func_name, params, body_expr);
            t || v
        }
        Statement::Loop { body } => inline_calls_in_block(body, func_name, params, body_expr),
        Statement::Break | Statement::Continue => false,
        Statement::Switch { subject, cases } => {
            let mut changed = inline_calls_in_expr(subject, func_name, params, body_expr);
            for case in cases {
                if let Some(ref mut sel) = case.selector
                    && inline_calls_in_expr(sel, func_name, params, body_expr)
                {
                    changed = true;
                }
                if inline_calls_in_block(&mut case.body, func_name, params, body_expr) {
                    changed = true;
                }
            }
            changed
        }
    }
}

fn inline_calls_in_expr(
    expr: &mut Expr,
    func_name: &str,
    params: &[String],
    body_expr: &Expr,
) -> bool {
    // First recurse into children.
    match expr {
        Expr::Binary(left, _, right) => {
            let l = inline_calls_in_expr(left, func_name, params, body_expr);
            let r = inline_calls_in_expr(right, func_name, params, body_expr);
            if l || r {
                return true;
            }
        }
        Expr::Unary(_, inner) => {
            if inline_calls_in_expr(inner, func_name, params, body_expr) {
                return true;
            }
        }
        Expr::MemberAccess(base, _) => {
            if inline_calls_in_expr(base, func_name, params, body_expr) {
                return true;
            }
        }
        Expr::IndexAccess(base, index) => {
            let b = inline_calls_in_expr(base, func_name, params, body_expr);
            let i = inline_calls_in_expr(index, func_name, params, body_expr);
            if b || i {
                return true;
            }
        }
        Expr::TypeConstructor(_, args) => {
            let mut changed = false;
            for arg in args.iter_mut() {
                if inline_calls_in_expr(arg, func_name, params, body_expr) {
                    changed = true;
                }
            }
            if changed {
                return true;
            }
        }
        _ => {}
    }

    // Check if this is the target call.
    if let Expr::Call(name, args) = expr {
        if name == func_name && args.len() == params.len() {
            // Substitute parameters with arguments in the body expression.
            let mut inlined = body_expr.clone();
            for (param, arg) in params.iter().zip(args.iter()) {
                substitute_ident(&mut inlined, param, arg);
            }
            *expr = inlined;
            return true;
        }
        // Recurse into call arguments.
        let mut changed = false;
        for arg in args.iter_mut() {
            if inline_calls_in_expr(arg, func_name, params, body_expr) {
                changed = true;
            }
        }
        return changed;
    }

    false
}

/// Replace all occurrences of identifier `name` with `replacement` in `expr`.
fn substitute_ident(expr: &mut Expr, name: &str, replacement: &Expr) {
    match expr {
        Expr::Ident(id) if id == name => {
            *expr = replacement.clone();
        }
        Expr::Binary(left, _, right) => {
            substitute_ident(left, name, replacement);
            substitute_ident(right, name, replacement);
        }
        Expr::Unary(_, inner) => substitute_ident(inner, name, replacement),
        Expr::Call(_, args) | Expr::TypeConstructor(_, args) => {
            for arg in args.iter_mut() {
                substitute_ident(arg, name, replacement);
            }
        }
        Expr::MemberAccess(base, _) => substitute_ident(base, name, replacement),
        Expr::IndexAccess(base, index) => {
            substitute_ident(base, name, replacement);
            substitute_ident(index, name, replacement);
        }
        _ => {}
    }
}

// -----------------------------------------------------------------------
// Convenience: run all optimization passes
// -----------------------------------------------------------------------

/// Configuration for the optimization pipeline.
#[derive(Debug, Clone)]
pub struct AstOptimizationConfig {
    pub enable_dead_code_elimination: bool,
    pub enable_constant_folding: bool,
    pub enable_function_inlining: bool,
    pub inline_max_statements: usize,
    pub inline_max_call_sites: usize,
}

impl Default for AstOptimizationConfig {
    fn default() -> Self {
        Self {
            enable_dead_code_elimination: true,
            enable_constant_folding: true,
            enable_function_inlining: true,
            inline_max_statements: 1,
            inline_max_call_sites: 3,
        }
    }
}

/// Run all enabled optimization passes on a module.
///
/// Returns descriptions of what changed.
pub fn optimize(
    module: &mut WgslModule,
    config: &AstOptimizationConfig,
) -> Vec<OptimizationResult> {
    let mut results = Vec::new();

    if config.enable_dead_code_elimination {
        results.push(dead_code_elimination(module));
    }

    if config.enable_constant_folding {
        results.push(constant_folding(module));
    }

    if config.enable_function_inlining {
        let inlining_result = function_inlining(
            module,
            config.inline_max_statements,
            config.inline_max_call_sites,
        );

        // Re-run DCE after inlining to remove functions that became dead
        // after their call sites were replaced with inlined code.
        if inlining_result.changed && config.enable_dead_code_elimination {
            let dce_result = dead_code_elimination(module);
            if dce_result.changed {
                results.push(dce_result);
            }
        }

        results.push(inlining_result);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_function(name: &str, attrs: Vec<Attribute>) -> Function {
        Function {
            name: name.to_string(),
            parameters: vec![Parameter {
                name: "value".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
                attributes: vec![],
            }],
            return_type: Some(WgslType::Scalar(ScalarType::F32)),
            body: Block::new(vec![Statement::Return(Some(Expr::Ident(
                "value".to_string(),
            )))]),
            attributes: attrs,
            return_attributes: vec![],
        }
    }

    #[test]
    fn test_dead_code_elimination_removes_unused() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![
                // Entry point that only calls "used_fn"
                Function {
                    name: "vs_main".to_string(),
                    parameters: vec![],
                    return_type: Some(WgslType::Scalar(ScalarType::F32)),
                    body: Block::new(vec![Statement::Return(Some(Expr::Call(
                        "used_fn".to_string(),
                        vec![Expr::Literal(Literal::Float(1.0))],
                    )))]),
                    attributes: vec![Attribute::Vertex],
                    return_attributes: vec![],
                },
                make_simple_function("used_fn", vec![]),
                make_simple_function("unused_fn", vec![]),
            ],
        };

        let result = dead_code_elimination(&mut module);
        assert!(result.changed);
        assert_eq!(module.functions.len(), 2);
        assert!(module.find_function("vs_main").is_some());
        assert!(module.find_function("used_fn").is_some());
        assert!(module.find_function("unused_fn").is_none());
    }

    #[test]
    fn test_dead_code_elimination_keeps_transitive() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![
                Function {
                    name: "vs_main".to_string(),
                    parameters: vec![],
                    return_type: Some(WgslType::Scalar(ScalarType::F32)),
                    body: Block::new(vec![Statement::Return(Some(Expr::Call(
                        "a".to_string(),
                        vec![Expr::Literal(Literal::Float(1.0))],
                    )))]),
                    attributes: vec![Attribute::Vertex],
                    return_attributes: vec![],
                },
                // a calls b
                Function {
                    name: "a".to_string(),
                    parameters: vec![Parameter {
                        name: "x".to_string(),
                        ty: WgslType::Scalar(ScalarType::F32),
                        attributes: vec![],
                    }],
                    return_type: Some(WgslType::Scalar(ScalarType::F32)),
                    body: Block::new(vec![Statement::Return(Some(Expr::Call(
                        "b".to_string(),
                        vec![Expr::Ident("x".to_string())],
                    )))]),
                    attributes: vec![],
                    return_attributes: vec![],
                },
                make_simple_function("b", vec![]),
                make_simple_function("unreachable", vec![]),
            ],
        };

        let result = dead_code_elimination(&mut module);
        assert!(result.changed);
        assert_eq!(module.functions.len(), 3);
        assert!(module.find_function("unreachable").is_none());
    }

    #[test]
    fn test_constant_folding_literal_arithmetic() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![Function {
                name: "test".to_string(),
                parameters: vec![],
                return_type: Some(WgslType::Scalar(ScalarType::F32)),
                body: Block::new(vec![Statement::Return(Some(Expr::Binary(
                    Box::new(Expr::Literal(Literal::Float(2.0))),
                    BinaryOp::Add,
                    Box::new(Expr::Literal(Literal::Float(3.0))),
                )))]),
                attributes: vec![],
                return_attributes: vec![],
            }],
        };

        let result = constant_folding(&mut module);
        assert!(result.changed);

        // Should fold to 5.0
        match &module.functions[0].body.statements[0] {
            Statement::Return(Some(Expr::Literal(Literal::Float(v)))) => {
                assert!((v - 5.0).abs() < f64::EPSILON);
            }
            other => panic!("expected folded literal, got {other:?}"),
        }
    }

    #[test]
    fn test_constant_folding_identity_mul() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![Function {
                name: "test".to_string(),
                parameters: vec![Parameter {
                    name: "x".to_string(),
                    ty: WgslType::Scalar(ScalarType::F32),
                    attributes: vec![],
                }],
                return_type: Some(WgslType::Scalar(ScalarType::F32)),
                body: Block::new(vec![Statement::Return(Some(Expr::Binary(
                    Box::new(Expr::Ident("x".to_string())),
                    BinaryOp::Mul,
                    Box::new(Expr::Literal(Literal::Float(1.0))),
                )))]),
                attributes: vec![],
                return_attributes: vec![],
            }],
        };

        let result = constant_folding(&mut module);
        assert!(result.changed);

        // x * 1.0 → x
        match &module.functions[0].body.statements[0] {
            Statement::Return(Some(Expr::Ident(name))) => {
                assert_eq!(name, "x");
            }
            other => panic!("expected folded identity, got {other:?}"),
        }
    }

    #[test]
    fn test_constant_folding_zero_add() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![Function {
                name: "test".to_string(),
                parameters: vec![Parameter {
                    name: "x".to_string(),
                    ty: WgslType::Scalar(ScalarType::F32),
                    attributes: vec![],
                }],
                return_type: Some(WgslType::Scalar(ScalarType::F32)),
                body: Block::new(vec![Statement::Return(Some(Expr::Binary(
                    Box::new(Expr::Literal(Literal::Float(0.0))),
                    BinaryOp::Add,
                    Box::new(Expr::Ident("x".to_string())),
                )))]),
                attributes: vec![],
                return_attributes: vec![],
            }],
        };

        let result = constant_folding(&mut module);
        assert!(result.changed);

        // 0.0 + x → x
        match &module.functions[0].body.statements[0] {
            Statement::Return(Some(Expr::Ident(name))) => {
                assert_eq!(name, "x");
            }
            other => panic!("expected folded zero add, got {other:?}"),
        }
    }

    #[test]
    fn test_function_inlining_single_return() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![
                // Small function to inline
                Function {
                    name: "double".to_string(),
                    parameters: vec![Parameter {
                        name: "x".to_string(),
                        ty: WgslType::Scalar(ScalarType::F32),
                        attributes: vec![],
                    }],
                    return_type: Some(WgslType::Scalar(ScalarType::F32)),
                    body: Block::new(vec![Statement::Return(Some(Expr::Binary(
                        Box::new(Expr::Ident("x".to_string())),
                        BinaryOp::Mul,
                        Box::new(Expr::Literal(Literal::Float(2.0))),
                    )))]),
                    attributes: vec![],
                    return_attributes: vec![],
                },
                // Caller
                Function {
                    name: "main".to_string(),
                    parameters: vec![Parameter {
                        name: "val".to_string(),
                        ty: WgslType::Scalar(ScalarType::F32),
                        attributes: vec![],
                    }],
                    return_type: Some(WgslType::Scalar(ScalarType::F32)),
                    body: Block::new(vec![Statement::Return(Some(Expr::Call(
                        "double".to_string(),
                        vec![Expr::Ident("val".to_string())],
                    )))]),
                    attributes: vec![Attribute::Vertex],
                    return_attributes: vec![],
                },
            ],
        };

        let result = function_inlining(&mut module, 1, 3);
        assert!(result.changed);

        // The call to double(val) should be replaced with val * 2.0
        match &module.functions[1].body.statements[0] {
            Statement::Return(Some(Expr::Binary(left, BinaryOp::Mul, right))) => {
                assert!(matches!(left.as_ref(), Expr::Ident(n) if n == "val"));
                assert!(
                    matches!(right.as_ref(), Expr::Literal(Literal::Float(v)) if (*v - 2.0).abs() < f64::EPSILON)
                );
            }
            other => panic!("expected inlined expression, got {other:?}"),
        }
    }

    #[test]
    fn test_optimize_all_passes() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![
                Function {
                    name: "vs_main".to_string(),
                    parameters: vec![],
                    return_type: Some(WgslType::Scalar(ScalarType::F32)),
                    body: Block::new(vec![Statement::Return(Some(Expr::Binary(
                        Box::new(Expr::Literal(Literal::Float(2.0))),
                        BinaryOp::Add,
                        Box::new(Expr::Literal(Literal::Float(3.0))),
                    )))]),
                    attributes: vec![Attribute::Vertex],
                    return_attributes: vec![],
                },
                make_simple_function("unused", vec![]),
            ],
        };

        let results = optimize(&mut module, &AstOptimizationConfig::default());
        assert!(!results.is_empty());

        // Dead code elimination should remove "unused"
        assert_eq!(module.functions.len(), 1);
        // Constant folding should simplify 2.0 + 3.0 → 5.0
        match &module.functions[0].body.statements[0] {
            Statement::Return(Some(Expr::Literal(Literal::Float(v)))) => {
                assert!((v - 5.0).abs() < f64::EPSILON);
            }
            other => panic!("expected folded and cleaned result, got {other:?}"),
        }
    }

    // --- Compute shader DCE tests ---

    #[test]
    fn test_dce_compute_entry_point_kept() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![
                Function {
                    name: "compute_main".to_string(),
                    parameters: vec![],
                    return_type: None,
                    body: Block::new(vec![Statement::Expression(Expr::Call(
                        "helper".to_string(),
                        vec![],
                    ))]),
                    attributes: vec![
                        Attribute::Compute,
                        Attribute::WorkgroupSize(256, None, None),
                    ],
                    return_attributes: vec![],
                },
                Function {
                    name: "helper".to_string(),
                    parameters: vec![],
                    return_type: Some(WgslType::Scalar(ScalarType::F32)),
                    body: Block::new(vec![Statement::Return(Some(Expr::Literal(
                        Literal::Float(1.0),
                    )))]),
                    attributes: vec![],
                    return_attributes: vec![],
                },
                Function {
                    name: "unused_func".to_string(),
                    parameters: vec![],
                    return_type: None,
                    body: Block::new(vec![Statement::Return(None)]),
                    attributes: vec![],
                    return_attributes: vec![],
                },
            ],
        };

        let result = dead_code_elimination(&mut module);
        assert!(result.changed);
        // compute_main (entry) and helper (called) kept; unused_func removed
        assert_eq!(module.functions.len(), 2);
        assert!(module.functions.iter().any(|f| f.name == "compute_main"));
        assert!(module.functions.iter().any(|f| f.name == "helper"));
        assert!(!module.functions.iter().any(|f| f.name == "unused_func"));
    }

    #[test]
    fn test_dce_multiple_compute_entry_points() {
        let mut module = WgslModule {
            structs: vec![],
            globals: vec![],
            constants: vec![],
            functions: vec![
                Function {
                    name: "pass1".to_string(),
                    parameters: vec![],
                    return_type: None,
                    body: Block::new(vec![Statement::Expression(Expr::Call(
                        "shared_helper".to_string(),
                        vec![],
                    ))]),
                    attributes: vec![
                        Attribute::Compute,
                        Attribute::WorkgroupSize(256, None, None),
                    ],
                    return_attributes: vec![],
                },
                Function {
                    name: "pass2".to_string(),
                    parameters: vec![],
                    return_type: None,
                    body: Block::new(vec![Statement::Expression(Expr::Call(
                        "shared_helper".to_string(),
                        vec![],
                    ))]),
                    attributes: vec![Attribute::Compute, Attribute::WorkgroupSize(64, None, None)],
                    return_attributes: vec![],
                },
                Function {
                    name: "shared_helper".to_string(),
                    parameters: vec![],
                    return_type: None,
                    body: Block::empty(),
                    attributes: vec![],
                    return_attributes: vec![],
                },
                Function {
                    name: "dead_code".to_string(),
                    parameters: vec![],
                    return_type: None,
                    body: Block::empty(),
                    attributes: vec![],
                    return_attributes: vec![],
                },
            ],
        };

        let result = dead_code_elimination(&mut module);
        assert!(result.changed);
        // Both entry points + shared helper kept; dead_code removed
        assert_eq!(module.functions.len(), 3);
        assert!(!module.functions.iter().any(|f| f.name == "dead_code"));
    }

    #[test]
    fn test_dce_on_parsed_compute_shader() {
        use crate::shader_ast::parser::parse_wgsl;

        // Parse a real compute shader, add an unused function, verify DCE removes it
        let source = r#"
struct Config { count: u32, }

@group(0) @binding(0) var<storage, read> data: array<f32>;
@group(0) @binding(1) var<uniform> config: Config;

fn helper(x: f32) -> f32 {
    return x * 2.0;
}

fn dead_helper(x: f32) -> f32 {
    return x * 3.0;
}

@compute @workgroup_size(256)
fn compute_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let val = helper(data[gid.x]);
    return;
}
"#;

        let mut module = parse_wgsl(source).unwrap();
        assert_eq!(module.functions.len(), 3);

        let result = dead_code_elimination(&mut module);
        assert!(result.changed);
        // compute_main + helper kept; dead_helper removed
        assert_eq!(module.functions.len(), 2);
        assert!(!module.functions.iter().any(|f| f.name == "dead_helper"));
    }
}
