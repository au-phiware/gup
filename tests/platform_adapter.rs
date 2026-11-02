// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Prints the adapter the integration tests draw on (GUP-590, GUP-591).
//!
//! The Platforms workflow runs this first, so a failure in a later golden
//! test can be read against the adapter that produced it. With
//! `WGPU_BACKEND=dx12` (or `metal`) it also asserts that wgpu picked that
//! backend; on Metal it also rejects a software adapter. Without the
//! variable it only prints.

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
    let expected =
        if requested.eq_ignore_ascii_case("dx12") || requested.eq_ignore_ascii_case("d3d12") {
            Some("Dx12")
        } else if requested.eq_ignore_ascii_case("metal") {
            Some("Metal")
        } else {
            None
        };
    if let Some(expected) = expected {
        assert_eq!(
            format!("{:?}", info.backend),
            expected,
            "WGPU_BACKEND={requested} but the adapter is {info:?}"
        );
        if expected == "Metal" {
            assert_ne!(
                format!("{:?}", info.device_type),
                "Cpu",
                "software Metal adapter: {info:?}"
            );
        }
    }
}
