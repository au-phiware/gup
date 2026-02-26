// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WGSL reserved keyword detection for compile-time validation.
//!
//! Validates that identifiers used in shader functions (parameter names,
//! function names) do not collide with WGSL reserved keywords. This catches
//! errors at compile time rather than producing confusing GPU shader
//! compilation failures at runtime.
//!
//! Keywords are sourced from the WGSL specification:
//! <https://www.w3.org/TR/WGSL/#keyword-summary>

use syn::{Error, Ident};

/// WGSL reserved keywords that cannot be used as identifiers.
///
/// From the WGSL specification §14.1 — these words are always reserved and
/// may never appear as user-defined identifiers.
const WGSL_RESERVED_KEYWORDS: &[&str] = &[
    // Declaration and control flow
    "alias",
    "break",
    "case",
    "const",
    "const_assert",
    "continue",
    "continuing",
    "default",
    "diagnostic",
    "discard",
    "else",
    "enable",
    "false",
    "fn",
    "for",
    "if",
    "let",
    "loop",
    "override",
    "requires",
    "return",
    "struct",
    "switch",
    "true",
    "var",
    "while",
];

/// WGSL contextual keywords that are reserved in certain contexts and should
/// be avoided as parameter names to prevent confusion.
const WGSL_CONTEXTUAL_KEYWORDS: &[&str] = &[
    // Built-in value names
    "read",
    "read_write",
    "write",
    // Address spaces
    "function",
    "private",
    "workgroup",
    "uniform",
    "storage",
];

/// WGSL built-in type keywords that should not be used as parameter names.
const WGSL_TYPE_KEYWORDS: &[&str] = &[
    // Scalar types
    "bool",
    "f16",
    "f32",
    "i32",
    "u32",
    // Vector types
    "vec2",
    "vec3",
    "vec4",
    // Matrix types
    "mat2x2",
    "mat2x3",
    "mat2x4",
    "mat3x2",
    "mat3x3",
    "mat3x4",
    "mat4x2",
    "mat4x3",
    "mat4x4",
    // Sampler and texture types
    "sampler",
    "sampler_comparison",
    "texture_1d",
    "texture_2d",
    "texture_2d_array",
    "texture_3d",
    "texture_cube",
    "texture_cube_array",
    "texture_multisampled_2d",
    "texture_storage_1d",
    "texture_storage_2d",
    "texture_storage_2d_array",
    "texture_storage_3d",
    "texture_depth_2d",
    "texture_depth_2d_array",
    "texture_depth_cube",
    "texture_depth_cube_array",
    "texture_depth_multisampled_2d",
    // Other built-in types
    "array",
    "atomic",
    "ptr",
];

/// WGSL reserved words for future use. These are not currently used in the
/// language but are reserved to prevent future breaking changes.
const WGSL_FUTURE_RESERVED: &[&str] = &[
    "NULL",
    "Self",
    "abstract",
    "active",
    "alignas",
    "alignof",
    "as",
    "asm",
    "asm_fragment",
    "async",
    "attribute",
    "auto",
    "await",
    "become",
    "binding_array",
    "cast",
    "catch",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "coherent",
    "column_major",
    "common",
    "compile",
    "compile_fragment",
    "concept",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "crate",
    "debugger",
    "decltype",
    "delete",
    "demote",
    "demote_to_helper",
    "do",
    "dynamic_cast",
    "enum",
    "explicit",
    "export",
    "extends",
    "extern",
    "external",
    "fallthrough",
    "filter",
    "final",
    "finally",
    "friend",
    "from",
    "fxgroup",
    "get",
    "goto",
    "groupshared",
    "highp",
    "impl",
    "implements",
    "import",
    "in",
    "inline",
    "instanceof",
    "interface",
    "layout",
    "lowp",
    "macro",
    "match",
    "mediump",
    "meta",
    "mod",
    "module",
    "move",
    "mut",
    "mutable",
    "namespace",
    "new",
    "nil",
    "noexcept",
    "noinline",
    "nointerpolation",
    "noperspective",
    "null",
    "nullptr",
    "of",
    "operator",
    "package",
    "packoffset",
    "partition",
    "pass",
    "patch",
    "pixelfragment",
    "precise",
    "precision",
    "premerge",
    "priv",
    "protected",
    "pub",
    "public",
    "readonly",
    "ref",
    "regardless",
    "register",
    "reinterpret_cast",
    "require",
    "resource",
    "restrict",
    "self",
    "set",
    "shared",
    "sizeof",
    "smooth",
    "snorm",
    "static",
    "static_assert",
    "static_cast",
    "std",
    "subroutine",
    "super",
    "target",
    "template",
    "this",
    "thread_local",
    "throw",
    "trait",
    "try",
    "type",
    "typedef",
    "typeid",
    "typename",
    "typeof",
    "union",
    "unless",
    "unorm",
    "unsafe",
    "unsized",
    "use",
    "using",
    "varying",
    "virtual",
    "volatile",
    "wgsl",
    "with",
    "writeonly",
    "yield",
];

/// Category of a WGSL reserved word, used for generating helpful error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordCategory {
    /// A core WGSL language keyword (e.g., `let`, `var`, `fn`).
    Reserved,
    /// A contextual keyword with special meaning (e.g., `uniform`, `storage`).
    Contextual,
    /// A built-in type name (e.g., `f32`, `vec3`, `sampler`).
    BuiltInType,
    /// Reserved for future use by the WGSL specification.
    FutureReserved,
}

impl KeywordCategory {
    /// A human-readable label for the category.
    pub fn label(self) -> &'static str {
        match self {
            KeywordCategory::Reserved => "a WGSL reserved keyword",
            KeywordCategory::Contextual => "a WGSL contextual keyword",
            KeywordCategory::BuiltInType => "a WGSL built-in type name",
            KeywordCategory::FutureReserved => "reserved for future use in WGSL",
        }
    }
}

/// Check whether `name` is a WGSL reserved word.
///
/// Returns the [`KeywordCategory`] if the name matches, or `None` if the
/// name is safe to use.
pub fn check_wgsl_keyword(name: &str) -> Option<KeywordCategory> {
    if WGSL_RESERVED_KEYWORDS.contains(&name) {
        Some(KeywordCategory::Reserved)
    } else if WGSL_CONTEXTUAL_KEYWORDS.contains(&name) {
        Some(KeywordCategory::Contextual)
    } else if WGSL_TYPE_KEYWORDS.contains(&name) {
        Some(KeywordCategory::BuiltInType)
    } else if WGSL_FUTURE_RESERVED.contains(&name) {
        Some(KeywordCategory::FutureReserved)
    } else {
        None
    }
}

/// Suggest an alternative name for a reserved keyword.
///
/// The suggestion simply appends an underscore suffix, which is a common
/// convention for avoiding keyword collisions.
fn suggest_alternative(name: &str) -> String {
    format!("{name}_val")
}

/// Validate that a parameter name is not a WGSL reserved keyword.
///
/// Returns `Ok(())` if the name is safe, or a descriptive `syn::Error`
/// pointing at the offending identifier span.
pub fn validate_param_name(ident: &Ident) -> Result<(), Error> {
    let name = ident.to_string();
    if let Some(category) = check_wgsl_keyword(&name) {
        let suggestion = suggest_alternative(&name);
        Err(Error::new_spanned(
            ident,
            format!(
                "`{name}` is {category}. Using it as a parameter name will cause \
                 GPU shader compilation errors. Try `{suggestion}` instead.",
                category = category.label(),
            ),
        ))
    } else {
        Ok(())
    }
}

/// Validate that a function name is not a WGSL reserved keyword.
///
/// Returns `Ok(())` if the name is safe, or a descriptive `syn::Error`
/// pointing at the offending identifier span.
pub fn validate_function_name(ident: &Ident) -> Result<(), Error> {
    let name = ident.to_string();
    if let Some(category) = check_wgsl_keyword(&name) {
        let suggestion = suggest_alternative(&name);
        Err(Error::new_spanned(
            ident,
            format!(
                "`{name}` is {category}. Using it as a function name will cause \
                 GPU shader compilation errors. Try `{suggestion}` instead.",
                category = category.label(),
            ),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_reserved_keywords() {
        assert_eq!(check_wgsl_keyword("let"), Some(KeywordCategory::Reserved),);
        assert_eq!(check_wgsl_keyword("var"), Some(KeywordCategory::Reserved),);
        assert_eq!(check_wgsl_keyword("fn"), Some(KeywordCategory::Reserved),);
        assert_eq!(
            check_wgsl_keyword("return"),
            Some(KeywordCategory::Reserved),
        );
        assert_eq!(check_wgsl_keyword("if"), Some(KeywordCategory::Reserved),);
        assert_eq!(check_wgsl_keyword("for"), Some(KeywordCategory::Reserved),);
        assert_eq!(check_wgsl_keyword("while"), Some(KeywordCategory::Reserved),);
        assert_eq!(
            check_wgsl_keyword("switch"),
            Some(KeywordCategory::Reserved),
        );
        assert_eq!(
            check_wgsl_keyword("struct"),
            Some(KeywordCategory::Reserved),
        );
        assert_eq!(check_wgsl_keyword("true"), Some(KeywordCategory::Reserved),);
        assert_eq!(check_wgsl_keyword("false"), Some(KeywordCategory::Reserved),);
    }

    #[test]
    fn detects_contextual_keywords() {
        assert_eq!(
            check_wgsl_keyword("uniform"),
            Some(KeywordCategory::Contextual),
        );
        assert_eq!(
            check_wgsl_keyword("storage"),
            Some(KeywordCategory::Contextual),
        );
        assert_eq!(
            check_wgsl_keyword("private"),
            Some(KeywordCategory::Contextual),
        );
        assert_eq!(
            check_wgsl_keyword("function"),
            Some(KeywordCategory::Contextual),
        );
        assert_eq!(
            check_wgsl_keyword("workgroup"),
            Some(KeywordCategory::Contextual),
        );
    }

    #[test]
    fn detects_builtin_type_keywords() {
        assert_eq!(
            check_wgsl_keyword("f32"),
            Some(KeywordCategory::BuiltInType),
        );
        assert_eq!(
            check_wgsl_keyword("vec3"),
            Some(KeywordCategory::BuiltInType),
        );
        assert_eq!(
            check_wgsl_keyword("sampler"),
            Some(KeywordCategory::BuiltInType),
        );
        assert_eq!(
            check_wgsl_keyword("array"),
            Some(KeywordCategory::BuiltInType),
        );
        assert_eq!(
            check_wgsl_keyword("mat4x4"),
            Some(KeywordCategory::BuiltInType),
        );
    }

    #[test]
    fn detects_future_reserved_keywords() {
        assert_eq!(
            check_wgsl_keyword("target"),
            Some(KeywordCategory::FutureReserved),
        );
        assert_eq!(
            check_wgsl_keyword("enum"),
            Some(KeywordCategory::FutureReserved),
        );
        assert_eq!(
            check_wgsl_keyword("impl"),
            Some(KeywordCategory::FutureReserved),
        );
        assert_eq!(
            check_wgsl_keyword("self"),
            Some(KeywordCategory::FutureReserved),
        );
        assert_eq!(
            check_wgsl_keyword("async"),
            Some(KeywordCategory::FutureReserved),
        );
        assert_eq!(
            check_wgsl_keyword("class"),
            Some(KeywordCategory::FutureReserved),
        );
    }

    #[test]
    fn allows_valid_identifiers() {
        assert_eq!(check_wgsl_keyword("value"), None);
        assert_eq!(check_wgsl_keyword("scale"), None);
        assert_eq!(check_wgsl_keyword("offset"), None);
        assert_eq!(check_wgsl_keyword("color"), None);
        assert_eq!(check_wgsl_keyword("position"), None);
        assert_eq!(check_wgsl_keyword("my_variable"), None);
        assert_eq!(check_wgsl_keyword("x"), None);
        assert_eq!(check_wgsl_keyword("data"), None);
    }

    #[test]
    fn suggest_alternative_appends_suffix() {
        assert_eq!(suggest_alternative("target"), "target_val");
        assert_eq!(suggest_alternative("uniform"), "uniform_val");
        assert_eq!(suggest_alternative("let"), "let_val");
    }

    #[test]
    fn validate_param_rejects_reserved() {
        let ident = Ident::new("target", proc_macro2::Span::call_site());
        let result = validate_param_name(&ident);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("target"));
        assert!(err.contains("reserved"));
        assert!(err.contains("target_val"));
    }

    #[test]
    fn validate_param_accepts_valid() {
        let ident = Ident::new("my_value", proc_macro2::Span::call_site());
        assert!(validate_param_name(&ident).is_ok());
    }

    #[test]
    fn validate_function_rejects_reserved() {
        let ident = Ident::new("discard", proc_macro2::Span::call_site());
        let result = validate_function_name(&ident);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("discard"));
        assert!(err.contains("reserved"));
    }

    #[test]
    fn validate_function_accepts_valid() {
        let ident = Ident::new("linear_scale", proc_macro2::Span::call_site());
        assert!(validate_function_name(&ident).is_ok());
    }

    #[test]
    fn category_labels_are_descriptive() {
        assert!(KeywordCategory::Reserved.label().contains("reserved"));
        assert!(KeywordCategory::Contextual.label().contains("contextual"));
        assert!(
            KeywordCategory::BuiltInType
                .label()
                .contains("built-in type")
        );
        assert!(KeywordCategory::FutureReserved.label().contains("future"));
    }
}
