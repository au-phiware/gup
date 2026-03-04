// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! JNI/NDK shim for embedding Gup charts in Android applications.
//!
//! This crate exposes a set of `#[unsafe(no_mangle)] extern "C"` functions
//! that form the native side of the JNI bridge.  The companion Kotlin
//! wrapper at `pkg/android/GupKotlin/` declares matching `external fun`
//! bindings.
//!
//! The entry points mirror the Android `SurfaceHolder.Callback` lifecycle:
//!
//! | Function                         | Purpose                                    |
//! |----------------------------------|--------------------------------------------|
//! | [`gup_context_create`]           | Initialise the GPU context (device + queue).|
//! | [`gup_context_destroy`]          | Tear down the GPU context.                 |
//! | [`gup_surface_created`]          | Attach an `ANativeWindow` surface.         |
//! | [`gup_surface_changed`]          | Handle swapchain resize.                   |
//! | [`gup_surface_destroyed`]        | Detach and release the surface.            |
//! | [`gup_render_frame`]             | Render a single frame on a surface.        |
//! | [`gup_on_touch_event`]           | Forward a `MotionEvent` pointer.           |
//! | [`gup_pause`]                    | Notify that the Activity is pausing.       |
//! | [`gup_resume`]                   | Notify that the Activity is resuming.      |
//!
//! # Safety
//!
//! Every function in this crate is `unsafe` because it operates on raw
//! pointers that cross the FFI boundary.  The Kotlin wrapper is responsible
//! for upholding the documented invariants (non-null pointers, correct
//! lifetimes, render-thread calls for surface operations).
//!
//! # Panic safety
//!
//! Each JNI entry point wraps its body in [`std::panic::catch_unwind`] so
//! that a Rust panic does not unwind across the JNI boundary (which would
//! abort the process).  Caught panics are logged via `eprintln!` and the
//! function returns a safe default value.

#![allow(clippy::missing_safety_doc)] // docs are on the public functions

use std::ffi::c_void;
use std::panic;
use std::sync::Arc;

use gup::context::GupContext;
use gup::platform::android_touch::{RawAndroidTouch, translate_motion_event};

// ---------------------------------------------------------------------------
// Opaque handle
// ---------------------------------------------------------------------------

/// Opaque handle to a `GupContext` passed across the JNI boundary.
///
/// The Kotlin side stores this as a `Long` (pointer-sized).
pub type GupContextHandle = *mut c_void;

// ---------------------------------------------------------------------------
// Panic-catching wrapper
// ---------------------------------------------------------------------------

/// Run `f` inside [`catch_unwind`](panic::catch_unwind).  On panic, log
/// the message and return `default`.
fn catch<T, F: FnOnce() -> T + panic::UnwindSafe>(default: T, f: F) -> T {
    match panic::catch_unwind(f) {
        Ok(val) => val,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            eprintln!("[gup-android] panic caught at JNI boundary: {msg}");
            default
        }
    }
}

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
/// May be called from any thread, but the returned handle must only be
/// used from a single thread at a time (or protected by external
/// synchronisation).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_context_create() -> GupContextHandle {
    catch(std::ptr::null_mut(), || {
        match pollster::block_on(GupContext::headless()) {
            Ok(arc_ctx) => match Arc::into_inner(arc_ctx) {
                Some(ctx) => {
                    let boxed = Box::new(ctx);
                    Box::into_raw(boxed) as GupContextHandle
                }
                None => std::ptr::null_mut(),
            },
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Destroy a Gup GPU context previously created with [`gup_context_create`].
///
/// # Safety
///
/// `handle` must be a non-null pointer returned by [`gup_context_create`]
/// and must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_context_destroy(handle: GupContextHandle) {
    catch((), || {
        if !handle.is_null() {
            let _ = unsafe { Box::from_raw(handle as *mut GupContext) };
        }
    });
}

// ---------------------------------------------------------------------------
// Surface lifecycle (mirrors SurfaceHolder.Callback)
// ---------------------------------------------------------------------------

/// Attach an `ANativeWindow` to the Gup context (`surfaceCreated`).
///
/// Returns the new surface ID (> 0) on success, or 0 on failure.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * `a_native_window` must point to a live `ANativeWindow` obtained via
///   `ANativeWindow_fromSurface()`.
/// * Must be called from the render thread.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_surface_created(
    handle: GupContextHandle,
    a_native_window: *mut c_void,
    width: u32,
    height: u32,
) -> u64 {
    catch(0u64, || {
        if handle.is_null() {
            return 0;
        }
        let ctx = unsafe { &mut *(handle as *mut GupContext) };
        match unsafe {
            gup::platform::android::attach_native_window(ctx, a_native_window, width, height)
        } {
            Ok(id) => id.raw(),
            Err(_) => 0,
        }
    })
}

/// Handle a `surfaceChanged` callback (resize / format change).
///
/// Returns `true` on success.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * `surface_id` must refer to an attached surface.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_surface_changed(
    handle: GupContextHandle,
    surface_id: u64,
    new_width: u32,
    new_height: u32,
) -> bool {
    catch(false, || {
        if handle.is_null() {
            return false;
        }
        let ctx = unsafe { &mut *(handle as *mut GupContext) };
        let id = gup::context::SurfaceId(surface_id);
        gup::platform::android::handle_surface_changed(ctx, id, new_width, new_height).is_ok()
    })
}

/// Handle a `surfaceDestroyed` callback — detach the surface and release
/// all associated GPU resources.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * `surface_id` must be a surface ID previously returned by
///   [`gup_surface_created`].
/// * After this call the surface ID is invalid and must not be reused.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_surface_destroyed(handle: GupContextHandle, surface_id: u64) {
    catch((), || {
        if handle.is_null() {
            return;
        }
        let ctx = unsafe { &mut *(handle as *mut GupContext) };
        let id = gup::context::SurfaceId(surface_id);
        let _ = ctx.remove_surface(id);
    });
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render a single frame on the specified surface.
///
/// Returns `true` on success, `false` on error.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * `surface_id` must refer to an attached surface.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_render_frame(handle: GupContextHandle, surface_id: u64) -> bool {
    catch(false, || {
        if handle.is_null() {
            return false;
        }
        let _ctx = unsafe { &mut *(handle as *mut GupContext) };
        let _id = gup::context::SurfaceId(surface_id);

        // NOTE: Full rendering integration requires scene/chart state which
        // is story-specific.  This stub proves the JNI→Rust pipeline works
        // end-to-end.  A follow-up story will wire in chart-level rendering.
        true
    })
}

// ---------------------------------------------------------------------------
// Touch events
// ---------------------------------------------------------------------------

/// Forward a single pointer from an Android `MotionEvent` into the Gup
/// event pipeline.
///
/// The Kotlin wrapper should call this for each active pointer in the
/// `MotionEvent` (iterating `pointerCount`).
///
/// # Safety
///
/// * `handle` must be a valid context handle.
/// * All numeric parameters must be finite.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_on_touch_event(
    _handle: GupContextHandle,
    pointer_id: u64,
    x: f32,
    y: f32,
    action: u8,
    density: f32,
    event_time_ms: f64,
    view_width: f32,
    view_height: f32,
) {
    catch((), || {
        let raw = RawAndroidTouch::new(pointer_id, x, y, action, density, event_time_ms);
        let _events = translate_motion_event(&[raw], Some((view_width, view_height)));
        // NOTE: Event dispatch into the interaction system will be wired
        // when GUP-013 (Event Handling System) provides the dispatch API.
    });
}

// ---------------------------------------------------------------------------
// Activity lifecycle helpers
// ---------------------------------------------------------------------------

/// Notify Gup that the hosting Activity is pausing.
///
/// This is a hint that rendering should stop; the surface may still be
/// valid until `surfaceDestroyed` fires.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_pause(handle: GupContextHandle) {
    catch((), || {
        if handle.is_null() {
            return;
        }
        // Pausing is a no-op at this layer — the render loop is driven
        // externally (from Choreographer callbacks in Kotlin).
        let _ = handle;
    });
}

/// Notify Gup that the hosting Activity is resuming.
///
/// # Safety
///
/// * `handle` must be a valid context handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gup_resume(handle: GupContextHandle) {
    catch((), || {
        if handle.is_null() {
            return;
        }
        let _ = handle;
    });
}
