// swift-tools-version:6.0
import PackageDescription
let package = Package(
    name: "Shop",
    targets: [
        .target(name: "Shop"),
        .testTarget(name: "ShopTests", dependencies: ["Shop"]),
    ]
)
