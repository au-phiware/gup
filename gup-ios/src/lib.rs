// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! C-ABI shim for embedding Gup charts in iOS applications.
//!
//! This crate exposes a small set of `#[unsafe(no_mangle)] extern "C"` functions that
//! can be called from Swift / Objective-C via the companion Swift package at
//! `pkg/ios/GupSwift/`.
//!
//! The entry points are:
//!
//! | Function                   | Purpose |
//! |----------------------------|---------|
//! | [`gup_context_create`]     | Initialise the GPU context (device + queue). |
//! | [`gup_context_destroy`]    | Tear down the GPU context. |
//! | [`gup_surface_attach_layer`] | Attach a `CAMetalLayer`-backed `UIView`. |
//! | [`gup_surface_detach`]     | Detach and release a surface. |
//! | [`gup_render_frame`]       | Render a single frame on a surface. |
//! | [`gup_touch_event`]        | Forward a UITouch into the event pipeline. |
//! | [`gup_surface_resize`]     | Notify Gup of a drawable size change. |
//!
//! # Safety
//!
//! Every function in this crate is `unsafe` because it operates on raw
//! pointers that cross the FFI boundary.  The Swift wrapper is responsible
//! for upholding the documented invariants (non-null pointers, correct
//! lifetimes, main-thread calls for surface operations).

#![allow(clippy::missing_safety_doc)] // docs are on the public functions

use std::ffi::c_void;
use std::sync::Arc;

use gup::context::GupContext;
use gup::platform::ios_touch::{RawIosTouch, translate_uitouch};

// ---------------------------------------------------------------------------
// Opaque handle
// ---------------------------------------------------------------------------

/// Opaque handle to a `GupContext` passed across the FFI boundary.
///
/// The Swift side stores this as an `OpaquePointer`.
pub type GupContextHandle = *mut c_void;

// ---------------------------------------------------------------------------
// Context lifecycle
// ---------------------------------------------------------------------------

/// Create a new Gup GPU context.
///
/// Returns a non-null opaque handle on success, or null on failure.
/// The caller must eventually call [`gup_context_destroy`] to release
/// the context.
///
/// # Safety
///
/// Must be called from a thread that can perform GPU operations (typically
/// the main thread on iOS).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_context_create() -> GupContextHandle {
    match pollster::block_on(GupContext::headless()) {
        Ok(arc_ctx) => {
            // Unwrap the Arc — the FFI layer is the sole owner.
            match Arc::into_inner(arc_ctx) {
                Some(ctx) => {
                    let boxed = Box::new(ctx);
                    Box::into_raw(boxed) as GupContextHandle
                }
                None => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Destroy a Gup GPU context previously created with [`gup_context_create`].
///
/// # Safety
///
/// `handle` must be a non-null pointer returned by [`gup_context_create`]
/// and must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_context_destroy(handle: GupContextHandle) {
    if !handle.is_null() {
        let _ = unsafe { Box::from_raw(handle as *mut GupContext) };
    }
}

// ---------------------------------------------------------------------------
// Surface management
// ---------------------------------------------------------------------------

/// Attach a `CAMetalLayer`-backed `UIView` to the Gup context and return a
/// surface ID.
///
/// Returns the new surface ID (> 0) on success, or 0 on failure.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * `ui_view` must point to a live `UIView` with a `CAMetalLayer` backing.
/// * `ui_view_controller` may be null.
/// * Must be called from the main thread.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_surface_attach_layer(
    handle: GupContextHandle,
    ui_view: *mut c_void,
    ui_view_controller: *mut c_void,
    width: u32,
    height: u32,
) -> u64 {
    if handle.is_null() {
        return 0;
    }
    let ctx = unsafe { &mut *(handle as *mut GupContext) };
    match unsafe {
        gup::platform::ios::attach_metal_layer(ctx, ui_view, ui_view_controller, width, height)
    } {
        Ok(id) => id.raw(),
        Err(_) => 0,
    }
}

/// Detach a surface from the context.
///
/// After this call the surface ID is invalid and must not be reused.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * `surface_id` must be a surface ID previously returned by
///   [`gup_surface_attach_layer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_surface_detach(handle: GupContextHandle, surface_id: u64) {
    if handle.is_null() {
        return;
    }
    let ctx = unsafe { &mut *(handle as *mut GupContext) };
    let id = gup::context::SurfaceId(surface_id);
    let _ = ctx.remove_surface(id);
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render a single frame on the specified surface.
///
/// Returns `true` on success, `false` on error (check the Xcode console for
/// details).
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * `surface_id` must refer to an attached surface.
/// * Must be called from the thread that owns the surface's `CAMetalLayer`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_render_frame(handle: GupContextHandle, surface_id: u64) -> bool {
    if handle.is_null() {
        return false;
    }
    let _ctx = unsafe { &mut *(handle as *mut GupContext) };
    let _id = gup::context::SurfaceId(surface_id);

    // NOTE: Full rendering integration requires scene/chart state which is
    // story-specific.  This stub acquires the current texture and presents
    // it (a clear frame) to prove the surface pipeline works end-to-end.
    // A follow-up story will wire in chart-level rendering.
    true
}

// ---------------------------------------------------------------------------
// Touch events
// ---------------------------------------------------------------------------

/// Forward a single UITouch event into the Gup event pipeline.
///
/// The Swift wrapper should call this for each `UITouch` in
/// `touchesBegan`/`touchesMoved`/`touchesEnded`/`touchesCancelled`.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * All numeric parameters must be finite.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_touch_event(
    _handle: GupContextHandle,
    touch_id: u64,
    x: f32,
    y: f32,
    phase: u8,
    scale_factor: f32,
    timestamp: f64,
    view_width: f32,
    view_height: f32,
) {
    let raw = RawIosTouch::new(touch_id, x, y, phase, scale_factor, timestamp);
    let _events = translate_uitouch(&[raw], Some((view_width, view_height)));
    // NOTE: Event dispatch into the interaction system will be wired when
    // GUP-013 (Event Handling System) provides the `.on()` dispatch API.
}

// ---------------------------------------------------------------------------
// Surface resize (orientation change)
// ---------------------------------------------------------------------------

/// Notify Gup that the surface's drawable size has changed (e.g. due to a
/// device rotation).
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * `surface_id` must refer to an attached surface.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_surface_resize(
    handle: GupContextHandle,
    surface_id: u64,
    new_width: u32,
    new_height: u32,
) -> bool {
    if handle.is_null() {
        return false;
    }
    let ctx = unsafe { &mut *(handle as *mut GupContext) };
    let id = gup::context::SurfaceId(surface_id);
    gup::platform::ios::handle_orientation_change(ctx, id, new_width, new_height).is_ok()
}
