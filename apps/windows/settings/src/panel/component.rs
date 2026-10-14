//! 根组件的 Reactor 生命周期：建状态、按消息落盘、画左侧导航 + 当前页。

use qingjian_platform::{
    CandidateRenderer, Config, DEFAULT_ENGLISH_CANDIDATES_OFF_WINDOWS, LayoutMode, LogLevel,
    PreeditMode, ShiftLetter, ThemeMode,
};
use windows_reactor::{Component, ComponentContext, ElementRef, View, ViewContext};

use super::cloud_status::CloudStatus;
use super::controls::{export_logs, log_dir, open_in_editor, open_with_explorer};
use super::notice::Notice;
use super::pages::{about, aux_code, cloud, dictionaries, general, shortcut};
use super::recorder::Recorder;
use super::{Message, Settings};

impl Component for Settings {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
        let path = Self::config_path();
        Self::ensure_config_file(&path);
        let config = Config::load(&path).unwrap_or_default();
        super::sync::watch(&path, context);
        schedule_window_chrome(context, 0);
        Self {
            config,
            path,
            page: "general".to_string(),
            cloud_status: CloudStatus::Idle,
            recorder: Recorder::Idle,
            hotkey: super::hotkey::HotkeyEditor::default(),
            voice_key_notice: Notice::default(),
            voice_key_revision: 0,
            record_box: ElementRef::new(),
            notice: Notice::default(),
            update_state: Self::update_state_path()
                .map(|path| qingjian_update::UpdateState::load(&path))
                .unwrap_or_default(),
            update_checking: false,
            update_error: None,
            dictionary_status: String::new(),
            families: qingjian_render::system_fonts::families(),
            font_query: None,
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::ApplyWindowChrome(attempt) => {
                if !crate::window_chrome::attach(
                    super::theme::HEADER_HEIGHT,
                    super::theme::CAPTION_BUTTONS_WIDTH,
                ) {
                    if attempt < 29 {
                        schedule_window_chrome(context, attempt + 1);
                    } else {
                        crate::log::warn("设置窗口无边框样式未能初始化".to_owned());
                    }
                }
            }
            Message::WindowDrag(true) => crate::window_chrome::begin_drag(),
            Message::WindowMinimize => crate::window_chrome::minimize(),
            Message::WindowMaximize => crate::window_chrome::maximize_or_restore(),
            Message::WindowClose => {
                let _ = context.window().request_close();
            }
            Message::ReloadConfig => {
                self.reload();
                super::sync::watch(&self.path, context);
            }
            Message::Navigate(Some(tag)) => {
                self.hotkey.cancel();
                self.reload();
                self.page = tag;
                // 上一页的导入提示不跟着过来
                self.notice.clear();
                self.voice_key_notice.clear();
            }
            Message::Navigate(None) => {}

            // 通用页
            Message::LearningLanguage(Some(i)) if i < general::LANGUAGES.len() => {
                self.save("general", "learning_language", general::LANGUAGES[i].1);
            }
            Message::PageSize(Some(value)) => {
                let size = (value.round() as i64).clamp(1, 9);
                self.save("general", "page_size", size);
            }
            Message::Scheme(Some(i)) if i < general::SCHEMES.len() => {
                self.save("general", "scheme", general::SCHEMES[i].1);
            }
            Message::ShuangpinRawPreedit(on) => self.save("general", "shuangpin_raw_preedit", on),
            Message::Wubi(on) => self.save("general", "wubi", if on { "wubi86" } else { "" }),
            Message::Traditional(on) => self.save("general", "traditional", on),
            Message::EnglishCandidates(on) => self.save("general", "english_candidates", on),
            Message::ChineseFirst(on) => self.save("general", "chinese_first", on),
            Message::FullWidthPunctuation(on) => {
                self.save("general", "full_width_punctuation", on);
            }
            Message::EnglishFullWidthPunctuation(on) => {
                self.save("general", "english_full_width_punctuation", on);
            }
            Message::EnglishOffInApps(on) => {
                let list: Vec<String> = if on {
                    DEFAULT_ENGLISH_CANDIDATES_OFF_WINDOWS
                        .iter()
                        .map(|s| (*s).to_owned())
                        .collect()
                } else {
                    Vec::new()
                };
                self.save_array("apps", "english_candidates_off", &list);
            }
            Message::SwitchKey(key, on) => {
                let keys = self.config.shortcut.switch_mode.with(key, on);
                let values: Vec<String> =
                    keys.config_values().into_iter().map(String::from).collect();
                self.save_array("shortcut", "switch_mode", &values);
            }
            Message::EnglishMode(on) => self.save("general", "english_mode", on),

            // 候选窗口页
            Message::Theme(Some(i)) if i < ThemeMode::ALL.len() => {
                self.save("general", "theme", ThemeMode::ALL[i].key());
            }
            Message::Layout(Some(i)) if i < LayoutMode::ALL.len() => {
                self.save("general", "layout", LayoutMode::ALL[i].key());
            }
            Message::Preedit(Some(i)) if i < PreeditMode::ALL.len() => {
                self.save("general", "preedit", PreeditMode::ALL[i].key());
            }
            Message::ShiftLetter(Some(i)) if i < ShiftLetter::ALL.len() => {
                self.save("general", "shift_letter", ShiftLetter::ALL[i].key());
            }
            Message::Renderer(Some(i)) if i < CandidateRenderer::ALL.len() => {
                self.save("general", "renderer", CandidateRenderer::ALL[i].key());
            }
            Message::FontQuery(text) => {
                let text = text.trim().to_owned();
                let exact = self
                    .families
                    .iter()
                    .find(|family| family.eq_ignore_ascii_case(&text))
                    .cloned();
                match exact {
                    Some(family) => {
                        self.font_query = None;
                        self.save("general", "font", family);
                    }
                    None if text.is_empty() => {
                        self.font_query = None;
                        self.save("general", "font", "");
                    }
                    None => self.font_query = Some(text),
                }
            }
            Message::Font(family) => {
                self.font_query = None;
                self.save("general", "font", family);
            }
            Message::StatusBar(on) => self.save("status_bar", "enabled", on),

            // 云服务页
            Message::LocalModel(on) => self.save("model", "enabled", on),
            Message::CloudEnabled(on) => self.save("predict", "enabled", on),
            Message::CloudApiKey(value) => self.save("predict", "api_key", value),
            Message::CloudModel(value) => self.save("predict", "model", value),
            Message::CloudBaseUrl(value) => self.save("predict", "base_url", value),
            Message::CloudSlots(Some(value)) => {
                let slots = (value.round() as i64).clamp(0, 9);
                self.save("predict", "slots", slots);
            }
            Message::CloudSentence(on) => self.save("predict", "sentence", on),
            Message::CloudTranslation(on) => {
                let engine = if on { "cloud" } else { "local" };
                self.save("predict", "translation_engine", engine);
            }
            Message::TestConnection => {
                if matches!(self.cloud_status, CloudStatus::Testing) {
                    return;
                }
                self.cloud_status = CloudStatus::Testing;
                let config = self.config.predict.clone();
                context.spawn_background(move |cancel| {
                    Message::CloudTestDone(cloud::run_test(&config, &cancel))
                });
            }
            Message::CloudTestDone(result) => {
                self.cloud_status = match result {
                    Ok(message) => CloudStatus::Ok(message),
                    Err(message) => CloudStatus::Failed(message),
                };
            }

            // 快捷键页
            Message::VoiceInput(on) => self.save("shortcut", "voice_input", on),
            Message::VoiceEnglishPractice(on) => {
                self.save("shortcut", "voice_english_practice", on);
            }
            Message::VoicePracticeDemo(on) => {
                self.save("shortcut", "voice_practice_demo", on);
            }
            Message::VoicePracticeLearnEnglish(on) => {
                self.save("shortcut", "voice_practice_learn_english", on);
            }
            Message::VoicePracticeSendTranslation(on) => {
                self.save("shortcut", "voice_practice_send_translation", on);
            }
            Message::TranslateSelectionIcon(on) => {
                self.save("shortcut", "translate_selection_icon", on);
            }
            Message::PracticeShortcutRecord if !self.hotkey.saving => {
                let (generation, cancelled) = self.hotkey.begin();
                context.spawn_background(move |cancel| {
                    Message::PracticeShortcutCaptured(
                        generation,
                        super::hotkey::record(&cancel, cancelled),
                    )
                });
            }
            Message::PracticeShortcutCancel => {
                self.hotkey.cancel();
                self.hotkey.message = "已取消录入，原快捷键保持不变。".to_owned();
                self.hotkey.failed = false;
            }
            Message::PracticeShortcutCaptured(generation, result)
                if generation == self.hotkey.generation =>
            {
                self.hotkey.recording = false;
                match result {
                    Ok(shortcut) => {
                        self.hotkey.candidate = Some(shortcut);
                        self.hotkey.message = format!(
                            "已录入 {}。点击保存后检查冲突并立即生效。",
                            shortcut.label()
                        );
                        self.hotkey.failed = false;
                    }
                    Err(message) => {
                        self.hotkey.message = message;
                        self.hotkey.failed = true;
                    }
                }
            }
            Message::PracticeShortcutSave if !self.hotkey.saving && !self.hotkey.recording => {
                if let Some(shortcut) = self.hotkey.candidate {
                    self.hotkey.saving = true;
                    self.hotkey.message = "正在检查系统快捷键冲突…".to_owned();
                    context.spawn_background(move |_| {
                        Message::PracticeShortcutSaved(super::hotkey::save(shortcut))
                    });
                }
            }
            Message::PracticeShortcutReset if !self.hotkey.saving => {
                self.hotkey.cancel();
                self.hotkey.saving = true;
                self.hotkey.message = "正在恢复默认快捷键…".to_owned();
                context.spawn_background(move |_| {
                    Message::PracticeShortcutSaved(super::hotkey::save(
                        qingjian_platform::PracticeShortcut::RIGHT_CTRL_ALT,
                    ))
                });
            }
            Message::PracticeShortcutSaved(result) => {
                self.hotkey.saving = false;
                match result {
                    Ok(saved) => {
                        self.reload();
                        self.hotkey.candidate = None;
                        self.hotkey.message = saved.message();
                        self.hotkey.failed = false;
                    }
                    Err(message) => {
                        self.hotkey.message = message;
                        self.hotkey.failed = true;
                    }
                }
            }
            Message::VoiceKey(Some(i)) if i < shortcut::VOICE_KEYS.len() => {
                self.save_voice_key(shortcut::VOICE_KEYS[i].1, shortcut::VOICE_KEYS[i].2);
            }
            Message::VoiceKeyReset => {
                self.save_voice_key(qingjian_platform::VoiceKey::LeftAlt, "left_alt");
            }
            Message::VoiceHold(Some(i)) => {
                let options = shortcut::voice_hold_options(self.config.shortcut.voice_hold_ms());
                if let Some((_, milliseconds)) = options.get(i) {
                    self.save("shortcut", "voice_hold_ms", i64::from(*milliseconds));
                }
            }
            Message::PageKeys(Some(i)) if i < shortcut::PAGE_KEYS.len() => {
                self.save("general", "page_keys", shortcut::PAGE_KEYS[i].1);
            }
            Message::ModeExpression(Some(i)) if i < shortcut::MODE_KEYS.len() => {
                self.save("shortcut", "expression", shortcut::MODE_KEYS[i]);
            }
            Message::ModeQuestion(Some(i)) if i < shortcut::MODE_KEYS.len() => {
                self.save("shortcut", "question", shortcut::MODE_KEYS[i]);
            }
            Message::QuestionMark(on) => self.save("shortcut", "question_mark", on),
            Message::Translation(Some(i)) if i < shortcut::MODIFIERS.len() => {
                self.save("shortcut", "translation", shortcut::MODIFIERS[i].1);
            }
            Message::TranslationSecond(Some(i)) if i < shortcut::MODIFIERS.len() => {
                self.save("shortcut", "translation_second", shortcut::MODIFIERS[i].1);
            }
            Message::DeleteCandidate(Some(i)) if i < shortcut::MODIFIERS.len() => {
                self.save("shortcut", "delete_candidate", shortcut::MODIFIERS[i].1);
            }
            Message::TranslateSelection(Some(i)) if i < shortcut::MODIFIERS.len() => {
                let key = self.config.shortcut.translate_selection.key;
                let combo = format!("{}+{key}", shortcut::MODIFIERS[i].1);
                self.save("shortcut", "translate_selection", combo);
            }

            // 模糊音页
            Message::Fuzzy(key, on) => self.save("fuzzy", key, on),

            // 词库页
            Message::ToggleDomain(name, on) => {
                let mut domains = self.config.dictionaries.domains.clone();
                if on {
                    if !domains.contains(&name) {
                        domains.push(name);
                    }
                } else {
                    domains.retain(|d| d != &name);
                }
                self.save_array("dictionaries", "domains", &domains);
            }
            Message::ToggleUserDict(name, on) => {
                // 用户词库缺省启用，`disabled` 列的是关掉的。
                let mut disabled = self.config.dictionaries.disabled.clone();
                if on {
                    disabled.retain(|d| d != &name);
                } else if !disabled.contains(&name) {
                    disabled.push(name);
                }
                self.save_array("dictionaries", "disabled", &disabled);
            }
            Message::RemoveUserDict(name) => {
                dictionaries::remove_user_dict(self, &name);
                self.reload();
            }
            Message::ImportDictionary => {
                dictionaries::import(self);
                self.reload();
            }

            // 辅码页
            Message::AuxCodeEnabled(on) => self.save("aux_code", "enabled", on),
            Message::AuxCodeShow(on) => self.save("general", "aux_code_show", on),
            Message::AuxCodeKeepEmpty(on) => self.save("general", "aux_code_keep_empty", on),
            Message::AuxRecordStart => self.recorder = self.recorder.waiting(),
            Message::AuxRecordCancel => self.recorder = Recorder::Idle,
            Message::AuxRecorded(text) => aux_code::record_key(self, &text),
            Message::ToggleAuxTable(name, on) => {
                // 码表缺省启用，`disabled` 列的是关掉的；随包笔画表也走这条
                let mut disabled = self.config.aux_code.disabled.clone();
                if on {
                    disabled.retain(|d| d != &name);
                } else if !disabled.contains(&name) {
                    disabled.push(name);
                }
                self.save_array("aux_code", "disabled", &disabled);
            }
            Message::RemoveAuxTable(name) => {
                aux_code::remove_table(self, &name);
                self.reload();
            }
            Message::ImportCodeTable => {
                aux_code::import(self);
                self.reload();
            }

            // 高级页
            Message::VerboseLog(on) => {
                let level = if on { LogLevel::Debug } else { LogLevel::Info };
                self.save("general", "log_level", level.key());
            }
            Message::InputLog(on) => self.save("general", "input_log", on),
            Message::Learning(on) => self.save("general", "learning", on),
            Message::OpenConfigFile => {
                Self::ensure_config_file(&self.path);
                open_in_editor(&self.path);
            }
            Message::OpenDataDir => {
                Self::ensure_config_file(&self.path);
                open_with_explorer(&self.data_dir().to_string_lossy());
            }
            Message::OpenLogDir => {
                if let Some(logs) = log_dir() {
                    open_with_explorer(&logs.to_string_lossy());
                }
            }
            Message::ExportLogs => export_logs(),
            Message::ClearInputLog => {
                let log = self.data_dir().join("input-log.jsonl");
                if let Err(error) = std::fs::remove_file(&log)
                    && error.kind() != std::io::ErrorKind::NotFound
                {
                    crate::log::warn(format!("清空输入日志失败: {error}"));
                }
            }

            // 关于页
            Message::OpenWebsite => open_with_explorer(about::WEBSITE_URL),
            // 未配置 Vakyu 更新源；兼容旧消息但不联网、不改用户已有更新偏好。
            Message::OpenDownload | Message::CheckUpdateNow => {
                self.update_checking = false;
                self.update_error = Some(about::UPDATE_NOTE.to_owned());
            }
            Message::UpdateCheck(_) | Message::UpdateChannel(_) => {}
            Message::UpdateChecked(result) => {
                self.update_checking = false;
                match result {
                    Some(Ok(state)) => self.update_state = state,
                    Some(Err(error)) => self.update_error = Some(error),
                    None => {}
                }
            }
            Message::OpenRepository => open_with_explorer(about::REPOSITORY_URL),

            // 下拉被清空 / 越界：不改
            _ => {}
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        super::theme::view(self, context)
    }
}

fn schedule_window_chrome(context: &ComponentContext<Settings>, attempt: u8) {
    context.spawn_background(move |_cancel| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        Message::ApplyWindowChrome(attempt)
    });
}
