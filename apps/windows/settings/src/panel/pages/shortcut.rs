//! 「快捷键」页：语音长按、翻页键、模式键，译词 / 删候选 / 翻译选中文字的修饰键。
//! 翻译选中文字只改修饰键，字母键固定用配置里当前的；要换字母直接改 `config.toml`。

use qingjian_platform::{Modifiers, VoiceKey};
use windows_reactor::{
    Button, Callback, ChildrenControl, Color, ComboBox, ContentControl, KeyedView, Orientation,
    StackPanel, TextBlock, TextWrapping, ToggleSwitch, View, ViewContext,
};

use crate::panel::controls::{feedback, field, index_of, page};
use crate::panel::{Message, Settings};

/// 翻页键对：界面名 + 配置写法。
pub(crate) const PAGE_KEYS: [(&str, &str); 3] = [
    ("方括号 [ ]", "[]"),
    ("逗号句号 , .", ",."),
    ("减号等号 - =", "-="),
];

/// 可当模式键的字母（与 Core `ModeKeys::CANDIDATES` 一致）。
pub(crate) const MODE_KEYS: [&str; 3] = ["v", "u", "i"];

/// 修饰键预设：界面名 + 配置写法。
pub(crate) const MODIFIERS: [(&str, &str); 6] = [
    ("Ctrl", "ctrl"),
    ("Alt", "alt"),
    ("Shift", "shift"),
    ("Ctrl + Shift", "shift+ctrl"),
    ("Ctrl + Alt", "ctrl+alt"),
    ("Alt + Shift", "shift+alt"),
];

/// 语音触发键：界面名、配置值与序列化写法。
pub(crate) const VOICE_KEYS: [(&str, VoiceKey, &str); 3] = [
    ("左 Alt（推荐）", VoiceKey::LeftAlt, "left_alt"),
    ("右 Alt", VoiceKey::RightAlt, "right_alt"),
    ("左右 Alt", VoiceKey::Alt, "alt"),
];

/// 保留手动配置的时长，避免设置页把它显示成另一个预设。
pub(crate) fn voice_hold_options(current: u32) -> Vec<(String, u32)> {
    let mut options = vec![
        ("快速（100 毫秒）".to_owned(), 100),
        ("标准（100 毫秒，推荐）".to_owned(), 100),
        ("稳妥（300 毫秒）".to_owned(), 300),
        ("减少误触（500 毫秒）".to_owned(), 500),
    ];
    if !options.iter().any(|(_, value)| *value == current) {
        options.push((format!("自定义（{current} 毫秒）"), current));
    }
    options
}

fn mode_combo(current: char, callback: Callback<Option<usize>>) -> ComboBox {
    let selected = MODE_KEYS
        .iter()
        .position(|key| key.starts_with(current))
        .unwrap_or(0);
    ComboBox::new()
        .items_source(MODE_KEYS)
        .selected_index(selected)
        .on_selection_changed(callback)
}

/// 按解析后相等找当前项，不依赖字符串写法。
fn modifier_combo(current: Modifiers, callback: Callback<Option<usize>>) -> ComboBox {
    let selected = MODIFIERS
        .iter()
        .position(|(_, value)| value.parse::<Modifiers>().ok() == Some(current))
        .unwrap_or(0);
    ComboBox::new()
        .items_source(MODIFIERS.iter().map(|(label, _)| *label))
        .selected_index(selected)
        .on_selection_changed(callback)
}

pub(crate) fn view(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let s = &settings.config.shortcut;
    let hold_ms = s.voice_hold_ms();
    let hold_options = voice_hold_options(hold_ms);
    let rows = [
        field(
            "启用语音输入",
            "同时控制普通语音和口语跟读。普通语音松键后直接填入；口语跟读需读完译句后填入。两者使用独立按键，都不会自动发送，语音均在本机识别。",
            ToggleSwitch::new()
                .is_on(s.voice_input)
                .on_toggled(context.callback(Message::VoiceInput)),
        ),
        field(
            "普通语音转文字按键",
            "推荐左 Alt：按住说话，松开直接填入原话，不翻译。开启口语跟读后仍可独立使用。左右 Alt 均开启时，右 Ctrl + 右 Alt 的跟读组合会保留右 Alt，普通语音使用左 Alt。改动自动保存并生效；单 Alt 会占用长按菜单键的原功能。",
            voice_key_control(settings, context),
        ),
        field(
            "启用跟读练习 / Reading practice",
            "默认关闭，可随时开关。仅使用下方「口语跟读快捷键」：按学习方向录原话，松键后显示译句并示范，随后自动录音跟读。完整读完才填入原话，不会自动发送；Esc 取消。翻译使用「云服务」中的服务，只发送本次文字。普通语音转文字不受此开关影响。",
            ToggleSwitch::new()
                .is_on(s.voice_english_practice)
                .on_toggled(context.callback(Message::VoiceEnglishPractice)),
        ),
        field(
            "学习英语 / Learn English",
            "开启：说中文 → 跟读英文 → 填入原中文。关闭：说英文 → 跟读中文（附拼音、英文词义）→ 填入原英文。Off: speak English, practise Chinese, then insert your original English. 方向保存后立即生效；切换时会取消尚未完成的一轮。",
            ToggleSwitch::new()
                .is_on(s.voice_practice_learn_english)
                .on_toggled(context.callback(Message::VoicePracticeLearnEnglish)),
        ),
        field(
            "读完发出译文 / Send the translation",
            "开启：说中文 → 跟读英文 → 填入英文（学中文时：说英文 → 跟读中文 → 填入中文）。关闭：填入原话。悬浮状态条上的「跟读」「发英」两格可直接切换，不用进设置。",
            ToggleSwitch::new()
                .is_on(s.voice_practice_send_translation)
                .on_toggled(context.callback(Message::VoicePracticeSendTranslation)),
        ),
        field(
            "口语跟读快捷键",
            "推荐右 Ctrl + 右 Alt：开启跟读后，两键同时按住录原话（顺序不限），松开进入跟读。也支持 Ctrl+Alt+E、F 功能键或与普通语音不同的单侧 Alt。录入后全部松开再保存，不允许单独字母、数字或空格。保存检查系统注册占用和两个语音入口的冲突；不能完整区分左右修饰键或检测软件内部快捷键。",
            practice_shortcut_control(settings, context),
        ),
        field(
            "跟读示范朗读",
            "默认开启。按学习方向在本机慢速读一遍英文或中文，示范结束后自动开启麦克风跟读；关闭后直接开始跟读。需本机安装对应语言的朗读声音。",
            ToggleSwitch::new()
                .is_on(s.voice_practice_demo)
                .on_toggled(context.callback(Message::VoicePracticeDemo)),
        ),
        field(
            "启动灵敏度",
            "按键保持多久才开始录音。时间越短启动越快，越长越不易误触；更改后下次录音生效。",
            ComboBox::new()
                .items_source(hold_options.iter().map(|(label, _)| label.as_str()))
                .selected_index(hold_options.iter().position(|(_, value)| *value == hold_ms))
                .is_enabled(s.voice_input)
                .on_selection_changed(context.callback(Message::VoiceHold)),
        ),
        field(
            "翻页键",
            "选「, .」或「- =」时组句中敲对应符号是翻页，不再是上屏加标点。",
            ComboBox::new()
                .items_source(PAGE_KEYS.iter().map(|(label, _)| *label))
                .selected_index(index_of(&PAGE_KEYS, &settings.config.general.page_keys))
                .on_selection_changed(context.callback(Message::PageKeys)),
        ),
        field(
            "表达式模式键",
            "",
            mode_combo(s.mode.expression, context.callback(Message::ModeExpression)),
        ),
        field(
            "问字模式键",
            "这两个字母开头进模式：v1+2 出 3，usangemu 问「三个木」（需要云服务）。两个键不能相同。",
            mode_combo(s.mode.question, context.callback(Message::ModeQuestion)),
        ),
        field(
            "没在输入拼音时敲 ? 也进入问字",
            "开着时 ? 先进问字（中英文模式都行），后面跟字母才是问题，跟其他键时还原成问号；关着问号就是问号。",
            ToggleSwitch::new()
                .is_on(s.mode.question_mark)
                .on_toggled(context.callback(Message::QuestionMark)),
        ),
        field(
            "译词上屏（第一个）",
            "按住修饰键再按候选序号，上屏候选右侧的译词而不是中文。",
            modifier_combo(s.translation, context.callback(Message::Translation)),
        ),
        field(
            "译词上屏（第二个）",
            "候选有两个译词时，这组键上屏后一个。两组不能相同。Ctrl + Shift 是 Windows 切换输入法的热键，同时装着别的输入法时别选它。",
            modifier_combo(
                s.translation_second,
                context.callback(Message::TranslationSecond),
            ),
        ),
        field(
            "删除候选",
            "按住修饰键再按候选序号：自己造的词、云端选过的词整删；词库里的词清掉学习记录，回到原排序。",
            modifier_combo(
                s.delete_candidate,
                context.callback(Message::DeleteCandidate),
            ),
        ),
        field(
            "翻译选中文字",
            "选中一段文字后按这组键 + 当前字母（缺省 Ctrl+Alt+T），把它译成学习语言，回车 / 空格替换、Esc 保留原文。需要云服务。这里只改修饰键，字母固定用当前的。",
            modifier_combo(
                s.translate_selection.modifiers,
                context.callback(Message::TranslateSelection),
            ),
        ),
        field(
            "选中文字浮出「译」图标",
            "在任意软件里用鼠标选中文字后，鼠标旁出现「译」小图标，点它弹出译文，可一键复制。需要云服务的 API 密钥；会临时借用剪贴板读取选中的文字，读完恢复原来的文字内容。",
            ToggleSwitch::new()
                .is_on(s.translate_selection_icon)
                .on_toggled(context.callback(Message::TranslateSelectionIcon)),
        ),
    ];
    page("快捷键", StackPanel::new().spacing(16.0).children(rows))
}

fn voice_key_control(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let selector: View = StackPanel::new().keyed_children([KeyedView::new(
        format!("voice-key-{}", settings.voice_key_revision),
        ComboBox::new()
            .items_source(VOICE_KEYS.iter().map(|(label, _, _)| *label))
            .selected_index(
                VOICE_KEYS
                    .iter()
                    .position(|(_, key, _)| *key == settings.config.shortcut.voice_key),
            )
            .is_enabled(!settings.hotkey.recording && !settings.hotkey.saving)
            .on_selection_changed(context.callback(Message::VoiceKey)),
    )]);
    StackPanel::new().spacing(8.0).children([
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .children([
                selector,
                Button::new()
                    .is_enabled(!settings.hotkey.recording && !settings.hotkey.saving)
                    .on_click(context.message(Message::VoiceKeyReset))
                    .content("恢复默认（左 Alt）")
                    .into(),
            ]),
        feedback(&settings.voice_key_notice),
    ])
}

fn practice_shortcut_control(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let editor = &settings.hotkey;
    let status: View = TextBlock::new()
        .text(if editor.message.is_empty() {
            format!(
                "当前：{}",
                settings.config.shortcut.voice_practice_shortcut.label()
            )
        } else {
            editor.message.clone()
        })
        .text_wrapping(TextWrapping::Wrap)
        .foreground(if editor.failed {
            Color::rgb(255, 174, 185)
        } else {
            crate::panel::theme::TEXT
        })
        .into();
    StackPanel::new().spacing(8.0).children([
        TextBlock::new()
            .text(format!(
                "当前：{}",
                settings.config.shortcut.voice_practice_shortcut.label()
            ))
            .into(),
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .children([
                Button::new()
                    .is_enabled(!editor.recording && !editor.saving)
                    .on_click(context.message(Message::PracticeShortcutRecord))
                    .content(if editor.recording {
                        "等待按键…"
                    } else if editor.candidate.is_some() {
                        "重新录入快捷键"
                    } else {
                        "按下新的快捷键"
                    }),
                Button::new()
                    .is_enabled(editor.candidate.is_some() && !editor.recording && !editor.saving)
                    .on_click(context.message(Message::PracticeShortcutSave))
                    .content("保存"),
                Button::new()
                    .is_enabled(!editor.saving)
                    .on_click(context.message(Message::PracticeShortcutReset))
                    .content("恢复默认"),
                Button::new()
                    .is_enabled(editor.recording)
                    .on_click(context.message(Message::PracticeShortcutCancel))
                    .content("取消录入"),
            ]),
        status,
    ])
}
