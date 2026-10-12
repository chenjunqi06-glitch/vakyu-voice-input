//! 填入失败后的短暂展示；代次隔离防止旧计时器收起新一轮。
import Foundation

final class CompletionPresentation {
    private let schedule: (TimeInterval, @escaping () -> Void) -> Void

    private var generation: UInt64 = 0

    private(set) var collapsed = false

    init(schedule: @escaping (TimeInterval, @escaping () -> Void) -> Void = { delay, work in
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
    }) {
        self.schedule = schedule
    }

    func presentFailure(collapse: @escaping () -> Void) {
        reset()
        let current = generation
        schedule(3) { [weak self] in
            guard let self, self.generation == current else { return }
            self.collapsed = true
            collapse()
        }
    }

    func reset() {
        generation &+= 1
        collapsed = false
    }

    func collapseNow() {
        reset()
        collapsed = true
    }
}
