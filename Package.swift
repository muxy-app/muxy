// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Muxy",
    platforms: [
        .macOS(.v14),
        .iOS(.v17),
    ],
    products: [
        .library(name: "MuxyShared", targets: ["MuxyShared"]),
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.9.1"),
        .package(url: "https://github.com/migueldeicaza/SwiftTerm", from: "1.2.0"),
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
            ],
            path: "Muxy",
            exclude: ["Info.plist", "Muxy.entitlements"],
            resources: [
                .process("Resources"),
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
