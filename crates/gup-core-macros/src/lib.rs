// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # gup-core-macros
//!
//! `#[wgsl_function]` (RFC-001 §6, GUP-420), re-exported and documented
//! as `gup_core::wgsl_function`. It completes a user's
//! `impl ShaderFn for T` from authored WGSL: at expansion it composes the
//! WGSL with naga_oil against Gup's library (`gup_wgsl::compose::library`),
//! flattens it as gup-core's build script flattens the library, checks the
//! entry function against the glue's calling convention, and emits the
//! impl's static half (`In`, `Out`, `Params`, `MODULE`, `ENTRY`,
//! `input_format`, and `image` for a `monotonic` function), a `Params`
//! twin with an `encase::ShaderType` impl, and the module's `WgslModule`
//! static. Every failure is a compile error at the user's code.
//!
//! `#[derive(Mark)]` (RFC-001 §4, GUP-422) reuses the same machinery to
//! generate a mark's channels and `Mark` impl and to check its WGSL input
//! struct against them.

mod convention;
mod mark;
mod root;

use convention::{Convention, Input, Member, Output};
use gup_wgsl::compose::{Library, PAD_PREFIX, Source, library, validate_wgsl};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{ImplItem, ItemImpl, LitStr};

/// See `gup_core::wgsl_function`.
#[proc_macro_attribute]
pub fn wgsl_function(attr: TokenStream, item: TokenStream) -> TokenStream {
    let (attr, item): (TokenStream2, TokenStream2) = (attr.into(), item.into());
    match expand(attr.clone(), item.clone()) {
        Ok(tokens) => tokens.into(),
        Err(e) => {
            let mut out = e.to_compile_error();
            out.extend(stub(attr, item));
            out.into()
        }
    }
}

/// See `gup_core::Mark`.
#[proc_macro_derive(Mark, attributes(mark, channel))]
pub fn derive_mark(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match mark::expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// After an error, a placeholder impl, so that the type's other uses
/// (`impl CpuMirror`, `encode`, `then`) do not add errors of their own.
fn stub(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let Ok(imp) = syn::parse2::<ItemImpl>(item) else {
        return TokenStream2::new();
    };
    let Some((None, trait_path, _)) = &imp.trait_ else {
        return TokenStream2::new();
    };
    let root = root::crate_root();
    let (impl_generics, _, where_clause) = imp.generics.split_for_impl();
    let self_ty = &imp.self_ty;
    // The user's items, but for those the macro derives and `params`,
    // whose return type (the twin) was never generated.
    let replaced = ["In", "Params", "MODULE", "ENTRY", "input_format", "params"];
    // `Out` as the WGSL spells it, read lexically (it may not compose).
    let colour = parse_args(attr)
        .ok()
        .and_then(|args| {
            let text = load(args.wgsl.as_ref()?).ok()?.text;
            let entry = args.entry.map(|e| e.value());
            Some(returns_colour(&text, entry.as_deref()))
        })
        .unwrap_or(false);
    let mut out = if colour {
        quote!(
            type Out = #root::Color;
        )
    } else {
        quote!(
            type Out = f32;
        )
    };
    let mut kept = Vec::new();
    for item in &imp.items {
        let name = match item {
            ImplItem::Fn(f) => f.sig.ident.to_string(),
            ImplItem::Type(t) if t.ident == "Out" => {
                out = quote!(#item);
                continue;
            }
            ImplItem::Type(t) => t.ident.to_string(),
            ImplItem::Const(c) => c.ident.to_string(),
            _ => String::new(),
        };
        if !replaced.contains(&name.as_str()) {
            kept.push(item);
        }
    }
    quote! {
        impl #impl_generics #trait_path for #self_ty #where_clause {
            type In = f32;
            #out
            type Params = f32;
            const MODULE: &'static #root::WgslModule = &#root::__private::wgsl::VIEW;
            const ENTRY: &'static str = "";
            fn params(&self) -> f32 {
                ::core::unreachable!()
            }
            fn input_format(&self) -> #root::column::ColumnFormat {
                ::core::unreachable!()
            }
            #(#kept)*
        }
    }
}

/// Whether the function `entry` (or else the last function) in `wgsl`
/// returns `vec4<f32>`, by a lexical scan.
fn returns_colour(wgsl: &str, entry: Option<&str>) -> bool {
    let mut found = None;
    for chunk in wgsl.split("fn ").skip(1) {
        let name: String = chunk
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let Some(ret) = chunk.split('{').next().and_then(|s| s.split("->").nth(1)) else {
            continue;
        };
        if entry.is_none_or(|e| e == name) {
            found = Some(ret.trim().replace(' ', "") == "vec4<f32>");
        }
    }
    found.unwrap_or(false)
}

/// A plainer explanation of naga_oil's terser reports.
fn hint(report: &str) -> String {
    if let Some(rest) = report.split("naga writeback rules: `").nth(1) {
        let name = rest.split('`').next().unwrap_or_default();
        let base = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_');
        let digits = name[base.len()..].trim_start_matches('_');
        let letter = digits
            .parse::<u8>()
            .ok()
            .filter(|n| *n < 26)
            .map_or('b', |n| char::from(b'a' + n));
        format!(
            "\nnote: naga's WGSL writer renames identifiers that end in a digit or contain \
             `__`, so a module's items and struct members cannot: rename `{name}` (to \
             `{base}_{letter}`, say)"
        )
    } else if report.contains("Requires") && report.contains("arguments") {
        "\nnote: a call passes a different number of arguments than the function takes".into()
    } else {
        String::new()
    }
}

/// Where the WGSL comes from.
pub(crate) enum Wgsl {
    /// `wgsl = "…"`.
    Inline(LitStr),
    /// `file = "…"`, relative to the crate's manifest directory.
    File(LitStr),
}

#[derive(Default)]
struct Args {
    wgsl: Option<Wgsl>,
    path: Option<LitStr>,
    entry: Option<LitStr>,
    monotonic: Option<Span>,
}

const USAGE: &str = "#[wgsl_function(wgsl = r#\"…\"#)] (or `file = \"src/f.wgsl\"`), with \
                     optional `monotonic`, `entry = \"fn name\"` and `path = \"crate::module\"`";

fn parse_args(attr: TokenStream2) -> syn::Result<Args> {
    let mut args = Args::default();
    let parser = syn::meta::parser(|meta| {
        let set_once = |slot: bool, name: &str| {
            if slot {
                Err(meta.error(format!("`{name}` is given twice")))
            } else {
                Ok(())
            }
        };
        if meta.path.is_ident("monotonic") {
            set_once(args.monotonic.is_some(), "monotonic")?;
            args.monotonic = Some(meta.path.span());
        } else if meta.path.is_ident("path") {
            set_once(args.path.is_some(), "path")?;
            args.path = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("entry") {
            set_once(args.entry.is_some(), "entry")?;
            args.entry = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("wgsl") || meta.path.is_ident("file") {
            if args.wgsl.is_some() {
                return Err(meta.error("give the WGSL once, as `wgsl = \"…\"` or `file = \"…\"`"));
            }
            let lit: LitStr = meta.value()?.parse()?;
            args.wgsl = Some(if meta.path.is_ident("wgsl") {
                Wgsl::Inline(lit)
            } else {
                Wgsl::File(lit)
            });
        } else {
            return Err(meta.error(format!("unknown argument; expected {USAGE}")));
        }
        Ok(())
    });
    syn::parse::Parser::parse2(parser, attr)?;
    Ok(args)
}

/// `EnergyScale` → `energy_scale`, `HSLColor` → `hsl_color`.
pub(crate) fn snake_case(ident: &str) -> String {
    let chars: Vec<char> = ident.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower = i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_numeric());
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let prev_upper = i > 0 && chars[i - 1].is_uppercase();
            if prev_lower || (prev_upper && next_lower) {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// `gup::scale::linear` → `SCALE_LINEAR`, as gup-core's build script
/// names the library statics.
pub(crate) fn static_name(import_path: &str) -> String {
    import_path
        .strip_prefix("gup::")
        .unwrap_or(import_path)
        .replace("::", "_")
        .to_uppercase()
}

pub(crate) fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The WGSL text, the label its reports use, and the span errors point at.
pub(crate) struct Text {
    pub(crate) label: String,
    /// The WGSL, preceded by whitespace so that naga_oil's line and column
    /// numbers are the Rust file's (inline WGSL).
    pub(crate) text: String,
    pub(crate) span: Span,
    /// `include_str!` of a `file`, so cargo rebuilds when it changes.
    pub(crate) track: Option<String>,
}

pub(crate) fn load(wgsl: &Wgsl) -> syn::Result<Text> {
    match wgsl {
        Wgsl::Inline(lit) => {
            let span = lit.span().unwrap();
            // The literal's own text starts after `r#"` (or `"`).
            let repr = lit.token().to_string();
            let prefix = repr.find('"').map_or(0, |i| i + 1);
            let text = format!(
                "{}{}{}",
                "\n".repeat(span.line().saturating_sub(1)),
                " ".repeat(span.column().saturating_sub(1) + prefix),
                lit.value()
            );
            Ok(Text {
                label: span.file(),
                text,
                // The literal's first character: naga_oil's report shows
                // the WGSL line itself, so underlining the whole literal
                // would only repeat it.
                span: Span::from(span.start()),
                track: None,
            })
        }
        Wgsl::File(lit) => {
            let dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
                syn::Error::new(
                    lit.span(),
                    "`file` needs CARGO_MANIFEST_DIR (build with cargo)",
                )
            })?;
            let path = std::path::Path::new(&dir).join(lit.value());
            let text = std::fs::read_to_string(&path).map_err(|e| {
                syn::Error::new(
                    lit.span(),
                    format!(
                        "cannot read `{}` ({e}); `file` is relative to the crate's \
                         manifest directory",
                        path.display()
                    ),
                )
            })?;
            Ok(Text {
                label: lit.value(),
                text,
                span: lit.span(),
                track: Some(path.display().to_string()),
            })
        }
    }
}

thread_local! {
    /// Gup's library, composed once per compiler thread: composing it takes
    /// about a third of a second in a debug-built macro, and a crate may
    /// declare many functions. Each expansion adds its module, reads it
    /// and removes it again.
    static LIBRARY: std::cell::RefCell<Option<Library>> = const { std::cell::RefCell::new(None) };
}

/// Run `f` on the cached library.
pub(crate) fn with_library<R>(
    f: impl FnOnce(&mut Library) -> Result<R, gup_wgsl::compose::Error>,
) -> Result<R, gup_wgsl::compose::Error> {
    LIBRARY.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let lib = Library::new(&library()).map_err(|e| {
                gup_wgsl::compose::Error::new(
                    e.module.clone(),
                    format!("gup's own WGSL library failed to compose (a gup bug):\n{e}"),
                )
            })?;
            *slot = Some(lib);
        }
        f(slot.as_mut().expect("composed above"))
    })
}

fn expand(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let args = parse_args(attr)?;
    let mut imp: ItemImpl = syn::parse2(item).map_err(|e| {
        syn::Error::new(
            e.span(),
            "#[wgsl_function] goes on an `impl ShaderFn for YourType { … }` block",
        )
    })?;
    let trait_path = match &imp.trait_ {
        Some((None, path, _)) if path.segments.last().is_some_and(|s| s.ident == "ShaderFn") => {
            path.clone()
        }
        _ => {
            return Err(syn::Error::new(
                imp.impl_token.span,
                "#[wgsl_function] goes on an `impl ShaderFn for YourType { … }` block",
            ));
        }
    };
    let type_ident = match &*imp.self_ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.clone())
            .ok_or_else(|| syn::Error::new(p.span(), "expected a type name"))?,
        other => {
            return Err(syn::Error::new(
                other.span(),
                "#[wgsl_function] implements `ShaderFn` for a named type (a struct or enum)",
            ));
        }
    };
    let Some(wgsl) = &args.wgsl else {
        return Err(syn::Error::new(
            Span::call_site(),
            format!("#[wgsl_function] needs its WGSL: {USAGE}"),
        ));
    };
    let text = load(wgsl)?;
    let at_wgsl = |msg: String| syn::Error::new(text.span, msg);

    // The import path.
    let (path, path_span) = match &args.path {
        Some(lit) => (lit.value(), lit.span()),
        None => {
            let krate = std::env::var("CARGO_CRATE_NAME").unwrap_or_else(|_| "crate".into());
            (
                format!("{krate}::{}", snake_case(&type_ident.to_string())),
                text.span,
            )
        }
    };
    if !path.split("::").all(is_ident) {
        return Err(syn::Error::new(
            path_span,
            format!("`{path}` is not an import path like `my_crate::my_function`"),
        ));
    }
    if path == "gup" || path.starts_with("gup::") {
        return Err(syn::Error::new(
            path_span,
            format!(
                "`{path}` is in the `gup::` namespace, which Gup's own library uses; \
                 give this function its own `path = \"…\"`"
            ),
        ));
    }

    // Compose and flatten, as gup-core's build script does the library.
    let source = Source {
        file: text.label.clone(),
        text: text.text.clone(),
    };
    let (flat, closure) = with_library(|lib| {
        let flat = lib.add_module(&source, &path)?;
        let closure = lib.closure_wgsl(&path);
        lib.remove_module(&path);
        Ok((flat, closure))
    })
    .map_err(|e| {
        at_wgsl(format!(
            "the WGSL of `{type_ident}` (module `{path}`) does not compose:\n{}{}",
            e.report.trim_end(),
            hint(&e.report)
        ))
    })?;
    let (module, info) = validate_wgsl(&path, &closure).map_err(|e| {
        at_wgsl(format!(
            "the flattened WGSL of `{path}` does not validate (a gup bug):\n{}",
            e.report.trim_end()
        ))
    })?;

    // The entry function.
    let functions = convention::own_functions(&module, &path, &flat.items);
    let (entry, entry_span) = match &args.entry {
        Some(lit) => (lit.value(), lit.span()),
        None => match functions.as_slice() {
            [only] => (only.clone(), text.span),
            [] => return Err(at_wgsl("the WGSL declares no function".into())),
            many => {
                return Err(at_wgsl(format!(
                    "the WGSL declares {} functions ({}); name the one the glue calls with \
                     `entry = \"…\"`",
                    many.len(),
                    many.join(", ")
                )));
            }
        },
    };
    if !functions.contains(&entry) {
        return Err(syn::Error::new(
            entry_span,
            format!(
                "the WGSL declares no function `{entry}` (it declares: {})",
                functions.join(", ")
            ),
        ));
    }
    let conv = convention::check(&module, &info, &path, &entry).map_err(|e| {
        syn::Error::new(
            entry_span,
            format!("#[wgsl_function] on `{type_ident}`: {e}"),
        )
    })?;

    check_items(&imp, &conv, &args, &type_ident)?;

    let user_out = imp.items.iter().find_map(|i| match i {
        ImplItem::Type(t) if t.ident == "Out" => Some(t.ty.clone()),
        _ => None,
    });
    let root = crate::root::crate_root();
    let private = quote!(#root::__private);
    let in_ty = match conv.input {
        Input::U32 => quote!(u32),
        _ => quote!(f32),
    };
    let format = match conv.input {
        Input::F32 => quote!(F32),
        Input::F32Relative => quote!(F32Relative),
        Input::F32x2Relative => quote!(F32x2Relative),
        Input::U32 => quote!(U32),
    };
    let out_default = match conv.output {
        Output::F32 => quote!(f32),
        Output::Color => quote!(#root::Color),
    };
    let params_ident = format_ident!("{}Params", type_ident);
    let vis = quote!(pub);

    // The `Params` twin: the user's members, without the padding the
    // library appended; the last member's `size` covers it instead.
    let real: Vec<&(String, Member, u32)> = conv
        .params
        .iter()
        .filter(|(n, ..)| !n.starts_with(PAD_PREFIX))
        .collect();
    let padded = real.len() < conv.params.len();
    let field_idents: Vec<syn::Ident> = real
        .iter()
        .map(|(n, ..)| syn::Ident::new(n, Span::call_site()))
        .collect();
    let field_types: Vec<TokenStream2> = real
        .iter()
        .map(|(_, m, _)| match m {
            Member::F32 => quote!(f32),
            Member::U32 => quote!(u32),
            Member::I32 => quote!(i32),
            Member::Color => quote!(#root::Color),
        })
        .collect();
    let field_docs: Vec<String> = real
        .iter()
        .map(|(n, m, _)| {
            let wgsl = match m {
                Member::F32 => "f32",
                Member::U32 => "u32",
                Member::I32 => "i32",
                Member::Color => "vec4<f32>",
            };
            format!("WGSL `{n}: {wgsl}`.")
        })
        .collect();
    let field_sizes: Vec<TokenStream2> = real
        .iter()
        .enumerate()
        .map(|(i, (_, _, offset))| {
            if padded && i + 1 == real.len() {
                let size = conv.params_span - offset;
                quote!(#[shader(size(#size))])
            } else {
                quote!()
            }
        })
        .collect();
    let twin: syn::DeriveInput = syn::parse_quote! {
        #vis struct #params_ident { #( #field_sizes #vis #field_idents: #field_types, )* }
    };
    let encase_impl =
        encase_derive_impl::derive_shader_type(twin, &syn::parse_quote!(#private::encase));
    let twin_doc = format!(
        "The uniform `Params` of `{type_ident}`'s WGSL (module `{path}`), generated by \
         `#[wgsl_function]`: one field per WGSL member, in order. `{type_ident}`'s \
         `ShaderFn::params` returns it."
    );
    let span_bytes = u64::from(conv.params_span);
    let size_msg = format!(
        "#[wgsl_function]: `{params_ident}` does not lay out as WGSL `Params` ({span_bytes} bytes)"
    );

    // The module static.
    let imports: Vec<TokenStream2> = flat
        .imports
        .iter()
        .map(|p| {
            let ident = format_ident!("{}", static_name(p));
            quote!(&#private::wgsl::#ident)
        })
        .collect();
    let flat_wgsl = &flat.wgsl;
    let track = text.track.as_ref().map(|p| {
        quote!(
            const _: &str = ::core::include_str!(#p);
        )
    });

    // The `Out` check: a user's `type Out` must be the WGSL result type.
    let out_check = user_out
        .as_ref()
        .filter(|_| imp.generics.params.is_empty())
        .map(|out| {
            let wgsl = conv.output.wgsl();
            let msg = format!(
                "#[wgsl_function]: `type Out` must be a type whose WGSL is `{wgsl}`, the return \
             type of `{entry}` (`f32` or `Px` for `f32`, `Color` for `vec4<f32>`)"
            );
            quote_spanned! {out.span()=>
                const _: () = ::core::assert!(
                    #private::str_eq(<#out as #root::GpuType>::WGSL, #wgsl),
                    #msg
                );
            }
        });

    let image = args.monotonic.map(|span| {
        let mirror = quote_spanned!(span=> <Self as #root::CpuMirror>::eval);
        quote! {
            fn image(&self, extent: (f64, f64)) -> ::core::option::Option<(f64, f64)> {
                #private::monotonic_image(|x| #mirror(self, x), extent)
            }
        }
    });
    // A colour function with a CPU mirror gets a sampled ramp legend; one
    // without (or a user-written `legend`) is unaffected.
    let sampled = (conv.output == Output::Color).then(|| {
        quote! {
            fn sampled_legend(
                &self,
                extent: (f64, f64),
            ) -> ::core::option::Option<#root::Legend> {
                #[allow(unused_imports)]
                use #private::{MirrorFallback as _, MirrorProbe, MirrorSampled as _};
                (&MirrorProbe(self)).sampled(extent)
            }
        }
    });
    // Likewise its CPU colour, for vector output.
    let cpu_color = (conv.output == Output::Color).then(|| {
        quote! {
            fn cpu_color(&self, x: f64) -> ::core::option::Option<#root::Color> {
                #[allow(unused_imports)]
                use #private::{MirrorColorFallback as _, MirrorColored as _, MirrorProbe};
                (&MirrorProbe(self)).color(x)
            }
        }
    });
    let out_item = if user_out.is_some() {
        quote!()
    } else {
        quote!(type Out = #out_default;)
    };

    let generics = imp.generics.clone();
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let self_ty = &imp.self_ty;
    let attrs = std::mem::take(&mut imp.attrs);
    let user_items = &imp.items;
    let module_doc = format!("`{path}`: `{type_ident}`'s WGSL, flattened by `#[wgsl_function]`.");

    Ok(quote! {
        #[doc = #twin_doc]
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        #vis struct #params_ident {
            #( #[doc = #field_docs] #vis #field_idents: #field_types, )*
        }

        #encase_impl

        const _: () = {
            #track

            #[doc = #module_doc]
            static __GUP_WGSL_MODULE: #root::WgslModule = #root::WgslModule {
                import_path: #path,
                wgsl: #flat_wgsl,
                imports: &[#(#imports),*],
            };

            const _: () = ::core::assert!(
                <#params_ident as #private::encase::ShaderSize>::SHADER_SIZE.get() == #span_bytes,
                #size_msg
            );

            #out_check

            #(#attrs)*
            impl #impl_generics #trait_path for #self_ty #where_clause {
                type In = #in_ty;
                #out_item
                type Params = #params_ident;
                const MODULE: &'static #root::WgslModule = &__GUP_WGSL_MODULE;
                const ENTRY: &'static str = #entry;

                fn input_format(&self) -> #root::column::ColumnFormat {
                    #root::column::ColumnFormat::#format
                }

                #image

                #sampled
                #cpu_color

                #(#user_items)*
            }
        };
    })
}

/// The items a user writes in the impl, against what the WGSL says.
fn check_items(
    imp: &ItemImpl,
    conv: &Convention,
    args: &Args,
    type_ident: &syn::Ident,
) -> syn::Result<()> {
    let mut errors: Vec<syn::Error> = Vec::new();
    let mut fns = Vec::new();
    for item in &imp.items {
        let (name, span) = match item {
            ImplItem::Fn(f) => {
                fns.push(f.sig.ident.to_string());
                (f.sig.ident.to_string(), f.sig.ident.span())
            }
            ImplItem::Type(t) => (t.ident.to_string(), t.ident.span()),
            ImplItem::Const(c) => (c.ident.to_string(), c.ident.span()),
            _ => continue,
        };
        let derived = match name.as_str() {
            "In" => Some("the type of the WGSL function's first argument"),
            "Params" => Some("the WGSL `Params` struct, as the generated twin"),
            "MODULE" => Some("the composed WGSL"),
            "ENTRY" => Some("the WGSL function (`entry = \"…\"` picks another)"),
            "input_format" => Some("the WGSL function's first arguments"),
            _ => None,
        };
        if let Some(from) = derived {
            errors.push(syn::Error::new(
                span,
                format!("#[wgsl_function] derives `{name}` from {from}; remove it"),
            ));
        }
    }
    let has = |n: &str| fns.iter().any(|f| f == n);
    let first_arg = |input: Input| match input {
        Input::U32 => "a `u32` dictionary code",
        Input::F32 => "an `f32`",
        Input::F32Relative => "an `f32` relative to its chunk",
        Input::F32x2Relative => "a hi/lo `vec2<f32>` relative to its chunk",
    };
    let span_of = |n: &str| {
        imp.items
            .iter()
            .find_map(|i| match i {
                ImplItem::Fn(f) if f.sig.ident == n => Some(f.sig.ident.span()),
                _ => None,
            })
            .unwrap_or_else(Span::call_site)
    };
    if conv.input == Input::U32 && has("fit_domain") {
        errors.push(syn::Error::new(
            span_of("fit_domain"),
            "this function reads dictionary codes (`code: u32`), which are fitted by \
             `fit_keys(&mut self, keys: &[Arc<str>])`, never `fit_domain`",
        ));
    }
    if conv.input != Input::U32 && has("fit_keys") {
        errors.push(syn::Error::new(
            span_of("fit_keys"),
            format!(
                "this function reads {}; only functions over dictionary codes \
                 (`code: u32`) are fitted by `fit_keys`",
                first_arg(conv.input)
            ),
        ));
    }
    if !conv.input.is_relative() && has("chunk_base") {
        errors.push(syn::Error::new(
            span_of("chunk_base"),
            "this function takes no `base` argument, so it has no chunk base: a relative \
             input is `fn f(v: f32, base: f32, p: Params)`",
        ));
    }
    if conv.lut && !has("resources") {
        errors.push(syn::Error::new(
            imp.self_ty.span(),
            format!(
                "the WGSL function takes a palette LUT (`texture_2d<f32>, sampler`), so \
                 `{type_ident}` must supply it: add \
                 `fn resources(&self) -> Vec<gup_core::Resource> {{ vec![gup_core::Resource::Lut(colours)] }}`"
            ),
        ));
    }
    if !conv.lut && has("resources") {
        errors.push(syn::Error::new(
            span_of("resources"),
            "the WGSL function takes no palette LUT, so it has no resources: a LUT is \
             `lut: texture_2d<f32>, smp: sampler` after `p: Params`",
        ));
    }
    if let Some(span) = args.monotonic {
        if conv.output == Output::Color {
            errors.push(syn::Error::new(
                span,
                "`monotonic` gives a numeric function its `image`; a colour has none",
            ));
        }
        if has("image") {
            errors.push(syn::Error::new(
                span_of("image"),
                "`monotonic` generates `image` from the CPU mirror; remove one of them",
            ));
        }
    }
    let mut errors = errors.into_iter();
    match errors.next() {
        None => Ok(()),
        Some(mut first) => {
            first.extend(errors);
            Err(first)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_names() {
        assert_eq!(snake_case("Energy"), "energy");
        assert_eq!(snake_case("EnergyScale"), "energy_scale");
        assert_eq!(snake_case("HSLColor"), "hsl_color");
        assert_eq!(snake_case("Log2Scale"), "log2_scale");
    }

    #[test]
    fn the_result_type_is_read_lexically() {
        let wgsl = "struct Params { k: f32 }\n\
                    fn ease(t: f32) -> f32 { return t; }\n\
                    fn paint(v: f32, p: Params) -> vec4 < f32 > { return vec4<f32>(v); }\n";
        assert!(returns_colour(wgsl, None));
        assert!(returns_colour(wgsl, Some("paint")));
        assert!(!returns_colour(wgsl, Some("ease")));
        assert!(!returns_colour("fn f(v: f32, p: Params) -> f32 {", None));
    }

    #[test]
    fn static_names_match_the_build_script() {
        assert_eq!(static_name("gup::scale::linear"), "SCALE_LINEAR");
        assert_eq!(static_name("gup::view"), "VIEW");
    }
}
