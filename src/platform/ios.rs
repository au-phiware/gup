// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! iOS platform integration — Metal surface, UITouch translation, and
//! orientation change handling.
//!
//! This module is gated behind `cfg(target_os = "ios")` *and* the `ios-shim`
//! Cargo feature so it never affects desktop or WASM builds.
//!
//! # Surface creation
//!
//! [`attach_metal_layer`] wraps a `CAMetalLayer` raw pointer in a
//! [`raw_window_handle`] `UiKitWindowHandle` and creates a wgpu surface via
//! [`wgpu::Instance::create_surface`].
//!
//! # Touch translation
//!
//! Touch translation is in [`super::ios_touch`] (available on all platforms
//! for testing) and re-exported here for convenience.
//!
//! # Orientation
//!
//! [`handle_orientation_change`] delegates to
//! [`GupContext::resize_surface`](crate::context::GupContext::resize_surface)
//! after clamping the new `drawableSize` to a sane minimum.

pub use super::ios_touch::{RawIosTouch, translate_uitouch};

use crate::context::{GupContext, PhysicalSize, SurfaceId};
use crate::error::{GupError, GupResult};
use raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    UiKitDisplayHandle, UiKitWindowHandle,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Surface creation
// ---------------------------------------------------------------------------

/// Wrapper that holds the raw pointers needed by `raw_window_handle` to
/// represent a `CAMetalLayer`-backed iOS view.
///
/// # Safety
///
/// The caller must guarantee that `ui_view` and `ui_view_controller` remain
/// valid for the lifetime of this handle.
pub struct IosSurfaceHandle {
    /// Pointer to the `UIView` whose backing layer is `CAMetalLayer`.
    pub ui_view: NonNull<c_void>,
    /// Optional pointer to the owning `UIViewController` (may be null).
    pub ui_view_controller: Option<NonNull<c_void>>,
}

// SAFETY: The pointer is only dereferenced on the main (UI) thread which
// is also the thread that drives the render loop on iOS.
unsafe impl Send for IosSurfaceHandle {}
unsafe impl Sync for IosSurfaceHandle {}

impl HasWindowHandle for IosSurfaceHandle {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let mut handle = UiKitWindowHandle::new(self.ui_view);
        if let Some(vc) = self.ui_view_controller {
            handle.ui_view_controller = Some(vc);
        }
        // SAFETY: the raw handle borrows `self` which keeps the pointers alive.
        let raw = RawWindowHandle::UiKit(handle);
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for IosSurfaceHandle {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        let handle = UiKitDisplayHandle::new();
        let raw = RawDisplayHandle::UiKit(handle);
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(raw) })
    }
}

/// Attach a `CAMetalLayer`-backed `UIView` to the given [`GupContext`] and
/// return the new [`SurfaceId`].
///
/// # Arguments
///
/// * `ctx`   – A mutable reference to the Gup GPU context.
/// * `ui_view` – Raw pointer to the `UIView` whose `layer` is a
///               `CAMetalLayer`.
/// * `ui_view_controller` – Optional raw pointer to the owning
///                          `UIViewController`.
/// * `width`  – Initial drawable width in physical pixels.
/// * `height` – Initial drawable height in physical pixels.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `ui_view` points to a valid `UIView` with a `CAMetalLayer` backing.
/// 2. The view (and its layer) outlive the returned `SurfaceId`.
/// 3. This function is called from the main (UI) thread.
///
/// # Errors
///
/// Returns [`GupError::SurfaceError`] if wgpu fails to create or configure
/// the Metal surface.
pub unsafe fn attach_metal_layer(
    ctx: &mut GupContext,
    ui_view: *mut c_void,
    ui_view_controller: *mut c_void,
    width: u32,
    height: u32,
) -> GupResult<SurfaceId> {
    let view_nn = NonNull::new(ui_view).ok_or_else(|| {
        GupError::resource_error("ui_view pointer is null")
    })?;
    let vc_nn = NonNull::new(ui_view_controller);

    let handle = Arc::new(IosSurfaceHandle {
        ui_view: view_nn,
        ui_view_controller: vc_nn,
    });

    let id = SurfaceId::new();

    // Use the existing multi-surface API to register the Metal surface.
    ctx.add_surface(id, handle)?;

    // Resize to the actual drawable dimensions.
    ctx.resize_surface(id, PhysicalSize::new(width, height))?;

    Ok(id)
}

// ---------------------------------------------------------------------------
// Orientation change
// ---------------------------------------------------------------------------

/// Handle a device orientation change by resizing the surface to the new
/// `CAMetalLayer.drawableSize`.
///
/// This delegates to [`GupContext::resize_surface`] which reconfigures the
/// swap chain without tearing down the wgpu device.
///
/// # Errors
///
/// Returns an error if the surface ID is unknown or resize fails.
pub fn handle_orientation_change(
    ctx: &mut GupContext,
    surface_id: SurfaceId,
    new_width: u32,
    new_height: u32,
) -> GupResult<()> {
    // Guard against zero-sized surfaces which Metal/wgpu reject.
    let w = new_width.max(1);
    let h = new_height.max(1);
    ctx.resize_surface(surface_id, PhysicalSize::new(w, h))
}

