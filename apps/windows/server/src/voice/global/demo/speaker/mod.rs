//! 示范播放器：英语优先用随包自然人声（本机 Piper 模型），组件缺失或出错时自动改用 Windows 系统声音。

mod natural;
mod system;

use qingjian_core::Language;

use self::natural::NaturalVoice;
use self::system::SystemVoice;

enum Backend {
    Natural(NaturalVoice),

    System(SystemVoice),
}

pub(super) struct Speaker {
    backend: Backend,

    language: Language,

    /// 创建时请求的自然人声音色（只对英语生效）；设置变了才重建播放器。
    requested: Option<u16>,
}

impl Speaker {
    pub fn new(language: Language, natural: Option<u16>) -> Result<Self, String> {
        let requested = english_only(language, natural);
        if let Some(speaker) = requested {
            match NaturalVoice::start(speaker) {
                Ok(voice) => {
                    return Ok(Self {
                        backend: Backend::Natural(voice),
                        language,
                        requested,
                    });
                }
                Err(reason) => tracing::warn!(%reason, "自然人声不可用，示范改用系统声音"),
            }
        }
        Ok(Self {
            backend: Backend::System(SystemVoice::new(language)?),
            language,
            requested,
        })
    }

    /// 同一语言、同一音色设置时复用；自然人声失败退回系统声音后也不反复重试。
    pub fn matches(&self, language: Language, natural: Option<u16>) -> bool {
        self.language == language && self.requested == english_only(language, natural)
    }

    pub fn speak(&mut self, text: &str, rate: i32) -> Result<u32, String> {
        if let Backend::Natural(voice) = &mut self.backend {
            match voice.speak(text, rate) {
                Ok(stream) => return Ok(stream),
                Err(reason) => {
                    tracing::warn!(%reason, "自然人声无法朗读，示范改用系统声音");
                    self.backend = Backend::System(SystemVoice::new(self.language)?);
                }
            }
        }
        match &self.backend {
            Backend::System(voice) => voice.speak(text, rate),
            Backend::Natural(_) => Err("示范朗读暂时不可用，可继续跟读".to_owned()),
        }
    }

    pub fn poll(&mut self, stream: u32) -> Result<bool, String> {
        match &mut self.backend {
            Backend::Natural(voice) => voice.poll(stream),
            Backend::System(voice) => voice.poll(stream),
        }
    }

    pub fn purge(&mut self) -> Result<(), String> {
        match &mut self.backend {
            Backend::Natural(voice) => voice.purge(),
            Backend::System(voice) => voice.purge(),
        }
    }
}

fn english_only(language: Language, natural: Option<u16>) -> Option<u16> {
    natural.filter(|_| language == Language::English)
}

#[cfg(test)]
mod tests {
    use super::english_only;
    use qingjian_core::Language;

    #[test]
    fn natural_voice_is_only_requested_for_english() {
        assert_eq!(english_only(Language::English, Some(20)), Some(20));
        assert_eq!(english_only(Language::Chinese, Some(20)), None);
        assert_eq!(english_only(Language::English, None), None);
    }
}
