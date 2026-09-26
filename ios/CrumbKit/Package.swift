// swift-tools-version:5.10
// Everything in the iOS app that isn't UI: the Crumb API client, models, session, offline
// cache, kitchen timers and theme, plus crumb-core (Rust, through UniFFI). The app and the
// share extension both link it; `swift test` runs its tests on a Mac without a simulator.
//
// Frameworks/CrumbCoreFFI.xcframework and Sources/CrumbCore/CrumbCore.swift are generated
// by ../scripts/build-core.sh (nothing generated is committed). Run it first.

import PackageDescription

let package = Package(
  name: "CrumbKit",
  platforms: [.iOS(.v17), .macOS(.v14)],
  products: [
    .library(name: "CrumbKit", targets: ["CrumbKit"])
  ],
  targets: [
    // libcrumb_ffi.a for each Apple platform, with the C header UniFFI generates
    .binaryTarget(name: "CrumbCoreFFI", path: "Frameworks/CrumbCoreFFI.xcframework"),
    // UniFFI's Swift half: the exported crumb-core functions as plain Swift
    .target(name: "CrumbCore", dependencies: ["CrumbCoreFFI"]),
    .target(name: "CrumbKit", dependencies: ["CrumbCore"]),
    .testTarget(name: "CrumbKitTests", dependencies: ["CrumbKit"]),
  ]
)
