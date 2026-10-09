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

impl Visual for Px {
    fn to_const(self) -> ConstValue {
        ConstValue::F32(self.0)
    }
}

/// An sRGB-encoded colour with straight (non-premultiplied) alpha, each
/// component in `0.0..=1.0`. Gup blends in sRGB space (RFC-001 §7).
#[derive(Copy, Clone, Debug, PartialEq, Default)]
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

impl Visual for Color {
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

/// A mark: a WGSL module implementing the mark contract plus its channels.
///
/// The contract (see `shaders/circle.wgsl`): the module defines
/// `<NAME>In` with one field per channel, `Varyings`, a
/// `vertex(m: <NAME>In, vertex_index: u32, row: u32, view: View) -> Varyings`
/// function and a `shade(v: Varyings) -> vec4<f32>` function returning
/// premultiplied sRGB colour. `Varyings`' `@builtin(position)` member is
/// named `clip`: for a row with a null position or size, the glue moves
/// every vertex to one point outside the clip volume, a degenerate quad
/// (RFC-001 S4b). Hand-written for now; RFC-001 S6 generates this impl
/// with `#[derive(Mark)]`, which can check the member.
pub trait Mark: Send + Sync + 'static {
    /// Mark name, also the prefix of its WGSL input struct (`CircleIn`).
    const NAME: &'static str;
    /// The WGSL module implementing the mark contract.
    const MODULE: &'static WgslModule;
    /// Channels, indexed by [`Channel::slot`].
    const CHANNELS: &'static [ChannelDesc];
    /// Vertices drawn per instance.
    const VERTICES_PER_INSTANCE: u32;
}
