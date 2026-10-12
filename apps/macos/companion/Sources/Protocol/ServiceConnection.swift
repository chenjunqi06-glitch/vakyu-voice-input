// 异步 JSON 行管道。音频与控制命令保持顺序，主线程不等待识别或翻译。
import Foundation

final class ServiceConnection {
    var onEvent: (([String: Any]) -> Void)?

    var onFailure: ((String) -> Void)?

    var onStopped: (() -> Void)?

    private var process: Process?

    private var input: FileHandle?

    private let writer = DispatchQueue(label: "app.vakyu.voice.protocol.writer", qos: .userInitiated)

    private let reader = DispatchQueue(label: "app.vakyu.voice.protocol.reader", qos: .userInitiated)

    private let lock = NSLock()

    private var bufferedWrites = 0

    private var stopping = false

    private var epoch: UInt64 = 0

    private var pendingEpoch: UInt64 = 0

    func launch() throws {
        guard process == nil else { throw NativeFailure("语音服务仍在关闭，请稍后重试。") }
        let paths = try ServicePaths()
        let child = Process()
        let incoming = Pipe()
        let outgoing = Pipe()
        let diagnostic = Pipe()
        child.executableURL = paths.executable
        child.arguments = paths.arguments
        child.standardInput = incoming
        child.standardOutput = outgoing
        child.standardError = diagnostic
        var environment = ProcessInfo.processInfo.environment
        environment["PYTHONUNBUFFERED"] = "1"
        environment["PYTHONUTF8"] = "1"
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        environment["PYTHONNOUSERSITE"] = "1"
        environment["OMP_NUM_THREADS"] = "2"
        child.environment = environment
        lock.lock()
        stopping = false
        epoch &+= 1
        let launchedEpoch = epoch
        lock.unlock()
        // stderr 不转写到用户日志；服务错误通过白名单事件展示。
        diagnostic.fileHandleForReading.readabilityHandler = { handle in
            if handle.availableData.isEmpty { handle.readabilityHandler = nil }
        }
        outgoing.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let chunk = handle.availableData
            if chunk.isEmpty { handle.readabilityHandler = nil; return }
            self?.reader.async { [weak self] in self?.parse(chunk, epoch: launchedEpoch) }
        }
        child.terminationHandler = { [weak self] terminated in
            DispatchQueue.main.async {
                guard let self, self.current(launchedEpoch), self.process === terminated else { return }
                self.process = nil
                self.lock.lock()
                self.input = nil
                let expected = self.stopping
                self.epoch &+= 1
                self.lock.unlock()
                if !expected { self.onFailure?("语音服务意外停止，正在自动恢复；若一直没有恢复，请在设置中点“重新启动语音服务”。") }
                self.onStopped?()
            }
        }
        try child.run()
        process = child
        lock.lock(); input = incoming.fileHandleForWriting; lock.unlock()
    }

    private var pending = Data()

    private func current(_ candidate: UInt64) -> Bool {
        lock.lock(); defer { lock.unlock() }
        return epoch == candidate
    }

    private func parse(_ chunk: Data, epoch launchedEpoch: UInt64) {
        guard current(launchedEpoch) else { return }
        if pendingEpoch != launchedEpoch {
            pending.removeAll(keepingCapacity: false)
            pendingEpoch = launchedEpoch
        }
        pending.append(chunk)
        guard pending.count < 4 * 1024 * 1024 else {
            pending.removeAll(keepingCapacity: false)
            fail("语音服务返回了过大的消息，本轮已停止。", epoch: launchedEpoch)
            return
        }
        while let end = pending.firstIndex(of: 10) {
            let line = pending[..<end]
            pending.removeSubrange(...end)
            guard !line.isEmpty,
                  let object = try? JSONSerialization.jsonObject(with: line),
                  let event = object as? [String: Any] else { continue }
            DispatchQueue.main.async { [weak self] in
                guard let self, self.current(launchedEpoch) else { return }
                self.onEvent?(event)
            }
        }
    }

    func send(_ object: [String: Any]) {
        guard var data = try? JSONSerialization.data(withJSONObject: object) else { return }
        data.append(10)
        lock.lock()
        guard let stream = input else { lock.unlock(); return }
        let sendingEpoch = epoch
        bufferedWrites += 1
        let overloaded = bufferedWrites > 100
        if overloaded { bufferedWrites -= 1 }
        lock.unlock()
        if overloaded {
            fail("语音处理跟不上录音速度，本轮已停止，请稍后重试。", epoch: sendingEpoch)
            return
        }
        writer.async { [weak self] in
            guard let self else { return }
            defer { self.lock.lock(); self.bufferedWrites -= 1; self.lock.unlock() }
            guard self.current(sendingEpoch) else { return }
            do { try stream.write(contentsOf: data) }
            catch { self.fail("无法连接语音服务，请重新启动语音服务。", epoch: sendingEpoch) }
        }
    }

    func sendAudio(generation: UInt64, data: Data) {
        send(["command": "audio", "generation": generation,
              "sample_rate": 16_000, "pcm": data.base64EncodedString()])
    }

    private func fail(_ message: String, epoch failedEpoch: UInt64) {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.current(failedEpoch) else { return }
            self.lock.lock(); let expected = self.stopping; self.lock.unlock()
            if !expected { self.onFailure?(message) }
        }
    }

    func stop() {
        lock.lock(); stopping = true; lock.unlock()
        send(["command": "shutdown"])
        guard let child = process else { onStopped?(); return }
        // 给服务机会关闭 Python；不能在主线程 waitUntilExit。
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + 2) {
            if child.isRunning { child.terminate() }
        }
    }
}
