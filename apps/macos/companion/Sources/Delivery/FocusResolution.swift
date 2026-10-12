//! 固定前台进程解析焦点；仅系统焦点缺失时回退该应用，不替换已返回的安全或其他控件。
enum FocusResolution<Node> {
    case element(Node)
    case missing
    case contextChanged
    case secureInput
    case secureElement
    case accessibilityUnavailable

    enum Source: String {
        case system
        case application = "app"
    }

    static func resolve(processID: Int32, currentProcessID: Int32,
                        frontmostPID: () -> Int32?, secureInput: () -> Bool,
                        systemFocus: () -> Node?, applicationFocus: (Int32) -> Node?,
                        nodePID: (Node) -> Int32?, isSecure: (Node) -> Bool,
                        accessibilityAvailable: () -> Bool = { true },
                        sourceObserved: (Source) -> Void = { _ in }) -> Self {
        func rejection() -> Self? {
            guard processID > 0, processID != currentProcessID,
                  frontmostPID() == processID else { return .contextChanged }
            if secureInput() { return .secureInput }
            return accessibilityAvailable() ? nil : .accessibilityUnavailable
        }

        if let rejected = rejection() { return rejected }
        let system = systemFocus()
        if let rejected = rejection() { return rejected }
        // 已返回的元素不能因安全标记、进程不符或角色不支持而被回退路径掩盖。
        let candidate: Node?
        if let system {
            candidate = system
            sourceObserved(.system)
        } else {
            candidate = applicationFocus(processID)
            if let _ = candidate { sourceObserved(.application) }
        }
        if let rejected = rejection() { return rejected }
        guard let candidate else { return .missing }
        guard nodePID(candidate) == processID else { return .contextChanged }
        if isSecure(candidate) { return .secureElement }
        if let rejected = rejection() { return rejected }
        return .element(candidate)
    }

    // 兜底仍绑定同一进程和窗口；焦点缺失不等于安全输入或权限失效。
    func permitsPasteFallback<Window: Equatable>(enabled: Bool, processID: Int32,
                                                savedWindow: Window?, frontmostPID: () -> Int32?,
                                                currentWindow: () -> Window?, secureInput: () -> Bool) -> Bool {
        guard enabled, processID > 0, let savedWindow else { return false }
        switch self {
        case .missing, .element: break
        case .contextChanged, .secureInput, .secureElement, .accessibilityUnavailable: return false
        }
        guard frontmostPID() == processID, !secureInput(), currentWindow() == savedWindow else { return false }
        return frontmostPID() == processID && !secureInput()
    }
}

extension FocusResolution: Equatable where Node: Equatable {}
