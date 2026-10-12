// 入口在按键按下时确定，本轮后续释放或配置变化都不能改成另一种入口。
enum VoiceEntryMode: String {
    case dictation

    case practice
}
