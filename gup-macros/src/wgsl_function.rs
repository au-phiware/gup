// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Procedural macro implementation for `#[wgsl_function]`
//!
//! This module implements the core procedural macro that parses WGSL function syntax
//! and generates corresponding Rust trait implementations.

use std::collections::HashMap;
use std::sync::LazyLock;

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{
    BinOp, Block, Error, Expr, ExprBinary, ExprField, ExprIf, ExprPath, ExprReturn, FnArg, Ident,
    ItemFn, Member, Pat, PatType, Result, Stmt, Type, Visibility,
    parse::{Parse, ParseStream},
};

/// Parsed WGSL function information extracted from the procedural macro input
#[derive(Debug, Clone)]
pub struct WgslFunctionInfo {
    /// Name of the function in WGSL (e.g., "linear_scale")
    pub function_name: String,
    /// Name of the generated Rust struct (e.g., "LinearScale")
    pub struct_name: Ident,
    /// Name of the generated uniforms struct (e.g., "LinearScaleUniforms")
    pub uniforms_name: Ident,
    /// Input type for the shader function
    pub input_type: Type,
    /// Output type for the shader function
    pub output_type: Type,
    /// Parameters that will become uniform fields
    pub uniform_params: Vec<UniformParam>,
    /// The original WGSL function body as a string
    pub wgsl_body: String,
    /// Custom types that may have WgslStruct definitions
    pub custom_types: Vec<Type>,
}

/// Information about a uniform parameter
#[derive(Debug, Clone)]
pub struct UniformParam {
    /// Parameter name
    pub name: Ident,
    /// Parameter type in Rust
    pub rust_type: Type,
    /// Parameter type in WGSL
    #[allow(dead_code)] // Used in WGSL code generation
    pub wgsl_type: String,
}

/// Parse the input to the `#[wgsl_function]` procedural macro
impl Parse for WgslFunctionInfo {
    fn parse(input: ParseStream) -> Result<Self> {
        let function: ItemFn = input.parse().map_err(|e| {
            Error::new(e.span(), "Expected a function definition. The #[wgsl_function] attribute can only be applied to functions.")
        })?;

        // Check function visibility - warn about private functions
        if matches!(function.vis, Visibility::Inherited) {
            // Note: This doesn't fail compilation, just generates a potentially less useful function
        }

        // Extract function name and validate it
        let function_name = function.sig.ident.to_string();
        if function_name.is_empty() {
            return Err(Error::new_spanned(
                &function.sig.ident,
                "Function name cannot be empty",
            ));
        }

        // Validate function name follows conventions
        if function_name.contains("__") {
            return Err(Error::new_spanned(
                &function.sig.ident,
                "Function names with double underscores are reserved. Use single underscores instead.",
            ));
        }

        // Generate struct names based on function name
        let struct_name = pascal_case(&function_name);
        if struct_name.is_empty() {
            return Err(Error::new_spanned(
                &function.sig.ident,
                "Could not generate struct name from function name. Use a valid identifier.",
            ));
        }

        let uniforms_name = format_ident!("{}Uniforms", struct_name);
        let struct_name = format_ident!("{}", struct_name);

        // Parse function signature to extract input/output types and uniform parameters
        let (input_type, output_type, uniform_params) = parse_function_signature(&function.sig)
            .map_err(|e| Error::new(e.span(), format!("Function signature error: {e}")))?;

        // Collect custom types (non-primitives) that might need struct definitions
        let mut custom_types = Vec::new();

        // Check input type
        if is_custom_type(&input_type) {
            custom_types.push(input_type.clone());
        }

        // Check output type
        if is_custom_type(&output_type) {
            custom_types.push(output_type.clone());
        }

        // Check uniform parameter types
        for param in &uniform_params {
            if is_custom_type(&param.rust_type) {
                custom_types.push(param.rust_type.clone());
            }
        }

        // Extract WGSL function body
        let wgsl_body = extract_wgsl_body(&function)
            .map_err(|e| Error::new(e.span(), format!("WGSL body generation error: {e}")))?;

        Ok(WgslFunctionInfo {
            function_name,
            struct_name,
            uniforms_name,
            input_type,
            output_type,
            uniform_params,
            wgsl_body,
            custom_types,
        })
    }
}

/// Parse function signature to extract types and parameters
fn parse_function_signature(sig: &syn::Signature) -> Result<(Type, Type, Vec<UniformParam>)> {
    let inputs = &sig.inputs;

    if inputs.is_empty() {
        return Err(Error::new_spanned(
            sig,
            "WGSL function must have at least one input parameter (the value to transform)",
        ));
    }

    // Check for unsupported features
    if !sig.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &sig.generics,
            "Generic functions are not yet supported in #[wgsl_function]. Consider using concrete types instead.",
        ));
    }

    if sig.asyncness.is_some() {
        return Err(Error::new_spanned(
            sig.asyncness,
            "Async functions are not supported in #[wgsl_function]. WGSL functions are synchronous.",
        ));
    }

    if sig.unsafety.is_some() {
        return Err(Error::new_spanned(
            sig.unsafety,
            "Unsafe functions are not supported in #[wgsl_function]. WGSL is memory-safe by design.",
        ));
    }

    // First parameter is the input value
    let input_type = match inputs.first().unwrap() {
        FnArg::Typed(PatType { ty, .. }) => {
            // Validate that the input type is supported
            rust_type_to_wgsl_type(ty).map_err(|e| {
                Error::new_spanned(ty, format!("Unsupported input type for WGSL function: {e}"))
            })?;
            (**ty).clone()
        }
        FnArg::Receiver(_) => {
            return Err(Error::new_spanned(
                inputs.first().unwrap(),
                "WGSL functions cannot have 'self' parameters. Use free functions instead.",
            ));
        }
    };

    // Extract output type from return type
    let output_type = match &sig.output {
        syn::ReturnType::Type(_, ty) => {
            // Validate that the output type is supported
            rust_type_to_wgsl_type(ty).map_err(|e| {
                Error::new_spanned(
                    ty,
                    format!("Unsupported return type for WGSL function: {e}"),
                )
            })?;
            (**ty).clone()
        }
        syn::ReturnType::Default => {
            return Err(Error::new_spanned(
                sig,
                "WGSL functions must have an explicit return type. Add '-> ReturnType' to your function signature.",
            ));
        }
    };

    // Remaining parameters become uniform parameters
    let mut uniform_params = Vec::new();
    for (i, input) in inputs.iter().skip(1).enumerate() {
        match input {
            FnArg::Typed(PatType { pat, ty, .. }) => {
                if let Pat::Ident(pat_ident) = &**pat {
                    // Validate uniform parameter type
                    let wgsl_type = rust_type_to_wgsl_type(ty).map_err(|e| {
                        Error::new_spanned(
                            ty,
                            format!("Unsupported uniform parameter type in parameter {}: {}. Only types that implement bytemuck::Pod + bytemuck::Zeroable are supported.", i + 2, e)
                        )
                    })?;

                    // Check if type is compatible with GPU uniforms
                    if !is_uniform_compatible_type(ty) {
                        return Err(Error::new_spanned(
                            ty,
                            format!(
                                "Type '{ty}' cannot be used in GPU uniforms. Use f32, i32, u32, Vec2, Vec3, Vec4, or arrays of these types.",
                                ty = quote!(#ty)
                            ),
                        ));
                    }

                    uniform_params.push(UniformParam {
                        name: pat_ident.ident.clone(),
                        rust_type: (**ty).clone(),
                        wgsl_type,
                    });
                } else {
                    return Err(Error::new_spanned(
                        pat,
                        "Only simple identifiers are allowed as parameter names. Complex patterns are not supported.",
                    ));
                }
            }
            FnArg::Receiver(_) => {
                return Err(Error::new_spanned(
                    input,
                    "WGSL functions cannot have 'self' parameters. Use free functions instead.",
                ));
            }
        }
    }

    Ok((input_type, output_type, uniform_params))
}

/// Extract WGSL function body from the Rust function
/// Optimized to pre-allocate string capacity and reduce reallocations
fn extract_wgsl_body(function: &ItemFn) -> Result<String> {
    let function_name = &function.sig.ident;
    let inputs = &function.sig.inputs;
    let output = &function.sig.output;

    // Pre-allocate capacity for parameters (estimate 30 chars per param)
    let mut wgsl_params = Vec::with_capacity(inputs.len());

    // First parameter is the input value
    if let Some(FnArg::Typed(PatType { pat, ty, .. })) = inputs.first()
        && let Pat::Ident(pat_ident) = &**pat
    {
        let wgsl_type = rust_type_to_wgsl_type(ty)?;
        wgsl_params.push(format!("{}: {}", pat_ident.ident, wgsl_type));
    }

    // Collect uniform struct definition if there are uniform parameters
    // Pre-allocate for typical function size (200 bytes base + 50 per line)
    let estimated_size = 200 + (inputs.len() * 50);
    let mut wgsl_output = String::with_capacity(estimated_size);

    if inputs.len() > 1 {
        let uniforms_name = format!("{}Uniforms", pascal_case(&function_name.to_string()));

        // Generate WGSL struct definition for uniforms
        let mut struct_fields = Vec::with_capacity(inputs.len() - 1);
        for input in inputs.iter().skip(1) {
            if let FnArg::Typed(PatType { pat, ty, .. }) = input
                && let Pat::Ident(pat_ident) = &**pat
            {
                let field_name = &pat_ident.ident;
                let wgsl_type = rust_type_to_wgsl_type(ty)?;
                struct_fields.push(format!("    {field_name}: {wgsl_type}"));
            }
        }

        // Use write! macro for more efficient string building
        use std::fmt::Write;
        write!(
            &mut wgsl_output,
            "struct {} {{\n{},\n}}\n\n",
            uniforms_name,
            struct_fields.join(",\n")
        )
        .unwrap();

        wgsl_params.push(format!("uniforms: {uniforms_name}"));
    }

    // Build return type
    let return_type = match output {
        syn::ReturnType::Type(_, ty) => rust_type_to_wgsl_type(ty)?,
        syn::ReturnType::Default => "void".to_string(),
    };

    // Parse function body and convert to WGSL
    let body_wgsl = translate_body_to_wgsl(&function.block, inputs)?;

    // Generate complete WGSL function using write! for efficiency
    use std::fmt::Write;
    write!(
        &mut wgsl_output,
        "fn {}({}) -> {} {{\n{}\n}}",
        function_name,
        wgsl_params.join(", "),
        return_type,
        body_wgsl
    )
    .unwrap();

    Ok(wgsl_output)
}

/// Translate a Rust function body to WGSL
/// Optimized with pre-allocated vectors
fn translate_body_to_wgsl(
    block: &Block,
    inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
) -> Result<String> {
    // Pre-allocate based on statement count
    let mut wgsl_statements = Vec::with_capacity(block.stmts.len());

    // Extract uniform parameter names for field access translation
    let uniform_params: Vec<String> = inputs
        .iter()
        .skip(1) // Skip first param (the value)
        .filter_map(|arg| {
            if let FnArg::Typed(PatType { pat, .. }) = arg
                && let Pat::Ident(pat_ident) = &**pat
            {
                return Some(pat_ident.ident.to_string());
            }
            None
        })
        .collect();

    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(expr, semi) => {
                let wgsl_expr = translate_expr_to_wgsl(expr, &uniform_params)?;
                if semi.is_some() {
                    wgsl_statements.push(format!("    {wgsl_expr};"));
                } else {
                    // Expression without semicolon (implicit return in Rust)
                    wgsl_statements.push(format!("    return {wgsl_expr};"));
                }
            }
            Stmt::Local(local) => {
                // Handle let bindings
                if let Pat::Ident(pat_ident) = &local.pat {
                    let var_name = &pat_ident.ident;
                    if let Some(init) = &local.init {
                        let init_expr = translate_expr_to_wgsl(&init.expr, &uniform_params)?;
                        wgsl_statements.push(format!("    let {var_name} = {init_expr};"));
                    }
                } else {
                    return Err(Error::new_spanned(
                        local,
                        "Only simple variable bindings are supported in WGSL functions",
                    ));
                }
            }
            Stmt::Item(_) => {
                return Err(Error::new_spanned(
                    stmt,
                    "Item definitions are not supported in WGSL function bodies",
                ));
            }
            Stmt::Macro(_) => {
                return Err(Error::new_spanned(
                    stmt,
                    "Macro invocations are not supported in WGSL function bodies",
                ));
            }
        }
    }

    Ok(wgsl_statements.join("\n"))
}

/// Translate a Rust expression to WGSL
fn translate_expr_to_wgsl(expr: &Expr, uniform_params: &[String]) -> Result<String> {
    match expr {
        Expr::Path(ExprPath { path, .. }) => {
            // Simple identifier reference
            let ident = path
                .get_ident()
                .ok_or_else(|| Error::new_spanned(path, "Complex paths not supported"))?;
            let ident_str = ident.to_string();

            // Check if this identifier is a uniform parameter
            if uniform_params.contains(&ident_str) {
                Ok(format!("uniforms.{ident_str}"))
            } else {
                Ok(ident_str)
            }
        }
        Expr::Field(ExprField { base, member, .. }) => {
            // Field access (e.g., scale.domain_min)
            let base_expr = translate_expr_to_wgsl(base, uniform_params)?;

            // Check if base is a uniform parameter - if so, prefix with "uniforms."
            let base_str = base_expr.as_str();
            let prefixed_base = if uniform_params.contains(&base_str.to_string()) {
                format!("uniforms.{base_str}")
            } else {
                base_expr
            };

            match member {
                Member::Named(field_name) => Ok(format!("{prefixed_base}.{field_name}")),
                Member::Unnamed(index) => Ok(format!("{prefixed_base}._{}", index.index)),
            }
        }
        Expr::Binary(ExprBinary {
            left, op, right, ..
        }) => {
            // Binary operations (e.g., a + b, a * b)
            let left_wgsl = translate_expr_to_wgsl(left, uniform_params)?;
            let right_wgsl = translate_expr_to_wgsl(right, uniform_params)?;
            let op_wgsl = translate_binop_to_wgsl(op)?;
            Ok(format!("{left_wgsl} {op_wgsl} {right_wgsl}"))
        }
        Expr::Paren(paren) => {
            // Parenthesized expression
            let inner = translate_expr_to_wgsl(&paren.expr, uniform_params)?;
            Ok(format!("({inner})"))
        }
        Expr::Return(ExprReturn { expr, .. }) => {
            // Return statement
            if let Some(ret_expr) = expr {
                let ret_wgsl = translate_expr_to_wgsl(ret_expr, uniform_params)?;
                Ok(format!("return {ret_wgsl}"))
            } else {
                Ok("return".to_string())
            }
        }
        Expr::Lit(lit) => {
            // Literal values
            Ok(quote!(#lit).to_string())
        }
        Expr::Call(call) => {
            // Function calls (e.g., vec2(x, y), clamp(value, 0.0, 1.0))
            let func_name = translate_expr_to_wgsl(&call.func, uniform_params)?;
            let args: Result<Vec<String>> = call
                .args
                .iter()
                .map(|arg| translate_expr_to_wgsl(arg, uniform_params))
                .collect();
            let args_wgsl = args?.join(", ");

            // Map common Rust functions to WGSL equivalents
            let wgsl_func_name = match func_name.as_str() {
                // Rust type constructors to WGSL
                "Vec2" => "vec2<f32>",
                "Vec3" => "vec3<f32>",
                "Vec4" => "vec4<f32>",
                "Mat2" => "mat2x2<f32>",
                "Mat3" => "mat3x3<f32>",
                "Mat4" => "mat4x4<f32>",
                // Keep WGSL built-ins as-is
                "abs" | "acos" | "asin" | "atan" | "atan2" | "ceil" | "clamp" | "cos" | "cross"
                | "distance" | "dot" | "exp" | "exp2" | "floor" | "fract" | "inverseSqrt"
                | "length" | "log" | "log2" | "max" | "min" | "mix" | "normalize" | "pow"
                | "round" | "sign" | "sin" | "smoothstep" | "sqrt" | "step" | "tan" | "trunc" => {
                    func_name.as_str()
                }
                _ => func_name.as_str(),
            };

            Ok(format!("{wgsl_func_name}({args_wgsl})"))
        }
        Expr::MethodCall(method_call) => {
            // Method calls - translate some common patterns
            let receiver = translate_expr_to_wgsl(&method_call.receiver, uniform_params)?;
            let method_name = &method_call.method;

            // Handle some common Rust methods that have WGSL equivalents
            match method_name.to_string().as_str() {
                "abs" => Ok(format!("abs({receiver})")),
                "sqrt" => Ok(format!("sqrt({receiver})")),
                "min" | "max" | "clamp" => {
                    let args: Result<Vec<String>> = method_call
                        .args
                        .iter()
                        .map(|arg| translate_expr_to_wgsl(arg, uniform_params))
                        .collect();
                    let args_wgsl = args?.join(", ");
                    Ok(format!("{}({receiver}, {args_wgsl})", method_name))
                }
                _ => Err(Error::new_spanned(
                    method_call,
                    format!(
                        "Method '{}' not supported in WGSL translation. Use function call syntax instead.",
                        method_name
                    ),
                )),
            }
        }
        Expr::If(ExprIf {
            cond,
            then_branch,
            else_branch,
            ..
        }) => {
            // If-else expressions
            let cond_wgsl = translate_expr_to_wgsl(cond, uniform_params)?;
            let then_wgsl =
                translate_body_to_wgsl(then_branch, &syn::punctuated::Punctuated::new())?;

            let mut wgsl = format!("if ({cond_wgsl}) {{\n{then_wgsl}\n    }}");

            if let Some((_, else_expr)) = else_branch {
                match &**else_expr {
                    Expr::Block(block) => {
                        let else_wgsl = translate_body_to_wgsl(
                            &block.block,
                            &syn::punctuated::Punctuated::new(),
                        )?;
                        wgsl.push_str(&format!(" else {{\n{else_wgsl}\n    }}"));
                    }
                    Expr::If(_) => {
                        let else_wgsl = translate_expr_to_wgsl(else_expr, uniform_params)?;
                        wgsl.push_str(&format!(" else {else_wgsl}"));
                    }
                    _ => {
                        return Err(Error::new_spanned(
                            else_expr,
                            "Only block or if expressions are supported in else branches",
                        ));
                    }
                }
            }

            Ok(wgsl)
        }
        Expr::Unary(unary) => {
            // Unary operations (-, !)
            let inner = translate_expr_to_wgsl(&unary.expr, uniform_params)?;
            match unary.op {
                syn::UnOp::Neg(_) => Ok(format!("-{inner}")),
                syn::UnOp::Not(_) => Ok(format!("!{inner}")),
                _ => Err(Error::new_spanned(
                    unary,
                    "Unsupported unary operator for WGSL",
                )),
            }
        }
        _ => Err(Error::new_spanned(
            expr,
            "This expression type is not yet supported in WGSL translation. Supported: binary ops, field access, function calls, literals, return statements.",
        )),
    }
}

/// Translate binary operator to WGSL
fn translate_binop_to_wgsl(op: &BinOp) -> Result<&'static str> {
    match op {
        BinOp::Add(_) => Ok("+"),
        BinOp::Sub(_) => Ok("-"),
        BinOp::Mul(_) => Ok("*"),
        BinOp::Div(_) => Ok("/"),
        BinOp::Rem(_) => Ok("%"),
        BinOp::And(_) => Ok("&&"),
        BinOp::Or(_) => Ok("||"),
        BinOp::BitXor(_) => Ok("^"),
        BinOp::BitAnd(_) => Ok("&"),
        BinOp::BitOr(_) => Ok("|"),
        BinOp::Shl(_) => Ok("<<"),
        BinOp::Shr(_) => Ok(">>"),
        BinOp::Eq(_) => Ok("=="),
        BinOp::Lt(_) => Ok("<"),
        BinOp::Le(_) => Ok("<="),
        BinOp::Ne(_) => Ok("!="),
        BinOp::Ge(_) => Ok(">="),
        BinOp::Gt(_) => Ok(">"),
        _ => Err(Error::new(
            proc_macro2::Span::call_site(),
            "Unsupported binary operator for WGSL",
        )),
    }
}

/// Type mapping cache for common Rust to WGSL type conversions
/// This cache reduces the number of string allocations during macro expansion
static TYPE_CACHE: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::with_capacity(32);

    // Scalar types
    map.insert("f32", "f32");
    map.insert("i32", "i32");
    map.insert("u32", "u32");
    map.insert("bool", "bool");

    // Vector types
    map.insert("Vec2", "vec2<f32>");
    map.insert("Vec3", "vec3<f32>");
    map.insert("Vec4", "vec4<f32>");

    // Matrix types - square
    map.insert("Mat2", "mat2x2<f32>");
    map.insert("Mat3", "mat3x3<f32>");
    map.insert("Mat4", "mat4x4<f32>");

    // Matrix types - non-square
    map.insert("Mat2x3", "mat2x3<f32>");
    map.insert("Mat2x4", "mat2x4<f32>");
    map.insert("Mat3x2", "mat3x2<f32>");
    map.insert("Mat3x4", "mat3x4<f32>");
    map.insert("Mat4x2", "mat4x2<f32>");
    map.insert("Mat4x3", "mat4x3<f32>");

    // Texture types
    map.insert("Texture1D", "texture_1d<f32>");
    map.insert("Texture2D", "texture_2d<f32>");
    map.insert("Texture3D", "texture_3d<f32>");
    map.insert("TextureCube", "texture_cube<f32>");
    map.insert("Texture2DArray", "texture_2d_array<f32>");
    map.insert("TextureCubeArray", "texture_cube_array<f32>");
    map.insert("TextureMultisampled2D", "texture_multisampled_2d<f32>");

    // Storage texture types
    map.insert("TextureStorage1D", "texture_storage_1d<rgba8unorm, read>");
    map.insert("TextureStorage2D", "texture_storage_2d<rgba8unorm, read>");
    map.insert("TextureStorage3D", "texture_storage_3d<rgba8unorm, read>");

    // Sampler types
    map.insert("Sampler", "sampler");
    map.insert("SamplerComparison", "sampler_comparison");

    map
});

/// Convert Rust type to WGSL type string (optimized version)
/// Uses a static cache for common types to reduce allocations
fn rust_type_to_wgsl_type(ty: &Type) -> Result<String> {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            if path.segments.len() == 1 {
                let segment = &path.segments[0];
                let type_name = segment.ident.to_string();

                // Fast path: Check cache first
                if let Some(&wgsl_type) = TYPE_CACHE.get(type_name.as_str()) {
                    return Ok(wgsl_type.to_string());
                }

                // Custom type - use as-is
                Ok(type_name)
            } else {
                Err(Error::new_spanned(
                    ty,
                    "Complex type paths not yet supported",
                ))
            }
        }
        Type::Array(type_array) => {
            // Handle array types like [f32; 4]
            let elem_type = rust_type_to_wgsl_type(&type_array.elem)?;
            match &type_array.len {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(lit_int),
                    ..
                }) => {
                    let len = lit_int
                        .base10_parse::<usize>()
                        .map_err(|_| Error::new_spanned(lit_int, "Invalid array length"))?;
                    Ok(format!("array<{elem_type}, {len}>"))
                }
                _ => Err(Error::new_spanned(
                    &type_array.len,
                    "Only literal array lengths are supported",
                )),
            }
        }
        _ => Err(Error::new_spanned(
            ty,
            "Unsupported type for WGSL conversion",
        )),
    }
}

/// Check if a Rust type can be used in uniforms (must implement Pod + Zeroable)
fn is_uniform_compatible_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            if path.segments.len() == 1 {
                let segment = &path.segments[0];
                match segment.ident.to_string().as_str() {
                    "f32" | "i32" | "u32" | "bool" => true,
                    "Vec2" | "Vec3" | "Vec4" => true,
                    // Square matrices
                    "Mat2" | "Mat3" | "Mat4" => true,
                    // Non-square matrices
                    "Mat2x3" | "Mat2x4" | "Mat3x2" | "Mat3x4" | "Mat4x2" | "Mat4x3" => true,
                    // Texture and sampler types cannot be used in uniforms
                    // They must be passed as bindings
                    "Texture1D"
                    | "Texture2D"
                    | "Texture3D"
                    | "TextureCube"
                    | "Texture2DArray"
                    | "TextureCubeArray"
                    | "TextureMultisampled2D"
                    | "TextureStorage1D"
                    | "TextureStorage2D"
                    | "TextureStorage3D"
                    | "Sampler"
                    | "SamplerComparison" => false,
                    // Custom types are assumed to be uniform-compatible if they implement Pod + Zeroable
                    // The derive macro or user will ensure this
                    _ => true,
                }
            } else {
                false
            }
        }
        Type::Array(type_array) => {
            // Arrays of uniform-compatible types are also uniform-compatible
            is_uniform_compatible_type(&type_array.elem)
        }
        _ => false,
    }
}

/// Check if a type is a custom (non-primitive) type that might have a WgslStruct definition
fn is_custom_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            if path.segments.len() == 1 {
                let segment = &path.segments[0];
                let type_name = segment.ident.to_string();
                // Return true if it's not a known primitive type
                !matches!(
                    type_name.as_str(),
                    "f32"
                        | "i32"
                        | "u32"
                        | "bool"
                        | "Vec2"
                        | "Vec3"
                        | "Vec4"
                        | "Mat2"
                        | "Mat3"
                        | "Mat4"
                        | "Mat2x3"
                        | "Mat2x4"
                        | "Mat3x2"
                        | "Mat3x4"
                        | "Mat4x2"
                        | "Mat4x3"
                        | "Texture1D"
                        | "Texture2D"
                        | "Texture3D"
                        | "TextureCube"
                        | "Texture2DArray"
                        | "TextureCubeArray"
                        | "TextureMultisampled2D"
                        | "TextureStorage1D"
                        | "TextureStorage2D"
                        | "TextureStorage3D"
                        | "Sampler"
                        | "SamplerComparison"
                )
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Convert snake_case to PascalCase
fn pascal_case(s: &str) -> String {
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

/// Generate the complete TokenStream for the parsed WGSL function
impl ToTokens for WgslFunctionInfo {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let struct_name = &self.struct_name;
        let uniforms_name = &self.uniforms_name;
        let input_type = &self.input_type;
        let output_type = &self.output_type;
        let function_name = &self.function_name;
        let wgsl_body = &self.wgsl_body;
        let custom_types = &self.custom_types;

        // Generate uniform struct fields
        let uniform_fields = self.uniform_params.iter().map(|param| {
            let name = &param.name;
            let ty = &param.rust_type;

            // Convert Vec types to arrays for uniforms
            match ty {
                Type::Path(type_path) if type_path.path.segments.len() == 1 => {
                    match type_path.path.segments[0].ident.to_string().as_str() {
                        "Vec2" => quote! { pub #name: [f32; 2] },
                        "Vec3" => quote! { pub #name: [f32; 3] },
                        "Vec4" => quote! { pub #name: [f32; 4] },
                        _ => quote! { pub #name: #ty },
                    }
                }
                _ => quote! { pub #name: #ty },
            }
        });

        // Generate struct fields (same as uniform fields for now)
        let struct_fields = self.uniform_params.iter().map(|param| {
            let name = &param.name;
            let ty = &param.rust_type;
            quote! { pub #name: #ty }
        });

        // Generate constructor parameters
        let constructor_params = self.uniform_params.iter().map(|param| {
            let name = &param.name;
            let ty = &param.rust_type;
            quote! { #name: #ty }
        });

        // Generate constructor field assignments
        let constructor_fields = self.uniform_params.iter().map(|param| {
            let name = &param.name;
            quote! { #name }
        });

        // Generate uniform creation
        let uniform_creation = if self.uniform_params.is_empty() {
            quote! { Some(#uniforms_name) }
        } else {
            let uniform_fields = self.uniform_params.iter().map(|param| {
                let name = &param.name;
                let ty = &param.rust_type;
                // Convert Vec types to arrays for uniforms
                match ty {
                    Type::Path(type_path) if type_path.path.segments.len() == 1 => {
                        match type_path.path.segments[0].ident.to_string().as_str() {
                            "Vec2" => quote! { #name: [self.#name.x, self.#name.y] },
                            "Vec3" => quote! { #name: [self.#name.x, self.#name.y, self.#name.z] },
                            "Vec4" => quote! { #name: [self.#name.x, self.#name.y, self.#name.z, self.#name.w] },
                            _ => quote! { #name: self.#name },
                        }
                    }
                    _ => quote! { #name: self.#name },
                }
            });
            quote! {
                Some(#uniforms_name {
                    #(#uniform_fields),*
                })
            }
        };

        // Generate uniform struct only if there are uniform parameters
        let (uniform_struct, shader_uniform_impl) = if self.uniform_params.is_empty() {
            let impl_uniform = quote! {
                // Implement ShaderUniform for unit type
                impl crate::shader_function::ShaderUniform for #uniforms_name {
                    fn wgsl_struct_definition() -> String {
                        "struct ".to_string() + stringify!(#uniforms_name) + " {}"
                    }

                    fn wgsl_type_name() -> &'static str {
                        stringify!(#uniforms_name)
                    }
                }
            };
            let struct_def = quote! {
                // Unit type for functions with no uniforms
                #[repr(C)]
                #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
                pub struct #uniforms_name;
            };
            (struct_def, impl_uniform)
        } else {
            // Build field names and types for ShaderUniform impl
            let field_infos: Vec<_> = self
                .uniform_params
                .iter()
                .map(|param| {
                    let name = &param.name;
                    let name_str = name.to_string();
                    let ty = &param.rust_type;
                    (name_str, ty.clone())
                })
                .collect();

            let wgsl_fields = field_infos.iter().map(|(name, ty)| {
                let wgsl_type = if let Type::Path(type_path) = ty {
                    if type_path.path.segments.len() == 1 {
                        match type_path.path.segments[0].ident.to_string().as_str() {
                            "Vec2" => "vec2<f32>".to_string(),
                            "Vec3" => "vec3<f32>".to_string(),
                            "Vec4" => "vec4<f32>".to_string(),
                            other => other.to_string(),
                        }
                    } else {
                        ty.to_token_stream().to_string()
                    }
                } else {
                    ty.to_token_stream().to_string()
                };
                format!("    {}: {}", name, wgsl_type)
            });

            let impl_uniform = quote! {
                // Implement ShaderUniform for the generated uniforms
                impl crate::shader_function::ShaderUniform for #uniforms_name {
                    fn wgsl_struct_definition() -> String {
                        format!(
                            "struct {} {{\n{}\n}}",
                            stringify!(#uniforms_name),
                            [#(#wgsl_fields),*].join(",\n")
                        )
                    }

                    fn wgsl_type_name() -> &'static str {
                        stringify!(#uniforms_name)
                    }
                }
            };

            let struct_def = quote! {
                #[repr(C)]
                #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
                pub struct #uniforms_name {
                    #(#uniform_fields),*
                }
            };
            (struct_def, impl_uniform)
        };

        // Generate constructor only if there are parameters
        let constructor = if self.uniform_params.is_empty() {
            quote! {
                impl #struct_name {
                    pub fn new() -> Self {
                        Self
                    }
                }

                impl Default for #struct_name {
                    fn default() -> Self {
                        Self::new()
                    }
                }
            }
        } else {
            quote! {
                impl #struct_name {
                    pub fn new(#(#constructor_params),*) -> Self {
                        Self {
                            #(#constructor_fields),*
                        }
                    }
                }
            }
        };

        // Generate struct definition
        let struct_def = if self.uniform_params.is_empty() {
            quote! {
                #[derive(Debug, Clone)]
                pub struct #struct_name;
            }
        } else {
            quote! {
                #[derive(Debug, Clone)]
                pub struct #struct_name {
                    #(#struct_fields),*
                }
            }
        };

        let generated = quote! {
            // Generate the uniform structure
            #uniform_struct

            // Implement ShaderUniform for the uniforms
            #shader_uniform_impl

            // Generate the main struct
            #struct_def

            // Generate constructor
            #constructor

            impl crate::shader_function::ComposableShaderFunction for #struct_name {
                type Input = #input_type;
                type Output = #output_type;
                type Uniforms = #uniforms_name;

                fn wgsl_function() -> &'static str {
                    #wgsl_body
                }

                fn generate_wgsl(&self) -> String {
                    // Collect custom struct definitions from types that implement ShaderType
                    let mut definitions: Vec<String> = Vec::new();

                    // Add definitions for each custom type if available
                    #(
                        if let Some(def) = <#custom_types as crate::shader_function::ShaderType>::wgsl_type_definition() {
                            definitions.push(def.to_string());
                        }
                    )*

                    // Prepend struct definitions to the function body
                    if definitions.is_empty() {
                        Self::wgsl_function().to_string()
                    } else {
                        format!("{}\n\n{}", definitions.join("\n\n"), Self::wgsl_function())
                    }
                }

                fn create_uniforms(&self) -> Option<Self::Uniforms> {
                    #uniform_creation
                }

                fn function_name() -> &'static str {
                    #function_name
                }
            }
        };

        tokens.extend(generated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::{parse_quote, parse2};

    #[test]
    fn test_pascal_case() {
        assert_eq!(pascal_case("linear_scale"), "LinearScale");
        assert_eq!(pascal_case("color_map"), "ColorMap");
        assert_eq!(pascal_case("simple"), "Simple");
        assert_eq!(pascal_case("position_transform"), "PositionTransform");
        assert_eq!(pascal_case("complex_function_name"), "ComplexFunctionName");
    }

    #[test]
    fn test_rust_type_to_wgsl_type() {
        // Basic scalar types
        let f32_type: Type = parse_quote!(f32);
        assert_eq!(rust_type_to_wgsl_type(&f32_type).unwrap(), "f32");

        let i32_type: Type = parse_quote!(i32);
        assert_eq!(rust_type_to_wgsl_type(&i32_type).unwrap(), "i32");

        let u32_type: Type = parse_quote!(u32);
        assert_eq!(rust_type_to_wgsl_type(&u32_type).unwrap(), "u32");

        let bool_type: Type = parse_quote!(bool);
        assert_eq!(rust_type_to_wgsl_type(&bool_type).unwrap(), "bool");

        // Vector types
        let vec2_type: Type = parse_quote!(Vec2);
        assert_eq!(rust_type_to_wgsl_type(&vec2_type).unwrap(), "vec2<f32>");

        let vec3_type: Type = parse_quote!(Vec3);
        assert_eq!(rust_type_to_wgsl_type(&vec3_type).unwrap(), "vec3<f32>");

        let vec4_type: Type = parse_quote!(Vec4);
        assert_eq!(rust_type_to_wgsl_type(&vec4_type).unwrap(), "vec4<f32>");

        // Matrix types
        let mat4_type: Type = parse_quote!(Mat4);
        assert_eq!(rust_type_to_wgsl_type(&mat4_type).unwrap(), "mat4x4<f32>");

        // Array types
        let array_type: Type = parse_quote!([f32; 4]);
        assert_eq!(
            rust_type_to_wgsl_type(&array_type).unwrap(),
            "array<f32, 4>"
        );

        // Non-square matrix types
        let mat2x3_type: Type = parse_quote!(Mat2x3);
        assert_eq!(rust_type_to_wgsl_type(&mat2x3_type).unwrap(), "mat2x3<f32>");

        let mat3x4_type: Type = parse_quote!(Mat3x4);
        assert_eq!(rust_type_to_wgsl_type(&mat3x4_type).unwrap(), "mat3x4<f32>");

        let mat4x2_type: Type = parse_quote!(Mat4x2);
        assert_eq!(rust_type_to_wgsl_type(&mat4x2_type).unwrap(), "mat4x2<f32>");

        // Texture types
        let texture2d_type: Type = parse_quote!(Texture2D);
        assert_eq!(
            rust_type_to_wgsl_type(&texture2d_type).unwrap(),
            "texture_2d<f32>"
        );

        let texture3d_type: Type = parse_quote!(Texture3D);
        assert_eq!(
            rust_type_to_wgsl_type(&texture3d_type).unwrap(),
            "texture_3d<f32>"
        );

        let texturecube_type: Type = parse_quote!(TextureCube);
        assert_eq!(
            rust_type_to_wgsl_type(&texturecube_type).unwrap(),
            "texture_cube<f32>"
        );

        // Sampler types
        let sampler_type: Type = parse_quote!(Sampler);
        assert_eq!(rust_type_to_wgsl_type(&sampler_type).unwrap(), "sampler");

        let sampler_comparison_type: Type = parse_quote!(SamplerComparison);
        assert_eq!(
            rust_type_to_wgsl_type(&sampler_comparison_type).unwrap(),
            "sampler_comparison"
        );
    }

    #[test]
    fn test_extended_matrix_types() {
        // Test all non-square matrix types
        let mat2x3: Type = parse_quote!(Mat2x3);
        assert_eq!(rust_type_to_wgsl_type(&mat2x3).unwrap(), "mat2x3<f32>");

        let mat2x4: Type = parse_quote!(Mat2x4);
        assert_eq!(rust_type_to_wgsl_type(&mat2x4).unwrap(), "mat2x4<f32>");

        let mat3x2: Type = parse_quote!(Mat3x2);
        assert_eq!(rust_type_to_wgsl_type(&mat3x2).unwrap(), "mat3x2<f32>");

        let mat3x4: Type = parse_quote!(Mat3x4);
        assert_eq!(rust_type_to_wgsl_type(&mat3x4).unwrap(), "mat3x4<f32>");

        let mat4x2: Type = parse_quote!(Mat4x2);
        assert_eq!(rust_type_to_wgsl_type(&mat4x2).unwrap(), "mat4x2<f32>");

        let mat4x3: Type = parse_quote!(Mat4x3);
        assert_eq!(rust_type_to_wgsl_type(&mat4x3).unwrap(), "mat4x3<f32>");
    }

    #[test]
    fn test_texture_and_sampler_types() {
        // Texture types
        let texture1d: Type = parse_quote!(Texture1D);
        assert_eq!(
            rust_type_to_wgsl_type(&texture1d).unwrap(),
            "texture_1d<f32>"
        );

        let texture2d: Type = parse_quote!(Texture2D);
        assert_eq!(
            rust_type_to_wgsl_type(&texture2d).unwrap(),
            "texture_2d<f32>"
        );

        let texture3d: Type = parse_quote!(Texture3D);
        assert_eq!(
            rust_type_to_wgsl_type(&texture3d).unwrap(),
            "texture_3d<f32>"
        );

        let texturecube: Type = parse_quote!(TextureCube);
        assert_eq!(
            rust_type_to_wgsl_type(&texturecube).unwrap(),
            "texture_cube<f32>"
        );

        let texture2darray: Type = parse_quote!(Texture2DArray);
        assert_eq!(
            rust_type_to_wgsl_type(&texture2darray).unwrap(),
            "texture_2d_array<f32>"
        );

        // Sampler types
        let sampler: Type = parse_quote!(Sampler);
        assert_eq!(rust_type_to_wgsl_type(&sampler).unwrap(), "sampler");

        let sampler_comparison: Type = parse_quote!(SamplerComparison);
        assert_eq!(
            rust_type_to_wgsl_type(&sampler_comparison).unwrap(),
            "sampler_comparison"
        );
    }

    #[test]
    fn test_is_uniform_compatible_type() {
        let f32_type: Type = parse_quote!(f32);
        assert!(is_uniform_compatible_type(&f32_type));

        let vec2_type: Type = parse_quote!(Vec2);
        assert!(is_uniform_compatible_type(&vec2_type));

        let array_type: Type = parse_quote!([f32; 4]);
        assert!(is_uniform_compatible_type(&array_type));

        // Matrix types should be uniform compatible
        let mat2_type: Type = parse_quote!(Mat2);
        assert!(is_uniform_compatible_type(&mat2_type));

        let mat3x4_type: Type = parse_quote!(Mat3x4);
        assert!(is_uniform_compatible_type(&mat3x4_type));

        // Texture and sampler types should NOT be uniform compatible
        let texture2d_type: Type = parse_quote!(Texture2D);
        assert!(!is_uniform_compatible_type(&texture2d_type));

        let sampler_type: Type = parse_quote!(Sampler);
        assert!(!is_uniform_compatible_type(&sampler_type));

        // Custom types should return false (need explicit verification)
        let custom_type: Type = parse_quote!(MyCustomType);
        assert!(!is_uniform_compatible_type(&custom_type));
    }

    #[test]
    fn test_parse_simple_function() {
        let input = quote! {
            fn linear_scale(value: f32, scale: f32) -> f32 {
                return value * scale;
            }
        };

        let parsed: WgslFunctionInfo = parse2(input).unwrap();
        assert_eq!(parsed.function_name, "linear_scale");
        assert_eq!(parsed.struct_name, "LinearScale");
        assert_eq!(parsed.uniform_params.len(), 1);
        assert_eq!(parsed.uniform_params[0].name, "scale");
        assert_eq!(parsed.uniform_params[0].wgsl_type, "f32");
    }

    #[test]
    fn test_parse_function_with_multiple_uniforms() {
        let input = quote! {
            fn transform(pos: Vec2, scale: f32, offset: Vec2) -> Vec2 {
                return pos * scale + offset;
            }
        };

        let parsed: WgslFunctionInfo = parse2(input).unwrap();
        assert_eq!(parsed.function_name, "transform");
        assert_eq!(parsed.struct_name, "Transform");
        assert_eq!(parsed.uniform_params.len(), 2);
        assert_eq!(parsed.uniform_params[0].name, "scale");
        assert_eq!(parsed.uniform_params[0].wgsl_type, "f32");
        assert_eq!(parsed.uniform_params[1].name, "offset");
        assert_eq!(parsed.uniform_params[1].wgsl_type, "vec2<f32>");
    }

    #[test]
    fn test_parse_function_with_no_uniforms() {
        let input = quote! {
            fn identity(value: f32) -> f32 {
                return value;
            }
        };

        let parsed: WgslFunctionInfo = parse2(input).unwrap();
        assert_eq!(parsed.function_name, "identity");
        assert_eq!(parsed.struct_name, "Identity");
        assert_eq!(parsed.uniform_params.len(), 0);
    }

    #[test]
    fn test_parse_function_with_vector_types() {
        let input = quote! {
            fn color_transform(color: Vec4, tint: Vec4) -> Vec4 {
                return color * tint;
            }
        };

        let parsed: WgslFunctionInfo = parse2(input).unwrap();
        assert_eq!(parsed.function_name, "color_transform");
        assert_eq!(parsed.struct_name, "ColorTransform");
        assert_eq!(parsed.uniform_params.len(), 1);
        assert_eq!(parsed.uniform_params[0].name, "tint");
        assert_eq!(parsed.uniform_params[0].wgsl_type, "vec4<f32>");
    }

    #[test]
    fn test_error_no_parameters() {
        let input = quote! {
            fn invalid() -> f32 {
                return 0.0;
            }
        };

        let result: Result<WgslFunctionInfo> = parse2(input);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must have at least one input parameter")
        );
    }

    #[test]
    fn test_error_no_return_type() {
        let input = quote! {
            fn invalid(value: f32) {
                // No return type
            }
        };

        let result: Result<WgslFunctionInfo> = parse2(input);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must have an explicit return type")
        );
    }

    #[test]
    fn test_error_async_function() {
        let input = quote! {
            async fn invalid(value: f32) -> f32 {
                return value;
            }
        };

        let result: Result<WgslFunctionInfo> = parse2(input);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Async functions are not supported")
        );
    }

    #[test]
    fn test_error_unsafe_function() {
        let input = quote! {
            unsafe fn invalid(value: f32) -> f32 {
                return value;
            }
        };

        let result: Result<WgslFunctionInfo> = parse2(input);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Unsafe functions are not supported")
        );
    }

    #[test]
    fn test_error_generic_function() {
        let input = quote! {
            fn invalid<T>(value: T) -> T {
                return value;
            }
        };

        let result: Result<WgslFunctionInfo> = parse2(input);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Generic functions are not yet supported")
        );
    }

    #[test]
    fn test_error_self_parameter() {
        let input = quote! {
            fn invalid(&self, value: f32) -> f32 {
                return value;
            }
        };

        let result: Result<WgslFunctionInfo> = parse2(input);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("cannot have 'self' parameters"));
    }

    #[test]
    fn test_error_reserved_name() {
        let input = quote! {
            fn invalid__name(value: f32) -> f32 {
                return value;
            }
        };

        let result: Result<WgslFunctionInfo> = parse2(input);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("double underscores are reserved")
        );
    }

    #[test]
    fn test_code_generation() {
        let input = quote! {
            fn linear_scale(value: f32, scale: f32, offset: f32) -> f32 {
                return value * scale + offset;
            }
        };

        let parsed: WgslFunctionInfo = parse2(input).unwrap();
        let mut tokens = proc_macro2::TokenStream::new();
        parsed.to_tokens(&mut tokens);

        let generated_code = tokens.to_string();

        // Check that the generated code contains expected elements
        assert!(generated_code.contains("struct LinearScale"));
        assert!(generated_code.contains("struct LinearScaleUniforms"));
        assert!(generated_code.contains("impl LinearScale"));
        assert!(generated_code.contains("ComposableShaderFunction for LinearScale"));
        assert!(generated_code.contains("pub scale : f32"));
        assert!(generated_code.contains("pub offset : f32"));
        assert!(generated_code.contains("fn new (scale : f32 , offset : f32)"));
    }

    #[test]
    fn test_code_generation_no_uniforms() {
        let input = quote! {
            fn identity(value: f32) -> f32 {
                return value;
            }
        };

        let parsed: WgslFunctionInfo = parse2(input).unwrap();
        let mut tokens = proc_macro2::TokenStream::new();
        parsed.to_tokens(&mut tokens);

        let generated_code = tokens.to_string();

        // Check that the generated code handles no-uniform case properly
        assert!(generated_code.contains("struct Identity"));
        assert!(generated_code.contains("struct IdentityUniforms"));
        assert!(generated_code.contains("fn new () -> Self"));
        assert!(generated_code.contains("impl Default for Identity"));
    }
}
