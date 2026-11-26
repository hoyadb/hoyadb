// swift-tools-version: 6.0
// The Swift Todo guide. It depends on the local HoyaDB package in bindings/swift, whose
// Artifacts/HoyaNative.xcframework must first be built from current source
// (python3 bindings/swift/build-artifacts.py).
import PackageDescription

let package = Package(
  name: "HoyaTodoGuide",
  platforms: [.macOS(.v13)],
  dependencies: [.package(name: "HoyaDB", path: "../../bindings/swift")],
  targets: [
    .executableTarget(
      name: "TodoGuide",
      dependencies: [
        .product(name: "HoyaDB", package: "HoyaDB"),
        .product(name: "HoyaDBTesting", package: "HoyaDB"),
      ])
  ]
)
