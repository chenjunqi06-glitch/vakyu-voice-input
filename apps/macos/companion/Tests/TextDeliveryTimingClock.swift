//! 可推进的单线程虚拟时钟，按截止时间执行交付回调；不使用真实等待。
import Foundation

final class TextDeliveryTimingClock {
    private(set) var now: TimeInterval = 0

    private var sequence = 0

    private var pending: [(at: TimeInterval, order: Int, work: () -> Void)] = []

    func schedule(_ delay: TimeInterval, _ work: @escaping () -> Void) {
        sequence += 1
        pending.append((now + delay, sequence, work))
    }

    func advance(to deadline: TimeInterval) {
        precondition(deadline >= now)
        while let next = pending.enumerated().min(by: {
            if $0.element.at == $1.element.at { return $0.element.order < $1.element.order }
            return $0.element.at < $1.element.at
        }), next.element.at <= deadline {
            let item = pending.remove(at: next.offset)
            now = item.at
            item.work()
        }
        now = deadline
    }
}
