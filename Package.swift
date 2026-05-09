// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Muxy",
    platforms: [
        .macOS(.v14),
    ],
    products: [
        .library(name: "MuxyShared", targets: ["MuxyShared"]),
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.9.1"),
        .package(url: "https://github.com/jpsim/Yams", from: "5.1.0"),
        .package(url: "https://github.com/getsentry/sentry-cocoa", from: "8.40.0"),
    ],
    targets: [
        .target(
            name: "MuxyShared",
            path: "MuxyShared"
        ),
        .target(
            name: "GhosttyKit",
            path: "GhosttyKit",
            publicHeadersPath: "."
        ),
        .target(
            name: "MuxyServer",
            dependencies: [
                "MuxyShared",
            ],
            path: "MuxyServer"
        ),
        .executableTarget(
            name: "Muxy",
            dependencies: [
                "GhosttyKit",
                "MuxyShared",
                "MuxyServer",
                .product(name: "Sparkle", package: "Sparkle"),
                .product(name: "Yams", package: "Yams"),
                .product(name: "Sentry", package: "sentry-cocoa"),
            ],
            path: "Muxy",
            exclude: ["Info.plist", "Muxy.entitlements", "Resources/ghostty", "Resources/terminfo", "Resources/rg"],
            resources: [
                .process("Resources"),
                .copy("Resources/ghostty"),
                .copy("Resources/terminfo"),
                .copy("Resources/rg"),
            ],
            linkerSettings: [
                .unsafeFlags([
                    "GhosttyKit.xcframework/macos-arm64_x86_64/ghostty-internal.a",
                ]),
                .linkedFramework("AppKit"),
                .linkedFramework("Carbon"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("CoreText"),
                .linkedFramework("Foundation"),
                .linkedFramework("IOKit"),
                .linkedFramework("Metal"),
                .linkedFramework("MetalKit"),
                .linkedFramework("QuartzCore"),
                .linkedLibrary("c++"),
            ]
        ),
        .testTarget(
            name: "MuxyTests",
            dependencies: [
                "Muxy",
                "MuxyShared",
                "MuxyServer",
                .product(name: "Yams", package: "Yams"),
            ],
            path: "Tests/MuxyTests",
            linkerSettings: [
                .unsafeFlags([
                    "GhosttyKit.xcframework/macos-arm64_x86_64/ghostty-internal.a",
                ]),
                .linkedFramework("AppKit"),
                .linkedFramework("Carbon"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("CoreText"),
                .linkedFramework("Foundation"),
                .linkedFramework("IOKit"),
                .linkedFramework("Metal"),
                .linkedFramework("MetalKit"),
                .linkedFramework("QuartzCore"),
                .linkedLibrary("c++"),
            ]
        ),
    ]
)
