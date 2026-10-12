// 与输入法共用打包素材，完整字标保留边缘，不裁切为单只小鸡。
import AppKit
import SwiftUI

enum Brand {
    static let gradient = LinearGradient(colors: [Color(red: 0.41, green: 0.22, blue: 0.76),
                                                  Color(red: 0.24, green: 0.12, blue: 0.48)],
                                         startPoint: .topLeading, endPoint: .bottomTrailing)

    static func image(_ name: String) -> NSImage? {
        guard let url = Bundle.main.resourceURL?.appendingPathComponent(name) else { return nil }
        return NSImage(contentsOf: url)
    }
}
