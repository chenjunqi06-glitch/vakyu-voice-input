//! 示范播放器的线程命令；正文只在内存中传递。

use qingjian_core::Language;

pub(super) enum Command {
    Prewarm {
        generation: u64,

        language: Language,

        natural: Option<u16>,
    },

    Speak {
        generation: u64,

        request_id: u64,

        text: String,

        rate: i32,

        language: Language,

        /// 英语示范用随包自然人声时的音色编号；`None` 用系统声音。
        natural: Option<u16>,
    },

    Cancel,

    Shutdown,
}
