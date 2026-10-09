// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # gup-wgsl
//!
//! Gup's shader composition (RFC-001 §6), split at the build boundary
//! (RFC-001 "Decisions (2026-10-09)", GUP-406):
//!
//! - **Build time** (feature `compose`): naga_oil composes the WGSL library
//!   modules once and they are written back out as plain WGSL in which every
//!   module item is named by [`flat_name`] (`gup::scale::linear` +
//!   `map_rel` → `gup_scale_linear_map_rel`). Authored top-level shaders are
//!   flattened completely. Composition errors fail the build with
//!   naga_oil's report, which names the source module and line.
//! - **Run time** (no features, no dependencies): [`link`] turns a
//!   generated top-level module, whose `#import` lines use naga_oil's
//!   syntax, into plain WGSL by qualifying the imported names, and appends
//!   the flattened modules it needs. The result goes to wgpu as
//!   `ShaderSource::Wgsl` on every target.
//!
//! Library modules may use only `#define_import_path` and `#import` (no
//! `#ifdef`), so flattening them once is exact.

mod lex;

#[cfg(feature = "compose")]
pub mod compose;

use std::fmt;

/// A WGSL library module, flattened at build time.
#[derive(Debug)]
pub struct WgslModule {
    /// The module's `#define_import_path`, e.g. `gup::scale::linear`.
    pub import_path: &'static str,
    /// Plain WGSL declaring the module's own items under their
    /// [`flat_name`]s. It refers to items of [`imports`](Self::imports) by
    /// their flat names too, and contains no preprocessor lines.
    pub wgsl: &'static str,
    /// Modules this module imports.
    pub imports: &'static [&'static WgslModule],
}

/// The plain-WGSL name of `item` from the module at `import_path`:
/// `::` becomes `_` and the item is appended (`gup::view`, `View` →
/// `gup_view_View`).
///
/// Library item names never end in a digit (a naga_oil rule) and the build
/// fails if two items share a flat name, so these names survive naga's WGSL
/// writer unchanged.
pub fn flat_name(import_path: &str, item: &str) -> String {
    format!("{}_{item}", import_path.replace("::", "_"))
}

/// Why [`link`] failed. Only generated code is linked at run time, so this
/// is a bug in the generator, not a user error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkError {
    /// The label of the module being linked.
    pub label: String,
    /// 1-based line in the module's source.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.label, self.line, self.message)
    }
}

impl std::error::Error for LinkError {}

/// One parsed `#import` line.
enum Import<'a> {
    /// `#import path as alias` (or `#import path`, aliased by its last
    /// segment).
    Alias { path: &'a str, alias: &'a str },
    /// `#import path::{A, B}`.
    Items { path: &'a str, items: Vec<&'a str> },
}

fn parse_import(line: &str) -> Option<Import<'_>> {
    let rest = line.trim().strip_prefix("#import")?.trim();
    if let Some((path, items)) = rest.split_once("::{") {
        let items = items.strip_suffix('}')?;
        let items: Vec<_> = items
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let valid = items.iter().all(|i| is_ident(i)) && is_path(path);
        return valid.then_some(Import::Items { path, items });
    }
    let (path, alias) = match rest.split_once(" as ") {
        Some((path, alias)) => (path.trim(), alias.trim()),
        None => (rest, rest.rsplit("::").next()?),
    };
    (is_path(path) && is_ident(alias)).then_some(Import::Alias { path, alias })
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_path(s: &str) -> bool {
    s.split("::").all(is_ident)
}

/// Link a generated top-level module against flattened library modules.
///
/// `source` may start with naga_oil-style `#import` lines
/// (`#import a::b as alias`, `#import a::b`, `#import a::b::{X, Y}`); every
/// imported path must be one of `modules` or imported by one of them. Uses
/// of `alias::item`, `full::path::item` and imported bare items become
/// [`flat_name`]s; comments and member accesses (`.x`) are left alone. The
/// `#import` lines become comments, so line numbers in the result match
/// `source`. The flattened text of each imported module and of its imports,
/// transitively and once each, follows the module.
pub fn link(
    label: &str,
    source: &str,
    modules: &[&'static WgslModule],
) -> Result<String, LinkError> {
    let err = |offset: usize, message: String| LinkError {
        label: label.to_string(),
        line: source[..offset].matches('\n').count() + 1,
        message,
    };
    let find = |path: &str| -> Option<&'static WgslModule> {
        fn search(m: &'static WgslModule, path: &str) -> Option<&'static WgslModule> {
            if m.import_path == path {
                return Some(m);
            }
            m.imports.iter().find_map(|d| search(d, path))
        }
        modules.iter().find_map(|m| search(m, path))
    };

    let toks = lex::lex(source);
    let mut aliases: Vec<(&str, &'static WgslModule)> = Vec::new();
    let mut items: Vec<(&str, &'static WgslModule)> = Vec::new();
    let mut used: Vec<&'static WgslModule> = Vec::new();
    for (range, tok) in &toks {
        let lex::Tok::Directive(line) = *tok else {
            continue;
        };
        let import = parse_import(line).ok_or_else(|| {
            err(
                range.start,
                format!("unsupported directive `{line}` (only `#import` can be linked)"),
            )
        })?;
        let path = match &import {
            Import::Alias { path, .. } | Import::Items { path, .. } => *path,
        };
        let module = find(path).ok_or_else(|| {
            err(
                range.start,
                format!("`{path}` is not one of the modules given to the linker"),
            )
        })?;
        match import {
            Import::Alias { alias, .. } => aliases.push((alias, module)),
            Import::Items { items: names, .. } => {
                items.extend(names.into_iter().map(|n| (n, module)));
            }
        }
        if !used.iter().any(|m| std::ptr::eq(*m, module)) {
            used.push(module);
        }
    }

    let mut out = String::with_capacity(source.len() * 2);
    let mut last = 0; // end of the source text copied so far
    let mut prev_significant: Option<lex::Tok<'_>> = None;
    let mut i = 0;
    while i < toks.len() {
        let (range, tok) = (&toks[i].0, toks[i].1);
        match tok {
            lex::Tok::Directive(_) => {
                out.push_str(&source[last..range.start]);
                out.push_str("// ");
                out.push_str(&source[range.clone()]);
                last = range.end;
            }
            lex::Tok::Ident(first) if prev_significant != Some(lex::Tok::Punct('.')) => {
                // The longest `a::b::…::item` chain starting here.
                let mut segments = vec![first];
                let mut end = range.end;
                let mut j = i + 1;
                while let (Some((_, lex::Tok::PathSep)), Some((r, lex::Tok::Ident(next)))) =
                    (toks.get(j), toks.get(j + 1))
                {
                    segments.push(next);
                    end = r.end;
                    j += 2;
                }
                let target = if segments.len() > 1 {
                    let (item, prefix) = segments.split_last().expect("two or more");
                    let module = if prefix.len() == 1 {
                        aliases
                            .iter()
                            .find(|(a, _)| *a == prefix[0])
                            .map(|&(_, m)| m)
                    } else {
                        None
                    }
                    .or_else(|| find(&prefix.join("::")))
                    .ok_or_else(|| {
                        err(
                            range.start,
                            format!("`{}` does not name an imported module", prefix.join("::")),
                        )
                    })?;
                    Some((module, *item))
                } else {
                    items
                        .iter()
                        .find(|(name, _)| *name == first)
                        .map(|&(name, m)| (m, name))
                };
                if let Some((module, item)) = target {
                    let flat = flat_name(module.import_path, item);
                    if !lex::declarations(module.wgsl)
                        .iter()
                        .any(|d| d.name == Some(flat.as_str()))
                    {
                        return Err(err(
                            range.start,
                            format!("`{}` has no item `{item}`", module.import_path),
                        ));
                    }
                    out.push_str(&source[last..range.start]);
                    out.push_str(&flat);
                    last = end;
                    if !used.iter().any(|m| std::ptr::eq(*m, module)) {
                        used.push(module);
                    }
                }
                prev_significant = Some(lex::Tok::Ident(segments[segments.len() - 1]));
                i = j;
                continue;
            }
            lex::Tok::Trivia => {}
            _ => {}
        }
        if tok != lex::Tok::Trivia {
            prev_significant = Some(tok);
        }
        i += 1;
    }
    out.push_str(&source[last..]);

    // Imported modules and their imports, dependencies first, once each.
    fn visit(m: &'static WgslModule, order: &mut Vec<&'static WgslModule>) {
        if order.iter().any(|o| std::ptr::eq(*o, m)) {
            return;
        }
        for dep in m.imports {
            visit(dep, order);
        }
        order.push(m);
    }
    let mut order = Vec::new();
    for m in used {
        visit(m, &mut order);
    }
    // Two distinct modules with one import path would declare the same
    // flat names twice.
    for (i, m) in order.iter().enumerate() {
        if order[..i].iter().any(|o| o.import_path == m.import_path) {
            return Err(err(
                0,
                format!(
                    "two different modules are both `{}`; import paths must be unique",
                    m.import_path
                ),
            ));
        }
    }
    for m in order {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n// ");
        out.push_str(m.import_path);
        out.push_str(" (flattened at build time)\n");
        out.push_str(m.wgsl);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    static VIEW: WgslModule = WgslModule {
        import_path: "gup::view",
        wgsl: "struct gup_view_View {\n    size: vec2<f32>,\n}\n\n\
               fn gup_view_px_to_clip(p: vec2<f32>, view: gup_view_View) -> vec4<f32> {\n    \
               return vec4<f32>(p / view.size, 0f, 1f);\n}\n",
        imports: &[],
    };
    static MARK: WgslModule = WgslModule {
        import_path: "gup::marks::dot",
        wgsl: "fn gup_marks_dot_vertex(view: gup_view_View) -> vec4<f32> {\n    \
               return gup_view_px_to_clip(vec2<f32>(0f), view);\n}\n",
        imports: &[&VIEW],
    };

    #[test]
    fn link_qualifies_aliases_items_and_full_paths() {
        let src = "// glue: dot::vertex in a comment stays\n\
                   #import gup::view::{View}\n\
                   #import gup::marks::dot as dot\n\
                   @group(0) @binding(0) var<uniform> u_view: View;\n\
                   @vertex fn vs_main() -> @builtin(position) vec4<f32> {\n    \
                   let t = u_view.View;\n    \
                   return dot::vertex(u_view) + gup::marks::dot::vertex(u_view);\n}\n";
        let out = link("glue", src, &[&MARK]).unwrap();
        let body = out.split("\n// gup::view (flattened").next().unwrap();
        assert_eq!(
            body,
            "// glue: dot::vertex in a comment stays\n\
             // #import gup::view::{View}\n\
             // #import gup::marks::dot as dot\n\
             @group(0) @binding(0) var<uniform> u_view: gup_view_View;\n\
             @vertex fn vs_main() -> @builtin(position) vec4<f32> {\n    \
             let t = u_view.View;\n    \
             return gup_marks_dot_vertex(u_view) + gup_marks_dot_vertex(u_view);\n}\n"
        );
        // Dependencies first, each once.
        let view_at = out.find("// gup::view (flattened").unwrap();
        let dot_at = out.find("// gup::marks::dot (flattened").unwrap();
        assert!(view_at < dot_at);
        assert_eq!(out.matches("struct gup_view_View").count(), 1);
    }

    #[test]
    fn link_errors_name_the_line() {
        let e = link("g", "\n#import gup::nope as n\n", &[&MARK]).unwrap_err();
        assert_eq!(e.line, 2, "{e}");
        assert!(e.message.contains("gup::nope"), "{e}");
        let e = link(
            "g",
            "#import gup::view as v\nfn f() { v::missing(); }\n",
            &[&VIEW],
        )
        .unwrap_err();
        assert_eq!(e.line, 2, "{e}");
        assert!(e.message.contains("no item `missing`"), "{e}");
        let e = link("g", "#ifdef X\n", &[]).unwrap_err();
        assert!(e.message.contains("unsupported directive"), "{e}");
    }

    #[test]
    fn link_refuses_two_modules_with_one_path() {
        static OTHER_VIEW: WgslModule = WgslModule {
            import_path: "gup::view",
            wgsl: "struct gup_view_View {\n    size: vec2<f32>,\n}\n",
            imports: &[],
        };
        static USER: WgslModule = WgslModule {
            import_path: "user::m",
            wgsl: "fn user_m_f() {}\n",
            imports: &[&OTHER_VIEW],
        };
        let e = link(
            "g",
            "#import gup::marks::dot as dot\n#import user::m as m\n",
            &[&MARK, &USER],
        )
        .unwrap_err();
        assert!(
            e.message
                .contains("two different modules are both `gup::view`"),
            "{e}"
        );
    }

    #[test]
    fn flat_names() {
        assert_eq!(
            flat_name("gup::scale::linear", "map_rel"),
            "gup_scale_linear_map_rel"
        );
    }
}
