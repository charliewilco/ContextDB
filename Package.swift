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
			url: "https://github.com/charliewilco/ContextDB/releases/download/v0.1.0/ContextDB.xcframework.zip",
			checksum: "2ae40df138e61416207382df471a2084b534e38a71cd4f9cef7728f9930553a8"
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
