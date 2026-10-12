// AVAudioEngine 只在当前代次收音；固定 16 kHz 单声道送共享识别器。
import AVFoundation

final class MicrophoneCapture {
    var onAudio: ((UInt64, Data) -> Void)?

    var onLevel: ((Float) -> Void)?

    var onFailure: ((UInt64, String) -> Void)?

    private let engine = AVAudioEngine()

    private let queue = DispatchQueue(label: "app.vakyu.voice.audio", qos: .userInitiated)

    private let gate = NSLock()

    private var requestedGeneration: UInt64?

    private var activeGeneration: UInt64?

    private var activeConverter: AVAudioConverter?

    private var activeOutput: AVAudioFormat?

    private var accepting = false

    private var epoch: UInt64 = 0

    private var tapInstalled = false

    private var queuedBuffers = 0

    private var meterAt = 0.0

    func start(generation: UInt64) {
        gate.lock()
        epoch &+= 1
        let captureEpoch = epoch
        requestedGeneration = generation; accepting = true
        gate.unlock()
        queue.async { [weak self] in self?.begin(generation: generation, epoch: captureEpoch) }
    }

    func stop(epoch requiredEpoch: UInt64? = nil) {
        // 先失效代次，排队中的老音频绝不会混入下一轮。
        gate.lock()
        if let requiredEpoch, epoch != requiredEpoch { gate.unlock(); return }
        let captureEpoch = epoch
        requestedGeneration = nil; accepting = false
        gate.unlock()
        queue.async { [weak self] in
            guard let self, self.isEpoch(captureEpoch) else { return }
            self.end()
        }
    }

    func finish(_ completion: @escaping () -> Void) {
        // 释放快捷键时先排空已经采到的声音，再把 release 排入协议管道。
        gate.lock()
        accepting = false
        let generation = requestedGeneration
        let captureEpoch = epoch
        queue.async { [weak self] in
            guard let self else { DispatchQueue.main.async(execute: completion); return }
            if let generation, self.isCurrent(generation, epoch: captureEpoch) { self.flush(generation: generation) }
            if self.isEpoch(captureEpoch) { self.end() }
            self.gate.lock()
            if self.epoch == captureEpoch && self.requestedGeneration == generation { self.requestedGeneration = nil }
            self.gate.unlock()
            DispatchQueue.main.async(execute: completion)
        }
        gate.unlock()
    }

    private func isCurrent(_ generation: UInt64, epoch captureEpoch: UInt64) -> Bool {
        gate.lock(); defer { gate.unlock() }
        return requestedGeneration == generation && epoch == captureEpoch
    }

    private func isEpoch(_ captureEpoch: UInt64) -> Bool {
        gate.lock(); defer { gate.unlock() }
        return epoch == captureEpoch
    }

    private func begin(generation: UInt64, epoch captureEpoch: UInt64) {
        guard isCurrent(generation, epoch: captureEpoch) else { return }
        end()
        guard AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else {
            fail("请在语音设置中允许麦克风权限。", generation: generation, epoch: captureEpoch)
            return
        }
        let node = engine.inputNode
        let input = node.outputFormat(forBus: 0)
        guard input.sampleRate > 0, input.channelCount > 0,
              let output = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000,
                                         channels: 1, interleaved: false),
              let converter = AVAudioConverter(from: input, to: output) else {
            fail("找不到可用的麦克风，请检查系统声音设置。", generation: generation, epoch: captureEpoch)
            return
        }
        converter.downmix = true
        activeGeneration = generation
        activeConverter = converter
        activeOutput = output
        meterAt = 0
        node.installTap(onBus: 0, bufferSize: 2048, format: input) { [weak self] buffer, _ in
            guard let self else { return }
            self.gate.lock()
            guard self.requestedGeneration == generation, self.epoch == captureEpoch, self.accepting else { self.gate.unlock(); return }
            self.queuedBuffers += 1
            let overloaded = self.queuedBuffers > 12
            if overloaded {
                self.queuedBuffers -= 1; self.gate.unlock()
                self.stop(epoch: captureEpoch)
                self.fail("电脑暂时无法及时处理麦克风音频，本轮已停止，请稍后重试。", generation: generation, epoch: captureEpoch)
                return
            }
            // 引擎会重用 tap 缓冲区，入队前复制；转换与编码不占音频回调线程。
            guard let copy = AVAudioPCMBuffer(pcmFormat: buffer.format, frameCapacity: buffer.frameLength) else {
                self.queuedBuffers -= 1; self.gate.unlock()
                return
            }
            copy.frameLength = buffer.frameLength
            let source = UnsafeMutableAudioBufferListPointer(buffer.mutableAudioBufferList)
            let destination = UnsafeMutableAudioBufferListPointer(copy.mutableAudioBufferList)
            for index in source.indices {
                if let from = source[index].mData, let to = destination[index].mData {
                    memcpy(to, from, Int(source[index].mDataByteSize))
                }
            }
            self.queue.async {
                defer { self.gate.lock(); self.queuedBuffers -= 1; self.gate.unlock() }
                guard self.isCurrent(generation, epoch: captureEpoch) else { return }
                self.convert(copy, with: converter, output: output, generation: generation, epoch: captureEpoch)
            }
            self.gate.unlock()
        }
        tapInstalled = true
        do { engine.prepare(); try engine.start() }
        catch { end(); fail("无法启动麦克风，请检查设备是否被断开或被系统禁用。", generation: generation, epoch: captureEpoch) }
    }

    private func convert(_ input: AVAudioPCMBuffer, with converter: AVAudioConverter,
                         output: AVAudioFormat, generation: UInt64, epoch captureEpoch: UInt64) {
        let capacity = AVAudioFrameCount(ceil(Double(input.frameLength) * 16_000 / input.format.sampleRate) + 32)
        guard let converted = AVAudioPCMBuffer(pcmFormat: output, frameCapacity: capacity) else { return }
        var consumed = false
        var error: NSError?
        let status = converter.convert(to: converted, error: &error) { _, state in
            if consumed { state.pointee = .noDataNow; return nil }
            consumed = true; state.pointee = .haveData; return input
        }
        guard status != .error, error == nil, let samples = converted.floatChannelData?[0] else {
            stop(epoch: captureEpoch); fail("麦克风格式转换失败，请重新选择系统默认麦克风。", generation: generation, epoch: captureEpoch)
            return
        }
        let count = Int(converted.frameLength)
        guard count > 0, isCurrent(generation, epoch: captureEpoch) else { return }
        let data = Data(bytes: samples, count: count * MemoryLayout<Float>.size)
        // Apple Silicon 和 Intel macOS 都使用 little-endian float32。
        onAudio?(generation, data)
        let now = ProcessInfo.processInfo.systemUptime
        if now - meterAt >= 1.0 / 30.0 {
            meterAt = now
            var squared: Float = 0
            for index in 0..<count { squared += samples[index] * samples[index] }
            let rms = sqrt(squared / Float(count))
            let level = min(1, max(0, (20 * log10(max(rms, 0.000_01)) + 55) / 50))
            DispatchQueue.main.async { [weak self] in
                guard let self, self.isCurrent(generation, epoch: captureEpoch) else { return }
                self.onLevel?(level)
            }
        }
    }

    private func end() {
        engine.stop()
        if tapInstalled { engine.inputNode.removeTap(onBus: 0); tapInstalled = false }
        activeGeneration = nil
        activeConverter = nil; activeOutput = nil
    }

    private func flush(generation: UInt64) {
        guard let converter = activeConverter, let format = activeOutput else { return }
        // 重采样器可能持有最后几个采样点，送 endOfStream 后再发送 release。
        for _ in 0..<4 {
            guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 2048) else { return }
            var error: NSError?
            let status = converter.convert(to: buffer, error: &error) { _, state in
                state.pointee = .endOfStream; return nil
            }
            if let samples = buffer.floatChannelData?[0], buffer.frameLength > 0 {
                onAudio?(generation, Data(bytes: samples, count: Int(buffer.frameLength) * 4))
            }
            if status == .endOfStream || status == .error || buffer.frameLength == 0 { return }
        }
    }

    private func fail(_ message: String, generation: UInt64, epoch failedEpoch: UInt64) {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.isEpoch(failedEpoch) else { return }
            self.onFailure?(generation, message)
        }
    }
}
