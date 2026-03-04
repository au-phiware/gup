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
    /// 32-bit floating point.
    F32,
    /// 32-bit signed integer.
    I32,
    /// 32-bit unsigned integer.
    U32,
    /// Boolean value.
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
    /// Atomic type: `atomic<{inner}>`.
    Atomic(Box<WgslType>),
    /// Pointer type: `ptr<{address_space}, {inner}>`.
    Pointer(AddressSpace, Box<WgslType>),
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
            WgslType::Atomic(inner) => write!(f, "atomic<{inner}>"),
            WgslType::Pointer(space, inner) => write!(f, "ptr<{space}, {inner}>"),
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
    /// - `f32` → `vec2<f32>`, `vec3<f32>`, `vec4<f32>` (broadcast)
    /// - `vec2<f32>` → `vec3<f32>`, `vec4<f32>` (zero-padded)
    /// - `vec3<f32>` → `vec4<f32>` (zero-padded)
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
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: WgslType,
    /// Attributes on the parameter (e.g., `@builtin(global_invocation_id)`).
    pub attributes: Vec<Attribute>,
}

/// A struct field definition.
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: WgslType,
    /// Attributes on the field.
    pub attributes: Vec<Attribute>,
}

/// A struct definition.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    /// Struct name.
    pub name: String,
    /// Fields of the struct.
    pub fields: Vec<StructField>,
}

/// A function attribute (e.g., `@vertex`, `@group(0)`, `@binding(0)`).
#[derive(Debug, Clone, PartialEq)]
pub enum Attribute {
    /// Vertex shader entry point.
    Vertex,
    /// Fragment shader entry point.
    Fragment,
    /// Compute shader entry point.
    Compute,
    /// `@workgroup_size(x)`, `@workgroup_size(x, y)`, or `@workgroup_size(x, y, z)`.
    WorkgroupSize(u32, Option<u32>, Option<u32>),
    /// Bind group index.
    Group(u32),
    /// Binding index within a group.
    Binding(u32),
    /// Location index for input/output.
    Location(u32),
    /// Built-in variable reference.
    Builtin(String),
    /// Custom attribute string.
    Custom(String),
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Attribute::Vertex => write!(f, "@vertex"),
            Attribute::Fragment => write!(f, "@fragment"),
            Attribute::Compute => write!(f, "@compute"),
            Attribute::WorkgroupSize(x, None, None) => write!(f, "@workgroup_size({x})"),
            Attribute::WorkgroupSize(x, Some(y), None) => write!(f, "@workgroup_size({x}, {y})"),
            Attribute::WorkgroupSize(x, Some(y), Some(z)) => {
                write!(f, "@workgroup_size({x}, {y}, {z})")
            }
            Attribute::WorkgroupSize(x, None, Some(z)) => {
                write!(f, "@workgroup_size({x}, 1, {z})")
            }
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
    /// Arithmetic negation: `-expr`.
    Negate,
    /// Logical negation: `!expr`.
    Not,
    /// Address-of: `&expr`.
    AddressOf,
    /// Dereference: `*expr`.
    Deref,
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// Addition (`+`).
    Add,
    /// Subtraction (`-`).
    Sub,
    /// Multiplication (`*`).
    Mul,
    /// Division (`/`).
    Div,
    /// Modulo (`%`).
    Mod,
    /// Logical AND (`&&`).
    And,
    /// Logical OR (`||`).
    Or,
    /// Equality (`==`).
    Equal,
    /// Inequality (`!=`).
    NotEqual,
    /// Less than (`<`).
    Less,
    /// Less than or equal (`<=`).
    LessEqual,
    /// Greater than (`>`).
    Greater,
    /// Greater than or equal (`>=`).
    GreaterEqual,
    /// Bitwise AND (`&`).
    BitwiseAnd,
    /// Bitwise OR (`|`).
    BitwiseOr,
    /// Bitwise XOR (`^`).
    BitwiseXor,
    /// Left shift (`<<`).
    ShiftLeft,
    /// Right shift (`>>`).
    ShiftRight,
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
            BinaryOp::BitwiseAnd => write!(f, "&"),
            BinaryOp::BitwiseOr => write!(f, "|"),
            BinaryOp::BitwiseXor => write!(f, "^"),
            BinaryOp::ShiftLeft => write!(f, "<<"),
            BinaryOp::ShiftRight => write!(f, ">>"),
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
    /// Floating-point literal.
    Float(f64),
    /// Signed integer literal.
    Int(i64),
    /// Unsigned integer literal.
    UInt(u64),
    /// Boolean literal.
    Bool(bool),
}

/// A statement in the WGSL AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Variable declaration: `let name: ty = expr;` or `var name: ty = expr;`.
    Let {
        /// Variable name.
        name: String,
        /// Optional explicit type annotation.
        ty: Option<WgslType>,
        /// Initializer expression.
        value: Expr,
        /// Whether the binding is mutable (`var` vs `let`).
        mutable: bool,
    },
    /// Assignment: `target = value;`.
    Assign(Expr, Expr),
    /// Compound assignment: `target += value;`, etc.
    CompoundAssign(Expr, BinaryOp, Expr),
    /// Return statement: `return expr;`.
    Return(Option<Expr>),
    /// If statement: `if (condition) { body } else { else_body }`.
    If {
        /// Condition expression.
        condition: Expr,
        /// Body executed when the condition is true.
        body: Block,
        /// Optional else branch.
        else_body: Option<Block>,
    },
    /// For loop: `for (init; condition; update) { body }`.
    For {
        /// Optional initializer statement.
        init: Option<Box<Statement>>,
        /// Optional loop condition.
        condition: Option<Expr>,
        /// Optional update statement.
        update: Option<Box<Statement>>,
        /// Loop body.
        body: Block,
    },
    /// Loop: `loop { body }`.
    Loop {
        /// Loop body.
        body: Block,
    },
    /// Break statement.
    Break,
    /// Continue statement.
    Continue,
    /// Switch statement: `switch (expr) { case N: { ... } default: { ... } }`.
    Switch {
        /// Expression being switched on.
        subject: Expr,
        /// Switch cases including the optional default.
        cases: Vec<SwitchCase>,
    },
    /// Expression statement (function call as statement).
    Expression(Expr),
    /// A block of statements.
    Block(Block),
}

/// A single case in a switch statement.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    /// The match expression, or `None` for `default`.
    pub selector: Option<Expr>,
    /// Statements executed when this case matches.
    pub body: Block,
}

/// A block of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Ordered list of statements in the block.
    pub statements: Vec<Statement>,
}

impl Block {
    /// Create a new block from a list of statements.
    pub fn new(statements: Vec<Statement>) -> Self {
        Self { statements }
    }

    /// Create an empty block with no statements.
    pub fn empty() -> Self {
        Self {
            statements: Vec::new(),
        }
    }
}

/// A function definition in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    /// Function name.
    pub name: String,
    /// Formal parameters.
    pub parameters: Vec<Parameter>,
    /// Return type, or `None` for void functions.
    pub return_type: Option<WgslType>,
    /// Attributes on the return type (e.g., `@location(0)`).
    pub return_attributes: Vec<Attribute>,
    /// Function body.
    pub body: Block,
    /// Attributes on the function (e.g., `@vertex`).
    pub attributes: Vec<Attribute>,
}

/// A global variable/uniform declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalVar {
    /// Variable name.
    pub name: String,
    /// Variable type.
    pub ty: WgslType,
    /// Address space (uniform, storage, etc.).
    pub address_space: AddressSpace,
    /// Attributes (e.g., `@group`, `@binding`).
    pub attributes: Vec<Attribute>,
}

/// Access mode for storage address space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessMode {
    /// Read-only access.
    Read,
    /// Read-write access.
    ReadWrite,
}

impl fmt::Display for AccessMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccessMode::Read => write!(f, "read"),
            AccessMode::ReadWrite => write!(f, "read_write"),
        }
    }
}

/// WGSL address spaces for global variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressSpace {
    /// Uniform address space for read-only data.
    Uniform,
    /// Storage address space with an access mode.
    Storage(AccessMode),
    /// Private address space (per-invocation).
    Private,
    /// Workgroup shared address space.
    Workgroup,
    /// Function-scope address space (used in pointer types).
    Function,
}

impl fmt::Display for AddressSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressSpace::Uniform => write!(f, "uniform"),
            AddressSpace::Storage(mode) => write!(f, "storage, {mode}"),
            AddressSpace::Private => write!(f, "private"),
            AddressSpace::Workgroup => write!(f, "workgroup"),
            AddressSpace::Function => write!(f, "function"),
        }
    }
}

/// A top-level constant declaration: `const NAME: TYPE = EXPR;`.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalConst {
    /// Constant name.
    pub name: String,
    /// Optional explicit type annotation.
    pub ty: Option<WgslType>,
    /// Constant value expression.
    pub value: Expr,
}

/// Top-level WGSL module containing all definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct WgslModule {
    /// Struct definitions.
    pub structs: Vec<StructDef>,
    /// Global variable declarations.
    pub globals: Vec<GlobalVar>,
    /// Top-level constant declarations.
    pub constants: Vec<GlobalConst>,
    /// Function definitions.
    pub functions: Vec<Function>,
}

impl WgslModule {
    /// Create an empty WGSL module.
    pub fn new() -> Self {
        Self {
            structs: Vec::new(),
            globals: Vec::new(),
            constants: Vec::new(),
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
            constants: vec![],
            functions: vec![Function {
                name: "my_func".to_string(),
                parameters: vec![],
                return_type: Some(WgslType::Scalar(ScalarType::F32)),
                body: Block::empty(),
                attributes: vec![],
                return_attributes: vec![],
            }],
        };

        assert!(module.find_function("my_func").is_some());
        assert!(module.find_function("missing").is_none());
        assert!(module.find_struct("Uniforms").is_some());
        assert!(module.find_struct("Missing").is_none());
    }
}
