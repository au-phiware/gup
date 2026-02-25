// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Converts Rust `syn` AST nodes into WGSL AST nodes.
//!
//! This is the core of the transpilation prototype: it walks a Rust
//! function parsed by `syn` and produces the equivalent WGSL AST that
//! can then be rendered to WGSL text by [`super::codegen`].

use std::collections::HashSet;

use proc_macro2::Span;
use syn::{BinOp, Expr, FnArg, Pat, Stmt, Type};

use super::ast::*;
use super::type_map::TypeMapper;

/// Errors that can occur during Rust-to-WGSL transpilation.
#[derive(Debug, Clone)]
pub struct TranspileError {
    pub message: String,
    pub span: Span,
}

impl TranspileError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    /// Convert to a `syn::Error` for integration with proc macro error reporting.
    pub fn into_syn_error(self) -> syn::Error {
        syn::Error::new(self.span, self.message)
    }
}

impl std::fmt::Display for TranspileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Converts Rust `syn` AST into WGSL AST.
///
/// The converter maintains a set of known uniform parameter names so that
/// bare references to uniform fields are translated to `uniforms.field`
/// access in WGSL. It delegates type mapping to [`TypeMapper`] for
/// comprehensive type support and clear error diagnostics.
pub struct RustToWgsl {
    /// Names of parameters that should be accessed via `uniforms.` prefix.
    uniform_params: HashSet<String>,
    /// Type mapper for Rust → WGSL type conversion.
    type_mapper: TypeMapper,
}

impl RustToWgsl {
    /// Create a new converter.
    ///
    /// `uniform_params` is the set of parameter names (beyond the first
    /// input parameter) that will be packed into a WGSL uniforms struct.
    pub fn new(uniform_params: impl IntoIterator<Item = String>) -> Self {
        Self {
            uniform_params: uniform_params.into_iter().collect(),
            type_mapper: TypeMapper::new(),
        }
    }

    /// Return a reference to the underlying type mapper.
    pub fn type_mapper(&self) -> &TypeMapper {
        &self.type_mapper
    }

    /// Return a mutable reference to the underlying type mapper.
    pub fn type_mapper_mut(&mut self) -> &mut TypeMapper {
        &mut self.type_mapper
    }

    /// Convert a complete `syn::ItemFn` into a [`WgslFunction`].
    pub fn convert_function(&mut self, func: &syn::ItemFn) -> Result<WgslFunction, TranspileError> {
        let name = func.sig.ident.to_string();

        // Convert parameters
        let mut params = Vec::new();
        if let Some(first) = func.sig.inputs.first() {
            if let FnArg::Typed(pt) = first {
                let param_name = extract_param_name(&pt.pat)?;
                let param_type = self.convert_type(&pt.ty)?;
                params.push(WgslParam {
                    name: param_name,
                    ty: param_type,
                });
            }
        }

        // If there are uniform params, add a single `uniforms` parameter
        if !self.uniform_params.is_empty() {
            let uniforms_struct_name = format!("{}Uniforms", to_pascal_case(&name));
            params.push(WgslParam {
                name: "uniforms".to_string(),
                ty: WgslType::Struct(uniforms_struct_name),
            });
        }

        // Convert return type
        let return_type = match &func.sig.output {
            syn::ReturnType::Type(_, ty) => self.convert_type(ty)?,
            syn::ReturnType::Default => WgslType::Void,
        };

        // Convert body
        let body = self.convert_block(&func.block)?;

        Ok(WgslFunction {
            name,
            params,
            return_type,
            body,
        })
    }

    /// Convert a Rust type to a WGSL type.
    ///
    /// Delegates to [`TypeMapper`] for comprehensive type support and
    /// clear error diagnostics.
    pub fn convert_type(&mut self, ty: &Type) -> Result<WgslType, TranspileError> {
        self.type_mapper
            .map_rust_type(ty)
            .map(|info| info.wgsl_type)
            .map_err(|e| TranspileError::new(e.message, e.span))
    }

    /// Convert a block of statements.
    fn convert_block(&mut self, block: &syn::Block) -> Result<Vec<WgslStatement>, TranspileError> {
        let mut stmts = Vec::with_capacity(block.stmts.len());
        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i == block.stmts.len() - 1;
            stmts.push(self.convert_stmt(stmt, is_last)?);
        }
        Ok(stmts)
    }

    /// Convert a single statement.
    fn convert_stmt(
        &mut self,
        stmt: &Stmt,
        is_last: bool,
    ) -> Result<WgslStatement, TranspileError> {
        match stmt {
            Stmt::Local(local) => {
                let name = extract_pat_name(&local.pat)?;
                let mutable = matches!(&local.pat, Pat::Ident(pi) if pi.mutability.is_some());
                let value = if let Some(init) = &local.init {
                    self.convert_expr(&init.expr)?
                } else {
                    return Err(TranspileError::new(
                        "Variable declarations must have an initialiser",
                        Span::call_site(),
                    ));
                };

                // Try to extract type annotation
                let ty = if let Pat::Type(pt) = &local.pat {
                    Some(self.convert_type(&pt.ty)?)
                } else {
                    None
                };

                Ok(WgslStatement::Let {
                    name,
                    ty,
                    value,
                    mutable,
                })
            }
            Stmt::Expr(expr, semi) => {
                // Handle assignment statements
                if let Expr::Assign(assign) = expr {
                    let target = self.convert_expr(&assign.left)?;
                    let value = self.convert_expr(&assign.right)?;
                    return Ok(WgslStatement::Assign(target, value));
                }

                // Handle compound assignment statements (+=, -=, *=, /=, etc.)
                if let Some(compound) = self.try_convert_compound_assign(expr)? {
                    return Ok(compound);
                }

                // Handle for loop
                if let Expr::ForLoop(for_loop) = expr {
                    return self.convert_for_loop(for_loop);
                }

                // Handle while loop
                if let Expr::While(while_loop) = expr {
                    return self.convert_while_loop(while_loop);
                }

                // Handle infinite loop
                if let Expr::Loop(loop_expr) = expr {
                    return self.convert_loop(loop_expr);
                }

                // Handle break
                if let Expr::Break(_) = expr {
                    return Ok(WgslStatement::Break);
                }

                // Handle continue
                if let Expr::Continue(_) = expr {
                    return Ok(WgslStatement::Continue);
                }

                if semi.is_some() {
                    // Check for return statements
                    if let Expr::Return(ret) = expr {
                        let val = ret
                            .expr
                            .as_ref()
                            .map(|e| self.convert_expr(e))
                            .transpose()?;
                        return Ok(WgslStatement::Return(val));
                    }
                    // Check for if-else as a statement (not expression)
                    if let Expr::If(if_expr) = expr {
                        return self.convert_if_statement(if_expr);
                    }
                    // Handle for loop with semicolon
                    if let Expr::ForLoop(for_loop) = expr {
                        return self.convert_for_loop(for_loop);
                    }
                    // Handle while loop with semicolon
                    if let Expr::While(while_loop) = expr {
                        return self.convert_while_loop(while_loop);
                    }
                    // Handle loop with semicolon
                    if let Expr::Loop(loop_expr) = expr {
                        return self.convert_loop(loop_expr);
                    }
                    // Handle break with semicolon
                    if let Expr::Break(_) = expr {
                        return Ok(WgslStatement::Break);
                    }
                    // Handle continue with semicolon
                    if let Expr::Continue(_) = expr {
                        return Ok(WgslStatement::Continue);
                    }
                    // Expression with semicolon — regular statement
                    let wgsl_expr = self.convert_expr(expr)?;
                    Ok(WgslStatement::Expression(wgsl_expr))
                } else if is_last {
                    // Check for if-else as a statement at end of block
                    if let Expr::If(if_expr) = expr {
                        // If at end of function with no semicolon, treat as
                        // statement (not as select-expression)
                        return self.convert_if_statement(if_expr);
                    }
                    // Expression without semicolon at end of block — implicit return
                    let wgsl_expr = self.convert_expr(expr)?;
                    Ok(WgslStatement::Return(Some(wgsl_expr)))
                } else {
                    // Check for if-else as a statement
                    if let Expr::If(if_expr) = expr {
                        return self.convert_if_statement(if_expr);
                    }
                    let wgsl_expr = self.convert_expr(expr)?;
                    Ok(WgslStatement::Expression(wgsl_expr))
                }
            }
            _ => Err(TranspileError::new(
                "Unsupported statement type (items and macros are not allowed)",
                Span::call_site(),
            )),
        }
    }

    /// Convert a Rust expression to a WGSL expression.
    pub fn convert_expr(&mut self, expr: &Expr) -> Result<WgslExpr, TranspileError> {
        match expr {
            // --- Identifiers ---
            Expr::Path(ep) => {
                let path = &ep.path;
                if let Some(ident) = path.get_ident() {
                    let name = ident.to_string();
                    if self.uniform_params.contains(&name) {
                        // Uniform parameter → uniforms.name
                        Ok(WgslExpr::MemberAccess(
                            Box::new(WgslExpr::Ident("uniforms".to_string())),
                            name,
                        ))
                    } else {
                        // Check for WGSL built-in constants
                        match name.as_str() {
                            "true" => Ok(WgslExpr::Literal(Literal::Bool(true))),
                            "false" => Ok(WgslExpr::Literal(Literal::Bool(false))),
                            _ => Ok(WgslExpr::Ident(name)),
                        }
                    }
                } else {
                    Err(TranspileError::new(
                        "Complex paths are not supported in WGSL",
                        Span::call_site(),
                    ))
                }
            }

            // --- Literals ---
            Expr::Lit(lit) => self.convert_literal(&lit.lit),

            // --- Binary operations ---
            Expr::Binary(bin) => {
                let left = self.convert_expr(&bin.left)?;
                let right = self.convert_expr(&bin.right)?;
                let op = convert_binop(&bin.op)?;
                Ok(WgslExpr::Binary(Box::new(left), op, Box::new(right)))
            }

            // --- Unary operations ---
            Expr::Unary(un) => {
                let operand = self.convert_expr(&un.expr)?;
                let op = match un.op {
                    syn::UnOp::Neg(_) => UnaryOp::Negate,
                    syn::UnOp::Not(_) => UnaryOp::Not,
                    _ => {
                        return Err(TranspileError::new(
                            "Unsupported unary operator",
                            Span::call_site(),
                        ));
                    }
                };
                Ok(WgslExpr::Unary(op, Box::new(operand)))
            }

            // --- Parenthesised ---
            Expr::Paren(p) => {
                let inner = self.convert_expr(&p.expr)?;
                Ok(WgslExpr::Paren(Box::new(inner)))
            }

            // --- Field access ---
            Expr::Field(field) => {
                let base = self.convert_expr(&field.base)?;
                let member = match &field.member {
                    syn::Member::Named(n) => n.to_string(),
                    syn::Member::Unnamed(idx) => format!("_{}", idx.index),
                };

                // If base is a uniform param identifier, wrap it
                if let WgslExpr::Ident(ref name) = base {
                    if self.uniform_params.contains(name) {
                        return Ok(WgslExpr::MemberAccess(
                            Box::new(WgslExpr::MemberAccess(
                                Box::new(WgslExpr::Ident("uniforms".to_string())),
                                name.clone(),
                            )),
                            member,
                        ));
                    }
                }

                Ok(WgslExpr::MemberAccess(Box::new(base), member))
            }

            // --- Function calls ---
            Expr::Call(call) => {
                let args: Result<Vec<WgslExpr>, _> =
                    call.args.iter().map(|a| self.convert_expr(a)).collect();
                let args = args?;

                // Extract function name
                if let Expr::Path(func_path) = &*call.func {
                    // Try simple identifier first
                    if let Some(ident) = func_path.path.get_ident() {
                        let name = ident.to_string();
                        return self.convert_function_call_by_name(&name, args);
                    }

                    // Handle qualified paths like f32::sin(x), i32::max(a, b)
                    if let Some(result) = self.try_convert_qualified_call(&func_path.path, args)? {
                        return Ok(result);
                    }
                }
                Err(TranspileError::new(
                    "Only simple function calls and qualified type calls \
                     (e.g. f32::sin(x)) are supported",
                    Span::call_site(),
                ))
            }

            // --- Method calls ---
            Expr::MethodCall(mc) => {
                let receiver = self.convert_expr(&mc.receiver)?;
                let method = mc.method.to_string();
                let args: Result<Vec<WgslExpr>, _> =
                    mc.args.iter().map(|a| self.convert_expr(a)).collect();
                let mut args = args?;

                // Map Rust method calls to WGSL function calls
                match method.as_str() {
                    // Single-argument WGSL built-in functions (receiver becomes first arg)
                    "abs" | "sqrt" | "floor" | "ceil" | "round" | "fract" | "sign" | "sin"
                    | "cos" | "tan" | "asin" | "acos" | "atan" | "exp" | "exp2" | "log"
                    | "log2" | "length" | "normalize" | "trunc" | "sinh" | "cosh" | "tanh"
                    | "asinh" | "acosh" | "atanh" | "saturate" | "degrees" | "radians"
                    | "inversesqrt" => Ok(WgslExpr::Call(method, vec![receiver])),

                    // Two-argument WGSL functions (receiver + one arg)
                    "min" | "max" | "pow" | "atan2" | "step" | "distance" | "dot" | "cross"
                    | "reflect" | "ldexp" => {
                        args.insert(0, receiver);
                        Ok(WgslExpr::Call(method, args))
                    }

                    // Three-argument WGSL functions
                    "clamp" | "mix" | "smoothstep" | "fma" | "refract" | "faceforward" => {
                        args.insert(0, receiver);
                        Ok(WgslExpr::Call(method, args))
                    }

                    // Swizzle access for vector components (x, y, z, w, xy, xyz, etc.)
                    // These look like methods with no args but are really field access;
                    // handle them as member access if no parentheses (already handled by
                    // field access), but method-style `.xy()` calls are not standard.

                    // Vector component access (treated as member access)
                    "x" | "y" | "z" | "w" if args.is_empty() => {
                        Ok(WgslExpr::MemberAccess(Box::new(receiver), method))
                    }

                    // Conversion methods: .to_f32() etc.
                    "to_f32" if args.is_empty() => Ok(WgslExpr::Cast(
                        WgslType::Scalar(ScalarType::F32),
                        Box::new(receiver),
                    )),
                    "to_i32" if args.is_empty() => Ok(WgslExpr::Cast(
                        WgslType::Scalar(ScalarType::I32),
                        Box::new(receiver),
                    )),
                    "to_u32" if args.is_empty() => Ok(WgslExpr::Cast(
                        WgslType::Scalar(ScalarType::U32),
                        Box::new(receiver),
                    )),

                    _ => Err(TranspileError::new(
                        format!(
                            "Method '{method}' has no WGSL equivalent. \
                             Supported methods: abs, sqrt, sin, cos, min, max, \
                             clamp, mix, length, normalize, dot, cross, pow, etc."
                        ),
                        mc.method.span(),
                    )),
                }
            }

            // --- Return ---
            Expr::Return(ret) => {
                // Return as expression (rare but valid)
                let val = ret
                    .expr
                    .as_ref()
                    .map(|e| self.convert_expr(e))
                    .transpose()?;
                // Wrap in an ident that represents the return — will be
                // handled at the statement level
                match val {
                    Some(v) => Ok(v),
                    None => Ok(WgslExpr::Ident("return".to_string())),
                }
            }

            // --- If/else expression → WGSL select() ---
            Expr::If(if_expr) => {
                // If-else as expression can be transpiled to WGSL select():
                //   select(false_value, true_value, condition)
                // Note: WGSL select has reversed order from typical ternary.
                if let Some(else_branch) = &if_expr.else_branch {
                    let condition = self.convert_expr(&if_expr.cond)?;

                    // Extract single expression from then block
                    let then_val = self.extract_block_expr(&if_expr.then_branch)?;

                    // Extract single expression from else block
                    let else_val = match else_branch.1.as_ref() {
                        Expr::Block(block) => self.extract_block_expr(&block.block)?,
                        other => self.convert_expr(other)?,
                    };

                    Ok(WgslExpr::Call(
                        "select".to_string(),
                        vec![else_val, then_val, condition],
                    ))
                } else {
                    Err(TranspileError::new(
                        "if expression without else cannot be used as an expression in WGSL. \
                         Use if-else as a statement instead.",
                        Span::call_site(),
                    ))
                }
            }

            // --- Type casts (`x as f32`) ---
            Expr::Cast(cast) => {
                let inner = self.convert_expr(&cast.expr)?;
                let target = self.convert_type(&cast.ty)?;
                Ok(WgslExpr::Cast(target, Box::new(inner)))
            }

            // --- Index access ---
            Expr::Index(idx) => {
                let base = self.convert_expr(&idx.expr)?;
                let index = self.convert_expr(&idx.index)?;
                Ok(WgslExpr::IndexAccess(Box::new(base), Box::new(index)))
            }

            // --- Assignment ---
            Expr::Assign(assign) => {
                let target = self.convert_expr(&assign.left)?;
                let value = self.convert_expr(&assign.right)?;
                // Return as a pseudo-expression; will be properly handled at
                // statement level for Assign statements.
                // For expression-level usage, wrap into a binary that the
                // statement converter can recognise.
                Ok(WgslExpr::Binary(
                    Box::new(target),
                    BinaryOp::Add, // placeholder — overridden at statement level
                    Box::new(value),
                ))
            }

            // --- Tuple expression → WGSL doesn't support tuples,
            //     but single-element tuples can be unwrapped ---
            Expr::Tuple(tuple) => {
                if tuple.elems.len() == 1 {
                    self.convert_expr(&tuple.elems[0])
                } else {
                    Err(TranspileError::new(
                        format!(
                            "Tuples with {} elements are not supported in WGSL. \
                             Consider using a struct or vector type instead.",
                            tuple.elems.len()
                        ),
                        Span::call_site(),
                    ))
                }
            }

            // --- Block expression ---
            Expr::Block(block) => {
                // Extract the last expression from a block
                self.extract_block_expr(&block.block)
            }

            // --- Reference expressions (strip the & since WGSL doesn't have refs in expressions) ---
            Expr::Reference(reference) => self.convert_expr(&reference.expr),

            // --- Group expression (invisible delimiter) ---
            Expr::Group(group) => self.convert_expr(&group.expr),

            _ => Err(TranspileError::new(
                format!(
                    "Unsupported expression type for WGSL transpilation: {}",
                    expr_type_name(expr)
                ),
                Span::call_site(),
            )),
        }
    }

    /// Convert a Rust literal to a WGSL literal expression.
    fn convert_literal(&self, lit: &syn::Lit) -> Result<WgslExpr, TranspileError> {
        match lit {
            syn::Lit::Float(f) => {
                let val: f64 = f
                    .base10_parse()
                    .map_err(|_| TranspileError::new("Invalid float literal", f.span()))?;
                Ok(WgslExpr::Literal(Literal::Float(val)))
            }
            syn::Lit::Int(i) => {
                let suffix = i.suffix();
                if suffix == "u32" || suffix == "u" {
                    let val: u64 = i.base10_parse().map_err(|_| {
                        TranspileError::new("Invalid unsigned int literal", i.span())
                    })?;
                    Ok(WgslExpr::Literal(Literal::UInt(val)))
                } else {
                    let val: i64 = i
                        .base10_parse()
                        .map_err(|_| TranspileError::new("Invalid int literal", i.span()))?;
                    Ok(WgslExpr::Literal(Literal::Int(val)))
                }
            }
            syn::Lit::Bool(b) => Ok(WgslExpr::Literal(Literal::Bool(b.value))),
            _ => Err(TranspileError::new(
                "Only numeric and boolean literals are supported in WGSL",
                Span::call_site(),
            )),
        }
    }

    /// Map a function call by name, handling type constructors and WGSL built-ins.
    fn convert_function_call_by_name(
        &self,
        name: &str,
        args: Vec<WgslExpr>,
    ) -> Result<WgslExpr, TranspileError> {
        match name {
            // Float vectors
            "Vec2" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::F32, 2),
                args,
            )),
            "Vec3" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::F32, 3),
                args,
            )),
            "Vec4" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::F32, 4),
                args,
            )),
            // Integer vectors
            "IVec2" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::I32, 2),
                args,
            )),
            "IVec3" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::I32, 3),
                args,
            )),
            "IVec4" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::I32, 4),
                args,
            )),
            // Unsigned integer vectors
            "UVec2" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::U32, 2),
                args,
            )),
            "UVec3" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::U32, 3),
                args,
            )),
            "UVec4" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::U32, 4),
                args,
            )),
            // Boolean vectors
            "BVec2" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::Bool, 2),
                args,
            )),
            "BVec3" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::Bool, 3),
                args,
            )),
            "BVec4" => Ok(WgslExpr::TypeConstructor(
                WgslType::Vector(ScalarType::Bool, 4),
                args,
            )),
            // Matrix constructors
            "Mat2" => Ok(WgslExpr::TypeConstructor(
                WgslType::Matrix(ScalarType::F32, 2, 2),
                args,
            )),
            "Mat3" => Ok(WgslExpr::TypeConstructor(
                WgslType::Matrix(ScalarType::F32, 3, 3),
                args,
            )),
            "Mat4" => Ok(WgslExpr::TypeConstructor(
                WgslType::Matrix(ScalarType::F32, 4, 4),
                args,
            )),
            // Regular function call
            _ => Ok(WgslExpr::Call(name.to_string(), args)),
        }
    }

    /// Try to convert a qualified function call like `f32::sin(x)` or `Vec3::new(...)`.
    fn try_convert_qualified_call(
        &self,
        path: &syn::Path,
        args: Vec<WgslExpr>,
    ) -> Result<Option<WgslExpr>, TranspileError> {
        if path.segments.len() != 2 {
            return Ok(None);
        }

        let type_name = path.segments[0].ident.to_string();
        let func_name = path.segments[1].ident.to_string();

        match type_name.as_str() {
            // f32::sin(x) → sin(x), f32::min(a, b) → min(a, b)
            "f32" | "f64" => self.convert_scalar_qualified_call(&func_name, args),
            // i32::min(a, b) → min(a, b)
            "i32" | "u32" => self.convert_scalar_qualified_call(&func_name, args),
            // Vec2::new(x, y) → vec2<f32>(x, y)
            "Vec2" => self.convert_vec_static_call(&func_name, ScalarType::F32, 2, args),
            "Vec3" => self.convert_vec_static_call(&func_name, ScalarType::F32, 3, args),
            "Vec4" => self.convert_vec_static_call(&func_name, ScalarType::F32, 4, args),
            "IVec2" => self.convert_vec_static_call(&func_name, ScalarType::I32, 2, args),
            "IVec3" => self.convert_vec_static_call(&func_name, ScalarType::I32, 3, args),
            "IVec4" => self.convert_vec_static_call(&func_name, ScalarType::I32, 4, args),
            "UVec2" => self.convert_vec_static_call(&func_name, ScalarType::U32, 2, args),
            "UVec3" => self.convert_vec_static_call(&func_name, ScalarType::U32, 3, args),
            "UVec4" => self.convert_vec_static_call(&func_name, ScalarType::U32, 4, args),
            _ => Ok(None),
        }
    }

    /// Convert a scalar-qualified function call (e.g. `f32::sin(x)` → `sin(x)`).
    fn convert_scalar_qualified_call(
        &self,
        func_name: &str,
        args: Vec<WgslExpr>,
    ) -> Result<Option<WgslExpr>, TranspileError> {
        // WGSL built-in functions that map from Rust's qualified syntax
        let mapped = match func_name {
            // Trig
            "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sinh" | "cosh" | "tanh"
            | "asinh" | "acosh" | "atanh" => Some(func_name),
            // Math
            "abs" | "sqrt" | "floor" | "ceil" | "round" | "fract" | "sign" | "exp" | "exp2"
            | "log" | "log2" | "trunc" => Some(func_name),
            // Min/max
            "min" | "max" | "clamp" => Some(func_name),
            // Power
            "pow" | "atan2" => Some(func_name),
            // Not directly available — generate as WGSL built-in anyway
            "saturate" | "radians" | "degrees" => Some(func_name),
            _ => None,
        };

        match mapped {
            Some(wgsl_name) => Ok(Some(WgslExpr::Call(wgsl_name.to_string(), args))),
            None => Ok(None),
        }
    }

    /// Convert a vector static method call (e.g. `Vec3::new(x, y, z)` → `vec3<f32>(x, y, z)`).
    fn convert_vec_static_call(
        &self,
        func_name: &str,
        scalar: ScalarType,
        dim: u8,
        args: Vec<WgslExpr>,
    ) -> Result<Option<WgslExpr>, TranspileError> {
        match func_name {
            "new" => Ok(Some(WgslExpr::TypeConstructor(
                WgslType::Vector(scalar, dim),
                args,
            ))),
            "splat" => {
                // Vec3::splat(v) → vec3<f32>(v)
                Ok(Some(WgslExpr::TypeConstructor(
                    WgslType::Vector(scalar, dim),
                    args,
                )))
            }
            "zero" | "ZERO" => Ok(Some(WgslExpr::TypeConstructor(
                WgslType::Vector(scalar, dim),
                vec![WgslExpr::Literal(Literal::Float(0.0))],
            ))),
            "one" | "ONE" => Ok(Some(WgslExpr::TypeConstructor(
                WgslType::Vector(scalar, dim),
                vec![WgslExpr::Literal(Literal::Float(1.0))],
            ))),
            _ => Ok(None),
        }
    }

    /// Extract the final expression from a block, for use in if-else → select().
    fn extract_block_expr(&mut self, block: &syn::Block) -> Result<WgslExpr, TranspileError> {
        if block.stmts.len() != 1 {
            return Err(TranspileError::new(
                "if-else as expression requires single-expression blocks for \
                 WGSL select() transpilation. Move complex logic to separate \
                 let bindings before the if-else.",
                Span::call_site(),
            ));
        }
        match &block.stmts[0] {
            Stmt::Expr(expr, None) => self.convert_expr(expr),
            _ => Err(TranspileError::new(
                "if-else block must contain a single expression (no semicolon) \
                 for WGSL select() transpilation",
                Span::call_site(),
            )),
        }
    }

    /// Try to convert a compound assignment expression (+=, -=, etc.) to a statement.
    fn try_convert_compound_assign(
        &mut self,
        expr: &Expr,
    ) -> Result<Option<WgslStatement>, TranspileError> {
        // syn doesn't have separate CompoundAssign variants; compound assignments
        // appear as `Expr::Assign` in newer syn, or as `Expr::Binary` with assign ops.
        // Check for syn's compound assignment representation.
        match expr {
            Expr::Binary(bin) => {
                let op = match &bin.op {
                    BinOp::AddAssign(_) => Some(BinaryOp::Add),
                    BinOp::SubAssign(_) => Some(BinaryOp::Sub),
                    BinOp::MulAssign(_) => Some(BinaryOp::Mul),
                    BinOp::DivAssign(_) => Some(BinaryOp::Div),
                    BinOp::RemAssign(_) => Some(BinaryOp::Mod),
                    BinOp::BitAndAssign(_) => Some(BinaryOp::BitAnd),
                    BinOp::BitOrAssign(_) => Some(BinaryOp::BitOr),
                    BinOp::BitXorAssign(_) => Some(BinaryOp::BitXor),
                    BinOp::ShlAssign(_) => Some(BinaryOp::Shl),
                    BinOp::ShrAssign(_) => Some(BinaryOp::Shr),
                    _ => None,
                };
                if let Some(op) = op {
                    let target = self.convert_expr(&bin.left)?;
                    let value = self.convert_expr(&bin.right)?;
                    return Ok(Some(WgslStatement::CompoundAssign(target, op, value)));
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Convert a Rust for-loop to a WGSL for statement.
    ///
    /// Supports `for i in 0..n` → `for (var i = 0; i < n; i++)`.
    fn convert_for_loop(
        &mut self,
        for_loop: &syn::ExprForLoop,
    ) -> Result<WgslStatement, TranspileError> {
        let var_name = extract_pat_name(&for_loop.pat)?;

        // Extract range bounds from the iterator expression
        let (init, limit) = self.extract_range_bounds(&for_loop.expr)?;

        let body = self.convert_block(&for_loop.body)?;

        Ok(WgslStatement::For {
            var_name: var_name.clone(),
            initialiser: init,
            condition: WgslExpr::Binary(
                Box::new(WgslExpr::Ident(var_name.clone())),
                BinaryOp::Less,
                Box::new(limit),
            ),
            update: WgslExpr::Ident(var_name),
            body,
        })
    }

    /// Extract the start and end expressions from a range expression (`start..end`).
    fn extract_range_bounds(
        &mut self,
        expr: &Expr,
    ) -> Result<(WgslExpr, WgslExpr), TranspileError> {
        if let Expr::Range(range) = expr {
            let start = if let Some(start) = &range.start {
                self.convert_expr(start)?
            } else {
                WgslExpr::Literal(Literal::Int(0))
            };
            let end = if let Some(end) = &range.end {
                self.convert_expr(end)?
            } else {
                return Err(TranspileError::new(
                    "Range expressions must have an upper bound for WGSL for-loops",
                    Span::call_site(),
                ));
            };
            Ok((start, end))
        } else {
            Err(TranspileError::new(
                "Only range expressions (e.g. 0..n) are supported as \
                 for-loop iterators in WGSL. Use `for i in 0..n { ... }`.",
                Span::call_site(),
            ))
        }
    }

    /// Convert a Rust while-loop to a WGSL while statement.
    fn convert_while_loop(
        &mut self,
        while_loop: &syn::ExprWhile,
    ) -> Result<WgslStatement, TranspileError> {
        let condition = self.convert_expr(&while_loop.cond)?;
        let body = self.convert_block(&while_loop.body)?;

        Ok(WgslStatement::While { condition, body })
    }

    /// Convert a Rust infinite `loop` to a WGSL `loop` statement.
    fn convert_loop(&mut self, loop_expr: &syn::ExprLoop) -> Result<WgslStatement, TranspileError> {
        let body = self.convert_block(&loop_expr.body)?;
        Ok(WgslStatement::Loop { body })
    }

    /// Convert an if expression used as a statement (not as an expression/select).
    fn convert_if_statement(
        &mut self,
        if_expr: &syn::ExprIf,
    ) -> Result<WgslStatement, TranspileError> {
        let condition = self.convert_expr(&if_expr.cond)?;
        let body = self.convert_block(&if_expr.then_branch)?;
        let else_body = if let Some((_, else_expr)) = &if_expr.else_branch {
            match else_expr.as_ref() {
                Expr::Block(block) => Some(self.convert_block(&block.block)?),
                Expr::If(nested_if) => {
                    // else if → nested if statement
                    let nested = self.convert_if_statement(nested_if)?;
                    Some(vec![nested])
                }
                _ => {
                    return Err(TranspileError::new(
                        "Unsupported else branch expression",
                        Span::call_site(),
                    ));
                }
            }
        } else {
            None
        };
        Ok(WgslStatement::If {
            condition,
            body,
            else_body,
        })
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Convert a `syn::BinOp` to a WGSL [`BinaryOp`].
fn convert_binop(op: &BinOp) -> Result<BinaryOp, TranspileError> {
    match op {
        BinOp::Add(_) => Ok(BinaryOp::Add),
        BinOp::Sub(_) => Ok(BinaryOp::Sub),
        BinOp::Mul(_) => Ok(BinaryOp::Mul),
        BinOp::Div(_) => Ok(BinaryOp::Div),
        BinOp::Rem(_) => Ok(BinaryOp::Mod),
        BinOp::And(_) => Ok(BinaryOp::And),
        BinOp::Or(_) => Ok(BinaryOp::Or),
        BinOp::BitAnd(_) => Ok(BinaryOp::BitAnd),
        BinOp::BitOr(_) => Ok(BinaryOp::BitOr),
        BinOp::BitXor(_) => Ok(BinaryOp::BitXor),
        BinOp::Shl(_) => Ok(BinaryOp::Shl),
        BinOp::Shr(_) => Ok(BinaryOp::Shr),
        BinOp::Eq(_) => Ok(BinaryOp::Equal),
        BinOp::Ne(_) => Ok(BinaryOp::NotEqual),
        BinOp::Lt(_) => Ok(BinaryOp::Less),
        BinOp::Le(_) => Ok(BinaryOp::LessEqual),
        BinOp::Gt(_) => Ok(BinaryOp::Greater),
        BinOp::Ge(_) => Ok(BinaryOp::GreaterEqual),
        _ => Err(TranspileError::new(
            "Unsupported binary operator for WGSL",
            Span::call_site(),
        )),
    }
}

/// Extract a parameter name from a pattern.
fn extract_param_name(pat: &Pat) -> Result<String, TranspileError> {
    if let Pat::Ident(pi) = pat {
        Ok(pi.ident.to_string())
    } else {
        Err(TranspileError::new(
            "Only simple identifier patterns are supported for parameters",
            Span::call_site(),
        ))
    }
}

/// Extract a variable name from a pattern (handles `let x`, `let mut x`,
/// `let x: ty`).
fn extract_pat_name(pat: &Pat) -> Result<String, TranspileError> {
    match pat {
        Pat::Ident(pi) => Ok(pi.ident.to_string()),
        Pat::Type(pt) => extract_pat_name(&pt.pat),
        _ => Err(TranspileError::new(
            "Only simple variable bindings are supported",
            Span::call_site(),
        )),
    }
}

/// Convert snake_case to PascalCase.
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}

/// Return a human-readable name for a `syn::Expr` variant (for error messages).
fn expr_type_name(expr: &Expr) -> &'static str {
    match expr {
        Expr::Array(_) => "array",
        Expr::Assign(_) => "assignment",
        Expr::Async(_) => "async block",
        Expr::Await(_) => "await",
        Expr::Binary(_) => "binary operation",
        Expr::Block(_) => "block",
        Expr::Break(_) => "break",
        Expr::Call(_) => "function call",
        Expr::Cast(_) => "type cast",
        Expr::Closure(_) => "closure",
        Expr::Continue(_) => "continue",
        Expr::Field(_) => "field access",
        Expr::ForLoop(_) => "for loop",
        Expr::Group(_) => "group",
        Expr::If(_) => "if expression",
        Expr::Index(_) => "index access",
        Expr::Let(_) => "let guard",
        Expr::Lit(_) => "literal",
        Expr::Loop(_) => "loop",
        Expr::Macro(_) => "macro invocation",
        Expr::Match(_) => "match expression",
        Expr::MethodCall(_) => "method call",
        Expr::Paren(_) => "parenthesised",
        Expr::Path(_) => "path",
        Expr::Range(_) => "range",
        Expr::Reference(_) => "reference",
        Expr::Repeat(_) => "repeat",
        Expr::Return(_) => "return",
        Expr::Struct(_) => "struct literal",
        Expr::Try(_) => "try (?)",
        Expr::Tuple(_) => "tuple",
        Expr::Unary(_) => "unary operation",
        Expr::While(_) => "while loop",
        Expr::Yield(_) => "yield",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn convert_simple_arithmetic() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(a + b);
        let result = converter.convert_expr(&expr).unwrap();
        assert_eq!(
            result,
            WgslExpr::Binary(
                Box::new(WgslExpr::Ident("a".to_string())),
                BinaryOp::Add,
                Box::new(WgslExpr::Ident("b".to_string())),
            )
        );
    }

    #[test]
    fn convert_nested_arithmetic() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(a * b + c);
        let result = converter.convert_expr(&expr).unwrap();
        // Should parse as (a * b) + c
        assert!(matches!(result, WgslExpr::Binary(_, BinaryOp::Add, _)));
    }

    #[test]
    fn convert_float_literal() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(1.5);
        let result = converter.convert_expr(&expr).unwrap();
        assert_eq!(result, WgslExpr::Literal(Literal::Float(1.5)));
    }

    #[test]
    fn convert_int_literal() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(42);
        let result = converter.convert_expr(&expr).unwrap();
        assert_eq!(result, WgslExpr::Literal(Literal::Int(42)));
    }

    #[test]
    fn convert_uniform_access() {
        let mut converter = RustToWgsl::new(["scale".to_string()]);
        let expr: Expr = parse_quote!(scale);
        let result = converter.convert_expr(&expr).unwrap();
        assert_eq!(
            result,
            WgslExpr::MemberAccess(
                Box::new(WgslExpr::Ident("uniforms".to_string())),
                "scale".to_string(),
            )
        );
    }

    #[test]
    fn convert_uniform_field_access() {
        let mut converter = RustToWgsl::new(["config".to_string()]);
        let expr: Expr = parse_quote!(config.min_val);
        let result = converter.convert_expr(&expr).unwrap();
        assert_eq!(
            result,
            WgslExpr::MemberAccess(
                Box::new(WgslExpr::MemberAccess(
                    Box::new(WgslExpr::Ident("uniforms".to_string())),
                    "config".to_string(),
                )),
                "min_val".to_string(),
            )
        );
    }

    #[test]
    fn convert_function_call() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(clamp(x, 0.0, 1.0));
        let result = converter.convert_expr(&expr).unwrap();
        assert!(matches!(result, WgslExpr::Call(name, args) if name == "clamp" && args.len() == 3));
    }

    #[test]
    fn convert_method_to_function() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(x.abs());
        let result = converter.convert_expr(&expr).unwrap();
        assert_eq!(
            result,
            WgslExpr::Call("abs".to_string(), vec![WgslExpr::Ident("x".to_string())],)
        );
    }

    #[test]
    fn convert_vec_constructor() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(Vec3(1.0, 2.0, 3.0));
        let result = converter.convert_expr(&expr).unwrap();
        assert!(
            matches!(result, WgslExpr::TypeConstructor(WgslType::Vector(ScalarType::F32, 3), args) if args.len() == 3)
        );
    }

    #[test]
    fn convert_type_cast() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(x as f32);
        let result = converter.convert_expr(&expr).unwrap();
        assert!(matches!(
            result,
            WgslExpr::Cast(WgslType::Scalar(ScalarType::F32), _)
        ));
    }

    #[test]
    fn convert_unary_negate() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(-x);
        let result = converter.convert_expr(&expr).unwrap();
        assert_eq!(
            result,
            WgslExpr::Unary(UnaryOp::Negate, Box::new(WgslExpr::Ident("x".to_string())),)
        );
    }

    #[test]
    fn convert_comparison() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(a > b);
        let result = converter.convert_expr(&expr).unwrap();
        assert!(matches!(result, WgslExpr::Binary(_, BinaryOp::Greater, _)));
    }

    #[test]
    fn convert_bool_literal() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(true);
        let result = converter.convert_expr(&expr).unwrap();
        assert_eq!(result, WgslExpr::Literal(Literal::Bool(true)));
    }

    #[test]
    fn convert_index_access() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(arr[i]);
        let result = converter.convert_expr(&expr).unwrap();
        assert!(matches!(result, WgslExpr::IndexAccess(_, _)));
    }

    #[test]
    fn convert_type_f32() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let ty: Type = parse_quote!(f32);
        assert_eq!(
            converter.convert_type(&ty).unwrap(),
            WgslType::Scalar(ScalarType::F32)
        );
    }

    #[test]
    fn convert_type_vec3() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let ty: Type = parse_quote!(Vec3);
        assert_eq!(
            converter.convert_type(&ty).unwrap(),
            WgslType::Vector(ScalarType::F32, 3)
        );
    }

    #[test]
    fn convert_type_array() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let ty: Type = parse_quote!([f32; 4]);
        assert_eq!(
            converter.convert_type(&ty).unwrap(),
            WgslType::Array(Box::new(WgslType::Scalar(ScalarType::F32)), 4)
        );
    }

    #[test]
    fn convert_function_simple() {
        let mut converter = RustToWgsl::new(["scale".to_string(), "offset".to_string()]);
        let func: syn::ItemFn = parse_quote! {
            fn transform(value: f32, scale: f32, offset: f32) -> f32 {
                return value * scale + offset;
            }
        };
        let result = converter.convert_function(&func).unwrap();
        assert_eq!(result.name, "transform");
        assert_eq!(result.params.len(), 2); // value + uniforms
        assert_eq!(result.params[0].name, "value");
        assert_eq!(result.params[1].name, "uniforms");
        assert_eq!(result.return_type, WgslType::Scalar(ScalarType::F32));
        assert_eq!(result.body.len(), 1);
    }

    #[test]
    fn unsupported_closure_error() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: Expr = parse_quote!(|x| x + 1);
        let result = converter.convert_expr(&expr);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("closure"));
    }

    #[test]
    fn pascal_case_conversion() {
        assert_eq!(to_pascal_case("linear_scale"), "LinearScale");
        assert_eq!(to_pascal_case("simple"), "Simple");
        assert_eq!(to_pascal_case("a_b_c"), "ABC");
    }
}
