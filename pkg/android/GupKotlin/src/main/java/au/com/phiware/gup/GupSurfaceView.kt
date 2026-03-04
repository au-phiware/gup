// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

package au.com.phiware.gup

import android.content.Context
import android.util.AttributeSet
import android.view.Choreographer
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView

/**
 * A [SurfaceView] that hosts a Gup GPU-accelerated chart.
 *
 * Place this view in your layout XML:
 *
 * ```xml
 * <au.com.phiware.gup.GupSurfaceView
 *     android:id="@+id/chart_view"
 *     android:layout_width="match_parent"
 *     android:layout_height="match_parent" />
 * ```
 *
 * Then attach a [GupContext] programmatically:
 *
 * ```kotlin
 * val chartView = findViewById<GupSurfaceView>(R.id.chart_view)
 * chartView.attachContext(gupContext)
 * ```
 *
 * The view implements [SurfaceHolder.Callback] and automatically manages
 * the native surface lifecycle (create / resize / destroy).  Touch events
 * are forwarded to the Gup event pipeline.
 */
class GupSurfaceView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : SurfaceView(context, attrs, defStyleAttr), SurfaceHolder.Callback, Choreographer.FrameCallback {

    private var gupContext: GupContext? = null
    private var surfaceId: Long = 0L
    private var rendering = false
    private val density: Float = context.resources.displayMetrics.density

    init {
        holder.addCallback(this)
    }

    /**
     * Convenience constructor that immediately attaches a [GupContext].
     */
    constructor(context: Context, gupContext: GupContext) : this(context) {
        attachContext(gupContext)
    }

    /**
     * Attach (or replace) the [GupContext] that drives this view.
     */
    fun attachContext(ctx: GupContext) {
        gupContext = ctx
    }

    // -- SurfaceHolder.Callback -----------------------------------------------

    override fun surfaceCreated(holder: SurfaceHolder) {
        val ctx = gupContext ?: return
        val h = ctx.handle
        if (h == 0L) return
        surfaceId = GupBridge.nativeSurfaceCreated(h, holder.surface, width, height)
        if (surfaceId != 0L) {
            rendering = true
            Choreographer.getInstance().postFrameCallback(this)
        }
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        val ctx = gupContext ?: return
        val h = ctx.handle
        if (h == 0L || surfaceId == 0L) return
        GupBridge.nativeSurfaceChanged(h, surfaceId, width, height)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        rendering = false
        val ctx = gupContext ?: return
        val h = ctx.handle
        if (h == 0L || surfaceId == 0L) return
        GupBridge.nativeSurfaceDestroyed(h, surfaceId)
        surfaceId = 0L
    }

    // -- Choreographer.FrameCallback (vsync-driven render loop) ---------------

    override fun doFrame(frameTimeNanos: Long) {
        if (!rendering) return
        val ctx = gupContext ?: return
        val h = ctx.handle
        if (h != 0L && surfaceId != 0L) {
            GupBridge.nativeRenderFrame(h, surfaceId)
        }
        if (rendering) {
            Choreographer.getInstance().postFrameCallback(this)
        }
    }

    // -- Touch events ---------------------------------------------------------

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val ctx = gupContext ?: return super.onTouchEvent(event)
        val h = ctx.handle
        if (h == 0L) return super.onTouchEvent(event)

        val maskedAction = event.actionMasked
        val gupAction: Int = when (maskedAction) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> 0
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> 1
            MotionEvent.ACTION_MOVE -> 2
            MotionEvent.ACTION_CANCEL -> 3
            else -> return super.onTouchEvent(event)
        }

        // For MOVE events, forward all pointers; for other events, forward
        // the pointer indicated by actionIndex.
        if (maskedAction == MotionEvent.ACTION_MOVE) {
            for (i in 0 until event.pointerCount) {
                GupBridge.nativeOnTouchEvent(
                    h,
                    event.getPointerId(i).toLong(),
                    event.getX(i),
                    event.getY(i),
                    gupAction,
                    density,
                    event.eventTime.toDouble(),
                    width.toFloat(),
                    height.toFloat()
                )
            }
        } else {
            val idx = event.actionIndex
            GupBridge.nativeOnTouchEvent(
                h,
                event.getPointerId(idx).toLong(),
                event.getX(idx),
                event.getY(idx),
                gupAction,
                density,
                event.eventTime.toDouble(),
                width.toFloat(),
                height.toFloat()
            )
        }
        return true
    }
}
