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
			url: "https://github.com/charliewilco/ContextDB/releases/download/v0.1.1/ContextDB.xcframework.zip",
			checksum: "04f3c5ef8718e3224e99afbfcd98d6449e0821c9c032c278e10e6773e9e96e82"
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
