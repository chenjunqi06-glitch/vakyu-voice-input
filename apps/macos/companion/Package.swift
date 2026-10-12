// swift-tools-version: 5.9
// 原生语音宿主只调用系统接口，识别与学习由共享服务处理。
import PackageDescription

let package = Package(
    name: "VakyuVoice",
    platforms: [.macOS(.v13)],
    products: [.executable(name: "VakyuVoice", targets: ["VakyuVoice"])],
    targets: [
        .executableTarget(
            name: "VakyuVoice",
            path: "Sources",
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("SwiftUI"),
                .linkedFramework("AVFoundation"),
                .linkedFramework("ApplicationServices"),
                .linkedFramework("Carbon")
            ]
        ),
        .testTarget(name: "VakyuVoiceTests", dependencies: ["VakyuVoice"], path: "Tests")
    ]
)
