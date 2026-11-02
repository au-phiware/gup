// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Prints the adapter the integration tests draw on (GUP-590).
//!
//! The Platforms workflow runs this first, so a failure in a later golden
//! test can be read against the adapter that produced it. With
//! `WGPU_BACKEND=dx12` it also asserts that wgpu picked the DX12 backend;
//! without the variable it only prints.

use gup::prelude::*;

#[test]
fn prints_the_adapter() {
    let cx = Context::shared().expect("a GPU context");
    let info = cx
        .adapter_info()
        .expect("the shared context owns its adapter");
    println!(
        "adapter: name={:?} backend={:?} device_type={:?} driver={:?} driver_info={:?}",
        info.name, info.backend, info.device_type, info.driver, info.driver_info
    );
    let requested = std::env::var("WGPU_BACKEND").unwrap_or_default();
    if requested.eq_ignore_ascii_case("dx12") || requested.eq_ignore_ascii_case("d3d12") {
        assert_eq!(
            format!("{:?}", info.backend),
            "Dx12",
            "WGPU_BACKEND={requested} but the adapter is {info:?}"
        );
    }
}
