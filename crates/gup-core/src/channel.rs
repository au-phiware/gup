// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed channels and visual value types (RFC-001 §4).
//!
//! A [`Channel<M, V>`] names one input of mark `M` that consumes a visual
//! value of type `V` ([`Px`], [`Color`], …). Channels are associated
//! constants on the mark (`Circle::RADIUS`), so a typo is "no associated
//! item", a channel of another mark does not unify, and a value of the wrong
//! visual type does not satisfy [`IntoEncoding`](crate::IntoEncoding): every
//! mistake is a compile error.

use crate::shader::WgslModule;
use std::marker::PhantomData;

/// One channel (input) of mark `M`, consuming visual values of type `V`.
pub struct Channel<M, V> {
    slot: u8,
    name: &'static str,
    _p: PhantomData<fn() -> (M, V)>,
}

impl<M, V> Channel<M, V> {
    /// Define channel `slot` (an index into [`Mark::CHANNELS`]) called
    /// `name`. Mark definitions call this for their associated constants.
    pub const fn new(slot: u8, name: &'static str) -> Self {
        Self {
            slot,
            name,
            _p: PhantomData,
        }
    }

    /// The index of this channel in [`Mark::CHANNELS`].
    pub const fn slot(&self) -> u8 {
        self.slot
    }

    /// The channel's name (also its WGSL field name).
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl<M, V> Clone for Channel<M, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M, V> Copy for Channel<M, V> {}

impl<M, V> std::fmt::Debug for Channel<M, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Channel({}#{})", self.name, self.slot)
    }
}

/// A type that exists on the GPU: it has a WGSL spelling and a CPU-side
/// representation that CPU mirrors produce.
pub trait GpuType: 'static {
    /// The WGSL type name.
    const WGSL: &'static str;
    /// The CPU-side (f64-precision where it matters) representation.
    type Cpu;
}

impl GpuType for f32 {
    const WGSL: &'static str = "f32";
    type Cpu = f64;
}

/// Dictionary codes ([`ColumnFormat::U32`](crate::column::ColumnFormat::U32)).
impl GpuType for u32 {
    const WGSL: &'static str = "u32";
    type Cpu = u32;
}

/// A post-encoding value that a mark consumes. Constants of a visual type
/// become uniform fields rather than columns.
pub trait Visual: GpuType + Copy + Send + Sync {
    /// The zero value as uniform data: the default of a channel that
    /// `#[derive(Mark)]` gives no `default`. An associated constant because
    /// trait methods cannot be called in `const` context.
    const ZERO: ConstValue;

    /// The value as uniform data.
    fn to_const(self) -> ConstValue;
}

/// Logical pixels. `Ndc` exists only inside the `gup::view` WGSL module.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Default)]
pub struct Px(pub f32);

impl GpuType for Px {
    const WGSL: &'static str = "f32";
    type Cpu = f64;
}

impl Px {
    /// The value as uniform data, in `const` context (what
    /// `#[derive(Mark)]` calls for a channel default).
    pub const fn const_value(self) -> ConstValue {
        ConstValue::F32(self.0)
    }
}

impl Visual for Px {
    const ZERO: ConstValue = ConstValue::F32(0.0);

    fn to_const(self) -> ConstValue {
        ConstValue::F32(self.0)
    }
}

/// A dimensionless fraction, typically `0.0..=1.0` (an opacity).
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Default)]
pub struct Unit(pub f32);

impl GpuType for Unit {
    const WGSL: &'static str = "f32";
    type Cpu = f64;
}

impl Unit {
    /// The value as uniform data, in `const` context.
    pub const fn const_value(self) -> ConstValue {
        ConstValue::F32(self.0)
    }
}

impl Visual for Unit {
    const ZERO: ConstValue = ConstValue::F32(0.0);

    fn to_const(self) -> ConstValue {
        ConstValue::F32(self.0)
    }
}

/// An sRGB-encoded colour with straight (non-premultiplied) alpha, each
/// component in `0.0..=1.0`. Gup blends in sRGB space (RFC-001 §7).
///
/// In a uniform it is a WGSL `vec4<f32>` (`[r, g, b, a]`), so a shader
/// function's `Params` may hold colours.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Color {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Alpha.
    pub a: f32,
}

impl Color {
    /// Opaque black.
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    /// Opaque white.
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    /// Fully transparent.
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// An opaque colour from sRGB components in `0.0..=1.0`.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// An opaque colour from a `0xRRGGBB` literal.
    pub const fn hex(rgb: u32) -> Self {
        Self::rgb(
            ((rgb >> 16) & 0xff) as f32 / 255.0,
            ((rgb >> 8) & 0xff) as f32 / 255.0,
            (rgb & 0xff) as f32 / 255.0,
        )
    }

    /// The colour as `[r, g, b, a]`.
    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// The colour as 8-bit sRGB `[r, g, b, a]`, rounded to nearest.
    pub fn to_rgba8(self) -> [u8; 4] {
        self.to_array()
            .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    /// Premultiplied `[r·a, g·a, b·a, a]`, as GPU targets store it.
    pub(crate) fn premultiplied(self) -> [f32; 4] {
        [self.r * self.a, self.g * self.a, self.b * self.a, self.a]
    }
}

impl GpuType for Color {
    const WGSL: &'static str = "vec4<f32>";
    type Cpu = Color;
}

impl Color {
    /// The colour as uniform data, in `const` context (what
    /// `#[derive(Mark)]` calls for a channel default).
    pub const fn const_value(self) -> ConstValue {
        ConstValue::Vec4(self.to_array())
    }
}

impl Visual for Color {
    const ZERO: ConstValue = ConstValue::Vec4([0.0; 4]);

    fn to_const(self) -> ConstValue {
        ConstValue::Vec4(self.to_array())
    }
}

/// A constant channel value as uniform data.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ConstValue {
    /// A WGSL `f32`.
    F32(f32),
    /// A WGSL `vec4<f32>`.
    Vec4([f32; 4]),
}

impl ConstValue {
    /// The value's bytes, as laid out in a uniform buffer.
    pub(crate) fn bytes(&self) -> Vec<u8> {
        match self {
            ConstValue::F32(v) => v.to_le_bytes().to_vec(),
            ConstValue::Vec4(v) => bytemuck::cast_slice(v).to_vec(),
        }
    }
}

/// Which shared role a channel plays in a plot: roles tell the plot which
/// scale slot a channel joins and which range it gets (RFC-001 §4).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Horizontal position: joins the plot's x scale.
    X,
    /// Vertical position: joins the plot's y scale.
    Y,
    /// Mark size.
    Size,
    /// Colour.
    Color,
}

/// Static description of one mark channel.
#[derive(Copy, Clone, Debug)]
pub struct ChannelDesc {
    /// Field name in the mark's WGSL input struct.
    pub name: &'static str,
    /// Plot role, if any.
    pub role: Option<Role>,
    /// WGSL type of the visual value.
    pub wgsl_type: &'static str,
    /// The value used when the channel is not set.
    pub default: ConstValue,
}

/// How a mark's edges are antialiased (RFC-001 §7).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Antialias {
    /// The mark's shader computes its own edge coverage (a signed distance
    /// or the box coverage of `gup::coverage`), so it looks the same at
    /// any sample count. Points, lines and rects.
    #[default]
    Analytic,
    /// The mark relies on MSAA for its edges: a [`Scene`](crate::Scene)
    /// holding it is drawn with 4 samples per pixel on any Gup-owned
    /// target, whatever the target's own count
    /// ([`Scene::min_samples`](crate::Scene::min_samples)). Areas and
    /// polygons, whose edges are arbitrary triangles.
    Msaa,
}

/// How a mark's instances map to rows (RFC-001 §3, GUP-432).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Geometry {
    /// One quad per row: points, rects, segments.
    #[default]
    Quad,
    /// One quad per pair of consecutive rows: a polyline, `N - 1` instances
    /// for `N` rows. Each position (`X`, `Y` role) channel is read twice, at
    /// the instance's row and at the next row: the mark's input struct ends
    /// with one `<channel>_next` member per position channel, in channel
    /// order, after the channels. Other channels are read at the instance's
    /// row only. A null position on either row hides the instance, so a
    /// null row breaks the line. Rows always draw in data order: a
    /// polyline never sorts by size (GUP-431).
    Polyline,
}

/// How CPU picking hit-tests a mark's rows (RFC-001 §9).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum HitShape {
    /// Never picked on the CPU (the default for user marks).
    #[default]
    Unpickable,
    /// A disc at (`x`, `y`) of radius `radius`.
    Disc,
    /// An axis-aligned box: per axis, centre and extent (`width`, `height`)
    /// when the extent is non-zero, else the edges `x`..`x_end` (`y`..`y_end`).
    AlignedBox,
    /// `(x, y)`–`(x_end, y_end)`, `width` wide, square caps.
    Segment,
    /// Capsules from each row to the next, `width` wide (`Geometry::Polyline`).
    Polyline,
}

/// A mark: a WGSL module implementing the mark contract plus its channels.
///
/// The contract (see `crates/gup-wgsl/library/circle.wgsl`): the module defines
/// `<NAME>In` with one field per channel, `Varyings`, a
/// `vertex(m: <NAME>In, vertex_index: u32, row: u32, view: View) -> Varyings`
/// function and a `shade(v: Varyings) -> vec4<f32>` function returning
/// premultiplied sRGB colour. `Varyings`' `@builtin(position)` member is
/// named `clip`: for a row with a null position or size, the glue moves
/// every vertex to one point outside the clip volume, a degenerate quad
/// (RFC-001 S4b). Two things hide a row: a null numeric non-colour channel
/// (validity bits, RFC-001 S4b) and a key-encoded non-colour channel equal
/// to `NULL_CODE` (RFC-001 S5b; the `geometry_keys` loop in `glue.rs`).
/// Either way every vertex moves to the same point outside the clip volume.
///
/// Implement it with `#[derive(Mark)]` (RFC-001 S6), which also checks the
/// module's input struct against the channels at compile time.
pub trait Mark: Send + Sync + 'static {
    /// Mark name, also the prefix of its WGSL input struct (`CircleIn`).
    const NAME: &'static str;
    /// The WGSL module implementing the mark contract.
    const MODULE: &'static WgslModule;
    /// Channels, indexed by [`Channel::slot`].
    const CHANNELS: &'static [ChannelDesc];
    /// Vertices drawn per instance.
    const VERTICES_PER_INSTANCE: u32;
    /// How the mark's edges are antialiased.
    const ANTIALIAS: Antialias = Antialias::Analytic;
    /// How instances map to rows.
    const GEOMETRY: Geometry = Geometry::Quad;
    /// How CPU picking hit-tests the mark's rows.
    const HIT: HitShape = HitShape::Unpickable;
    /// Whether the GPU pick ([`Plot::request_pick`](crate::Plot::request_pick))
    /// draws the mark: its module defines `hit_distance` and `hit_row`
    /// (`#[mark(.., gpu_pick)]`). Layers of other marks are skipped there.
    const GPU_PICK: bool = false;
}

/// `[r, g, b, a]`: a WGSL `vec4<f32>` in a uniform.
impl AsRef<[f32; 4]> for Color {
    fn as_ref(&self) -> &[f32; 4] {
        bytemuck::cast_ref(self)
    }
}

impl AsMut<[f32; 4]> for Color {
    fn as_mut(&mut self) -> &mut [f32; 4] {
        bytemuck::cast_mut(self)
    }
}

impl From<[f32; 4]> for Color {
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Self { r, g, b, a }
    }
}

encase::impl_vector!(4, Color, f32; using AsRef AsMut From);
