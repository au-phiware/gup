// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lightweight WGSL AST types for the transpilation prototype.
//!
//! These mirror the essential types from `gup::shader_ast::types` but
//! are defined here to avoid a circular dependency between the proc
//! macro crate and the main library crate.

use std::fmt;

/// Scalar types in WGSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarType {
    F32,
    I32,
    U32,
    Bool,
}

impl fmt::Display for ScalarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScalarType::F32 => write!(f, "f32"),
            ScalarType::I32 => write!(f, "i32"),
            ScalarType::U32 => write!(f, "u32"),
            ScalarType::Bool => write!(f, "bool"),
        }
    }
}

/// WGSL type representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WgslType {
    /// Scalar type (f32, i32, u32, bool).
    Scalar(ScalarType),
    /// Vector type: `vec{dim}<{scalar}>`.
    Vector(ScalarType, u8),
    /// Matrix type: `mat{cols}x{rows}<{scalar}>`.
    Matrix(ScalarType, u8, u8),
    /// Array type: `array<{element}, {size}>`.
    Array(Box<WgslType>, u32),
    /// Named struct type.
    Struct(String),
    /// Void (no return type).
    Void,
}

impl fmt::Display for WgslType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WgslType::Scalar(s) => write!(f, "{s}"),
            WgslType::Vector(s, dim) => write!(f, "vec{dim}<{s}>"),
            WgslType::Matrix(s, cols, rows) => write!(f, "mat{cols}x{rows}<{s}>"),
            WgslType::Array(elem, size) => write!(f, "array<{elem}, {size}>"),
            WgslType::Struct(name) => write!(f, "{name}"),
            WgslType::Void => write!(f, "void"),
        }
    }
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "+"),
            BinaryOp::Sub => write!(f, "-"),
            BinaryOp::Mul => write!(f, "*"),
            BinaryOp::Div => write!(f, "/"),
            BinaryOp::Mod => write!(f, "%"),
            BinaryOp::And => write!(f, "&&"),
            BinaryOp::Or => write!(f, "||"),
            BinaryOp::BitAnd => write!(f, "&"),
            BinaryOp::BitOr => write!(f, "|"),
            BinaryOp::BitXor => write!(f, "^"),
            BinaryOp::Shl => write!(f, "<<"),
            BinaryOp::Shr => write!(f, ">>"),
            BinaryOp::Equal => write!(f, "=="),
            BinaryOp::NotEqual => write!(f, "!="),
            BinaryOp::Less => write!(f, "<"),
            BinaryOp::LessEqual => write!(f, "<="),
            BinaryOp::Greater => write!(f, ">"),
            BinaryOp::GreaterEqual => write!(f, ">="),
        }
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Negate => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
        }
    }
}

/// A literal value.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Float(f64),
    Int(i64),
    UInt(u64),
    Bool(bool),
}

/// An expression in the WGSL AST.
#[derive(Debug, Clone, PartialEq)]
pub enum WgslExpr {
    /// A literal value.
    Literal(Literal),
    /// A variable or identifier reference.
    Ident(String),
    /// A binary operation: `lhs op rhs`.
    Binary(Box<WgslExpr>, BinaryOp, Box<WgslExpr>),
    /// A unary operation: `op expr`.
    Unary(UnaryOp, Box<WgslExpr>),
    /// A function call: `name(args...)`.
    Call(String, Vec<WgslExpr>),
    /// A type constructor: `vec3<f32>(x, y, z)`.
    TypeConstructor(WgslType, Vec<WgslExpr>),
    /// Member access: `expr.field`.
    MemberAccess(Box<WgslExpr>, String),
    /// Index access: `expr[index]`.
    IndexAccess(Box<WgslExpr>, Box<WgslExpr>),
    /// Parenthesised expression.
    Paren(Box<WgslExpr>),
    /// Type cast: `type(expr)` (e.g. `f32(x)`).
    Cast(WgslType, Box<WgslExpr>),
}

/// A statement in the WGSL AST.
#[derive(Debug, Clone, PartialEq)]
pub enum WgslStatement {
    /// Variable declaration: `let name = expr;` or `var name = expr;`.
    Let {
        name: String,
        ty: Option<WgslType>,
        value: WgslExpr,
        mutable: bool,
    },
    /// Return statement: `return expr;`.
    Return(Option<WgslExpr>),
    /// If statement.
    If {
        condition: WgslExpr,
        body: Vec<WgslStatement>,
        else_body: Option<Vec<WgslStatement>>,
    },
    /// For loop: `for (var i = init; i < limit; i++) { body }`.
    For {
        var_name: String,
        initialiser: WgslExpr,
        condition: WgslExpr,
        update: WgslExpr,
        body: Vec<WgslStatement>,
    },
    /// While loop: `while (condition) { body }`.
    While {
        condition: WgslExpr,
        body: Vec<WgslStatement>,
    },
    /// Infinite loop: `loop { body }`.
    Loop { body: Vec<WgslStatement> },
    /// Break statement.
    Break,
    /// Continue statement.
    Continue,
    /// Switch statement: `switch(selector) { case N: { ... } default: { ... } }`.
    Switch {
        selector: WgslExpr,
        cases: Vec<SwitchCase>,
        default_body: Option<Vec<WgslStatement>>,
    },
    /// Expression statement (e.g. function call as statement).
    Expression(WgslExpr),
    /// Assignment: `target = value;`.
    Assign(WgslExpr, WgslExpr),
    /// Compound assignment: `target op= value;` (e.g., `x += 1;`).
    CompoundAssign(WgslExpr, BinaryOp, WgslExpr),
}

/// A case in a WGSL switch statement.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    /// Integer literal selectors for this case.
    pub selectors: Vec<WgslExpr>,
    /// Body statements of the case.
    pub body: Vec<WgslStatement>,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct WgslParam {
    pub name: String,
    pub ty: WgslType,
}

/// A complete function definition.
#[derive(Debug, Clone, PartialEq)]
pub struct WgslFunction {
    pub name: String,
    pub params: Vec<WgslParam>,
    pub return_type: WgslType,
    pub body: Vec<WgslStatement>,
}

/// A struct field definition.
#[derive(Debug, Clone, PartialEq)]
pub struct WgslField {
    pub name: String,
    pub ty: WgslType,
}

/// A struct definition.
#[derive(Debug, Clone, PartialEq)]
pub struct WgslStructDef {
    pub name: String,
    pub fields: Vec<WgslField>,
}

/// A complete WGSL module (collection of structs and functions).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WgslModule {
    pub structs: Vec<WgslStructDef>,
    pub functions: Vec<WgslFunction>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_type_display() {
        assert_eq!(ScalarType::F32.to_string(), "f32");
        assert_eq!(ScalarType::I32.to_string(), "i32");
        assert_eq!(ScalarType::U32.to_string(), "u32");
        assert_eq!(ScalarType::Bool.to_string(), "bool");
    }

    #[test]
    fn wgsl_type_display() {
        assert_eq!(WgslType::Scalar(ScalarType::F32).to_string(), "f32");
        assert_eq!(
            WgslType::Vector(ScalarType::F32, 3).to_string(),
            "vec3<f32>"
        );
        assert_eq!(
            WgslType::Matrix(ScalarType::F32, 4, 4).to_string(),
            "mat4x4<f32>"
        );
        assert_eq!(
            WgslType::Array(Box::new(WgslType::Scalar(ScalarType::F32)), 4).to_string(),
            "array<f32, 4>"
        );
    }

    #[test]
    fn binary_op_display() {
        assert_eq!(BinaryOp::Add.to_string(), "+");
        assert_eq!(BinaryOp::Equal.to_string(), "==");
        assert_eq!(BinaryOp::And.to_string(), "&&");
    }

    #[test]
    fn unary_op_display() {
        assert_eq!(UnaryOp::Negate.to_string(), "-");
        assert_eq!(UnaryOp::Not.to_string(), "!");
    }
}
