// 系统本地 TTS；改变音色不调用云 API，也不引入 GPU 模型。
import AVFoundation

final class SpeechPlayer: NSObject, AVSpeechSynthesizerDelegate {
    var onFinished: ((UInt64, Bool) -> Void)?

    private let synthesizer = AVSpeechSynthesizer()

    private var active: (id: UInt64, utterance: AVSpeechUtterance)?

    private var watchdog: DispatchWorkItem?

    override init() {
        super.init()
        synthesizer.delegate = self
    }

    func play(id: UInt64, text: String, language: String, rate: Int) {
        cancel()
        let speech = AVSpeechUtterance(string: text)
        let key = language == "zh" ? "voice.chineseIdentifier" : "voice.englishIdentifier"
        let preferred = UserDefaults.standard.string(forKey: key) ?? ""
        speech.voice = AVSpeechSynthesisVoice(identifier: preferred)
            ?? AVSpeechSynthesisVoice(language: language == "zh" ? "zh-CN" : "en-US")
        guard speech.voice != nil, !text.isEmpty else { onFinished?(id, false); return }
        speech.rate = rate <= -2 ? 0.38 : rate < 0 ? 0.43 : AVSpeechUtteranceDefaultSpeechRate
        speech.preUtteranceDelay = 0.08
        speech.postUtteranceDelay = 0.1
        active = (id, speech)
        synthesizer.speak(speech)
        let timeout = DispatchWorkItem { [weak self, weak speech] in
            guard let self, let speech, self.active?.utterance === speech else { return }
            self.active = nil
            self.synthesizer.stopSpeaking(at: .immediate)
            self.onFinished?(id, false)
        }
        watchdog = timeout
        // 只防 TTS 系统回调永久丢失；按字数给足朗读时间，不限制用户录音时长。
        DispatchQueue.main.asyncAfter(deadline: .now() + max(30, Double(text.count) * 0.4 + 20), execute: timeout)
    }

    func cancel() {
        watchdog?.cancel(); watchdog = nil
        let previous = active
        active = nil
        synthesizer.stopSpeaking(at: .immediate)
        if let previous { onFinished?(previous.id, false) }
    }

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) {
        finish(utterance, ok: true)
    }

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didCancel utterance: AVSpeechUtterance) {
        finish(utterance, ok: false)
    }

    private func finish(_ utterance: AVSpeechUtterance, ok: Bool) {
        guard let current = active, current.utterance === utterance else { return }
        active = nil
        watchdog?.cancel(); watchdog = nil
        onFinished?(current.id, ok)
    }

    static func voices(language: String) -> [AVSpeechSynthesisVoice] {
        AVSpeechSynthesisVoice.speechVoices().filter { $0.language.hasPrefix(language) }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }
}
