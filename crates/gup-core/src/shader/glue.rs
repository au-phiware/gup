// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The typed glue emitter (RFC-001 §6).
//!
//! For one `(mark, encoding signature)` pair it builds a small expression
//! tree per channel and prints **one** top-level WGSL module: imports, the
//! `Encodings` uniform struct (one field per constant channel and per link
//! of each column channel's chain: a shader function's `Params` or a
//! constant), the per-chunk uniform, bindings, the vertex column inputs and
//! the two entry points. Library modules are imported (naga_oil syntax,
//! resolved by `gup_wgsl::link` against the modules flattened at build
//! time), never edited, and entry points live only here.
//!
//! A column channel is a **chain** of shader functions (`a.then(b)`,
//! RFC-001 §5): the first link reads the vertex column, each later link
//! reads the previous link's result, all in one expression
//! (`b::map(a::map_rel(col.x, chunk.x_base, enc.x), enc.x_link1)`). The
//! first link's `Encodings` field and `Chunk` base are named after the
//! channel, so a one-link chain prints exactly what the one-function
//! emitter did; link `k ≥ 1` adds `<channel>_link<k>`.

use super::{StructLayout, VIEW, WgslModule};
use crate::channel::Role;
use crate::column::{ColumnFormat, NULL_CODE};
use crate::encoding::{DynShaderFn, Resource};
use crate::scale::NULL_COLOR;
use std::fmt::{self, Write as _};

/// Where a channel's value comes from.
pub(crate) enum ChannelSource<'a> {
    /// A field of the `Encodings` uniform.
    Const,
    /// A vertex column passed through a chain of shader functions, in
    /// evaluation order (never empty).
    Column(Vec<&'a dyn DynShaderFn>),
}

/// One mark channel, in mark order.
pub(crate) struct GlueChannel<'a> {
    pub name: &'static str,
    pub wgsl_type: &'static str,
    pub role: Option<Role>,
    pub source: ChannelSource<'a>,
}

/// What to generate glue for.
pub(crate) struct GlueSpec<'a> {
    pub mark_name: &'static str,
    pub mark_module: &'static WgslModule,
    pub channels: Vec<GlueChannel<'a>>,
    /// Whether the columns have nulls, so the vertex stage reads validity
    /// bits (one plane per numeric column, in column order; see
    /// [`crate::column`]).
    pub validity: bool,
}

/// A texture + sampler pair bound for one link's LUT.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LutBinding {
    /// Index into the spec's channels.
    pub channel: usize,
    /// Index of the link in the channel's chain.
    pub link: usize,
    /// Binding of the texture in group 1 (the sampler is `binding + 1`).
    pub binding: u32,
}

/// A per-chunk base in the `Chunk` uniform: one per link whose entry
/// point is relative.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChunkBase {
    /// Index into the spec's channels.
    pub channel: usize,
    /// Index of the link in the channel's chain. Only link 0 reads the
    /// stored column, so only its base comes from the chunk's origin; a
    /// later link's input is absolute (origin 0).
    pub link: usize,
    /// The link's input format: one `f32` member (`<field>_base`), or a
    /// hi/lo pair (`<field>_base`, `<field>_base_lo`).
    pub format: ColumnFormat,
    /// The `Chunk` member holding the (hi) base.
    pub member: String,
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
    /// Channel index and storage format of each vertex column, by
    /// `@location`.
    pub columns: Vec<(usize, ColumnFormat)>,
    /// Whether group 2 binds the chunk's validity bits at binding 1.
    pub validity: bool,
    /// The per-chunk bases (relative links), in `Chunk` member order.
    pub relative: Vec<ChunkBase>,
    /// LUT bindings in group 1.
    pub luts: Vec<LutBinding>,
    /// Layout of the `Encodings` uniform: each field starts on a 16-byte
    /// boundary, so offsets follow from the encase sizes alone (GUP-401,
    /// GUP-406). A test checks them against naga's layout of the linked
    /// module.
    pub encodings: StructLayout,
    /// Layout of the per-chunk `Chunk` uniform (`row_base`, then each
    /// relative link's base: `<field>_base: f32`, plus
    /// `<field>_base_lo: f32` for a hi/lo input).
    pub chunk: StructLayout,
}

/// The `Encodings` field (and `Chunk` base, LUT and sampler prefix) of
/// link `link` of channel `channel`: the channel's name for the first
/// link, `<channel>_link<k>` after it.
pub(crate) fn link_field(channel: &str, link: usize) -> String {
    if link == 0 {
        channel.to_string()
    } else {
        format!("{channel}_link{link}")
    }
}

/// A WGSL expression in the vertex entry point.
#[derive(Clone, Debug, PartialEq)]
enum Expr {
    /// `col.<name>`.
    Column(String),
    /// `chunk.<field>_base`, or for a hi/lo base
    /// `vec2<f32>(chunk.<field>_base, chunk.<field>_base_lo)`.
    ChunkBase { field: String, pair: bool },
    /// `enc.<field>`.
    Uniform(String),
    /// `<field>_lut`.
    Lut(String),
    /// `<field>_smp`.
    Sampler(String),
    /// `vec2<f32>(expr, 0.0)`: an absolute value as a hi/lo pair.
    Pair(Box<Expr>),
    /// `<alias>::<function>(args…)`.
    Call {
        alias: String,
        function: &'static str,
        args: Vec<Expr>,
    },
    /// `expr`, or the null colour where validity plane `plane` is unset.
    OrNull { expr: Box<Expr>, plane: usize },
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Column(n) => write!(f, "col.{n}"),
            Expr::ChunkBase { field, pair: false } => write!(f, "chunk.{field}_base"),
            Expr::ChunkBase { field, pair: true } => {
                write!(f, "vec2<f32>(chunk.{field}_base, chunk.{field}_base_lo)")
            }
            Expr::Uniform(n) => write!(f, "enc.{n}"),
            Expr::Lut(n) => write!(f, "{n}_lut"),
            Expr::Sampler(n) => write!(f, "{n}_smp"),
            Expr::Pair(e) => write!(f, "vec2<f32>({e}, 0.0)"),
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
            Expr::OrNull { expr, plane } => {
                let [r, g, b, a] = NULL_COLOR.to_array();
                write!(
                    f,
                    "select(vec4<f32>({r:?}, {g:?}, {b:?}, {a:?}), {expr}, {})",
                    valid(*plane)
                )
            }
        }
    }
}

/// Whether validity plane `plane` is set for this row.
fn valid(plane: usize) -> String {
    format!("((validity[valid_group + {plane}u] >> valid_bit) & 1u) == 1u")
}

/// Names the glue itself declares; module aliases must avoid them.
const RESERVED: &[&str] = &[
    "col",
    "chunk",
    "enc",
    "m",
    "v",
    "u_view",
    "View",
    "validity",
    "valid_group",
    "valid_bit",
    "out",
];

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

/// Size in bytes of a constant channel's WGSL type (the `GpuType`s
/// channels use).
fn const_size(wgsl_type: &str) -> u64 {
    match wgsl_type {
        "f32" | "u32" | "i32" => 4,
        "vec2<f32>" => 8,
        "vec4<f32>" => 16,
        other => unreachable!("no constant channel has WGSL type {other}"),
    }
}

/// Generate the glue module for `spec`.
pub(crate) fn emit(spec: &GlueSpec<'_>) -> Glue {
    let mut aliases = Aliases(Vec::new());
    let mark_alias = aliases.get(spec.mark_module);

    // Per-channel expressions and bookkeeping.
    let mut exprs = Vec::new();
    let mut columns = Vec::new();
    // Validity planes (numeric columns, in column order) of channels that
    // position or size the mark: a null in any of them hides the row.
    let mut geometry_planes = Vec::new();
    // Dictionary-code (`U32`) columns of channels that position or size
    // the mark: a null key (`NULL_CODE`) hides the row. Codes have no
    // validity plane; the reserved code is compared instead (RFC-001 S5b).
    let mut geometry_keys = Vec::new();
    let mut planes = 0;
    let mut relative = Vec::new();
    let mut luts = Vec::new();
    let mut fields = Vec::new();
    let mut signature_parts = Vec::new();
    let mut next_binding = 1;
    for (i, ch) in spec.channels.iter().enumerate() {
        match &ch.source {
            ChannelSource::Const => {
                fields.push((
                    ch.name.to_string(),
                    ch.wgsl_type.to_string(),
                    const_size(ch.wgsl_type),
                ));
                exprs.push((ch.name, Expr::Uniform(ch.name.to_string())));
                signature_parts.push(format!("{}: const {}", ch.name, ch.wgsl_type));
            }
            ChannelSource::Column(links) => {
                assert!(!links.is_empty(), "{}: an empty chain", ch.name);
                let format = links[0].input_format();
                columns.push((i, format));
                let plane = format.has_validity().then(|| {
                    planes += 1;
                    planes - 1
                });
                let mut expr = Expr::Column(ch.name.to_string());
                for (k, func) in links.iter().enumerate() {
                    let field = link_field(ch.name, k);
                    let alias = aliases.get(func.module());
                    let size = func.params_size();
                    // A struct member must be followed by roundUp(16, size)
                    // bytes before the next member, and padding members
                    // would break that, so the struct itself spans a
                    // multiple of 16.
                    assert!(
                        size % 16 == 0,
                        "{}::Params is {size} bytes; uniform Params structs must span a \
                         multiple of 16 bytes (pad them in WGSL and in the encase struct)",
                        func.module().import_path
                    );
                    fields.push((field.clone(), format!("{alias}::Params"), size));
                    let input_format = func.input_format();
                    // A later link reads an absolute value: as a hi/lo pair
                    // if its entry point takes one.
                    if k > 0 && input_format == ColumnFormat::F32x2Relative {
                        expr = Expr::Pair(Box::new(expr));
                    }
                    let mut args = vec![expr];
                    if input_format.is_relative() {
                        relative.push(ChunkBase {
                            channel: i,
                            link: k,
                            format: input_format,
                            member: format!("{field}_base"),
                        });
                        args.push(Expr::ChunkBase {
                            field: field.clone(),
                            pair: input_format == ColumnFormat::F32x2Relative,
                        });
                    }
                    args.push(Expr::Uniform(field.clone()));
                    for resource in func.resources() {
                        match resource {
                            Resource::Lut(_) => {
                                luts.push(LutBinding {
                                    channel: i,
                                    link: k,
                                    binding: next_binding,
                                });
                                next_binding += 2;
                                args.push(Expr::Lut(field.clone()));
                                args.push(Expr::Sampler(field.clone()));
                            }
                        }
                    }
                    expr = Expr::Call {
                        alias,
                        function: func.entry(),
                        args,
                    };
                }
                match plane {
                    // A null colour input draws in the null colour; a
                    // null position or size hides the row.
                    Some(plane) if spec.validity && ch.role == Some(Role::Color) => {
                        expr = Expr::OrNull {
                            expr: Box::new(expr),
                            plane,
                        };
                    }
                    Some(plane) => geometry_planes.push(plane),
                    None if format == ColumnFormat::U32 && ch.role != Some(Role::Color) => {
                        geometry_keys.push(ch.name);
                    }
                    None => {}
                }
                exprs.push((ch.name, expr));
                let chain: Vec<String> = links.iter().map(|f| f.signature()).collect();
                signature_parts.push(format!("{}: {}", ch.name, chain.join(" then ")));
            }
        }
    }
    // Validity bits only when the columns have a null (and only numeric
    // columns have bits).
    let validity = spec.validity && planes > 0;
    let mut signature = format!("{} {{{}}}", spec.mark_name, signature_parts.join(", "));
    if validity {
        signature.push_str(" with nulls");
    }

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
    let mut encodings = StructLayout {
        span: 0,
        members: Vec::new(),
    };
    for (name, ty, size) in &fields {
        // Every field starts on a 16-byte boundary by construction: `Params`
        // structs span multiples of 16 and scalar constants get `u32`
        // padding, so offsets follow from the sizes alone. Not
        // `@align(16)`: library structs pass through naga's WGSL writer at
        // build time, which drops layout attributes, and a browser with
        // `uniform_buffer_standard_layout` accepts the natural, unaligned
        // offsets, so the shader and the uniform bytes would disagree
        // (GUP-401).
        let _ = writeln!(w, "    {name}: {ty},");
        encodings.members.push((name.clone(), encodings.span));
        for (k, pad) in ["a", "b", "c"].iter().enumerate() {
            let offset = *size as u32 + 4 * k as u32;
            if size.next_multiple_of(16) > u64::from(offset) {
                let _ = writeln!(w, "    {name}_pad_{pad}: u32,");
                encodings
                    .members
                    .push((format!("{name}_pad_{pad}"), encodings.span + offset));
            }
        }
        encodings.span += size.next_multiple_of(16) as u32;
    }
    let _ = writeln!(w, "}}\n\nstruct Chunk {{\n    row_base: u32,");
    let mut chunk = StructLayout {
        span: 4,
        members: vec![("row_base".to_string(), 0)],
    };
    for base in &relative {
        let mut members = vec![base.member.clone()];
        if base.format == ColumnFormat::F32x2Relative {
            members.push(format!("{}_lo", base.member));
        }
        for member in members {
            let _ = writeln!(w, "    {member}: f32,");
            chunk.members.push((member, chunk.span));
            chunk.span += 4;
        }
    }
    let _ = writeln!(w, "}}\n");

    let _ = writeln!(w, "@group(0) @binding(0) var<uniform> u_view: View;");
    let _ = writeln!(w, "@group(1) @binding(0) var<uniform> enc: Encodings;");
    for lut in &luts {
        let name = link_field(spec.channels[lut.channel].name, lut.link);
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
    if validity {
        let _ = writeln!(
            w,
            "@group(2) @binding(1) var<storage, read> validity: array<u32>;"
        );
    }

    let col_param = if columns.is_empty() {
        ""
    } else {
        let _ = writeln!(w, "\nstruct Columns {{");
        for (loc, (i, format)) in columns.iter().enumerate() {
            let _ = writeln!(
                w,
                "    @location({loc}) {}: {},",
                spec.channels[*i].name,
                format.wgsl_type()
            );
        }
        let _ = writeln!(w, "}}");
        ", col: Columns"
    };

    let _ = writeln!(
        w,
        "\n@vertex\nfn vs_main(@builtin(vertex_index) vertex_index: u32, \
         @builtin(instance_index) instance_index: u32{col_param}) -> {mark_alias}::Varyings {{"
    );
    if validity {
        // This row's bits: its 32-row group of `planes` words, then its bit.
        let _ = writeln!(
            w,
            "    let valid_group = (instance_index / 32u) * {planes}u;\n    \
             let valid_bit = instance_index % 32u;"
        );
    }
    let _ = writeln!(w, "    var m: {mark_alias}::{}In;", spec.mark_name);
    for (name, expr) in &exprs {
        let _ = writeln!(w, "    m.{name} = {expr};");
    }
    let vertex =
        format!("{mark_alias}::vertex(m, vertex_index, chunk.row_base + instance_index, u_view)");
    let mut drawn = Vec::new();
    if validity && !geometry_planes.is_empty() {
        let words = geometry_planes
            .iter()
            .map(|p| format!("validity[valid_group + {p}u]"))
            .collect::<Vec<_>>()
            .join(" & ");
        drawn.push(format!("(({words}) >> valid_bit) & 1u"));
    }
    for name in &geometry_keys {
        drawn.push(format!("u32(col.{name} != {NULL_CODE}u)"));
    }
    if !drawn.is_empty() {
        // A null position or size: every corner at one point outside the
        // clip volume, a degenerate quad that is never rasterised. The mark
        // contract names the position member `clip`.
        let drawn = drawn.join(" & ");
        let _ = writeln!(
            w,
            "    var out = {vertex};\n    \
             let drawn = {drawn};\n    \
             out.clip = select(vec4<f32>(2.0, 2.0, 2.0, 1.0), out.clip, drawn == 1u);\n    \
             return out;\n}}"
        );
    } else {
        let _ = writeln!(w, "    return {vertex};\n}}");
    }
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
        validity,
        relative,
        luts,
        encodings,
        chunk,
    }
}

fn find_module(spec: &GlueSpec<'_>, path: &str) -> Option<&'static WgslModule> {
    if spec.mark_module.import_path == path {
        return Some(spec.mark_module);
    }
    spec.channels.iter().find_map(|ch| match &ch.source {
        ChannelSource::Column(links) => links
            .iter()
            .map(|f| f.module())
            .find(|m| m.import_path == path),
        ChannelSource::Const => None,
    })
}
