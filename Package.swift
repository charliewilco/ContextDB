// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ContextDB",
    platforms: [
        .iOS(.v14),
        .macOS(.v12),
    ],
    products: [
        .library(
            name: "ContextDB",
            targets: ["ContextDB"]
        ),
    ],
    targets: [
        .binaryTarget(
            name: "ContextDB",
            path: "dist/ContextDB.xcframework"
        ),
    ]
)
