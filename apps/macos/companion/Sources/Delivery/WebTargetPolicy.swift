//! 有限遍历输入框祖先，网页控件走能触发编辑事件的粘贴路径。
import Foundation

enum WebTargetPolicy {
    static func hasWebArea<Node>(from node: Node, limit: Int = 12,
                                 role: (Node) -> String?, parent: (Node) -> Node?,
                                 same: (Node, Node) -> Bool) -> Bool {
        var current = node
        var visited: [Node] = []
        for _ in 0..<max(0, limit) {
            guard !visited.contains(where: { same($0, current) }) else { return false }
            visited.append(current)
            if role(current) == "AXWebArea" { return true }
            guard let next = parent(current) else { return false }
            current = next
        }
        return false
    }
}
