// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Marks. S0a has one: [`Circle`]. Rect, Rule and Segment/Line are RFC-001 S6.

use crate::channel::{Color, Px};

/// A filled disc centred on `(X, Y)`, sized in logical pixels so it stays
/// round on any aspect ratio.
///
/// The `#[derive(Mark)]` generates the channel constants
/// ([`Circle::X`], [`Circle::Y`], [`Circle::RADIUS`], [`Circle::FILL`]), the
/// `Mark` impl and the [`CircleAttrs`] setters, which the prelude brings
/// into scope:
///
/// ```
/// use gup_core::prelude::*;
///
/// # fn main() -> gup_core::Result<()> {
/// struct Row {
///     x: f64,
///     y: f64,
/// }
/// let rows = vec![Row { x: 1.0, y: 2.0 }, Row { x: 3.0, y: 1.0 }];
///
/// let x = Linear::new().domain(0.0, 4.0);
/// let y = Linear::new().domain(0.0, 4.0);
/// let mut points = Selection::<Row, Circle>::new(rows);
/// points
///     .x(x.encode(|r: &Row| r.x))
///     .y(y.encode(|r: &Row| r.y))
///     .radius(Px(4.0))
///     .fill(Color::hex(0xd55e00));
/// # let _ = points;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, gup_core_macros::Mark)]
#[mark(module = "gup::marks::circle", geometry = Quad)]
pub struct Circle {
    /// Centre x (logical pixels).
    #[channel(role = X)]
    x: Px,
    /// Centre y (logical pixels, y down).
    #[channel(role = Y)]
    y: Px,
    /// Radius (logical pixels). Default 3.5.
    #[channel(role = Size, default = Px(3.5))]
    radius: Px,
    /// Fill colour. Default Okabe-Ito blue.
    #[channel(role = Color, default = Circle::DEFAULT_FILL)]
    fill: Color,
}

impl Circle {
    /// The default fill (`#0072B2`).
    pub const DEFAULT_FILL: Color = Color::hex(0x0072b2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::{Antialias, ConstValue, Mark, Role};
    use crate::shader::MARKS_CIRCLE;

    /// The derive reproduces the table Circle had when it was hand-written.
    #[test]
    fn derive_reproduces_the_hand_written_table() {
        let expected = [
            ("x", Role::X, "f32", ConstValue::F32(0.0)),
            ("y", Role::Y, "f32", ConstValue::F32(0.0)),
            ("radius", Role::Size, "f32", ConstValue::F32(3.5)),
            (
                "fill",
                Role::Color,
                "vec4<f32>",
                ConstValue::Vec4(Color::hex(0x0072b2).to_array()),
            ),
        ];
        assert_eq!(Circle::CHANNELS.len(), expected.len());
        for (desc, (name, role, wgsl_type, default)) in Circle::CHANNELS.iter().zip(expected) {
            assert_eq!(desc.name, name);
            assert_eq!(desc.role, Some(role));
            assert_eq!(desc.wgsl_type, wgsl_type);
            assert_eq!(desc.default, default);
        }
        assert_eq!(Circle::NAME, "Circle");
        assert_eq!(Circle::VERTICES_PER_INSTANCE, 6);
        assert_eq!(Circle::ANTIALIAS, Antialias::Analytic);
    }

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
        use crate::shader::testing::{parse, struct_layout};
        let module = parse(
            "circle",
            &crate::shader::link("circle", "#import gup::marks::circle\n", &[&MARKS_CIRCLE])
                .unwrap(),
        );
        let layout = struct_layout(&module, "gup_marks_circle_CircleIn").expect("CircleIn");
        let names: Vec<_> = layout.members.iter().map(|(n, _)| n.clone()).collect();
        let channels: Vec<_> = Circle::CHANNELS.iter().map(|c| c.name).collect();
        assert_eq!(names, channels);
    }
}
