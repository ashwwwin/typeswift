// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "VoicySwift",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "VoicySwift",
            type: .dynamic,
            targets: ["VoicySwift"]
        ),
    ],
    dependencies: [
        .package(url: "https://github.com/FluidInference/FluidAudio.git", branch: "main")
    ],
    targets: [
        .target(
            name: "VoicySwift",
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