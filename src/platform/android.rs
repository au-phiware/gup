// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Android platform integration — `ANativeWindow` surface management and
//! lifecycle handling.
//!
//! This module is gated behind `cfg(target_os = "android")` *and* the
//! `android-shim` Cargo feature so it never affects desktop, iOS, or WASM
//! builds.
//!
//! # Surface creation
//!
//! [`attach_native_window`] wraps a raw `ANativeWindow` pointer in a
//! [`raw_window_handle`] `AndroidNdkWindowHandle` and creates a wgpu
//! surface via [`wgpu::Instance::create_surface`].
//!
//! # Touch translation
//!
//! Touch translation lives in [`super::android_touch`] (available on all
//! platforms for testing) and is re-exported here for convenience.
//!
//! # Surface lifecycle
//!
//! [`handle_surface_changed`] handles `surfaceChanged` (resize / format
//! change) by delegating to
//! [`GupContext::resize_surface`](crate::context::GupContext::resize_surface).

pub use super::android_touch::{RawAndroidTouch, translate_motion_event};

use crate::context::{GupContext, PhysicalSize, SurfaceId};
use crate::error::{GupError, GupResult};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Surface creation
// ---------------------------------------------------------------------------

/// Wrapper that holds the raw `ANativeWindow` pointer needed by
/// `raw_window_handle` to represent an Android surface.
///
/// # Safety
///
/// The caller must guarantee that the `ANativeWindow` pointer remains valid
/// for the lifetime of this handle.  On Android this means the handle must
/// be dropped (via [`GupContext::remove_surface`]) before the corresponding
/// `surfaceDestroyed` callback returns.
pub struct AndroidSurfaceHandle {
    /// Pointer to the `ANativeWindow` obtained from `ANativeWindow_fromSurface`.
    pub a_native_window: NonNull<c_void>,
}

// SAFETY: The pointer is only dereferenced on the render thread which is
// also the thread that drives the wgpu surface on Android.
unsafe impl Send for AndroidSurfaceHandle {}
unsafe impl Sync for AndroidSurfaceHandle {}

impl HasWindowHandle for AndroidSurfaceHandle {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let handle = AndroidNdkWindowHandle::new(self.a_native_window);
        let raw = RawWindowHandle::AndroidNdk(handle);
        // SAFETY: the raw handle borrows `self` which keeps the pointer alive.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for AndroidSurfaceHandle {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        let handle = AndroidDisplayHandle::new();
        let raw = RawDisplayHandle::Android(handle);
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(raw) })
    }
}

/// Attach an `ANativeWindow` to the given [`GupContext`] and return the
/// new [`SurfaceId`].
///
/// This is called from the JNI bridge in response to `surfaceCreated`.
///
/// # Arguments
///
/// * `ctx`              – A mutable reference to the Gup GPU context.
/// * `a_native_window`  – Raw pointer to the `ANativeWindow` obtained via
///                        `ANativeWindow_fromSurface` in the NDK.
/// * `width`            – Initial surface width in physical pixels.
/// * `height`           – Initial surface height in physical pixels.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `a_native_window` points to a valid `ANativeWindow`.
/// 2. The window outlives the returned `SurfaceId` (i.e. detach before
///    `surfaceDestroyed` returns).
/// 3. This function is called from the thread that owns the surface.
///
/// # Errors
///
/// Returns [`GupError`] if wgpu fails to create or configure the Vulkan
/// or GLES surface.
pub unsafe fn attach_native_window(
    ctx: &mut GupContext,
    a_native_window: *mut c_void,
    width: u32,
    height: u32,
) -> GupResult<SurfaceId> {
    let nn = NonNull::new(a_native_window)
        .ok_or_else(|| GupError::resource_error("ANativeWindow pointer is null"))?;

    let handle = Arc::new(AndroidSurfaceHandle {
        a_native_window: nn,
    });

    let id = SurfaceId::new();

    // Use the existing multi-surface API to register the Android surface.
    ctx.add_surface(id, handle)?;

    // Resize to the actual surface dimensions.
    ctx.resize_surface(id, PhysicalSize::new(width, height))?;

    Ok(id)
}

// ---------------------------------------------------------------------------
// Surface lifecycle helpers
// ---------------------------------------------------------------------------

/// Handle a `surfaceChanged` callback by resizing the surface to the new
/// dimensions.
///
/// This delegates to [`GupContext::resize_surface`] which reconfigures the
/// swap chain without tearing down the wgpu device.
///
/// # Errors
///
/// Returns an error if the surface ID is unknown or resize fails.
pub fn handle_surface_changed(
    ctx: &mut GupContext,
    surface_id: SurfaceId,
    new_width: u32,
    new_height: u32,
) -> GupResult<()> {
    // Guard against zero-sized surfaces which Vulkan/GLES reject.
    let w = new_width.max(1);
    let h = new_height.max(1);
    ctx.resize_surface(surface_id, PhysicalSize::new(w, h))
}
