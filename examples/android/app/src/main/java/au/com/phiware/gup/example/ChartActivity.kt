// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

package au.com.phiware.gup.example

import android.app.Activity
import android.os.Bundle
import au.com.phiware.gup.GupContext
import au.com.phiware.gup.GupSurfaceView

/**
 * Minimal Activity that renders a Gup GPU-accelerated chart.
 *
 * The chart displays a live line chart with simulated streaming data
 * (a simple sine wave).  Touch the surface to interact.
 */
class ChartActivity : Activity() {
    private lateinit var gupContext: GupContext

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        gupContext = GupContext()
        val chartView = GupSurfaceView(this, gupContext)
        setContentView(chartView)
    }

    override fun onPause() {
        gupContext.pause()
        super.onPause()
    }

    override fun onResume() {
        super.onResume()
        gupContext.resume()
    }

    override fun onDestroy() {
        gupContext.destroy()
        super.onDestroy()
    }
}
