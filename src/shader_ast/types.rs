// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WGSL Abstract Syntax Tree types.
//!
//! Defines the AST representation for a subset of WGSL that covers
//! shader function composition: functions, types, expressions, and statements.

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
    /// Array type: `array<{element}, {size}>` or `array<{element}>`.
    Array(Box<WgslType>, Option<u32>),
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
            WgslType::Array(elem, Some(size)) => write!(f, "array<{elem}, {size}>"),
            WgslType::Array(elem, None) => write!(f, "array<{elem}>"),
            WgslType::Struct(name) => write!(f, "{name}"),
            WgslType::Void => write!(f, "void"),
        }
    }
}

impl WgslType {
    /// Returns `true` if the type is a scalar.
    pub fn is_scalar(&self) -> bool {
        matches!(self, WgslType::Scalar(_))
    }

    /// Returns `true` if the type is a vector.
    pub fn is_vector(&self) -> bool {
        matches!(self, WgslType::Vector(_, _))
    }

    /// Returns `true` if the type is a matrix.
    pub fn is_matrix(&self) -> bool {
        matches!(self, WgslType::Matrix(_, _, _))
    }

    /// Returns `true` if `self` can be automatically promoted to `target`.
    ///
    /// Promotion rules:
    /// - f32 → vec2<f32>, vec3<f32>, vec4<f32> (broadcast)
    /// - vec2<f32> → vec3<f32>, vec4<f32> (zero-padded)
    /// - vec3<f32> → vec4<f32> (zero-padded)
    pub fn can_promote_to(&self, target: &WgslType) -> bool {
        match (self, target) {
            // Same types are always compatible.
            (a, b) if a == b => true,
            // Scalar f32 can promote to any f32 vector.
            (WgslType::Scalar(ScalarType::F32), WgslType::Vector(ScalarType::F32, _)) => true,
            // Smaller f32 vectors promote to larger f32 vectors.
            (WgslType::Vector(ScalarType::F32, a), WgslType::Vector(ScalarType::F32, b)) => a < b,
            _ => false,
        }
    }

    /// Returns the WGSL expression to promote `expr` of type `self` to `target`.
    ///
    /// Returns `None` if no promotion is needed or possible.
    pub fn promotion_wgsl(&self, expr: &str, target: &WgslType) -> Option<String> {
        if self == target {
            return None;
        }
        match (self, target) {
            (WgslType::Scalar(ScalarType::F32), WgslType::Vector(ScalarType::F32, dim)) => {
                let zeros = ", 0.0".repeat((*dim as usize) - 1);
                Some(format!("vec{dim}<f32>({expr}{zeros})"))
            }
            (WgslType::Vector(ScalarType::F32, from), WgslType::Vector(ScalarType::F32, to)) => {
                let extra = (*to as usize) - (*from as usize);
                let zeros = ", 0.0".repeat(extra);
                Some(format!("vec{to}<f32>({expr}{zeros})"))
            }
            _ => None,
        }
    }
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: WgslType,
}

/// A struct field definition.
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub ty: WgslType,
    pub attributes: Vec<Attribute>,
}

/// A struct definition.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
}

/// A function attribute (e.g., `@vertex`, `@group(0)`, `@binding(0)`).
#[derive(Debug, Clone, PartialEq)]
pub enum Attribute {
    Vertex,
    Fragment,
    Compute,
    Group(u32),
    Binding(u32),
    Location(u32),
    Builtin(String),
    Custom(String),
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Attribute::Vertex => write!(f, "@vertex"),
            Attribute::Fragment => write!(f, "@fragment"),
            Attribute::Compute => write!(f, "@compute"),
            Attribute::Group(n) => write!(f, "@group({n})"),
            Attribute::Binding(n) => write!(f, "@binding({n})"),
            Attribute::Location(n) => write!(f, "@location({n})"),
            Attribute::Builtin(name) => write!(f, "@builtin({name})"),
            Attribute::Custom(text) => write!(f, "@{text}"),
        }
    }
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
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
            BinaryOp::Equal => write!(f, "=="),
            BinaryOp::NotEqual => write!(f, "!="),
            BinaryOp::Less => write!(f, "<"),
            BinaryOp::LessEqual => write!(f, "<="),
            BinaryOp::Greater => write!(f, ">"),
            BinaryOp::GreaterEqual => write!(f, ">="),
        }
    }
}

/// An expression in the WGSL AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal value.
    Literal(Literal),
    /// A variable or identifier reference.
    Ident(String),
    /// A binary operation.
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    /// A unary operation.
    Unary(UnaryOp, Box<Expr>),
    /// A function call: `name(args...)`.
    Call(String, Vec<Expr>),
    /// A type constructor: `vec3<f32>(x, y, z)`.
    TypeConstructor(WgslType, Vec<Expr>),
    /// Member access: `expr.field`.
    MemberAccess(Box<Expr>, String),
    /// Index access: `expr[index]`.
    IndexAccess(Box<Expr>, Box<Expr>),
}

/// A literal value.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Float(f64),
    Int(i64),
    UInt(u64),
    Bool(bool),
}

/// A statement in the WGSL AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Variable declaration: `let name: ty = expr;` or `var name: ty = expr;`.
    Let {
        name: String,
        ty: Option<WgslType>,
        value: Expr,
        mutable: bool,
    },
    /// Assignment: `target = value;`.
    Assign(Expr, Expr),
    /// Return statement: `return expr;`.
    Return(Option<Expr>),
    /// If statement: `if (condition) { body } else { else_body }`.
    If {
        condition: Expr,
        body: Block,
        else_body: Option<Block>,
    },
    /// For loop: `for (init; condition; update) { body }`.
    For {
        init: Option<Box<Statement>>,
        condition: Option<Expr>,
        update: Option<Box<Statement>>,
        body: Block,
    },
    /// Expression statement (function call as statement).
    Expression(Expr),
    /// A block of statements.
    Block(Block),
}

/// A block of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
}

impl Block {
    pub fn new(statements: Vec<Statement>) -> Self {
        Self { statements }
    }

    pub fn empty() -> Self {
        Self {
            statements: Vec::new(),
        }
    }
}

/// A function definition in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<WgslType>,
    pub body: Block,
    pub attributes: Vec<Attribute>,
}

/// A global variable/uniform declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalVar {
    pub name: String,
    pub ty: WgslType,
    pub address_space: AddressSpace,
    pub attributes: Vec<Attribute>,
}

/// WGSL address spaces for global variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpace {
    Uniform,
    Storage,
    Private,
    Workgroup,
}

impl fmt::Display for AddressSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressSpace::Uniform => write!(f, "uniform"),
            AddressSpace::Storage => write!(f, "storage"),
            AddressSpace::Private => write!(f, "private"),
            AddressSpace::Workgroup => write!(f, "workgroup"),
        }
    }
}

/// Top-level WGSL module containing all definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct WgslModule {
    pub structs: Vec<StructDef>,
    pub globals: Vec<GlobalVar>,
    pub functions: Vec<Function>,
}

impl WgslModule {
    pub fn new() -> Self {
        Self {
            structs: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Find a function by name.
    pub fn find_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Find a struct by name.
    pub fn find_struct(&self, name: &str) -> Option<&StructDef> {
        self.structs.iter().find(|s| s.name == name)
    }
}

impl Default for WgslModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_type_display() {
        assert_eq!(ScalarType::F32.to_string(), "f32");
        assert_eq!(ScalarType::I32.to_string(), "i32");
        assert_eq!(ScalarType::U32.to_string(), "u32");
        assert_eq!(ScalarType::Bool.to_string(), "bool");
    }

    #[test]
    fn test_wgsl_type_display() {
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
            WgslType::Array(Box::new(WgslType::Scalar(ScalarType::F32)), Some(10)).to_string(),
            "array<f32, 10>"
        );
        assert_eq!(
            WgslType::Array(Box::new(WgslType::Scalar(ScalarType::F32)), None).to_string(),
            "array<f32>"
        );
        assert_eq!(
            WgslType::Struct("MyStruct".to_string()).to_string(),
            "MyStruct"
        );
    }

    #[test]
    fn test_type_promotion() {
        let f32_ty = WgslType::Scalar(ScalarType::F32);
        let vec2_ty = WgslType::Vector(ScalarType::F32, 2);
        let vec3_ty = WgslType::Vector(ScalarType::F32, 3);
        let vec4_ty = WgslType::Vector(ScalarType::F32, 4);

        // f32 promotes to all vectors
        assert!(f32_ty.can_promote_to(&vec2_ty));
        assert!(f32_ty.can_promote_to(&vec3_ty));
        assert!(f32_ty.can_promote_to(&vec4_ty));

        // vec2 promotes to vec3 and vec4
        assert!(vec2_ty.can_promote_to(&vec3_ty));
        assert!(vec2_ty.can_promote_to(&vec4_ty));

        // vec3 promotes to vec4
        assert!(vec3_ty.can_promote_to(&vec4_ty));

        // No downward promotion
        assert!(!vec4_ty.can_promote_to(&vec3_ty));
        assert!(!vec3_ty.can_promote_to(&vec2_ty));
    }

    #[test]
    fn test_promotion_wgsl() {
        let f32_ty = WgslType::Scalar(ScalarType::F32);
        let vec3_ty = WgslType::Vector(ScalarType::F32, 3);

        assert_eq!(
            f32_ty.promotion_wgsl("x", &vec3_ty),
            Some("vec3<f32>(x, 0.0, 0.0)".to_string())
        );

        let vec2_ty = WgslType::Vector(ScalarType::F32, 2);
        let vec4_ty = WgslType::Vector(ScalarType::F32, 4);
        assert_eq!(
            vec2_ty.promotion_wgsl("v", &vec4_ty),
            Some("vec4<f32>(v, 0.0, 0.0)".to_string())
        );
    }

    #[test]
    fn test_wgsl_module_find() {
        let module = WgslModule {
            structs: vec![StructDef {
                name: "Uniforms".to_string(),
                fields: vec![],
            }],
            globals: vec![],
            functions: vec![Function {
                name: "my_func".to_string(),
                parameters: vec![],
                return_type: Some(WgslType::Scalar(ScalarType::F32)),
                body: Block::empty(),
                attributes: vec![],
            }],
        };

        assert!(module.find_function("my_func").is_some());
        assert!(module.find_function("missing").is_none());
        assert!(module.find_struct("Uniforms").is_some());
        assert!(module.find_struct("Missing").is_none());
    }
}
