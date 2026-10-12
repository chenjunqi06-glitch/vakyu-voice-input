//! 语音与双向跟读开关；快捷键和系统权限由常驻语音程序统一管理。

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSButton;
use qingjian_platform::Config;

use crate::preferences::controls::{
    GROUP_GAP, button, checkbox, note_full, row_checkbox, set_checked,
};
use crate::preferences::layout::{Layout, PAGE_PADDING, ROW_HEIGHT};
use crate::preferences::setting::Setting;
use crate::preferences::target::PreferencesTarget;

pub struct VoicePage {
    enabled: Retained<NSButton>,

    practice: Retained<NSButton>,

    learn_english: Retained<NSButton>,

    demo: Retained<NSButton>,
}

impl VoicePage {
    pub fn build(layout: &mut Layout, mtm: MainThreadMarker, target: &PreferencesTarget) -> Self {
        let enabled = checkbox(mtm, "启用全局语音输入", Setting::VoiceInput, target);
        row_checkbox(layout, &enabled);
        note_full(
            layout,
            mtm,
            "普通语音默认按住左 Option，松开直接填入文字，不进入跟读。可跨应用使用，不必切换输入源；只填入，不自动发送。",
        );
        layout.space(GROUP_GAP);
        let practice = checkbox(
            mtm,
            "启用跟读：读完整句后再填入原话",
            Setting::VoicePractice,
            target,
        );
        row_checkbox(layout, &practice);
        let learn_english = checkbox(
            mtm,
            "学习英语（关闭后学习中文）",
            Setting::LearnEnglish,
            target,
        );
        row_checkbox(layout, &learn_english);
        note_full(
            layout,
            mtm,
            "学习英语：说中文、跟读英文、填入原中文。学习中文：说英文、跟读中文、填入原英文；中文配拼音和英文词义。",
        );
        let demo = checkbox(mtm, "跟读前用本机声音示范朗读", Setting::VoiceDemo, target);
        row_checkbox(layout, &demo);
        note_full(
            layout,
            mtm,
            "点击词语查看离线释义和发音。读错可以重试、跳过或点“我读对了”；已读进度保留。Esc 取消，点击窗口内外都不会关闭练习。",
        );
        layout.space(GROUP_GAP);
        let settings = button(
            mtm,
            "快捷键、音色与麦克风权限…",
            Setting::OpenVoiceSettings,
            target,
        );
        layout.place(&settings, PAGE_PADDING, 300.0, ROW_HEIGHT + 6.0);
        layout.next_row(ROW_HEIGHT + 6.0);
        note_full(
            layout,
            mtm,
            "跟读开启后，默认按住右 Control＋右 Option 进入跟读；左 Option 仍只做普通语音。跟读组合可修改并检查系统注册冲突；其他应用内部按键和纯修饰键无法完整检测。",
        );
        layout.space(GROUP_GAP);
        note_full(
            layout,
            mtm,
            "语音识别、词典和示范发音在本机完成，不使用显卡。只有开启跟读后的一次整句翻译使用云服务；关闭云联想仍可跟读。先在“云服务”中设置接口和密钥。",
        );
        Self {
            enabled,
            practice,
            learn_english,
            demo,
        }
    }

    pub fn sync(&self, config: &Config) {
        let keys = &config.shortcut;
        set_checked(&self.enabled, keys.voice_input);
        set_checked(&self.practice, keys.voice_english_practice);
        set_checked(&self.learn_english, keys.voice_practice_learn_english);
        set_checked(&self.demo, keys.voice_practice_demo);
    }
}
