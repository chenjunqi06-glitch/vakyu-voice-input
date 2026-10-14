//! Windows 系统声音（SAPI）：按练习语言选择声音，所有 COM 调用留在专用线程；自然人声不可用时的后备。

use qingjian_core::Language;
use windows::Win32::Media::Speech::{
    ISpObjectToken, ISpObjectTokenCategory, ISpVoice, SPCAT_VOICES, SPEI_END_INPUT_STREAM,
    SPEI_RESERVED1, SPEI_RESERVED2, SPEVENT, SPF_ASYNC, SPF_IS_NOT_XML, SPF_PURGEBEFORESPEAK,
    SPVOICESTATUS, SpObjectTokenCategory, SpVoice,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::{PCWSTR, w};

use crate::voice::event::Event;

pub(super) struct SystemVoice {
    voice: ISpVoice,
}

impl SystemVoice {
    pub fn new(language: Language) -> Result<Self, String> {
        unsafe {
            let voice: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_INPROC_SERVER)
                .map_err(|_| "Windows 语音示范组件不可用，可继续跟读".to_owned())?;
            voice
                .SetVoice(&language_token(language)?)
                .map_err(|_| "无法加载示范声音，可继续跟读".to_owned())?;
            let interest = (1u64 << SPEI_RESERVED1.0)
                | (1u64 << SPEI_RESERVED2.0)
                | (1u64 << SPEI_END_INPUT_STREAM.0);
            voice
                .SetInterest(interest, interest)
                .map_err(|_| "无法监听示范朗读结束，可继续跟读".to_owned())?;
            Ok(Self { voice })
        }
    }

    pub fn speak(&self, text: &str, rate: i32) -> Result<u32, String> {
        self.purge()?;
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let mut stream = 0;
        unsafe {
            self.voice
                .SetRate(rate.clamp(-10, 10))
                .map_err(|_| "无法调整示范朗读速度，可继续跟读".to_owned())?;
            self.voice
                .Speak(
                    PCWSTR(text.as_ptr()),
                    (SPF_ASYNC.0 | SPF_IS_NOT_XML.0 | SPF_PURGEBEFORESPEAK.0) as u32,
                    Some(&mut stream),
                )
                .map_err(|_| "无法播放示范朗读，请检查默认扬声器".to_owned())?;
        }
        Ok(stream)
    }

    pub fn poll(&self, stream: u32) -> Result<bool, String> {
        // 只订阅结束事件；仍通过通用封装释放事件携带的任何 COM 资源。
        for _ in 0..64 {
            let Some(event) = self.next_event()? else {
                return Ok(false);
            };
            if event.id() == SPEI_END_INPUT_STREAM.0 && event.0.ulStreamNum == stream {
                let mut status = SPVOICESTATUS::default();
                unsafe {
                    self.voice
                        .GetStatus(&mut status, std::ptr::null_mut())
                        .and_then(|()| status.hrLastResult.ok())
                        .map_err(|_| "示范朗读播放未完成，请检查默认扬声器".to_owned())?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn purge(&self) -> Result<(), String> {
        unsafe {
            self.voice.Speak(
                PCWSTR::null(),
                (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0 | SPF_IS_NOT_XML.0) as u32,
                None,
            )
        }
        .map_err(|_| "示范朗读无法停止，已关闭播放组件".to_owned())?;
        while self.next_event()?.is_some() {}
        Ok(())
    }

    fn next_event(&self) -> Result<Option<Event>, String> {
        let mut event = SPEVENT::default();
        let mut fetched = 0;
        unsafe { self.voice.GetEvents(1, &mut event, &mut fetched) }
            .map_err(|_| "读取示范朗读播放状态失败，可继续跟读".to_owned())?;
        Ok((fetched != 0).then_some(Event(event)))
    }
}

impl Drop for SystemVoice {
    fn drop(&mut self) {
        let _ = self.purge();
    }
}

fn language_token(language: Language) -> Result<ISpObjectToken, String> {
    let (identifiers, preferred, language_name) = match language {
        Language::Chinese => (
            &["804", "1004", "404"][..],
            w!("Name=Microsoft Huihui Desktop"),
            "中文",
        ),
        Language::English => (
            &[
                "409", "809", "C09", "1009", "1409", "1809", "1C09", "2009", "2409", "2809",
                "2C09", "3009", "3409",
            ][..],
            w!("Name=Microsoft Zira Desktop"),
            "英语",
        ),
        _ => return Err("示范朗读目前支持中文和英语".to_owned()),
    };
    unsafe {
        let category: ISpObjectTokenCategory =
            CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_INPROC_SERVER)
                .map_err(|_| "无法读取 Windows 朗读声音，可继续跟读".to_owned())?;
        category
            .SetId(SPCAT_VOICES, false)
            .map_err(|_| "Windows 没有可用的示范声音，可继续跟读".to_owned())?;
        // 只在同种语言的地区声音之间降级；不把未安装中文误当作英语播放成功。
        for identifier in identifiers {
            let filter: Vec<u16> = format!("Language={identifier}\0").encode_utf16().collect();
            if let Ok(tokens) = category.EnumTokens(PCWSTR(filter.as_ptr()), preferred)
                && let Ok(token) = tokens.Item(0)
            {
                return Ok(token);
            }
        }
    }
    Err(format!(
        "未安装 Windows {language_name}声音；可继续跟读，或在 Windows 语言设置安装{language_name}语音"
    ))
}

#[cfg(test)]
mod tests {
    use super::SystemVoice;
    use qingjian_core::Language;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

    #[test]
    #[ignore = "需要本机安装 Windows 英语声音；不播放任何正文或访问麦克风"]
    fn english_voice_is_available_without_playing() {
        voice_is_available_without_playing(Language::English);
    }

    #[test]
    #[ignore = "需要本机安装 Windows 中文声音；不播放任何正文或访问麦克风"]
    fn chinese_voice_is_available_without_playing() {
        voice_is_available_without_playing(Language::Chinese);
    }

    fn voice_is_available_without_playing(language: Language) {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.unwrap();
        let result = SystemVoice::new(language);
        let created = result.is_ok();
        let error = result.err();
        // 如果创建成功，Result 在提取错误时已释放其播放器，之后才退出 COM。
        unsafe { CoUninitialize() };
        assert!(created, "{error:?}");
    }
}
