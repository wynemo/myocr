// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "MyOCR",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "MyOCR", targets: ["MyOCR"])
    ],
    targets: [
        .executableTarget(
            name: "MyOCR",
            path: "Sources/MyOCR",
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("ScreenCaptureKit"),
                .linkedFramework("Vision"),
                .linkedFramework("Carbon")
            ]
        )
    ]
)
