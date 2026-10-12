//! 焦点解析的内存边界，记录查询并模拟查询期间切应用或开启安全输入。
@testable import VakyuVoice

final class FocusMemory {
    var frontmost: Int32? = 42

    var ownPID: Int32 = 7

    var globalSecure = false

    var system: Int? = 1

    var application: Int? = 2

    var pids: [Int: Int32] = [1: 42, 2: 42]

    var secureNodes: Set<Int> = []

    var systemReads = 0

    var applicationQueries: [Int32] = []

    var sources: [FocusResolution<Int>.Source] = []

    var onSystemRead: (() -> Void)?

    var onApplicationRead: (() -> Void)?

    var onPIDRead: (() -> Void)?

    var onSecurityRead: (() -> Void)?

    func resolve(processID: Int32 = 42) -> FocusResolution<Int> {
        FocusResolution.resolve(processID: processID, currentProcessID: ownPID,
            frontmostPID: { self.frontmost }, secureInput: { self.globalSecure },
            systemFocus: {
                self.systemReads += 1
                self.onSystemRead?()
                return self.system
            }, applicationFocus: { pid in
                self.applicationQueries.append(pid)
                self.onApplicationRead?()
                return self.application
            }, nodePID: { node in
                self.onPIDRead?()
                return self.pids[node]
            }, isSecure: { node in
                self.onSecurityRead?()
                return self.secureNodes.contains(node)
            }, sourceObserved: { self.sources.append($0) })
    }
}
