// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.
//
// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

import PackageDescription

let package = Package(
    name: "GupSwift",
    platforms: [
        .iOS(.v15),
    ],
    products: [
        .library(
            name: "GupSwift",
            targets: ["GupSwift"]
        ),
    ],
    targets: [
        .target(
            name: "GupSwift",
            path: "Sources/GupSwift",
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency"),
            ]
        ),
    ]
)
