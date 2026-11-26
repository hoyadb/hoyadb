// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "HoyaDB",
  platforms: [.macOS(.v13), .iOS("15.1")],
  products: [
    .library(name: "HoyaDB", targets: ["HoyaDB"]),
    .library(name: "HoyaDBTesting", targets: ["HoyaDBTesting"]),
  ],
  targets: [
    .binaryTarget(name: "HoyaNative", path: "Artifacts/HoyaNative.xcframework"),
    .target(
      name: "HoyaDB", dependencies: ["HoyaNative"],
      linkerSettings: [.linkedFramework("Security"), .linkedFramework("CoreFoundation")]),
    .target(name: "HoyaDBTesting", dependencies: ["HoyaDB"]),
    .testTarget(name: "HoyaDBTests", dependencies: ["HoyaDB"]),
    .testTarget(name: "HoyaDBTestingTests", dependencies: ["HoyaDB", "HoyaDBTesting"]),
  ]
)
