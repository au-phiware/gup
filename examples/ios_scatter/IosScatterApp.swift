// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

/// SwiftUI entry point for the iOS scatter-plot example.

import SwiftUI
import GupSwift

@main
struct IosScatterApp: App {
    @State private var context: GupContext? = nil

    var body: some Scene {
        WindowGroup {
            if let ctx = context {
                GupChart(context: ctx)
                    .ignoresSafeArea()
            } else {
                Text("Initialising GPU…")
                    .onAppear {
                        context = try? GupContext()
                    }
            }
        }
    }
}
