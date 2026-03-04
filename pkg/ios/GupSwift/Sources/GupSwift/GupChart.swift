// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

/// SwiftUI wrapper for `GupChartView`.
///
/// Usage:
/// ```swift
/// struct ContentView: View {
///     let context = try! GupContext()
///     var body: some View {
///         GupChart(context: context)
///             .frame(maxWidth: .infinity, maxHeight: .infinity)
///     }
/// }
/// ```

#if canImport(SwiftUI) && canImport(UIKit)
import SwiftUI

/// A SwiftUI view that embeds a Gup chart.
///
/// Wraps `GupChartView` via `UIViewRepresentable`.
public struct GupChart: UIViewRepresentable {
    private let context: GupContext

    /// Create a chart view backed by the given GPU context.
    public init(context: GupContext) {
        self.context = context
    }

    public func makeUIView(context _: Context) -> GupChartView {
        GupChartView(context: context)
    }

    public func updateUIView(_: GupChartView, context _: Context) {
        // Configuration updates can be handled here in the future.
    }
}
#endif
