// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A channel of another mark does not unify with `Selection<_, Circle>`.
use gup_core::prelude::*;
use gup_core::{Channel, ChannelDesc, ConstValue, Mark, WgslModule};

/// A minimal second mark, defined only for this test.
enum Bar {}

static BAR_MODULE: WgslModule = WgslModule {
    import_path: "test::bar",
    source: "#define_import_path test::bar\n",
    imports: &[],
};

impl Bar {
    const WIDTH: Channel<Bar, Px> = Channel::new(0, "width");
}

impl Mark for Bar {
    const NAME: &'static str = "Bar";
    const MODULE: &'static WgslModule = &BAR_MODULE;
    const CHANNELS: &'static [ChannelDesc] = &[ChannelDesc {
        name: "width",
        role: None,
        wgsl_type: "f32",
        default: ConstValue::F32(1.0),
    }];
    const VERTICES_PER_INSTANCE: u32 = 6;
}

struct Row;

fn main() {
    let mut sel = Selection::<Row, Circle>::new(vec![Row]);
    sel.attr(Bar::WIDTH, Px(3.0));
}
