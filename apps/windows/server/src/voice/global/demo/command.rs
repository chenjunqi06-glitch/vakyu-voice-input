//! 示范播放器的线程命令；正文只在内存中传递。

use qingjian_core::Language;

pub(super) enum Command {
    Speak {
        generation: u64,

        request_id: u64,

        text: String,

        rate: i32,

        language: Language,
    },

    Cancel,

    Shutdown,
}
