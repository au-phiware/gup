// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

package au.com.phiware.gup

/**
 * Manages the Gup GPU context lifecycle.
 *
 * Create one instance per application (or Activity), and pass it to one or
 * more [GupSurfaceView]s.  Call [destroy] when the context is no longer
 * needed (typically in `Activity.onDestroy`).
 *
 * ```kotlin
 * class ChartActivity : AppCompatActivity() {
 *     private lateinit var gupContext: GupContext
 *
 *     override fun onCreate(savedInstanceState: Bundle?) {
 *         super.onCreate(savedInstanceState)
 *         gupContext = GupContext()
 *         val chartView = GupSurfaceView(this, gupContext)
 *         setContentView(chartView)
 *     }
 *
 *     override fun onDestroy() {
 *         gupContext.destroy()
 *         super.onDestroy()
 *     }
 * }
 * ```
 */
class GupContext {
    @Volatile
    internal var handle: Long = GupBridge.nativeCreate()
        private set

    /** `true` if the native context is still alive. */
    val isValid: Boolean get() = handle != 0L

    /** Pause rendering (called from `Activity.onPause`). */
    fun pause() {
        val h = handle
        if (h != 0L) GupBridge.nativePause(h)
    }

    /** Resume rendering (called from `Activity.onResume`). */
    fun resume() {
        val h = handle
        if (h != 0L) GupBridge.nativeResume(h)
    }

    /**
     * Destroy the GPU context and release all native resources.
     *
     * After this call, [isValid] returns `false` and all methods are no-ops.
     */
    fun destroy() {
        val h = handle
        if (h != 0L) {
            handle = 0L
            GupBridge.nativeDestroy(h)
        }
    }

    protected fun finalize() {
        destroy()
    }
}
