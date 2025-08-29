// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ParakeetTranscription",
    platforms: [
        .macOS(.v14)
    ],
    dependencies: [
        .package(url: "https://github.com/FluidInference/FluidAudio.git", branch: "main")
    ],
    targets: [
        .executableTarget(
            name: "ParakeetTranscription",
            dependencies: [
                .product(name: "FluidAudio", package: "FluidAudio")
            ],
            path: "Sources"
        )
    ]
)