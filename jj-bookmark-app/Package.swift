// swift-tools-version:6.2
import PackageDescription

// 纯 SwiftPM executable target；无 .xcodeproj / Storyboard / xib。
// 产物 .build/<config>/jj-bookmark-app 由 package.sh 组装进 .app bundle。
// defaultIsolation(MainActor)：AppKit App 全程主线程，整 target 默认主 actor，
// 与框架现实一致并消除 Swift 6 并发告警；后台工作显式 hop 到全局队列。
let package = Package(
    name: "jj-bookmark-app",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(
            name: "jj-bookmark-app",
            path: "Sources/jj-bookmark-app",
            swiftSettings: [
                .defaultIsolation(MainActor.self)
            ]
        )
    ]
)
