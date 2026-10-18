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
    /// Whether the mark's geometry is [`Geometry::Polyline`]: each
    /// position channel is read again at the next row, as
    /// `<channel>_next`.
    ///
    /// [`Geometry::Polyline`]: crate::Geometry::Polyline
    pub polyline: bool,
    /// Whether the mark defines `hit_distance` and `hit_row`
    /// ([`Mark::GPU_PICK`](crate::Mark::GPU_PICK)), so the glue has a pick
    /// variant ([`Glue::pick`]).
    pub gpu_pick: bool,
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
    /// Whether this is the base of a polyline's next-row column
    /// (`<channel>_next_base`): the next chunk's origin in a bridge
    /// entry, the chunk's own elsewhere.
    pub next: bool,
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
    /// A polyline's next-row vertex columns, at the locations after
    /// `columns`: each an index into `columns`, whose column it reads one
    /// row on.
    pub successors: Vec<usize>,
    /// Whether the glue is a polyline's (see [`GlueSpec::polyline`]).
    /// With validity, group 2 binds the next row's validity bits at
    /// binding 2 and the `Chunk` uniform holds `valid_offset` and
    /// `next_offset`.
    pub polyline: bool,
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
    /// The pick variant of [`source`](Self::source) (RFC-001 S9d), if the
    /// mark draws in GPU picks: the same module with `vs_pick` and
    /// `fs_pick` in place of `vs_main` and `fs_main`, and the `Pick`
    /// uniform at group 3. Linked only when a pick needs it, so colour
    /// pipelines never pay for it.
    pub pick: Option<String>,
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
                write!(
                    f,
                    "select(enc.{NULL_COLOR_FIELD}, {expr}, {})",
                    valid(*plane)
                )
            }
        }
    }
}

impl Expr {
    /// The same expression read at the next row: column `name` becomes
    /// `name_next`, and its chunk base `name_next_base` (a polyline's
    /// successor, GUP-432). Uniforms, LUTs and later links' bases are
    /// shared.
    fn at_next_row(&self, name: &str) -> Expr {
        match self {
            Expr::Column(n) if n == name => Expr::Column(format!("{n}_next")),
            Expr::ChunkBase { field, pair } if field == name => Expr::ChunkBase {
                field: format!("{field}_next"),
                pair: *pair,
            },
            Expr::Pair(e) => Expr::Pair(Box::new(e.at_next_row(name))),
            Expr::Call {
                alias,
                function,
                args,
            } => Expr::Call {
                alias: alias.clone(),
                function,
                args: args.iter().map(|a| a.at_next_row(name)).collect(),
            },
            Expr::OrNull { expr, plane } => Expr::OrNull {
                expr: Box::new(expr.at_next_row(name)),
                plane: *plane,
            },
            other => other.clone(),
        }
    }
}

/// The `Encodings` field that holds the theme's null colour, present
/// when any channel is an [`Expr::OrNull`].
pub(crate) const NULL_COLOR_FIELD: &str = "null_color";

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
    "valid_row",
    "validity_next",
    "next_row",
    "next_group",
    "next_bit",
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
    // A polyline's next-row reads of its position channels: the member
    // (`<channel>_next`) and its expression, and the vertex columns,
    // chunk bases and key checks they add.
    let mut next_exprs = Vec::new();
    let mut successors = Vec::new();
    let mut next_bases = Vec::new();
    let mut next_keys = Vec::new();
    let position = |ch: &GlueChannel<'_>| matches!(ch.role, Some(Role::X | Role::Y));
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
                if spec.polyline && position(ch) {
                    next_exprs.push((
                        format!("{}_next", ch.name),
                        Expr::Uniform(ch.name.to_string()),
                    ));
                }
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
                            next: false,
                        });
                        if spec.polyline && position(ch) && k == 0 {
                            next_bases.push(ChunkBase {
                                channel: i,
                                link: 0,
                                format: input_format,
                                member: format!("{field}_next_base"),
                                next: true,
                            });
                        }
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
                        if spec.polyline && position(ch) {
                            next_keys.push(format!("{}_next", ch.name));
                        }
                    }
                    None => {}
                }
                if spec.polyline && position(ch) {
                    successors.push(columns.len() - 1);
                    next_exprs.push((format!("{}_next", ch.name), expr.at_next_row(ch.name)));
                }
                exprs.push((ch.name, expr));
                let chain: Vec<String> = links.iter().map(|f| f.signature()).collect();
                signature_parts.push(format!("{}: {}", ch.name, chain.join(" then ")));
            }
        }
    }
    // A null colour input reads the theme's colour from a uniform field,
    // so the source never depends on the colour.
    if exprs.iter().any(|(_, e)| matches!(e, Expr::OrNull { .. })) {
        fields.push((NULL_COLOR_FIELD.to_string(), "vec4<f32>".to_string(), 16));
    }
    // Validity bits only when the columns have a null (and only numeric
    // columns have bits).
    let validity = spec.validity && planes > 0;
    // The next-row bases follow every other base in `Chunk`.
    relative.extend(next_bases);
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
    // A polyline's draw may start mid-chunk (a bridge between chunks), so
    // the validity row of instance 0 in binding 1, and of its next row in
    // binding 2, are uniforms.
    if validity && spec.polyline {
        for member in ["valid_offset", "next_offset"] {
            let _ = writeln!(w, "    {member}: u32,");
            chunk.members.push((member.to_string(), chunk.span));
            chunk.span += 4;
        }
    }
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
        if spec.polyline {
            let _ = writeln!(
                w,
                "@group(2) @binding(2) var<storage, read> validity_next: array<u32>;"
            );
        }
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
        for (j, &c) in successors.iter().enumerate() {
            let (i, format) = columns[c];
            let _ = writeln!(
                w,
                "    @location({}) {}_next: {},",
                columns.len() + j,
                spec.channels[i].name,
                format.wgsl_type()
            );
        }
        let _ = writeln!(w, "}}");
        ", col: Columns"
    };

    let head_end = w.len();
    let _ = writeln!(
        w,
        "\n@vertex\nfn vs_main(@builtin(vertex_index) vertex_index: u32, \
         @builtin(instance_index) instance_index: u32{col_param}) -> {mark_alias}::Varyings {{"
    );
    let body_start = w.len();
    if validity && spec.polyline {
        // A polyline reads this row's bits and the next row's, each at an
        // offset from the draw's first instance (a bridge between chunks
        // reads the last row of one chunk and the first of the next).
        let _ = writeln!(
            w,
            "    let valid_row = instance_index + chunk.valid_offset;\n    \
             let valid_group = (valid_row / 32u) * {planes}u;\n    \
             let valid_bit = valid_row % 32u;\n    \
             let next_row = instance_index + chunk.next_offset;\n    \
             let next_group = (next_row / 32u) * {planes}u;\n    \
             let next_bit = next_row % 32u;"
        );
    } else if validity {
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
    for (name, expr) in &next_exprs {
        let _ = writeln!(w, "    m.{name} = {expr};");
    }
    let body_end = w.len();
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
        if spec.polyline {
            // A null on the next row hides the segment too, so a null row
            // breaks the line.
            let words = geometry_planes
                .iter()
                .map(|p| format!("validity_next[next_group + {p}u]"))
                .collect::<Vec<_>>()
                .join(" & ");
            drawn.push(format!("(({words}) >> next_bit) & 1u"));
        }
    }
    for name in &geometry_keys {
        drawn.push(format!("u32(col.{name} != {NULL_CODE}u)"));
    }
    for name in &next_keys {
        drawn.push(format!("u32(col.{name} != {NULL_CODE}u)"));
    }
    let drawn = (!drawn.is_empty()).then(|| drawn.join(" & "));
    if let Some(drawn) = &drawn {
        // A null position or size: every corner at one point outside the
        // clip volume, a degenerate quad that is never rasterised. The mark
        // contract names the position member `clip`.
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
    let pick = spec.gpu_pick.then(|| {
        pick_source(
            &s[..head_end],
            &s[body_start..body_end],
            &vertex,
            drawn.as_deref(),
            &mark_alias,
            col_param,
        )
    });

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
        successors,
        polyline: spec.polyline,
        validity,
        relative,
        luts,
        encodings,
        chunk,
        pick,
    }
}

/// The pick variant of a glue module (RFC-001 S9d): `head` (imports,
/// structs, bindings) and the vertex `body` of `vs_main`, with entry points
/// that draw into a one-texel `Rg32Uint` target whose centre is the
/// pointer. `vs_pick` moves the pointer to the texel's centre, scaling one
/// logical pixel to one texel; `fs_pick` writes `(layer + 1, row + 1)` and
/// a depth that encodes the mark's signed distance, so a `LessEqual` test
/// in draw order keeps the CPU pick's winner.
fn pick_source(
    head: &str,
    body: &str,
    vertex: &str,
    drawn: Option<&str>,
    mark_alias: &str,
    col_param: &str,
) -> String {
    let tol = crate::pick::PICK_TOLERANCE.0;
    let mut s = String::from(head);
    let w = &mut s;
    let _ = writeln!(
        w,
        "\n// The pick: the pointer in clip space and the layer's index.\n\
         struct Pick {{\n    at: vec2<f32>,\n    layer: u32,\n    pad: u32,\n}}\n\n\
         @group(3) @binding(0) var<uniform> u_pick: Pick;\n\n\
         struct PickOut {{\n    @location(0) id: vec2<u32>,\n    \
         @builtin(frag_depth) depth: f32,\n}}"
    );
    let _ = writeln!(
        w,
        "\n@vertex\nfn vs_pick(@builtin(vertex_index) vertex_index: u32, \
         @builtin(instance_index) instance_index: u32{col_param}) -> {mark_alias}::Varyings {{"
    );
    w.push_str(body);
    let _ = writeln!(w, "    var out = {vertex};");
    if let Some(drawn) = drawn {
        let _ = writeln!(
            w,
            "    let drawn = {drawn};\n    \
             out.clip = select(vec4<f32>(2.0, 2.0, 2.0, 1.0), out.clip, drawn == 1u);"
        );
    }
    let _ = writeln!(
        w,
        "    out.clip = vec4<f32>((out.clip.xy - u_pick.at * out.clip.w) * u_view.size, \
         out.clip.zw);\n    return out;\n}}"
    );
    let _ = writeln!(
        w,
        "\n@fragment\nfn fs_pick(v: {mark_alias}::Varyings) -> PickOut {{\n    \
         let d = {mark_alias}::hit_distance(v);\n    \
         if (!(d <= {tol:?})) {{\n        discard;\n    }}\n    \
         var out: PickOut;\n    \
         out.id = vec2<u32>(u_pick.layer + 1u, {mark_alias}::hit_row(v) + 1u);\n    \
         out.depth = select(0.5 + 0.5 * d / {tol:?}, 0.0, d <= 0.0);\n    \
         return out;\n}}"
    );
    s
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
