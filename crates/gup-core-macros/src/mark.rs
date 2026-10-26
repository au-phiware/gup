// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `#[derive(Mark)]` (RFC-001 §4, GUP-422): the channel constants, the
//! `Mark` impl and a typed attribute-setter trait of a mark, plus the
//! compile-time check that the mark's WGSL declares an input struct matching
//! the channels field for field.

use crate::convention::{own_functions, type_name};
use crate::{Wgsl, hint, is_ident, load, snake_case, static_name, with_library};
use gup_wgsl::compose::{FlatModule, Source, validate_wgsl};
use gup_wgsl::flat_name;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Expr, Fields, Ident, LitStr};

const USAGE: &str = "#[mark(module = \"gup::marks::circle\", geometry = Quad)] (`geometry` is \
                     `Quad` or `Polyline`; or \
                     `wgsl = \"…\"` / `file = \"…\"` for your own module, with an optional \
                     `path = \"crate::module\"`), and optionally `antialias = Msaa`, `hit = Disc` (CPU \
                     picking: `Disc`, `AlignedBox`, `Segment` or `Polyline`) and `gpu_pick` \
                     (GPU picking: the module defines `hit_distance` and `hit_row`)";

/// `Selection` inherent methods that a generated setter would shadow.
const SELECTION_METHODS: &[&str] = &[
    "new",
    "max_chunk_rows",
    "len",
    "is_empty",
    "retain",
    "rows_released",
    "attr",
    "z",
    "legend_title",
    "on",
    "tooltip",
];

#[derive(Default)]
struct Args {
    module: Option<LitStr>,
    wgsl: Option<Wgsl>,
    path: Option<LitStr>,
    geometry: Option<Ident>,
    antialias: Option<Ident>,
    hit: Option<Ident>,
    gpu_pick: Option<Span>,
}

fn parse_args(input: &DeriveInput) -> syn::Result<Args> {
    let mut args = Args::default();
    let mut seen = false;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("mark")) {
        seen = true;
        attr.parse_nested_meta(|meta| {
            let once = |present: bool, name: &str| {
                if present {
                    Err(meta.error(format!("`{name}` is given twice")))
                } else {
                    Ok(())
                }
            };
            if meta.path.is_ident("module") {
                once(args.module.is_some(), "module")?;
                args.module = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("path") {
                once(args.path.is_some(), "path")?;
                args.path = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("geometry") {
                once(args.geometry.is_some(), "geometry")?;
                args.geometry = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("antialias") {
                once(args.antialias.is_some(), "antialias")?;
                args.antialias = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("hit") {
                once(args.hit.is_some(), "hit")?;
                args.hit = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("gpu_pick") {
                once(args.gpu_pick.is_some(), "gpu_pick")?;
                args.gpu_pick = Some(meta.path.span());
            } else if meta.path.is_ident("wgsl") || meta.path.is_ident("file") {
                if args.wgsl.is_some() {
                    return Err(
                        meta.error("give the WGSL once, as `wgsl = \"…\"` or `file = \"…\"`")
                    );
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
        })?;
    }
    if !seen {
        return Err(syn::Error::new(
            input.ident.span(),
            format!("#[derive(Mark)] needs its module: {USAGE}"),
        ));
    }
    Ok(args)
}

struct Channel {
    ident: Ident,
    ty: syn::Type,
    docs: Vec<syn::Attribute>,
    role: Option<Ident>,
    default: Option<Expr>,
}

fn parse_fields(input: &DeriveInput) -> syn::Result<Vec<Channel>> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "#[derive(Mark)] goes on a struct with named fields, one per channel",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new(
            input.ident.span(),
            "#[derive(Mark)] goes on a struct with named fields, one per channel",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "#[derive(Mark)] does not support generics",
        ));
    }
    let mut out = Vec::new();
    for field in &named.named {
        let ident = field.ident.clone().expect("named");
        if SELECTION_METHODS.contains(&ident.to_string().as_str()) {
            return Err(syn::Error::new(
                ident.span(),
                format!(
                    "a channel cannot be named `{ident}`: the generated setter would clash with \
                     `Selection::{ident}`"
                ),
            ));
        }
        let mut role = None;
        let mut default = None;
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("channel")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("role") {
                    let r: Ident = meta.value()?.parse()?;
                    if !["X", "Y", "Size", "Width", "Height", "Color"]
                        .contains(&r.to_string().as_str())
                    {
                        return Err(syn::Error::new(
                            r.span(),
                            "`role` is one of `X`, `Y`, `Size` or `Color`",
                        ));
                    }
                    role = Some(r);
                } else if meta.path.is_ident("default") {
                    default = Some(meta.value()?.parse()?);
                } else {
                    return Err(meta.error("unknown argument; expected `role` or `default`"));
                }
                Ok(())
            })?;
        }
        out.push(Channel {
            ident,
            ty: field.ty.clone(),
            docs: field
                .attrs
                .iter()
                .filter(|a| a.path().is_ident("doc"))
                .cloned()
                .collect(),
            role,
            default,
        });
    }
    if out.is_empty() {
        return Err(syn::Error::new(
            input.ident.span(),
            "a mark needs at least one channel",
        ));
    }
    Ok(out)
}

pub fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let args = parse_args(input)?;
    let channels = parse_fields(input)?;
    let name = &input.ident;
    let name_str = name.to_string();
    let vis = &input.vis;
    let root = crate::root::crate_root();
    let private = quote!(#root::__private);

    let Some(geometry) = &args.geometry else {
        return Err(syn::Error::new(
            name.span(),
            format!("#[mark] needs `geometry = Quad`: {USAGE}"),
        ));
    };
    let polyline = match geometry.to_string().as_str() {
        "Quad" => false,
        "Polyline" => true,
        _ => {
            return Err(syn::Error::new(
                geometry.span(),
                "`geometry` is `Quad` (one quad per row) or `Polyline` (one per pair of \
                 consecutive rows)",
            ));
        }
    };
    let antialias = match &args.antialias {
        Some(a) if a == "Msaa" => Some(quote!(#root::Antialias::Msaa)),
        Some(a) if a == "Analytic" => Some(quote!(#root::Antialias::Analytic)),
        Some(a) => {
            return Err(syn::Error::new(
                a.span(),
                "`antialias` is `Msaa` (the default is `Analytic`)",
            ));
        }
        None => None,
    };

    let hit = match &args.hit {
        None => None,
        Some(h) => {
            let (reads, wants_polyline): (&[&str], bool) = match h.to_string().as_str() {
                "Unpickable" => (&[], polyline),
                "Disc" => (&["x", "y", "radius"], false),
                "AlignedBox" => (&["x", "y", "width", "height", "x_end", "y_end"], false),
                "Segment" => (&["x", "y", "x_end", "y_end", "width"], false),
                "Polyline" => (&["x", "y", "width"], true),
                "Area" => (&["x", "y", "y_end"], true),
                _ => {
                    return Err(syn::Error::new(
                        h.span(),
                        "`hit` is `Unpickable`, `Disc`, `AlignedBox`, `Segment`, `Polyline` or `Area`",
                    ));
                }
            };
            for want in reads {
                let Some(ch) = channels.iter().find(|c| c.ident == want) else {
                    let list = reads
                        .iter()
                        .map(|r| format!("`{r}`"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(syn::Error::new(
                        h.span(),
                        format!("`hit = {h}` reads channels {list}; this mark has no `{want}`"),
                    ));
                };
                let is_px = matches!(&ch.ty, syn::Type::Path(p)
                    if p.path.segments.last().is_some_and(|s| s.ident == "Px"));
                if !is_px {
                    return Err(syn::Error::new(
                        ch.ident.span(),
                        format!("`hit = {h}` reads channel `{want}`, which must be a `Px`"),
                    ));
                }
            }
            if wants_polyline != polyline {
                return Err(syn::Error::new(
                    h.span(),
                    if polyline {
                        format!("`hit = {h}` needs `geometry = Quad`; this mark is a `Polyline`")
                    } else {
                        format!("`hit = {h}` needs `geometry = Polyline`; this mark is a `Quad`")
                    },
                ));
            }
            Some(quote!(#root::HitShape::#h))
        }
    };

    // The module: a library module, or the user's own.
    enum Source_ {
        Library(String, Span),
        User(crate::Text, String),
    }
    let module = match (&args.module, &args.wgsl) {
        (Some(_), Some(_)) => {
            return Err(syn::Error::new(
                name.span(),
                "give either `module = \"gup::…\"` or `wgsl`/`file`, not both",
            ));
        }
        (None, None) => {
            return Err(syn::Error::new(
                name.span(),
                format!("#[mark] needs a module: {USAGE}"),
            ));
        }
        (Some(m), None) => {
            if let Some(path) = &args.path {
                return Err(syn::Error::new(
                    path.span(),
                    "`path` goes with `wgsl`/`file`, not `module`",
                ));
            }
            Source_::Library(m.value(), m.span())
        }
        (None, Some(wgsl)) => {
            let text = load(wgsl)?;
            let (path, path_span) = match &args.path {
                Some(lit) => (lit.value(), lit.span()),
                None => {
                    let krate =
                        std::env::var("CARGO_CRATE_NAME").unwrap_or_else(|_| "crate".into());
                    (format!("{krate}::{}", snake_case(&name_str)), text.span)
                }
            };
            if !path.split("::").all(is_ident) {
                return Err(syn::Error::new(
                    path_span,
                    format!("`{path}` is not an import path like `my_crate::my_mark`"),
                ));
            }
            if path == "gup" || path.starts_with("gup::") {
                return Err(syn::Error::new(
                    path_span,
                    format!(
                        "`{path}` is in the `gup::` namespace, which Gup's own library uses; \
                         give this mark its own `path = \"…\"`"
                    ),
                ));
            }
            Source_::User(text, path)
        }
    };

    // Compose and flatten.
    let (path, flat, closure, at, track): (String, FlatModule, String, Span, _) = match &module {
        Source_::Library(path, span) => {
            let found = with_library(|lib| {
                Ok(match lib.module(path) {
                    Some(m) => Ok((m.clone(), lib.closure_wgsl(path))),
                    None => Err(lib
                        .modules()
                        .iter()
                        .filter(|m| m.import_path.starts_with("gup::marks::"))
                        .map(|m| m.import_path.clone())
                        .collect::<Vec<_>>()),
                })
            })
            .map_err(|e| syn::Error::new(*span, e.report))?;
            match found {
                Ok((flat, closure)) => (path.clone(), flat, closure, *span, None),
                Err(marks) => {
                    return Err(syn::Error::new(*span, unknown_module_message(path, &marks)));
                }
            }
        }
        Source_::User(text, path) => {
            let source = Source {
                file: text.label.clone(),
                text: text.text.clone(),
            };
            let (flat, closure) = with_library(|lib| {
                let flat = lib.add_module(&source, path)?;
                let closure = lib.closure_wgsl(path);
                lib.remove_module(path);
                Ok((flat, closure))
            })
            .map_err(|e| {
                syn::Error::new(
                    text.span,
                    format!(
                        "the WGSL of `{name}` (module `{path}`) does not compose:\n{}{}",
                        e.report.trim_end(),
                        hint(&e.report)
                    ),
                )
            })?;
            (path.clone(), flat, closure, text.span, text.track.clone())
        }
    };
    let (wgsl_module, info_unused) = validate_wgsl(&path, &closure).map_err(|e| {
        syn::Error::new(
            at,
            format!(
                "the flattened WGSL of `{path}` does not validate:\n{}",
                e.report.trim_end()
            ),
        )
    })?;
    let _ = info_unused;

    // The contract: `<NAME>In`, `vertex` and `shade`.
    let in_name = format!("{name_str}In");
    let flat_in = flat_name(&path, &in_name);
    let prefix = flat_name(&path, "");
    let Some(in_struct) = wgsl_module
        .types
        .iter()
        .find_map(|(handle, ty)| match &ty.inner {
            naga::TypeInner::Struct { members, .. }
                if ty.name.as_deref() == Some(flat_in.as_str()) =>
            {
                Some((handle, members.clone()))
            }
            _ => None,
        })
    else {
        return Err(syn::Error::new(
            name.span(),
            format!("the WGSL of `{path}` declares no struct `{in_name}`, the mark's input"),
        ));
    };
    let functions = own_functions(&wgsl_module, &path, &flat.items);
    for needed in ["vertex", "shade"] {
        if !functions.iter().any(|f| f == needed) {
            return Err(syn::Error::new(
                name.span(),
                format!(
                    "the WGSL of `{path}` declares no function `{needed}` (the mark contract \
                     needs `vertex` and `shade`)"
                ),
            ));
        }
    }
    // `gpu_pick`: the pick pass's fragment stage calls these (RFC-001 S9d).
    if args.gpu_pick.is_some() {
        for (needed, ret) in [("hit_distance", "f32"), ("hit_row", "u32")] {
            let flat = flat_name(&path, needed);
            let found = wgsl_module
                .functions
                .iter()
                .find(|(_, f)| f.name.as_deref() == Some(flat.as_str()));
            let returns = found.and_then(|(_, f)| f.result.as_ref()).map(|r| {
                match &wgsl_module.types[r.ty].inner {
                    naga::TypeInner::Scalar(naga::Scalar {
                        kind: naga::ScalarKind::Float,
                        width: 4,
                    }) => "f32",
                    naga::TypeInner::Scalar(naga::Scalar {
                        kind: naga::ScalarKind::Uint,
                        width: 4,
                    }) => "u32",
                    _ => "another type",
                }
            });
            let ok = found.is_some_and(|(_, f)| f.arguments.len() == 1) && returns == Some(ret);
            if !ok {
                return Err(syn::Error::new(
                    name.span(),
                    format!(
                        "`gpu_pick` needs `fn {needed}(v: Varyings) -> {ret}` in the WGSL of \
                         `{path}`{}",
                        match (found, returns) {
                            (None, _) => String::from("; it declares no such function"),
                            (Some(_), Some(r)) if r != ret => format!("; it returns {r}"),
                            _ => String::from("; it has another signature"),
                        }
                    ),
                ));
            }
        }
    }
    let (_, members) = in_struct;
    let member_names: Vec<String> = members
        .iter()
        .map(|m| m.name.clone().unwrap_or_default())
        .collect();
    // The members: the channels, then (a polyline) each position
    // channel again, read at the next row, as `<channel>_next`.
    let mut expected: Vec<(String, &Channel)> =
        channels.iter().map(|c| (c.ident.to_string(), c)).collect();
    if polyline {
        expected.extend(
            channels
                .iter()
                .filter(|c| c.role.as_ref().is_some_and(|r| r == "X" || r == "Y"))
                .map(|c| (format!("{}_next", c.ident), c)),
        );
        if expected.len() == channels.len() {
            return Err(syn::Error::new(
                name.span(),
                "a `Polyline` mark needs a position channel (`role = X` or `role = Y`) to join \
                 consecutive rows by",
            ));
        }
    }
    for (i, (field, ch)) in expected.iter().enumerate() {
        let field = field.clone();
        match member_names.get(i) {
            Some(m) if *m == field => {}
            _ => {
                let msg = match member_names.iter().position(|m| *m == field) {
                    Some(j) => format!(
                        "`{name_str}`'s channel `{field}` is number {} but `{in_name}.{field}` \
                         is number {} in the WGSL: the struct's members must be the channels, \
                         in order{} ({})",
                        i + 1,
                        j + 1,
                        if polyline {
                            ", then `<channel>_next` for each position channel"
                        } else {
                            ""
                        },
                        member_names.join(", ")
                    ),
                    None => format!(
                        "`{name_str}`'s {} `{field}` is missing from `{in_name}` in the \
                         WGSL of `{path}`, which has: {}",
                        if i < channels.len() {
                            "channel"
                        } else {
                            "next-row position"
                        },
                        member_names.join(", ")
                    ),
                };
                return Err(syn::Error::new(ch.ident.span(), msg));
            }
        }
    }
    if member_names.len() > expected.len() {
        return Err(syn::Error::new(
            name.span(),
            format!(
                "`{in_name}` in the WGSL of `{path}` has members that `{name_str}` has no \
                 channel for: {}",
                member_names[expected.len()..].join(", ")
            ),
        ));
    }

    // Generated items.
    let consts = channels.iter().enumerate().map(|(i, ch)| {
        let const_ident = format_ident!("{}", ch.ident.to_string().to_uppercase());
        let (ty, docs, field) = (&ch.ty, &ch.docs, ch.ident.to_string());
        let slot = u8::try_from(i).expect("fewer than 256 channels");
        quote! {
            #(#docs)*
            pub const #const_ident: #root::Channel<#name, #ty> =
                #root::Channel::new(#slot, #field);
        }
    });
    let descs = channels.iter().map(|ch| {
        let (ty, field) = (&ch.ty, ch.ident.to_string());
        let role = match &ch.role {
            Some(r) => quote!(::core::option::Option::Some(#root::Role::#r)),
            None => quote!(::core::option::Option::None),
        };
        let default = match &ch.default {
            Some(expr) => quote_spanned! {expr.span()=>
                { let v: #ty = #expr; v.const_value() }
            },
            None => quote!(<#ty as #root::Visual>::ZERO),
        };
        quote! {
            #root::ChannelDesc {
                name: #field,
                role: #role,
                wgsl_type: <#ty as #root::GpuType>::WGSL,
                default: #default,
            }
        }
    });
    let module_expr;
    let module_static;
    match &module {
        Source_::Library(path, _) => {
            let ident = format_ident!("{}", static_name(path));
            module_expr = quote!(&#private::wgsl::#ident);
            module_static = quote!();
        }
        Source_::User(..) => {
            let imports = flat.imports.iter().map(|p| {
                let ident = format_ident!("{}", static_name(p));
                quote!(&#private::wgsl::#ident)
            });
            let flat_wgsl = &flat.wgsl;
            let module_doc = format!("`{path}`: `{name}`'s WGSL, flattened by `#[derive(Mark)]`.");
            let track = track.as_ref().map(|p| {
                quote!(
                    const _: &str = ::core::include_str!(#p);
                )
            });
            module_static = quote! {
                #track

                #[doc = #module_doc]
                static __GUP_WGSL_MODULE: #root::WgslModule = #root::WgslModule {
                    import_path: #path,
                    wgsl: #flat_wgsl,
                    imports: &[#(#imports),*],
                };
            };
            module_expr = quote!(&__GUP_WGSL_MODULE);
        }
    }

    // One assertion per field: the Rust type's WGSL is the member's.
    let asserts = expected.iter().zip(&members).map(|((field, ch), member)| {
        let wgsl = type_name(&wgsl_module, member.ty, &prefix);
        let ty = &ch.ty;
        let msg = format!(
            "#[derive(Mark)]: `{name_str}`'s channel `{field}` is `{}` but `{in_name}.{field}` is \
             `{wgsl}` in the WGSL of `{path}`",
            quote!(#ty).to_string().replace(' ', "")
        );
        quote_spanned! {ch.ident.span()=>
            const _: () = ::core::assert!(
                #private::str_eq(<#ty as #root::GpuType>::WGSL, #wgsl),
                #msg
            );
        }
    });
    let field_idents: Vec<&Ident> = channels.iter().map(|c| &c.ident).collect();
    let antialias_item = antialias.map(|a| quote!(const ANTIALIAS: #root::Antialias = #a;));
    let geometry_item =
        polyline.then(|| quote!(const GEOMETRY: #root::Geometry = #root::Geometry::Polyline;));

    let hit_item = hit.map(|h| quote!(const HIT: #root::HitShape = #h;));
    let gpu_pick_item = args.gpu_pick.map(|_| {
        quote!(
            const GPU_PICK: bool = true;
        )
    });

    let trait_ident = format_ident!("{}Attrs", name);
    let trait_doc = format!(
        "Typed setters for `{name}`'s channels, generated by `#[derive(Mark)]`: each takes a \
         constant or an encoding of the channel's visual type, as `Selection::attr` does."
    );
    let is_position = |ch: &Channel| ch.role.as_ref().is_some_and(|r| r == "X" || r == "Y");
    let methods = channels.iter().map(|ch| {
        let (field, ty) = (&ch.ident, &ch.ty);
        let docs = &ch.docs;
        if is_position(ch) {
            let with = format_ident!("{}_with", field);
            let with_doc = format!(
                "Like [`{field}`](Self::{field}), for a constant or an explicit encoding: \
                 `.{with}(log.encode(|r| ..))`."
            );
            let plain_doc = "A closure of each row, encoded through the plot's scale for this \
                 axis when the plot resolves. For a constant or an explicit encoding, see the \
                 `_with` form.";
            quote! {
                #(#docs)*
                #[doc = #plain_doc]
                fn #field<F, D>(&mut self, accessor: F) -> &mut Self
                where
                    F: ::core::ops::Fn(&T) -> D + ::core::marker::Send + ::core::marker::Sync + 'static,
                    D: #root::ColumnValue;

                #[doc = #with_doc]
                fn #with<E, K>(&mut self, encoding: E) -> &mut Self
                where
                    E: #root::IntoEncoding<T, #ty, K>;
            }
        } else {
            quote! {
                #(#docs)*
                fn #field<E, K>(&mut self, encoding: E) -> &mut Self
                where
                    E: #root::IntoEncoding<T, #ty, K>;
            }
        }
    });
    let impls = channels.iter().map(|ch| {
        let (field, ty) = (&ch.ident, &ch.ty);
        let const_ident = format_ident!("{}", field.to_string().to_uppercase());
        if is_position(ch) {
            let with = format_ident!("{}_with", field);
            quote! {
                fn #field<F, D>(&mut self, accessor: F) -> &mut Self
                where
                    F: ::core::ops::Fn(&T) -> D + ::core::marker::Send + ::core::marker::Sync + 'static,
                    D: #root::ColumnValue,
                {
                    self.attr_axis(#name::#const_ident, accessor)
                }

                fn #with<E, K>(&mut self, encoding: E) -> &mut Self
                where
                    E: #root::IntoEncoding<T, #ty, K>,
                {
                    self.attr(#name::#const_ident, encoding)
                }
            }
        } else {
            quote! {
                fn #field<E, K>(&mut self, encoding: E) -> &mut Self
                where
                    E: #root::IntoEncoding<T, #ty, K>,
                {
                    self.attr(#name::#const_ident, encoding)
                }
            }
        }
    });

    Ok(quote! {
        impl #name {
            #(#consts)*
        }

        #[doc = #trait_doc]
        #vis trait #trait_ident<T> {
            #(#methods)*
        }

        impl<T: ::core::marker::Send + ::core::marker::Sync + 'static> #trait_ident<T>
            for #root::Selection<T, #name>
        {
            #(#impls)*
        }

        const _: () = {
            #module_static

            impl #root::Mark for #name {
                const NAME: &'static str = #name_str;
                const MODULE: &'static #root::WgslModule = #module_expr;
                const CHANNELS: &'static [#root::ChannelDesc] = &[#(#descs),*];
                const VERTICES_PER_INSTANCE: u32 = 6;
                #antialias_item
                #geometry_item
                #hit_item
                #gpu_pick_item
            }

            #(#asserts)*

            // The descriptor fields are read through the table, not by
            // name: this keeps rustc from calling them dead.
            impl #name {
                #[allow(dead_code)]
                fn __gup_mark_fields(&self) {
                    let _ = (#(&self.#field_idents,)*);
                }
            }
        };
    })
}

/// The error for a `module = "…"` that is not in Gup's library: the
/// closest mark module when there is one, else every mark module.
fn unknown_module_message(path: &str, marks: &[String]) -> String {
    match gup_wgsl::compose::closest(path, marks.iter().map(String::as_str)) {
        Some(k) => format!("`{path}` is not a module of Gup's library (did you mean `{k}`?)"),
        None => format!(
            "`{path}` is not a module of Gup's library; its marks are: {}",
            marks.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With no close match the message lists every mark module; with one
    /// it names only that module.
    #[test]
    fn unknown_module_message_lists_marks_only_without_a_close_match() {
        let all = with_library(|lib| {
            Ok(lib
                .modules()
                .iter()
                .filter(|m| m.import_path.starts_with("gup::marks::"))
                .map(|m| m.import_path.clone())
                .collect::<Vec<_>>())
        })
        .unwrap();
        assert!(!all.is_empty());
        let far = unknown_module_message("gup::marks::completely_unrelated", &all);
        for m in &all {
            assert!(far.contains(m.as_str()), "{far}");
        }
        let near = unknown_module_message("gup::marks::circel", &all);
        assert!(
            near.contains("did you mean `gup::marks::circle`?"),
            "{near}"
        );
        assert!(!near.contains("its marks are"), "{near}");
    }
}
