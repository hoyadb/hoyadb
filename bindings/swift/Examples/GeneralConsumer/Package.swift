// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "HoyaGeneralConsumer",
  platforms: [.macOS(.v13), .iOS("15.1")],
  dependencies: [.package(name: "HoyaDB", path: "../..")],
  targets: [
    .executableTarget(
      name: "GeneralConsumer",
      dependencies: [
        .product(name: "HoyaDB", package: "HoyaDB"),
        .product(name: "HoyaDBTesting", package: "HoyaDB"),
      ])
  ]
)
