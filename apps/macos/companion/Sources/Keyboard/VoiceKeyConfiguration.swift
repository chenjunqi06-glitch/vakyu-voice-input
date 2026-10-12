// 配置热更新延迟到整组按键松开，不能改变已经开始的普通录音入口。
struct VoiceKeyConfiguration {
    let enabled: Bool

    let practiceEnabled: Bool

    let dictationKey: String

    let shortcut: Shortcut
}
