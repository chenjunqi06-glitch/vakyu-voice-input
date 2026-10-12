//! 每条写入路径都精确读回；网页优先粘贴，结果不确定时禁止重复注入。
import AppKit
import ApplicationServices

final class TextDelivery {
    static let eventMarker: Int64 = 0x56414B5955

    var target: InputTarget?

    var pasteFallbackEnabled = true

    private let access: TextDeliveryAccess

    private let schedule: (TimeInterval, @escaping () -> Void) -> Void

    private var completed: Set<UInt64> = []

    private var uncertain = false

    var hasUnconfirmedWrite: Bool { uncertain }

    private(set) var pastedWithoutVerification = false

    var failureStatus: String {
        pastedWithoutVerification ? "已粘贴，无法核对" : (uncertain ? "请检查填入结果" : "未能填入")
    }

    private var inFlight = false

    private var epoch: UInt64 = 0

    init(access: TextDeliveryAccess = SystemTextDeliveryAccess(),
         schedule: @escaping (TimeInterval, @escaping () -> Void) -> Void = { delay, work in
             DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
         }) {
        self.access = access
        self.schedule = schedule
    }

    func reset() {
        epoch &+= 1
        target = nil
        completed.removeAll()
        uncertain = false
        pastedWithoutVerification = false
        inFlight = false
    }

    func commit(id: UInt64, text: String, completion: @escaping (Bool, String) -> Void) {
        if completed.contains(id) { trace("already_confirmed", true); completion(true, ""); return }
        if inFlight { trace("busy", false); completion(false, "正在确认填入结果，请稍候。"); return }
        if uncertain { trace("uncertain", false); completion(false, uncertaintyMessage); return }
        guard let target else { completion(false, "没有保存输入目标，请按 Esc 后重新录音。"); return }
        guard !text.isEmpty else { completion(false, "没有可填入的识别文字。"); return }
        let operation = epoch
        inFlight = true
        do { try access.validate(target, protectDraft: true) }
        catch { finish(id: id, operation: operation, path: "validation", confirmed: false,
                       uncertain: false, message: error.localizedDescription, completion: completion); return }
        guard epoch == operation else { return }
        let before = SelectionSnapshot(text: target.originalValue, range: target.originalRange)
        let expected = before.replacing(with: text)
        if before.text != nil, before.range != nil, expected == nil {
            finish(id: id, operation: operation, path: "invalid_selection", confirmed: false,
                   uncertain: false, message: "无法确认原输入框选区，请重新点击输入框后录音。", completion: completion)
            return
        }
        // 所选内容本就等于结果时不再注入，但仍要求完整原值和选区可读且未变。
        if expected != nil, expected == before.text {
            let current = access.snapshot(target)
            guard current.range != nil, before.unchanged(text: current.text, range: current.range) else {
                finish(id: id, operation: operation, path: "already_equal", confirmed: false,
                       uncertain: true, completion: completion)
                return
            }
            finish(id: id, operation: operation, path: "already_equal", confirmed: true,
                   uncertain: false, completion: completion)
            return
        }
        if target.prefersPaste {
            paste(id: id, text: text, target: target, operation: operation, completion: completion)
            return
        }
        let attribute: String
        let replacement: String
        if access.editable(target, attribute: kAXSelectedTextAttribute) {
            attribute = kAXSelectedTextAttribute
            replacement = text
        } else if let expected, access.editable(target, attribute: kAXValueAttribute) {
            attribute = kAXValueAttribute
            replacement = expected
        } else {
            paste(id: id, text: text, target: target, operation: operation, completion: completion)
            return
        }
        guard epoch == operation else { return }
        do { try access.validate(target, protectDraft: true) }
        catch { finish(id: id, operation: operation, path: "validation", confirmed: false,
                       uncertain: false, message: error.localizedDescription, completion: completion); return }
        guard epoch == operation else { return }
        uncertain = true
        access.write(target, attribute: attribute, text: replacement)
        verifyNative(id: id, text: text, target: target, attribute: attribute, operation: operation,
                     allowDelay: true, completion: completion)
    }

    private func verifyNative(id: UInt64, text: String, target: InputTarget, attribute: String,
                              operation: UInt64, allowDelay: Bool,
                              completion: @escaping (Bool, String) -> Void) {
        guard epoch == operation else { return }
        let path = attribute == kAXValueAttribute ? "ax_value" : "ax_selected_text"
        do { try access.validate(target, protectDraft: false) }
        catch {
            finish(id: id, operation: operation, path: path, confirmed: false,
                   uncertain: true, completion: completion)
            return
        }
        guard epoch == operation else { return }
        let before = SelectionSnapshot(text: target.originalValue, range: target.originalRange)
        let after = access.snapshot(target)
        guard epoch == operation else { return }
        switch DeliveryVerification.inspect(before: before, insertion: text, after: after) {
        case .confirmed:
            if attribute == kAXValueAttribute, let range = target.originalRange {
                access.moveCaret(target, to: range.location + (text as NSString).length)
                guard epoch == operation else { return }
                let confirmed = DeliveryVerification.inspect(before: before, insertion: text,
                                                              after: access.snapshot(target)) == .confirmed
                finish(id: id, operation: operation, path: path, confirmed: confirmed,
                       uncertain: !confirmed, completion: completion)
            } else {
                finish(id: id, operation: operation, path: path, confirmed: true,
                       uncertain: false, completion: completion)
            }
        case .uncertain:
            finish(id: id, operation: operation, path: path, confirmed: false,
                   uncertain: true, completion: completion)
        case .unchanged:
            if allowDelay {
                schedule(0.15) { [weak self] in
                    self?.verifyNative(id: id, text: text, target: target, attribute: attribute,
                                       operation: operation, allowDelay: false, completion: completion)
                }
            } else {
                trace(path, false)
                // 第二次仍完全未变，paste 内还会重新验证同一焦点、窗口及原草稿。
                paste(id: id, text: text, target: target, operation: operation,
                      followingAX: true, completion: completion)
            }
        }
    }

    private func paste(id: UInt64, text: String, target: InputTarget, operation: UInt64,
                       followingAX: Bool = false,
                       completion: @escaping (Bool, String) -> Void) {
        guard epoch == operation else { return }
        let restore: () -> Void
        do {
            try requireFallbackEnabled(target)
            try access.validate(target, protectDraft: true)
            guard epoch == operation else { return }
            try requireFallbackEnabled(target)
            uncertain = true
            restore = try access.paste(target, text: text)
        } catch {
            finish(id: id, operation: operation, path: "paste_preparation", confirmed: false,
                   uncertain: followingAX, message: error.localizedDescription, completion: completion)
            return
        }
        // 先到的精确结果无需等待最终检查；剪贴板仍至少保留原来的 750 ms。
        schedule(0.75, restore)
        guard epoch == operation else { return }
        if target.usesPasteFallback {
            pastedWithoutVerification = true
            finish(id: id, operation: operation, path: "restricted_paste", confirmed: false,
                   uncertain: true, message: uncertaintyMessage, completion: completion)
            return
        }
        var settled = false
        let verify: (Bool) -> Void = { [weak self] final in
            guard let self, self.epoch == operation, !settled else { return }
            let before = SelectionSnapshot(text: target.originalValue, range: target.originalRange)
            var confirmed = false
            do {
                try self.access.validate(target, protectDraft: false)
                guard self.epoch == operation else { return }
                confirmed = DeliveryVerification.inspect(before: before, insertion: text,
                                                          after: self.access.snapshot(target)) == .confirmed
            } catch {
                // 早期观察到焦点或安全状态改变后，本轮不能再晚到确认。
                guard self.epoch == operation else { return }
                settled = true
                self.finish(id: id, operation: operation, path: "paste", confirmed: false,
                            uncertain: true, completion: completion)
                return
            }
            guard self.epoch == operation, confirmed || final else { return }
            settled = true
            self.finish(id: id, operation: operation, path: "paste", confirmed: confirmed,
                        uncertain: !confirmed, completion: completion)
        }
        schedule(0.05) { verify(false) }
        schedule(0.35) { verify(true) }
    }

    private var uncertaintyMessage: String {
        if pastedWithoutVerification {
            return "已粘贴，无法核对。请检查原输入框；本轮不会再次粘贴。若没有出现文字，可使用“复制文字”自行填入。"
        }
        return "已尝试填入，但该控件没有返回可验证的完整结果。请检查原输入框；为避免重复，本轮不会再次自动填入。"
    }

    private func requireFallbackEnabled(_ target: InputTarget) throws {
        if target.usesPasteFallback && !pasteFallbackEnabled {
            throw NativeFailure("受限粘贴已关闭，本轮未粘贴。可使用“复制文字”自行填入。")
        }
    }

    private func finish(id: UInt64, operation: UInt64, path: String, confirmed: Bool,
                        uncertain: Bool, message: String? = nil,
                        completion: (Bool, String) -> Void) {
        guard epoch == operation else { return }
        self.uncertain = uncertain
        inFlight = false
        if confirmed { completed.insert(id) }
        trace(path, confirmed)
        completion(confirmed, confirmed ? "" : (message ?? uncertaintyMessage))
    }

    private func trace(_ path: String, _ confirmed: Bool) {
        NSLog("Vakyu delivery path=%@ confirmed=%@", path, confirmed ? "true" : "false")
    }
}
