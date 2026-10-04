// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Resolution of the path used to reference the `gup` crate in generated code.
//!
//! All macros in this crate emit absolute paths rooted at `::gup` by default.
//! This works everywhere the `gup` crate is a dependency (external crates,
//! integration tests, examples, doctests) and also inside `gup` itself, which
//! declares `extern crate self as gup;` at its crate root so that `::gup`
//! resolves to the current crate.
//!
//! Crates that re-export `gup` under a different name can override the path:
//!
//! - Attribute macros: `#[wgsl_function(crate = "my_reexport::gup")]`
//! - Derive macros: `#[gup(crate = "my_reexport::gup")]`

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Attribute, Expr, ExprLit, Lit, Path, Result, meta::ParseNestedMeta};

/// Name of the helper attribute recognised by derive macros.
pub const HELPER_ATTR: &str = "gup";

/// The default path used to reference the `gup` crate.
pub fn default_crate_path() -> Path {
    syn::parse_quote!(::gup)
}

/// Parse the value of a `crate = ...` nested meta item.
///
/// Accepts either a string literal (`crate = "::my::gup"`) or a bare path
/// (`crate = ::my::gup`).
fn parse_crate_value(meta: &ParseNestedMeta) -> Result<Path> {
    let value: Expr = meta.value()?.parse()?;
    match value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(lit), ..
        }) => lit.parse::<Path>().map_err(|e| {
            syn::Error::new(
                lit.span(),
                format!("`crate` must be a valid path (e.g. \"::gup\"): {e}"),
            )
        }),
        Expr::Path(expr_path) => Ok(expr_path.path),
        other => Err(syn::Error::new_spanned(
            other,
            "`crate` must be a path or string literal, e.g. `crate = \"::gup\"`",
        )),
    }
}

/// Parse attribute-macro arguments of the form `crate = "path"`.
///
/// An empty argument list yields the default `::gup` path. Any other key is
/// rejected with a descriptive error naming the macro.
pub fn parse_attribute_args(args: TokenStream, macro_name: &str) -> Result<Path> {
    let mut crate_path = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("crate") {
            if crate_path.is_some() {
                return Err(meta.error(format!(
                    "duplicate `crate` argument in #[{macro_name}(...)]"
                )));
            }
            crate_path = Some(parse_crate_value(&meta)?);
            Ok(())
        } else {
            Err(meta.error(format!(
                "unsupported #[{macro_name}(...)] argument; expected `crate = \"path::to::gup\"`"
            )))
        }
    });
    syn::parse::Parser::parse2(parser, args)?;
    Ok(crate_path.unwrap_or_else(default_crate_path))
}

/// Find a `#[gup(crate = "path")]` helper attribute on a derive input.
///
/// Returns the default `::gup` path when no helper attribute is present.
pub fn from_derive_attrs(attrs: &[Attribute]) -> Result<Path> {
    let mut crate_path = None;
    for attr in attrs {
        if !attr.path().is_ident(HELPER_ATTR) {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                if crate_path.is_some() {
                    return Err(meta.error("duplicate `crate` argument in #[gup(...)]"));
                }
                crate_path = Some(parse_crate_value(&meta)?);
                Ok(())
            } else {
                Err(meta.error(
                    "unsupported #[gup(...)] argument; expected `crate = \"path::to::gup\"`",
                ))
            }
        })?;
    }
    Ok(crate_path.unwrap_or_else(default_crate_path))
}

/// Tokens for deriving `bytemuck::Pod` and `bytemuck::Zeroable` via the
/// `gup` re-export, so downstream crates do not need a direct `bytemuck`
/// dependency.
///
/// Expands to the derive paths plus the `#[bytemuck(crate = "...")]` helper
/// attribute that tells the bytemuck derives where to find their runtime crate.
pub struct BytemuckDerives {
    /// `Pod, Zeroable` derive paths, suitable for splicing into `#[derive(...)]`.
    pub derives: TokenStream,
    /// The `#[bytemuck(crate = "...")]` attribute.
    pub attr: TokenStream,
}

/// Build the bytemuck derive paths and helper attribute for `crate_path`.
pub fn bytemuck_derives(crate_path: &Path) -> BytemuckDerives {
    let bytemuck_path = quote!(#crate_path::__private::bytemuck);
    // Path tokens stringify with spaces (`:: gup :: __private`), which is
    // still a valid path when re-parsed by the bytemuck derive.
    let bytemuck_str = bytemuck_path.to_token_stream().to_string();
    BytemuckDerives {
        derives: quote!(#bytemuck_path::Pod, #bytemuck_path::Zeroable),
        attr: quote!(#[bytemuck(crate = #bytemuck_str)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{DeriveInput, parse_quote};

    fn path_str(path: &Path) -> String {
        path.to_token_stream().to_string()
    }

    #[test]
    fn default_is_absolute_gup() {
        assert_eq!(path_str(&default_crate_path()), ":: gup");
    }

    #[test]
    fn attribute_args_empty_yields_default() {
        let path = parse_attribute_args(TokenStream::new(), "wgsl_function").unwrap();
        assert_eq!(path_str(&path), ":: gup");
    }

    #[test]
    fn attribute_args_string_literal() {
        let path =
            parse_attribute_args(quote!(crate = "my_reexport::gup"), "wgsl_function").unwrap();
        assert_eq!(path_str(&path), "my_reexport :: gup");
    }

    #[test]
    fn attribute_args_bare_path() {
        let path = parse_attribute_args(quote!(crate = ::other::gup), "wgsl_struct").unwrap();
        assert_eq!(path_str(&path), ":: other :: gup");
    }

    #[test]
    fn attribute_args_crate_keyword() {
        let path = parse_attribute_args(quote!(crate = "crate"), "wgsl_function").unwrap();
        assert_eq!(path_str(&path), "crate");
    }

    #[test]
    fn attribute_args_unknown_key_errors() {
        let err = parse_attribute_args(quote!(path = "x"), "wgsl_function").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("wgsl_function"), "{msg}");
        assert!(msg.contains("crate"), "{msg}");
    }

    #[test]
    fn attribute_args_duplicate_errors() {
        let err =
            parse_attribute_args(quote!(crate = "a", crate = "b"), "wgsl_function").unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn attribute_args_invalid_path_errors() {
        let err = parse_attribute_args(quote!(crate = "not a path"), "wgsl_function").unwrap_err();
        assert!(err.to_string().contains("valid path"));
    }

    #[test]
    fn derive_attrs_default_and_override() {
        let input: DeriveInput = parse_quote! {
            struct Plain;
        };
        assert_eq!(
            path_str(&from_derive_attrs(&input.attrs).unwrap()),
            ":: gup"
        );

        let input: DeriveInput = parse_quote! {
            #[gup(crate = "reexport::gup")]
            struct Custom;
        };
        assert_eq!(
            path_str(&from_derive_attrs(&input.attrs).unwrap()),
            "reexport :: gup"
        );
    }

    #[test]
    fn derive_attrs_unknown_key_errors() {
        let input: DeriveInput = parse_quote! {
            #[gup(other = "x")]
            struct Bad;
        };
        assert!(from_derive_attrs(&input.attrs).is_err());
    }

    #[test]
    fn bytemuck_derives_use_reexport() {
        let derives = bytemuck_derives(&default_crate_path());
        assert_eq!(
            derives.derives.to_string(),
            ":: gup :: __private :: bytemuck :: Pod , :: gup :: __private :: bytemuck :: Zeroable"
        );
        let attr = derives.attr.to_string();
        assert!(attr.contains("bytemuck (crate ="), "{attr}");
        assert!(attr.contains(":: gup :: __private :: bytemuck"), "{attr}");
    }
}
