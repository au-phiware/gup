// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

package au.com.phiware.gup

/**
 * JNI bridge to the `gup-android` native library.
 *
 * All functions map 1:1 to the `extern "C"` functions exported by
 * `gup-android/src/lib.rs`.  The Kotlin-side wrappers ([GupContext],
 * [GupSurfaceView]) provide the ergonomic, lifecycle-aware API.
 */
internal object GupBridge {
    init {
        System.loadLibrary("gup_android")
    }

    // -- Context lifecycle ----------------------------------------------------

    /** Create a new GPU context.  Returns an opaque native handle (pointer). */
    external fun nativeCreate(): Long

    /** Destroy the GPU context. */
    external fun nativeDestroy(handle: Long)

    // -- Surface lifecycle (SurfaceHolder.Callback) ---------------------------

    /** Attach an ANativeWindow and return a surface ID (> 0), or 0 on error. */
    external fun nativeSurfaceCreated(handle: Long, surface: android.view.Surface, width: Int, height: Int): Long

    /** Notify the native side of a surface resize. */
    external fun nativeSurfaceChanged(handle: Long, surfaceId: Long, width: Int, height: Int): Boolean

    /** Detach the surface and release GPU resources. */
    external fun nativeSurfaceDestroyed(handle: Long, surfaceId: Long)

    // -- Rendering ------------------------------------------------------------

    /** Render a single frame.  Returns true on success. */
    external fun nativeRenderFrame(handle: Long, surfaceId: Long): Boolean

    // -- Touch events ---------------------------------------------------------

    /** Forward a single MotionEvent pointer into the Gup event pipeline. */
    external fun nativeOnTouchEvent(
        handle: Long,
        pointerId: Long,
        x: Float,
        y: Float,
        action: Int,
        density: Float,
        eventTimeMs: Double,
        viewWidth: Float,
        viewHeight: Float
    )

    // -- Activity lifecycle ---------------------------------------------------

    /** Notify the native side that the Activity is pausing. */
    external fun nativePause(handle: Long)

    /** Notify the native side that the Activity is resuming. */
    external fun nativeResume(handle: Long)
}
