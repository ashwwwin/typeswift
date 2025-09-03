// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "TypeswiftSwift",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "TypeswiftSwift",
            type: .dynamic,
            targets: ["TypeswiftSwift"]
        ),
    ],
    dependencies: [
        .package(url: "https://github.com/FluidInference/FluidAudio.git", branch: "main")
    ],
    targets: [
        .target(
            name: "TypeswiftSwift",
            dependencies: [
                .product(name: "FluidAudio", package: "FluidAudio")
            ],
            path: "Sources",
            publicHeadersPath: "include",
            linkerSettings: [
                .linkedFramework("CoreML"),
                .linkedFramework("Accelerate"),
                .linkedFramework("CoreAudio"),
                .linkedFramework("AVFoundation")
            ]
        ),
    ]
)
