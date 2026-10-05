// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The typed glue emitter (RFC-001 §6).
//!
//! For one `(mark, encoding signature)` pair it builds a small expression
//! tree per channel and prints **one** top-level WGSL module: imports, the
//! `Encodings` uniform struct (one field per channel: a shader function's
//! `Params` or a constant), the per-chunk uniform, bindings, the vertex
//! column inputs and the two entry points. naga_oil keeps entry points only
//! from the top-level shader, so entry points live only here. Library
//! modules are imported, never edited.

use super::{VIEW, WgslModule};
use crate::column::ColumnFormat;
use crate::encoding::{DynShaderFn, Resource};
use std::fmt::{self, Write as _};

/// Where a channel's value comes from.
pub(crate) enum ChannelSource<'a> {
    /// A field of the `Encodings` uniform.
    Const,
    /// A vertex column passed through a shader function.
    Column(&'a dyn DynShaderFn),
}

/// One mark channel, in mark order.
pub(crate) struct GlueChannel<'a> {
    pub name: &'static str,
    pub wgsl_type: &'static str,
    pub source: ChannelSource<'a>,
}

/// What to generate glue for.
pub(crate) struct GlueSpec<'a> {
    pub mark_name: &'static str,
    pub mark_module: &'static WgslModule,
    pub channels: Vec<GlueChannel<'a>>,
}

/// A texture + sampler pair bound for one channel's LUT.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LutBinding {
    /// Index into the spec's channels.
    pub channel: usize,
    /// Binding of the texture in group 1 (the sampler is `binding + 1`).
    pub binding: u32,
}

/// The generated top-level module and how to bind it.
#[derive(Clone, Debug)]
pub(crate) struct Glue {
    /// The WGSL source.
    pub source: String,
    /// Everything that determines the source: the pipeline cache key.
    pub signature: String,
    /// Library modules the source imports.
    pub modules: Vec<&'static WgslModule>,
    /// Channel index of each vertex column, by `@location`.
    pub columns: Vec<usize>,
    /// Channels that take a per-chunk base (relative columns).
    pub relative: Vec<usize>,
    /// LUT bindings in group 1.
    pub luts: Vec<LutBinding>,
}

/// A WGSL expression in the vertex entry point.
#[derive(Clone, Debug, PartialEq)]
enum Expr {
    /// `col.<name>`.
    Column(&'static str),
    /// `chunk.<name>_base`.
    ChunkBase(&'static str),
    /// `enc.<name>`.
    Uniform(&'static str),
    /// `<name>_lut`.
    Lut(&'static str),
    /// `<name>_smp`.
    Sampler(&'static str),
    /// `<alias>::<function>(args…)`.
    Call {
        alias: String,
        function: &'static str,
        args: Vec<Expr>,
    },
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Column(n) => write!(f, "col.{n}"),
            Expr::ChunkBase(n) => write!(f, "chunk.{n}_base"),
            Expr::Uniform(n) => write!(f, "enc.{n}"),
            Expr::Lut(n) => write!(f, "{n}_lut"),
            Expr::Sampler(n) => write!(f, "{n}_smp"),
            Expr::Call {
                alias,
                function,
                args,
            } => {
                write!(f, "{alias}::{function}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{a}")?;
                }
                f.write_str(")")
            }
        }
    }
}

/// Names the glue itself declares; module aliases must avoid them.
const RESERVED: &[&str] = &["col", "chunk", "enc", "m", "v", "u_view", "View"];

/// Import aliases: the last path segment, de-duplicated.
struct Aliases(Vec<(&'static str, String)>);

impl Aliases {
    fn get(&mut self, module: &'static WgslModule) -> String {
        if let Some((_, a)) = self.0.iter().find(|(p, _)| *p == module.import_path) {
            return a.clone();
        }
        let base = module
            .import_path
            .rsplit("::")
            .next()
            .unwrap_or(module.import_path);
        let mut alias = base.to_string();
        let mut n = 2;
        while RESERVED.contains(&alias.as_str()) || self.0.iter().any(|(_, a)| *a == alias) {
            alias = format!("{base}_{n}");
            n += 1;
        }
        self.0.push((module.import_path, alias.clone()));
        alias
    }
}

/// Generate the glue module for `spec`.
pub(crate) fn emit(spec: &GlueSpec<'_>) -> Glue {
    let mut aliases = Aliases(Vec::new());
    let mark_alias = aliases.get(spec.mark_module);

    // Per-channel expressions and bookkeeping.
    let mut exprs = Vec::new();
    let mut columns = Vec::new();
    let mut relative = Vec::new();
    let mut luts = Vec::new();
    let mut fields = Vec::new();
    let mut signature_parts = Vec::new();
    let mut next_binding = 1;
    for (i, ch) in spec.channels.iter().enumerate() {
        match ch.source {
            ChannelSource::Const => {
                fields.push((ch.name, ch.wgsl_type.to_string()));
                exprs.push((ch.name, Expr::Uniform(ch.name)));
                signature_parts.push(format!("{}: const {}", ch.name, ch.wgsl_type));
            }
            ChannelSource::Column(func) => {
                let alias = aliases.get(func.module());
                fields.push((ch.name, format!("{alias}::Params")));
                columns.push(i);
                let mut args = vec![Expr::Column(ch.name)];
                if func.input_format() == ColumnFormat::F32Relative {
                    relative.push(i);
                    args.push(Expr::ChunkBase(ch.name));
                }
                args.push(Expr::Uniform(ch.name));
                for resource in func.resources() {
                    match resource {
                        Resource::Lut(_) => {
                            luts.push(LutBinding {
                                channel: i,
                                binding: next_binding,
                            });
                            next_binding += 2;
                            args.push(Expr::Lut(ch.name));
                            args.push(Expr::Sampler(ch.name));
                        }
                    }
                }
                exprs.push((
                    ch.name,
                    Expr::Call {
                        alias,
                        function: func.entry(),
                        args,
                    },
                ));
                signature_parts.push(format!("{}: {}", ch.name, func.signature()));
            }
        }
    }
    let signature = format!("{} {{{}}}", spec.mark_name, signature_parts.join(", "));

    // Print the module.
    let mut s = String::new();
    let w = &mut s;
    let _ = writeln!(w, "// gup glue: {signature}");
    let _ = writeln!(w, "// Generated by gup-core's glue emitter; do not edit.");
    let _ = writeln!(w, "#import {}::{{View}}", VIEW.import_path);
    for (path, alias) in &aliases.0 {
        let _ = writeln!(w, "#import {path} as {alias}");
    }

    let _ = writeln!(w, "\nstruct Encodings {{");
    for (name, ty) in &fields {
        // Uniform layout: struct-typed members need 16-byte alignment and
        // 16 bytes before the next member; aligning every field is simplest.
        let _ = writeln!(w, "    @align(16) {name}: {ty},");
    }
    let _ = writeln!(w, "}}\n\nstruct Chunk {{\n    row_base: u32,");
    for &i in &relative {
        let _ = writeln!(w, "    {}_base: f32,", spec.channels[i].name);
    }
    let _ = writeln!(w, "}}\n");

    let _ = writeln!(w, "@group(0) @binding(0) var<uniform> u_view: View;");
    let _ = writeln!(w, "@group(1) @binding(0) var<uniform> enc: Encodings;");
    for lut in &luts {
        let name = spec.channels[lut.channel].name;
        let _ = writeln!(
            w,
            "@group(1) @binding({}) var {name}_lut: texture_2d<f32>;",
            lut.binding
        );
        let _ = writeln!(
            w,
            "@group(1) @binding({}) var {name}_smp: sampler;",
            lut.binding + 1
        );
    }
    let _ = writeln!(w, "@group(2) @binding(0) var<uniform> chunk: Chunk;");

    let col_param = if columns.is_empty() {
        ""
    } else {
        let _ = writeln!(w, "\nstruct Columns {{");
        for (loc, &i) in columns.iter().enumerate() {
            let _ = writeln!(w, "    @location({loc}) {}: f32,", spec.channels[i].name);
        }
        let _ = writeln!(w, "}}");
        ", col: Columns"
    };

    let _ = writeln!(
        w,
        "\n@vertex\nfn vs_main(@builtin(vertex_index) vertex_index: u32, \
         @builtin(instance_index) instance_index: u32{col_param}) -> {mark_alias}::Varyings {{"
    );
    let _ = writeln!(w, "    var m: {mark_alias}::{}In;", spec.mark_name);
    for (name, expr) in &exprs {
        let _ = writeln!(w, "    m.{name} = {expr};");
    }
    let _ = writeln!(
        w,
        "    return {mark_alias}::vertex(m, vertex_index, chunk.row_base + instance_index, u_view);\n}}"
    );
    let _ = writeln!(
        w,
        "\n@fragment\nfn fs_main(v: {mark_alias}::Varyings) -> @location(0) vec4<f32> {{\n    \
         return {mark_alias}::shade(v);\n}}"
    );

    let mut modules = vec![&VIEW];
    modules.extend(
        aliases
            .0
            .iter()
            .filter_map(|(path, _)| find_module(spec, path)),
    );
    Glue {
        source: s,
        signature,
        modules,
        columns,
        relative,
        luts,
    }
}

fn find_module(spec: &GlueSpec<'_>, path: &str) -> Option<&'static WgslModule> {
    if spec.mark_module.import_path == path {
        return Some(spec.mark_module);
    }
    spec.channels.iter().find_map(|ch| match ch.source {
        ChannelSource::Column(f) if f.module().import_path == path => Some(f.module()),
        _ => None,
    })
}
