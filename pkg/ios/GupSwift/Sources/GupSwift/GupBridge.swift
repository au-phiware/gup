// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

/// C-ABI bridge to the Rust `gup-ios` static library.
///
/// These declarations must stay in sync with `gup-ios/src/lib.rs`.
/// The static library (`libgup_ios.a`) is linked via Xcode build settings.

import Foundation

// MARK: - Opaque Handle

/// Opaque pointer to the Rust-side `GupContext`.
public typealias GupContextHandle = OpaquePointer?

// MARK: - C ABI Declarations

/// Create a new Gup GPU context. Returns `nil` on failure.
@_silgen_name("gup_context_create")
public func gup_context_create() -> GupContextHandle

/// Destroy a context previously created with `gup_context_create`.
@_silgen_name("gup_context_destroy")
public func gup_context_destroy(_ handle: GupContextHandle)

/// Attach a CAMetalLayer-backed UIView to the context.
/// Returns a surface ID (> 0) on success, 0 on failure.
@_silgen_name("gup_surface_attach_layer")
public func gup_surface_attach_layer(
    _ handle: GupContextHandle,
    _ uiView: UnsafeMutableRawPointer,
    _ uiViewController: UnsafeMutableRawPointer?,
    _ width: UInt32,
    _ height: UInt32
) -> UInt64

/// Detach a surface from the context.
@_silgen_name("gup_surface_detach")
public func gup_surface_detach(_ handle: GupContextHandle, _ surfaceId: UInt64)

/// Render one frame to the given surface.
@_silgen_name("gup_render_frame")
public func gup_render_frame(_ handle: GupContextHandle, _ surfaceId: UInt64) -> Bool

/// Forward a UITouch event into the Gup event pipeline.
@_silgen_name("gup_touch_event")
public func gup_touch_event(
    _ handle: GupContextHandle,
    _ touchId: UInt64,
    _ x: Float,
    _ y: Float,
    _ phase: UInt8,
    _ scaleFactor: Float,
    _ timestamp: Double,
    _ viewWidth: Float,
    _ viewHeight: Float
)

/// Notify Gup of a drawable-size change (orientation change).
@_silgen_name("gup_surface_resize")
public func gup_surface_resize(
    _ handle: GupContextHandle,
    _ surfaceId: UInt64,
    _ newWidth: UInt32,
    _ newHeight: UInt32
) -> Bool
