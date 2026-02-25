// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive Rust-to-WGSL type system mapping.
//!
//! Provides a [`TypeMapper`] that converts Rust [`syn::Type`] nodes into
//! [`WgslType`] values while tracking memory layout, struct definitions,
//! and providing clear error diagnostics for unsupported types.
//!
//! # Supported type categories
//!
//! | Rust type | WGSL type | Notes |
//! |-----------|-----------|-------|
//! | `f32` | `f32` | Direct mapping |
//! | `i32` | `i32` | Direct mapping |
//! | `u32` | `u32` | Direct mapping |
//! | `bool` | `bool` | Direct mapping |
//! | `Vec2` | `vec2<f32>` | Float vector |
//! | `Vec3` | `vec3<f32>` | Float vector |
//! | `Vec4` | `vec4<f32>` | Float vector |
//! | `IVec2` | `vec2<i32>` | Integer vector |
//! | `IVec3` | `vec3<i32>` | Integer vector |
//! | `IVec4` | `vec4<i32>` | Integer vector |
//! | `UVec2` | `vec2<u32>` | Unsigned integer vector |
//! | `UVec3` | `vec3<u32>` | Unsigned integer vector |
//! | `UVec4` | `vec4<u32>` | Unsigned integer vector |
//! | `Mat2` | `mat2x2<f32>` | Square matrix |
//! | `Mat3` | `mat3x3<f32>` | Square matrix |
//! | `Mat4` | `mat4x4<f32>` | Square matrix |
//! | `Mat2x3` etc. | `mat2x3<f32>` etc. | Non-square matrices |
//! | `[T; N]` | `array<T, N>` | Fixed-size arrays |
//! | Custom structs | WGSL struct | With proper alignment |

use std::collections::HashMap;
use std::fmt;

use proc_macro2::Span;
use syn::{Expr as SynExpr, Type};

use super::ast::*;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during type mapping.
#[derive(Debug, Clone)]
pub struct TypeMappingError {
    pub message: String,
    pub span: Span,
    pub suggestion: Option<String>,
    pub kind: TypeMappingErrorKind,
}

/// Classification of type mapping errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeMappingErrorKind {
    /// The type has no WGSL equivalent.
    UnsupportedType,
    /// A type is valid in Rust but not in WGSL (e.g., f64).
    IncompatibleType,
    /// Multi-segment paths are not supported.
    ComplexPath,
    /// Array length is not a literal integer.
    InvalidArrayLength,
    /// A struct field has an unsupported type.
    InvalidStructField,
    /// Alignment or layout issue.
    LayoutError,
    /// Type mismatch at function boundary.
    SignatureMismatch,
}

impl TypeMappingError {
    pub fn new(message: impl Into<String>, span: Span, kind: TypeMappingErrorKind) -> Self {
        Self {
            message: message.into(),
            span,
            suggestion: None,
            kind,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Convert to a `syn::Error` for proc-macro error reporting.
    pub fn into_syn_error(self) -> syn::Error {
        let mut msg = self.message;
        if let Some(ref sug) = self.suggestion {
            msg.push_str("\n  Suggestion: ");
            msg.push_str(sug);
        }
        syn::Error::new(self.span, msg)
    }
}

impl fmt::Display for TypeMappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(ref sug) = self.suggestion {
            write!(f, "\n  Suggestion: {sug}")?;
        }
        Ok(())
    }
}

impl std::error::Error for TypeMappingError {}

// ---------------------------------------------------------------------------
// Memory layout
// ---------------------------------------------------------------------------

/// Describes the memory layout of a WGSL type.
///
/// All sizes and alignments follow the WGSL specification for uniform
/// buffer layouts (std140-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLayout {
    /// Size in bytes.
    pub size: u32,
    /// Required alignment in bytes.
    pub align: u32,
}

impl MemoryLayout {
    pub const fn new(size: u32, align: u32) -> Self {
        Self { size, align }
    }
}

impl fmt::Display for MemoryLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "size={}, align={}", self.size, self.align)
    }
}

/// Return the memory layout for a WGSL type.
///
/// Uses the WGSL specification alignment rules for uniform buffers.
pub fn wgsl_type_layout(ty: &WgslType) -> MemoryLayout {
    match ty {
        // Scalars: 4 bytes, 4-byte aligned
        WgslType::Scalar(_) => MemoryLayout::new(4, 4),

        // Vectors: alignment = component_size * roundup(dim, 2)
        WgslType::Vector(_, dim) => {
            let component_size: u32 = 4; // f32/i32/u32 all 4 bytes
            let align_dim = if *dim == 3 { 4 } else { *dim as u32 };
            MemoryLayout::new(component_size * (*dim as u32), component_size * align_dim)
        }

        // Matrices: array of column vectors
        // mat{C}x{R}: C column vectors of vec{R}, each column aligned to vec{R}
        WgslType::Matrix(_, cols, rows) => {
            let col_layout = wgsl_type_layout(&WgslType::Vector(ScalarType::F32, *rows));
            let col_stride = round_up(col_layout.size, col_layout.align);
            MemoryLayout::new(col_stride * (*cols as u32), col_layout.align)
        }

        // Arrays: element stride rounded up to element alignment
        WgslType::Array(elem, size) => {
            let elem_layout = wgsl_type_layout(elem);
            // In uniform buffers, array element stride is rounded up to 16
            let stride = round_up(elem_layout.size, elem_layout.align.max(16));
            let total = stride * size;
            MemoryLayout::new(total, elem_layout.align.max(16))
        }

        // Structs: unknown without definition; use placeholder
        WgslType::Struct(_) => MemoryLayout::new(0, 16),

        WgslType::Void => MemoryLayout::new(0, 1),
    }
}

/// Round `value` up to the next multiple of `align`.
fn round_up(value: u32, align: u32) -> u32 {
    if align == 0 {
        return value;
    }
    (value + align - 1) / align * align
}

// ---------------------------------------------------------------------------
// Type info
// ---------------------------------------------------------------------------

/// Full information about a mapped WGSL type.
#[derive(Debug, Clone)]
pub struct WgslTypeInfo {
    /// The WGSL type.
    pub wgsl_type: WgslType,
    /// Memory layout (size and alignment).
    pub layout: MemoryLayout,
    /// Whether this type requires a struct definition in the output.
    pub requires_definition: bool,
}

impl WgslTypeInfo {
    pub fn new(wgsl_type: WgslType) -> Self {
        let layout = wgsl_type_layout(&wgsl_type);
        let requires_definition = matches!(&wgsl_type, WgslType::Struct(_));
        Self {
            wgsl_type,
            layout,
            requires_definition,
        }
    }
}

// ---------------------------------------------------------------------------
// Known type table
// ---------------------------------------------------------------------------

/// Entry in the known type mapping table.
struct KnownType {
    wgsl: WgslType,
}

/// Build the table of known Rust → WGSL type mappings.
fn known_types() -> HashMap<&'static str, KnownType> {
    let mut map = HashMap::with_capacity(32);

    let entry = |wgsl: WgslType| KnownType { wgsl };

    // --- Scalar types ---
    map.insert("f32", entry(WgslType::Scalar(ScalarType::F32)));
    map.insert("i32", entry(WgslType::Scalar(ScalarType::I32)));
    map.insert("u32", entry(WgslType::Scalar(ScalarType::U32)));
    map.insert("bool", entry(WgslType::Scalar(ScalarType::Bool)));

    // --- Float vector types ---
    map.insert("Vec2", entry(WgslType::Vector(ScalarType::F32, 2)));
    map.insert("Vec3", entry(WgslType::Vector(ScalarType::F32, 3)));
    map.insert("Vec4", entry(WgslType::Vector(ScalarType::F32, 4)));

    // --- Integer vector types ---
    map.insert("IVec2", entry(WgslType::Vector(ScalarType::I32, 2)));
    map.insert("IVec3", entry(WgslType::Vector(ScalarType::I32, 3)));
    map.insert("IVec4", entry(WgslType::Vector(ScalarType::I32, 4)));

    // --- Unsigned integer vector types ---
    map.insert("UVec2", entry(WgslType::Vector(ScalarType::U32, 2)));
    map.insert("UVec3", entry(WgslType::Vector(ScalarType::U32, 3)));
    map.insert("UVec4", entry(WgslType::Vector(ScalarType::U32, 4)));

    // --- Boolean vector types ---
    map.insert("BVec2", entry(WgslType::Vector(ScalarType::Bool, 2)));
    map.insert("BVec3", entry(WgslType::Vector(ScalarType::Bool, 3)));
    map.insert("BVec4", entry(WgslType::Vector(ScalarType::Bool, 4)));

    // --- Square matrix types ---
    map.insert("Mat2", entry(WgslType::Matrix(ScalarType::F32, 2, 2)));
    map.insert("Mat3", entry(WgslType::Matrix(ScalarType::F32, 3, 3)));
    map.insert("Mat4", entry(WgslType::Matrix(ScalarType::F32, 4, 4)));

    // --- Non-square matrix types ---
    map.insert("Mat2x3", entry(WgslType::Matrix(ScalarType::F32, 2, 3)));
    map.insert("Mat2x4", entry(WgslType::Matrix(ScalarType::F32, 2, 4)));
    map.insert("Mat3x2", entry(WgslType::Matrix(ScalarType::F32, 3, 2)));
    map.insert("Mat3x4", entry(WgslType::Matrix(ScalarType::F32, 3, 4)));
    map.insert("Mat4x2", entry(WgslType::Matrix(ScalarType::F32, 4, 2)));
    map.insert("Mat4x3", entry(WgslType::Matrix(ScalarType::F32, 4, 3)));

    map
}

/// Table of Rust types that are explicitly unsupported in WGSL,
/// paired with a helpful error message.
fn unsupported_types() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::with_capacity(16);
    map.insert("f64", "f64 is not supported in WGSL. Use f32 instead.");
    map.insert(
        "f16",
        "f16 requires the 'f16' WGSL extension. Use f32 for compatibility.",
    );
    map.insert("i8", "i8 is not supported in WGSL. Use i32 instead.");
    map.insert("i16", "i16 is not supported in WGSL. Use i32 instead.");
    map.insert("i64", "i64 is not supported in WGSL. Use i32 instead.");
    map.insert("i128", "i128 is not supported in WGSL. Use i32 instead.");
    map.insert("u8", "u8 is not supported in WGSL. Use u32 instead.");
    map.insert("u16", "u16 is not supported in WGSL. Use u32 instead.");
    map.insert("u64", "u64 is not supported in WGSL. Use u32 instead.");
    map.insert("u128", "u128 is not supported in WGSL. Use u32 instead.");
    map.insert("usize", "usize is not supported in WGSL. Use u32 instead.");
    map.insert("isize", "isize is not supported in WGSL. Use i32 instead.");
    map.insert(
        "String",
        "String is not supported in WGSL. Strings cannot be used in shaders.",
    );
    map.insert(
        "str",
        "str is not supported in WGSL. Strings cannot be used in shaders.",
    );
    map.insert(
        "char",
        "char is not supported in WGSL. Characters cannot be used in shaders.",
    );
    map
}

// ---------------------------------------------------------------------------
// TypeMapper
// ---------------------------------------------------------------------------

/// Context-aware type mapper that converts Rust types to WGSL types.
///
/// Tracks struct definitions encountered during conversion so that
/// they can be emitted as WGSL struct blocks by the code generator.
///
/// # Example
///
/// ```ignore
/// let mut mapper = TypeMapper::new();
/// let ty: syn::Type = syn::parse_quote!(Vec3);
/// let info = mapper.map_rust_type(&ty).unwrap();
/// assert_eq!(info.wgsl_type.to_string(), "vec3<f32>");
/// ```
pub struct TypeMapper {
    /// Cache of already-resolved types (keyed by string representation).
    type_cache: HashMap<String, WgslTypeInfo>,
    /// Struct definitions collected during conversion.
    struct_definitions: Vec<WgslStructDef>,
    /// Known Rust → WGSL type mappings.
    known: HashMap<&'static str, KnownType>,
    /// Known unsupported types with error messages.
    unsupported: HashMap<&'static str, &'static str>,
}

impl TypeMapper {
    /// Create a new `TypeMapper` with default mappings.
    pub fn new() -> Self {
        Self {
            type_cache: HashMap::new(),
            struct_definitions: Vec::new(),
            known: known_types(),
            unsupported: unsupported_types(),
        }
    }

    /// Map a Rust [`syn::Type`] to a [`WgslTypeInfo`].
    pub fn map_rust_type(&mut self, ty: &Type) -> Result<WgslTypeInfo, TypeMappingError> {
        match ty {
            Type::Path(type_path) => self.map_path_type(type_path),
            Type::Array(type_array) => self.map_array_type(type_array),
            Type::Reference(type_ref) => {
                // References are transparent in GPU context — map the inner type
                self.map_rust_type(&type_ref.elem)
            }
            Type::Tuple(tuple) if tuple.elems.is_empty() => {
                // () → void
                Ok(WgslTypeInfo::new(WgslType::Void))
            }
            _ => Err(TypeMappingError::new(
                format!(
                    "Unsupported type for WGSL conversion: {}",
                    quote::quote!(#ty)
                ),
                Span::call_site(),
                TypeMappingErrorKind::UnsupportedType,
            )
            .with_suggestion(
                "Supported types: f32, i32, u32, bool, Vec2-4, IVec2-4, UVec2-4, \
                 Mat2-4, Mat{C}x{R}, [T; N], and custom structs.",
            )),
        }
    }

    /// Map a path-based type (identifiers like `f32`, `Vec3`, custom structs).
    fn map_path_type(
        &mut self,
        type_path: &syn::TypePath,
    ) -> Result<WgslTypeInfo, TypeMappingError> {
        let path = &type_path.path;

        if path.segments.len() != 1 {
            return Err(TypeMappingError::new(
                "Multi-segment type paths are not supported in WGSL. \
                 Use a simple type name.",
                path.segments[0].ident.span(),
                TypeMappingErrorKind::ComplexPath,
            )
            .with_suggestion(
                "Import the type and use just the type name, e.g. `Vec3` instead of `glam::Vec3`.",
            ));
        }

        let ident = path.segments[0].ident.to_string();

        // Check cache first
        if let Some(cached) = self.type_cache.get(&ident) {
            return Ok(cached.clone());
        }

        // Check unsupported types (with helpful error)
        if let Some(msg) = self.unsupported.get(ident.as_str()) {
            return Err(TypeMappingError::new(
                *msg,
                path.segments[0].ident.span(),
                TypeMappingErrorKind::IncompatibleType,
            ));
        }

        // Check known types
        if let Some(known) = self.known.get(ident.as_str()) {
            let info = WgslTypeInfo::new(known.wgsl.clone());
            self.type_cache.insert(ident, info.clone());
            return Ok(info);
        }

        // Unknown identifier → treat as struct type
        let info = WgslTypeInfo::new(WgslType::Struct(ident.clone()));
        self.type_cache.insert(ident, info.clone());
        Ok(info)
    }

    /// Map an array type: `[T; N]` → `array<T, N>`.
    fn map_array_type(
        &mut self,
        type_array: &syn::TypeArray,
    ) -> Result<WgslTypeInfo, TypeMappingError> {
        let elem_info = self.map_rust_type(&type_array.elem)?;

        if let SynExpr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit_int),
            ..
        }) = &type_array.len
        {
            let size: u32 = lit_int.base10_parse().map_err(|_| {
                TypeMappingError::new(
                    "Invalid array length",
                    lit_int.span(),
                    TypeMappingErrorKind::InvalidArrayLength,
                )
            })?;

            let wgsl_type = WgslType::Array(Box::new(elem_info.wgsl_type), size);
            Ok(WgslTypeInfo::new(wgsl_type))
        } else {
            Err(TypeMappingError::new(
                "Only literal integer array lengths are supported in WGSL",
                Span::call_site(),
                TypeMappingErrorKind::InvalidArrayLength,
            )
            .with_suggestion("Use a constant literal, e.g. `[f32; 4]`."))
        }
    }

    /// Register a struct definition for WGSL output.
    ///
    /// Maps each field's Rust type to a WGSL type and stores the
    /// struct definition. Returns the overall [`WgslTypeInfo`] for
    /// the struct.
    pub fn register_struct(
        &mut self,
        name: &str,
        fields: &[(String, Type)],
    ) -> Result<WgslTypeInfo, TypeMappingError> {
        let mut wgsl_fields = Vec::with_capacity(fields.len());

        for (field_name, field_ty) in fields {
            let field_info = self.map_rust_type(field_ty).map_err(|e| {
                TypeMappingError::new(
                    format!("In struct '{name}', field '{field_name}': {}", e.message),
                    e.span,
                    TypeMappingErrorKind::InvalidStructField,
                )
            })?;
            wgsl_fields.push(WgslField {
                name: field_name.clone(),
                ty: field_info.wgsl_type,
            });
        }

        let def = WgslStructDef {
            name: name.to_string(),
            fields: wgsl_fields,
        };
        self.struct_definitions.push(def);

        let info = WgslTypeInfo::new(WgslType::Struct(name.to_string()));
        self.type_cache.insert(name.to_string(), info.clone());
        Ok(info)
    }

    /// Return accumulated struct definitions.
    pub fn struct_definitions(&self) -> &[WgslStructDef] {
        &self.struct_definitions
    }

    /// Generate WGSL text for all accumulated struct definitions.
    pub fn generate_struct_definitions(&self) -> String {
        use super::codegen::WgslCodeGen;

        if self.struct_definitions.is_empty() {
            return String::new();
        }

        let module = WgslModule {
            structs: self.struct_definitions.clone(),
            functions: Vec::new(),
        };
        let mut codegen = WgslCodeGen::new();
        codegen.generate_module(&module)
    }

    /// Validate that a function signature has compatible types for WGSL.
    ///
    /// Checks that all parameter types and the return type can be mapped
    /// to WGSL and reports any issues.
    pub fn validate_function_signature(
        &mut self,
        func: &syn::Signature,
    ) -> Result<(), Vec<TypeMappingError>> {
        let mut errors = Vec::new();

        for input in &func.inputs {
            if let syn::FnArg::Typed(pat_type) = input {
                if let Err(e) = self.map_rust_type(&pat_type.ty) {
                    errors.push(e);
                }
            }
        }

        if let syn::ReturnType::Type(_, ty) = &func.output {
            if let Err(e) = self.map_rust_type(ty) {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Compute the total byte size of a struct given its fields,
    /// including padding for alignment.
    pub fn compute_struct_layout(
        &mut self,
        fields: &[(String, Type)],
    ) -> Result<MemoryLayout, TypeMappingError> {
        let mut offset: u32 = 0;
        let mut max_align: u32 = 1;

        for (name, ty) in fields {
            let info = self.map_rust_type(ty).map_err(|e| {
                TypeMappingError::new(
                    format!("Layout error in field '{name}': {}", e.message),
                    e.span,
                    TypeMappingErrorKind::LayoutError,
                )
            })?;

            // Align offset to field alignment
            offset = round_up(offset, info.layout.align);
            offset += info.layout.size;
            if info.layout.align > max_align {
                max_align = info.layout.align;
            }
        }

        // Final struct size is rounded up to struct alignment
        // Struct alignment in WGSL is max(16, max field alignment) for uniform
        let struct_align = max_align.max(16);
        let struct_size = round_up(offset, struct_align);

        Ok(MemoryLayout::new(struct_size, struct_align))
    }
}

impl Default for TypeMapper {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    // --- Primitive type mapping ---

    #[test]
    fn map_f32() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(f32);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Scalar(ScalarType::F32));
        assert_eq!(info.layout, MemoryLayout::new(4, 4));
    }

    #[test]
    fn map_i32() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(i32);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Scalar(ScalarType::I32));
    }

    #[test]
    fn map_u32() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(u32);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Scalar(ScalarType::U32));
    }

    #[test]
    fn map_bool() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(bool);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Scalar(ScalarType::Bool));
    }

    // --- Vector type mapping ---

    #[test]
    fn map_vec2() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Vec2);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Vector(ScalarType::F32, 2));
        assert_eq!(info.layout, MemoryLayout::new(8, 8));
    }

    #[test]
    fn map_vec3() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Vec3);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Vector(ScalarType::F32, 3));
        // vec3<f32> has 12 bytes but 16-byte alignment
        assert_eq!(info.layout, MemoryLayout::new(12, 16));
    }

    #[test]
    fn map_vec4() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Vec4);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Vector(ScalarType::F32, 4));
        assert_eq!(info.layout, MemoryLayout::new(16, 16));
    }

    #[test]
    fn map_ivec2() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(IVec2);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Vector(ScalarType::I32, 2));
    }

    #[test]
    fn map_uvec3() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(UVec3);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Vector(ScalarType::U32, 3));
    }

    #[test]
    fn map_bvec4() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(BVec4);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Vector(ScalarType::Bool, 4));
    }

    // --- Matrix type mapping ---

    #[test]
    fn map_mat2() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Mat2);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Matrix(ScalarType::F32, 2, 2));
    }

    #[test]
    fn map_mat3() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Mat3);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Matrix(ScalarType::F32, 3, 3));
    }

    #[test]
    fn map_mat4() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Mat4);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Matrix(ScalarType::F32, 4, 4));
        // mat4x4<f32>: 4 columns of vec4<f32>, each 16 bytes = 64 bytes total
        assert_eq!(info.layout, MemoryLayout::new(64, 16));
    }

    #[test]
    fn map_mat2x3() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Mat2x3);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Matrix(ScalarType::F32, 2, 3));
    }

    #[test]
    fn map_mat4x2() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Mat4x2);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Matrix(ScalarType::F32, 4, 2));
    }

    // --- Array type mapping ---

    #[test]
    fn map_array_f32() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!([f32; 4]);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(
            info.wgsl_type,
            WgslType::Array(Box::new(WgslType::Scalar(ScalarType::F32)), 4)
        );
    }

    #[test]
    fn map_nested_array() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!([[f32; 3]; 2]);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(
            info.wgsl_type,
            WgslType::Array(
                Box::new(WgslType::Array(
                    Box::new(WgslType::Scalar(ScalarType::F32)),
                    3
                )),
                2
            )
        );
    }

    // --- Struct type mapping ---

    #[test]
    fn map_custom_struct() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(MyUniforms);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Struct("MyUniforms".to_string()));
        assert!(info.requires_definition);
    }

    #[test]
    fn register_struct_definition() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, Type)> = vec![
            ("position".to_string(), parse_quote!(Vec3)),
            ("scale".to_string(), parse_quote!(f32)),
            ("color".to_string(), parse_quote!(Vec4)),
        ];
        let info = mapper.register_struct("Transform", &fields).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Struct("Transform".to_string()));

        let defs = mapper.struct_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "Transform");
        assert_eq!(defs[0].fields.len(), 3);
        assert_eq!(defs[0].fields[0].ty, WgslType::Vector(ScalarType::F32, 3));
        assert_eq!(defs[0].fields[1].ty, WgslType::Scalar(ScalarType::F32));
        assert_eq!(defs[0].fields[2].ty, WgslType::Vector(ScalarType::F32, 4));
    }

    #[test]
    fn generate_struct_wgsl() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, Type)> = vec![
            ("diffuse".to_string(), parse_quote!(Vec3)),
            ("roughness".to_string(), parse_quote!(f32)),
        ];
        mapper.register_struct("Material", &fields).unwrap();

        let wgsl = mapper.generate_struct_definitions();
        assert!(wgsl.contains("struct Material {"));
        assert!(wgsl.contains("diffuse: vec3<f32>,"));
        assert!(wgsl.contains("roughness: f32,"));
    }

    // --- Error handling ---

    #[test]
    fn error_f64() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(f64);
        let err = mapper.map_rust_type(&ty).unwrap_err();
        assert_eq!(err.kind, TypeMappingErrorKind::IncompatibleType);
        assert!(err.message.contains("f64"));
        assert!(err.message.contains("f32"));
    }

    #[test]
    fn error_string() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(String);
        let err = mapper.map_rust_type(&ty).unwrap_err();
        assert_eq!(err.kind, TypeMappingErrorKind::IncompatibleType);
        assert!(err.message.contains("String"));
    }

    #[test]
    fn error_u64() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(u64);
        let err = mapper.map_rust_type(&ty).unwrap_err();
        assert_eq!(err.kind, TypeMappingErrorKind::IncompatibleType);
        assert!(err.message.contains("u32"));
    }

    #[test]
    fn error_usize() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(usize);
        let err = mapper.map_rust_type(&ty).unwrap_err();
        assert_eq!(err.kind, TypeMappingErrorKind::IncompatibleType);
    }

    #[test]
    fn error_complex_path() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(std::f32);
        let err = mapper.map_rust_type(&ty).unwrap_err();
        assert_eq!(err.kind, TypeMappingErrorKind::ComplexPath);
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn error_struct_field() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, Type)> = vec![
            ("good_field".to_string(), parse_quote!(f32)),
            ("bad_field".to_string(), parse_quote!(f64)),
        ];
        let err = mapper.register_struct("Bad", &fields).unwrap_err();
        assert_eq!(err.kind, TypeMappingErrorKind::InvalidStructField);
        assert!(err.message.contains("bad_field"));
    }

    // --- Reference transparency ---

    #[test]
    fn map_reference_transparent() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(&f32);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Scalar(ScalarType::F32));
    }

    // --- Unit type ---

    #[test]
    fn map_unit_to_void() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(());
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type, WgslType::Void);
    }

    // --- Caching ---

    #[test]
    fn caches_resolved_types() {
        let mut mapper = TypeMapper::new();
        let ty: Type = parse_quote!(Vec3);
        let info1 = mapper.map_rust_type(&ty).unwrap();
        let info2 = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info1.wgsl_type, info2.wgsl_type);
    }

    // --- Memory layout ---

    #[test]
    fn layout_scalar() {
        let layout = wgsl_type_layout(&WgslType::Scalar(ScalarType::F32));
        assert_eq!(layout, MemoryLayout::new(4, 4));
    }

    #[test]
    fn layout_vec2() {
        let layout = wgsl_type_layout(&WgslType::Vector(ScalarType::F32, 2));
        assert_eq!(layout, MemoryLayout::new(8, 8));
    }

    #[test]
    fn layout_vec3() {
        // vec3 has special alignment rules: size 12, alignment 16
        let layout = wgsl_type_layout(&WgslType::Vector(ScalarType::F32, 3));
        assert_eq!(layout, MemoryLayout::new(12, 16));
    }

    #[test]
    fn layout_vec4() {
        let layout = wgsl_type_layout(&WgslType::Vector(ScalarType::F32, 4));
        assert_eq!(layout, MemoryLayout::new(16, 16));
    }

    #[test]
    fn layout_mat4x4() {
        // mat4x4<f32>: 4 columns × vec4<f32> (16 bytes each) = 64 bytes
        let layout = wgsl_type_layout(&WgslType::Matrix(ScalarType::F32, 4, 4));
        assert_eq!(layout, MemoryLayout::new(64, 16));
    }

    #[test]
    fn layout_mat2x2() {
        // mat2x2<f32>: 2 columns × vec2<f32> (8 bytes, 8-byte aligned) = 16 bytes
        let layout = wgsl_type_layout(&WgslType::Matrix(ScalarType::F32, 2, 2));
        assert_eq!(layout, MemoryLayout::new(16, 8));
    }

    #[test]
    fn layout_array() {
        // array<f32, 4>: each element is rounded up to 16-byte stride
        let layout = wgsl_type_layout(&WgslType::Array(
            Box::new(WgslType::Scalar(ScalarType::F32)),
            4,
        ));
        assert_eq!(layout, MemoryLayout::new(64, 16));
    }

    // --- Function signature validation ---

    #[test]
    fn validate_good_signature() {
        let mut mapper = TypeMapper::new();
        let sig: syn::Signature = parse_quote! {
            fn transform(value: f32, scale: f32) -> Vec3
        };
        assert!(mapper.validate_function_signature(&sig).is_ok());
    }

    #[test]
    fn validate_bad_signature() {
        let mut mapper = TypeMapper::new();
        let sig: syn::Signature = parse_quote! {
            fn transform(value: f64) -> f32
        };
        let errors = mapper.validate_function_signature(&sig).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("f64"));
    }

    // --- Struct layout computation ---

    #[test]
    fn compute_struct_layout_basic() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, Type)> = vec![
            ("x".to_string(), parse_quote!(f32)),
            ("y".to_string(), parse_quote!(f32)),
        ];
        let layout = mapper.compute_struct_layout(&fields).unwrap();
        // Two f32 fields = 8 bytes, but struct align is min 16 → size 16
        assert_eq!(layout.align, 16);
        assert!(layout.size >= 8);
    }

    #[test]
    fn compute_struct_layout_with_vec3() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, Type)> = vec![
            ("position".to_string(), parse_quote!(Vec3)),
            ("scale".to_string(), parse_quote!(f32)),
        ];
        let layout = mapper.compute_struct_layout(&fields).unwrap();
        // Vec3 = 12 bytes at 16-byte align, then f32 at offset 12 (4-byte align OK)
        // Total = 16 bytes, struct align = 16
        assert_eq!(layout.align, 16);
        assert_eq!(layout.size, 16);
    }

    // --- TypeMappingError display ---

    #[test]
    fn error_display_with_suggestion() {
        let err = TypeMappingError::new(
            "f64 is not supported",
            Span::call_site(),
            TypeMappingErrorKind::IncompatibleType,
        )
        .with_suggestion("Use f32 instead");

        let display = format!("{err}");
        assert!(display.contains("f64 is not supported"));
        assert!(display.contains("Use f32 instead"));
    }

    #[test]
    fn error_to_syn_error() {
        let err = TypeMappingError::new(
            "test error",
            Span::call_site(),
            TypeMappingErrorKind::UnsupportedType,
        );
        let syn_err = err.into_syn_error();
        assert!(syn_err.to_string().contains("test error"));
    }
}
