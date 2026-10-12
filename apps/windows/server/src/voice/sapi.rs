//! 独立的 SAPI 识别器；不会启用 Windows 的全局语音命令或依赖 MSIX 身份。

use super::event::Event;
use super::transcript::Transcript;
use qingjian_core::Language;
use std::cell::Cell;
use windows::Win32::Media::Speech::{
    ISpAudio, ISpObjectTokenCategory, ISpRecoContext, ISpRecoGrammar, ISpRecognizer,
    SPCAT_RECOGNIZERS, SPEI_END_SR_STREAM, SPEI_HYPOTHESIS, SPEI_RECOGNITION, SPEI_RESERVED1,
    SPEI_RESERVED2, SPEI_SR_AUDIO_LEVEL, SPEVENT, SPLO_STATIC, SPRS_ACTIVE, SPRS_INACTIVE,
    SPRST_ACTIVE, SPRST_INACTIVE, SPRST_INACTIVE_WITH_PURGE, SpInprocRecognizer, SpMMAudioIn,
    SpObjectTokenCategory,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{IUnknown, Interface, PCWSTR, w};

pub(super) struct Sapi {
    // 字段按声明顺序释放；grammar/context 必须早于 recognizer。
    grammar: ISpRecoGrammar,

    context: ISpRecoContext,

    recognizer: ISpRecognizer,

    level: Cell<Option<u8>>,
}

impl Sapi {
    pub fn new_in_language(language: Language) -> Result<Self, String> {
        unsafe {
            let audio: ISpAudio = CoCreateInstance(&SpMMAudioIn, None, CLSCTX_ALL)
                .map_err(|e| failure("无法打开默认麦克风，请检查麦克风及桌面应用访问权限", e))?;
            Self::with_input(
                &audio.cast().map_err(|e| failure("麦克风接口不可用", e))?,
                language,
            )
        }
    }

    fn with_input(input: &IUnknown, language: Language) -> Result<Self, String> {
        let (filter, language_name) = match language {
            Language::Chinese => (w!("Language=804"), "简体中文"),
            Language::English => (w!("Language=409"), "英语"),
            _ => return Err("Windows 语音输入目前支持中文和英语".to_owned()),
        };
        unsafe {
            let recognizer: ISpRecognizer = CoCreateInstance(&SpInprocRecognizer, None, CLSCTX_ALL)
                .map_err(|e| failure("无法创建 Windows 语音识别器", e))?;
            recognizer
                .SetRecoState(SPRST_INACTIVE_WITH_PURGE)
                .map_err(|e| failure("无法初始化语音识别器", e))?;
            let category: ISpObjectTokenCategory =
                CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_ALL)
                    .map_err(|e| failure("无法读取 Windows 语音语言", e))?;
            category
                .SetId(SPCAT_RECOGNIZERS, false)
                .map_err(|e| failure("没有安装 Windows 语音识别", e))?;
            let tokens = category
                .EnumTokens(filter, PCWSTR::null())
                .map_err(|e| failure(&format!("无法查找{language_name}语音识别器"), e))?;
            let token = tokens.Item(0).map_err(|e| {
                failure(
                    &format!("请在 Windows 语言设置安装{language_name}的语音识别组件"),
                    e,
                )
            })?;
            recognizer
                .SetRecognizer(&token)
                .map_err(|e| failure(&format!("无法加载{language_name}语音识别器"), e))?;
            recognizer
                .SetInput(input, true)
                .map_err(|e| failure("无法连接音频输入，请检查默认麦克风", e))?;
            let context = recognizer
                .CreateRecoContext()
                .map_err(|e| failure("无法创建语音识别上下文", e))?;
            // SAPI 的 SPFEI 宏还带两个校验位；只移位事件编号会返回 E_INVALIDARG。
            let interest = (1u64 << SPEI_RESERVED1.0)
                | (1u64 << SPEI_RESERVED2.0)
                | (1u64 << SPEI_HYPOTHESIS.0)
                | (1u64 << SPEI_RECOGNITION.0)
                | (1u64 << SPEI_SR_AUDIO_LEVEL.0)
                | (1u64 << SPEI_END_SR_STREAM.0);
            context
                .SetInterest(interest, interest)
                .map_err(|e| failure("无法监听语音识别结果", e))?;
            let grammar = context
                .CreateGrammar(1)
                .map_err(|e| failure("无法创建语音听写", e))?;
            grammar
                .LoadDictation(PCWSTR::null(), SPLO_STATIC)
                .map_err(|e| failure("无法加载听写，请检查 Windows 语音组件", e))?;
            Ok(Self {
                grammar,
                context,
                recognizer,
                level: Cell::new(None),
            })
        }
    }

    pub fn start(&self) -> Result<(), String> {
        unsafe {
            self.grammar
                .SetDictationState(SPRS_ACTIVE)
                .map_err(|e| failure("无法开始语音听写", e))?;
            self.recognizer
                .SetRecoState(SPRST_ACTIVE)
                .map_err(|e| failure("无法开始录音，请检查麦克风权限或是否被占用", e))
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        // 不使用 PURGE：停止新采集，但保留已经进入识别器的最后几个字。
        unsafe { self.recognizer.SetRecoState(SPRST_INACTIVE) }
            .map_err(|e| failure("无法结束录音", e))
    }

    pub fn cancel(&self) {
        self.level.set(None);
        unsafe {
            let _ = self.grammar.SetDictationState(SPRS_INACTIVE);
            let _ = self.recognizer.SetRecoState(SPRST_INACTIVE_WITH_PURGE);
        }
        // 排空队列，同时释放事件持有的 COM 引用。
        while matches!(self.next_event(), Ok(Some(_))) {}
    }

    pub fn take_level(&self) -> Option<u8> {
        self.level.take()
    }

    fn next_event(&self) -> Result<Option<Event>, String> {
        let mut raw = SPEVENT::default();
        let mut fetched = 0;
        unsafe { self.context.GetEvents(1, &mut raw, &mut fetched) }
            .map_err(|e| failure("读取语音识别结果失败", e))?;
        Ok((fetched != 0).then_some(Event(raw)))
    }

    /// 返回（文本是否变化、音频流是否结束）。
    pub fn drain(&self, transcript: &mut Transcript) -> Result<(bool, bool), String> {
        let mut changed = false;
        let mut ended = false;
        while let Some(event) = self.next_event()? {
            match event.id() {
                id if id == SPEI_SR_AUDIO_LEVEL.0 => {
                    self.level.set(Some(event.0.wParam.0.min(100) as u8));
                }
                id if id == SPEI_HYPOTHESIS.0 || id == SPEI_RECOGNITION.0 => {
                    let text = event.text().map_err(|e| failure("读取识别文字失败", e))?;
                    if !text.is_empty() {
                        if id == SPEI_RECOGNITION.0 {
                            transcript.recognize(&text);
                        } else {
                            transcript.hypothesis(text);
                        }
                        changed = true;
                    }
                }
                id if id == SPEI_END_SR_STREAM.0 => {
                    // END_SR_STREAM 的 lParam 是音频错误 HRESULT（成功时为 0）。
                    let status = windows::core::HRESULT(event.0.lParam.0 as i32);
                    status
                        .ok()
                        .map_err(|e| failure("音频输入已中断，请检查麦克风", e))?;
                    ended = true;
                }
                _ => {}
            }
        }
        Ok((changed, ended))
    }

    #[cfg(test)]
    fn from_wave(path: &std::path::Path) -> Result<Self, String> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Media::Speech::{ISpStream, SPFM_OPEN_READONLY, SpFileStream};
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            let stream: ISpStream = CoCreateInstance(&SpFileStream, None, CLSCTX_ALL)
                .map_err(|e| failure("无法创建测试音频流", e))?;
            stream
                .BindToFile(PCWSTR(path.as_ptr()), SPFM_OPEN_READONLY, None, None, 0)
                .map_err(|e| failure("无法读取测试 WAV", e))?;
            Self::with_input(
                &stream
                    .cast()
                    .map_err(|e| failure("测试音频接口不可用", e))?,
                Language::Chinese,
            )
        }
    }
}

impl Drop for Sapi {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn failure(context: &str, error: windows::core::Error) -> String {
    // HRESULT 足够定位系统错误；不写入录音内容或识别文字。
    format!("{context}（0x{:08X}）", error.code().0 as u32)
}

#[cfg(test)]
mod tests {
    use super::Sapi;
    use crate::voice::transcript::Transcript;
    use std::time::{Duration, Instant};
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

    /// 明确指定合成的 WAV 后运行；本测试从不连接麦克风。
    #[test]
    #[ignore = "需要 QINGJIAN_VOICE_TEST_WAV 指向合成的中文 WAV"]
    fn transcribes_chinese_wave_fixture() {
        let path = std::env::var_os("QINGJIAN_VOICE_TEST_WAV").expect("指定合成的 WAV fixture");
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        }
        let result = (|| -> Result<String, String> {
            let sapi = Sapi::from_wave(std::path::Path::new(&path))?;
            let mut transcript = Transcript::default();
            sapi.start()?;
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(30) {
                let (_, ended) = sapi.drain(&mut transcript)?;
                if ended {
                    return Ok(transcript.text());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err("测试音频识别超时".to_owned())
        })();
        unsafe {
            CoUninitialize();
        }
        let text = result.expect("SAPI WAV 识别");
        assert!(!text.is_empty(), "合成中文音频必须产生识别文字");
        println!("合成测试音频识别结果：{text}");
        if let Ok(expected) = std::env::var("QINGJIAN_VOICE_TEST_EXPECTED") {
            assert!(text.contains(&expected), "识别结果应包含合成语句中的关键词");
        }
    }
}
