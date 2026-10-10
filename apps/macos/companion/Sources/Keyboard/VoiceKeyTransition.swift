// 纯按键状态机产生的转换，不访问系统键盘或识别服务。
enum VoiceKeyTransition: Equatable {
    case none

    case pressed(VoiceEntryMode)

    case released(VoiceEntryMode)

    case interrupted(VoiceEntryMode)
}
