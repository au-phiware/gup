// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

/// Minimal iOS scatter-plot example.
///
/// This file demonstrates embedding a `GupChartView` in a UIKit app.
/// It is intended to be used as the main view controller in the
/// `IosScatter` Xcode project.

import UIKit
import GupSwift

class ScatterViewController: UIViewController {
    private var chartView: GupChartView!
    private var context: GupContext!

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black

        do {
            context = try GupContext()
        } catch {
            fatalError("Failed to create GupContext: \(error)")
        }

        chartView = GupChartView(context: context)
        chartView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(chartView)

        NSLayoutConstraint.activate([
            chartView.topAnchor.constraint(equalTo: view.topAnchor),
            chartView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            chartView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            chartView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])
    }

    override var supportedInterfaceOrientations: UIInterfaceOrientationMask {
        .all
    }
}
