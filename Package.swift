// swift-tools-version: 5.9

import PackageDescription

let package = Package(
	name: "ContextDB",
	platforms: [
		.iOS(.v15),
		.macOS(.v12),
	],
	products: [
		.library(name: "ContextDB", targets: ["ContextDB"]),
	],
	targets: [
		.binaryTarget(
			name: "CContextDB",
			path: "dist/ContextDB.xcframework"
		),
		.target(
			name: "ContextDB",
			dependencies: ["CContextDB"]
		),
		.testTarget(
			name: "ContextDBTests",
			dependencies: ["ContextDB"],
			path: "tests/ContextDBTests"
		),
	]
)
