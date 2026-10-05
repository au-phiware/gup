// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Marks. S0a has one: [`Circle`]. Rect, Rule and Segment/Line and the
//! `#[derive(Mark)]` generator are RFC-001 S6.

use crate::channel::{Channel, ChannelDesc, Color, ConstValue, Mark, Px, Role};
use crate::shader::{MARK_CIRCLE, WgslModule};

/// A filled disc centred on `(X, Y)`, sized in logical pixels so it stays
/// round on any aspect ratio.
#[derive(Debug)]
pub enum Circle {}

impl Circle {
    /// Centre x (logical pixels).
    pub const X: Channel<Circle, Px> = Channel::new(0, "x");
    /// Centre y (logical pixels, y down).
    pub const Y: Channel<Circle, Px> = Channel::new(1, "y");
    /// Radius (logical pixels). Default 3.5.
    pub const RADIUS: Channel<Circle, Px> = Channel::new(2, "radius");
    /// Fill colour. Default Okabe-Ito blue.
    pub const FILL: Channel<Circle, Color> = Channel::new(3, "fill");

    /// The default fill (`#0072B2`).
    pub const DEFAULT_FILL: Color = Color::hex(0x0072b2);
}

impl Mark for Circle {
    const NAME: &'static str = "Circle";
    const MODULE: &'static WgslModule = &MARK_CIRCLE;
    const CHANNELS: &'static [ChannelDesc] = &[
        ChannelDesc {
            name: "x",
            role: Some(Role::X),
            wgsl_type: "f32",
            default: ConstValue::F32(0.0),
        },
        ChannelDesc {
            name: "y",
            role: Some(Role::Y),
            wgsl_type: "f32",
            default: ConstValue::F32(0.0),
        },
        ChannelDesc {
            name: "radius",
            role: Some(Role::Size),
            wgsl_type: "f32",
            default: ConstValue::F32(3.5),
        },
        ChannelDesc {
            name: "fill",
            role: Some(Role::Color),
            wgsl_type: "vec4<f32>",
            default: ConstValue::Vec4(Circle::DEFAULT_FILL.to_array()),
        },
    ];
    const VERTICES_PER_INSTANCE: u32 = 6;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_consts_index_the_descriptor_table() {
        let slots = [
            (Circle::X.slot(), Circle::X.name()),
            (Circle::Y.slot(), Circle::Y.name()),
            (Circle::RADIUS.slot(), Circle::RADIUS.name()),
            (Circle::FILL.slot(), Circle::FILL.name()),
        ];
        for (slot, name) in slots {
            assert_eq!(Circle::CHANNELS[slot as usize].name, name);
        }
    }

    /// The WGSL `CircleIn` struct has exactly the channels, in order.
    #[test]
    fn wgsl_input_struct_matches_channels() {
        let mut lib = crate::shader::ShaderLibrary::new();
        let composed = lib
            .compose(
                "circle_in_probe",
                "#import gup::marks::circle as circle\n\
                 @vertex fn main() -> @builtin(position) vec4<f32> {\n\
                 var m: circle::CircleIn; return vec4<f32>(m.x, m.y, m.radius, m.fill.a); }\n",
                &[],
            )
            .unwrap();
        let ty = composed
            .module
            .types
            .iter()
            .find_map(|(_, t)| match &t.inner {
                naga::TypeInner::Struct { members, .. }
                    if t.name.as_deref().is_some_and(|n| n.starts_with("CircleIn")) =>
                {
                    Some(members.clone())
                }
                _ => None,
            })
            .expect("CircleIn in composed module");
        let names: Vec<_> = ty.iter().map(|m| m.name.clone().unwrap()).collect();
        let channels: Vec<_> = Circle::CHANNELS.iter().map(|c| c.name).collect();
        assert_eq!(names, channels);
    }
}
