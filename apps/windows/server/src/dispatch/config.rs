use qingjian_platform::protocol::KeyModifiers;
use qingjian_platform::{
    AppsConfig, CandidateRenderer, Config, KeyCombo, LayoutMode, PracticeShortcut, PreeditMode,
    Scheme, SwitchKeys, ThemeMode, VoiceKey,
};

use super::RenderSettings;

/// Router 要用的配置项，与 macOS 壳的 `Host` 字段对齐。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterConfig {
    /// 每页候选数（`[general] page_size`）。
    pub page_size: usize,

    /// 云端候选在第一页预留的格数（`[predict] slots`）。
    pub cloud_slots: usize,

    /// 联网翻译 / 联想总开关（`[predict] enabled`），供托盘即时展示。
    pub cloud_enabled: bool,

    /// 中文模式下 Shift+字母收进组句缓冲区（`[general] shift_letter = "compose"`）。
    /// 关着（缺省）时壳把大写字母交给应用，与以前一致。
    pub shift_letter_compose: bool,

    /// 候选排布（`[general] layout`）。
    pub layout: LayoutMode,

    /// 候选窗口外观（`[general] theme`）。
    pub theme: ThemeMode,

    /// 候选窗口 / 状态条由 Miracle 渲染器还是 GDI 画（`[general] renderer`）。
    pub renderer: CandidateRenderer,

    /// 候选窗口字体的字族名（`[general] font`），空为系统字体；只对 Miracle 渲染器生效。
    pub font: String,

    /// 拼音显示位置（`[general] preedit`）。
    pub preedit: PreeditMode,

    /// 翻页键对（`[general] page_keys`，上一页 / 下一页）。
    pub page_keys: (char, char),

    /// 英文模式给不给英文候选（`[general] english_candidates`）。
    pub english_candidates: bool,

    /// 内置英文模式总开关（`[general] english_mode`）：关掉后状态条上的「中 / 英」不再切模式
    /// （切换键与语言栏按钮由 DLL 按同一项拦住，见 `com::service::mode`）。
    pub english_mode: bool,

    /// 中英切换键（`[shortcut] switch_mode`）：由 Server 经协议下发给 DLL，由它认键。
    pub switch_mode: SwitchKeys,

    /// Windows 按住 Alt 语音输入开关。
    pub voice_input: bool,

    pub voice_key: VoiceKey,

    pub voice_hold_ms: u32,

    /// 自愿开启的英语跟读；完整跟读后才填入本轮中文。
    pub voice_english_practice: bool,

    pub voice_practice_learn_english: bool,

    /// 跟读读完后填入译文（说中文、读英文、发英文）；关着填入原话。
    pub voice_practice_send_translation: bool,

    pub voice_practice_demo: bool,

    /// 示范用随包自然人声（`[shortcut] voice_practice_natural_voice`）。
    pub voice_practice_natural_voice: bool,

    /// 自然人声的音色编号（`[shortcut] voice_practice_speaker`）。
    pub voice_practice_speaker: u16,

    pub voice_practice_shortcut: PracticeShortcut,

    pub voice_practice_similarity: u8,

    pub voice_practice_name_similarity: u8,

    /// 中文模式下不在组句时的标点转全角（`[general] full_width_punctuation`）；状态条可切。
    pub full_width: bool,

    /// 英文模式的那一份（`[general] english_full_width_punctuation`）。
    pub english_full_width: bool,

    /// 大千注音（[general] zhuyin）。
    pub zhuyin: bool,

    /// 按应用的设置（`[apps]`），按宿主 exe 名认。
    pub apps: AppsConfig,

    /// 上屏第一 / 第二个译词的修饰键（`[shortcut] translation` / `translation_second`）。
    pub translation_keys: (KeyModifiers, KeyModifiers),

    /// 删候选的修饰键（`[shortcut] delete_candidate`）。
    pub delete_keys: KeyModifiers,

    /// 「翻译选中文字」快捷键（`[shortcut] translate_selection`）。
    pub translate_selection: KeyCombo,

    /// 任意软件选中文字后浮出「译」小图标（`[shortcut] translate_selection_icon`）。
    pub translate_selection_icon: bool,

    /// 悬浮状态条开关（`[status_bar] enabled`）。
    pub status_enabled: bool,

    /// 状态条记住的位置（`[status_bar] x` / `y`，内容左上角物理像素）。
    pub status_pos: Option<(i32, i32)>,

    /// 拼音侧方案（`[general] scheme`）。
    pub scheme: Scheme,

    /// 形码侧开没开（`[general] wubi`）。与拼音同时开着就是混输。
    pub wubi: bool,

    /// 辅码触发键（`[general] aux_code_key`，缺省 `;`）；非法值退回缺省。
    pub aux_code_key: char,

    /// 候选上是否显示辅码（`[general] aux_code_show`）。随帧下发给候选窗。
    pub aux_code_show: bool,
}

impl RouterConfig {
    /// 全局开关开着，且应用不在 `[apps] english_candidates_off` 里；没报 exe 名按不关。
    pub fn english_candidates_in(&self, app: Option<&str>) -> bool {
        self.english_candidates && !app.is_some_and(|app| self.apps.english_candidates_off(app))
    }

    /// 交给 UI 线程的画法。
    pub fn render_settings(&self) -> RenderSettings {
        RenderSettings {
            renderer: self.renderer,
            font: self.font.clone(),
        }
    }
}

impl From<&Config> for RouterConfig {
    fn from(config: &Config) -> Self {
        Self {
            page_size: config.general.page_size(),
            cloud_slots: config.predict.slots,
            cloud_enabled: config.predict.enabled,
            shift_letter_compose: config.general.shift_letter.compose(),
            layout: config.general.layout,
            theme: config.general.theme,
            renderer: config.general.renderer,
            font: config.general.font.trim().to_owned(),
            preedit: config.general.preedit,
            page_keys: config.general.page_keys(),
            english_candidates: config.general.english_candidates,
            english_mode: config.general.english_mode,
            switch_mode: config.shortcut.switch_mode,
            voice_input: config.shortcut.voice_input,
            voice_key: config.shortcut.voice_key,
            voice_hold_ms: config.shortcut.voice_hold_ms(),
            voice_english_practice: config.shortcut.voice_english_practice,
            voice_practice_learn_english: config.shortcut.voice_practice_learn_english,
            voice_practice_send_translation: config.shortcut.voice_practice_send_translation,
            voice_practice_demo: config.shortcut.voice_practice_demo,
            voice_practice_natural_voice: config.shortcut.voice_practice_natural_voice,
            voice_practice_speaker: config.shortcut.voice_practice_speaker,
            voice_practice_shortcut: config.shortcut.voice_practice_shortcut,
            voice_practice_similarity: config.shortcut.voice_practice_similarity.min(100),
            voice_practice_name_similarity: config.shortcut.voice_practice_name_similarity.min(100),
            full_width: config.general.full_width_punctuation,
            english_full_width: config.general.english_full_width_punctuation,
            zhuyin: config.general.is_zhuyin(),
            apps: config.apps.clone(),
            translation_keys: {
                let (first, second) = config.shortcut.translation_keys();
                (first.into(), second.into())
            },
            delete_keys: config.shortcut.delete_keys().into(),
            translate_selection: config.shortcut.translate_selection,
            translate_selection_icon: config.shortcut.translate_selection_icon,
            status_enabled: config.status_bar.enabled,
            status_pos: config.status_bar.x.zip(config.status_bar.y),
            scheme: config.general.scheme(),
            wubi: config.general.wubi(),
            aux_code_key: config.general.aux_code_key(),
            aux_code_show: config.general.aux_code_show,
        }
    }
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self::from(&Config::default())
    }
}
