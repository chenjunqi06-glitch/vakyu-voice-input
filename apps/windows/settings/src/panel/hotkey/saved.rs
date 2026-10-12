//! 跟读快捷键的保存结果：后台是否当场确认决定了界面怎么提示生效时机。

use qingjian_platform::PracticeShortcut;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Saved {
    pub shortcut: PracticeShortcut,

    /// 后台已检查冲突并立即生效；`false` 表示后台没回应，设置程序直接写进了配置文件。
    pub confirmed: bool,

    /// 后台在运行，配置热加载会接上；`false` 表示下次启动 Vakyu 才生效。
    pub running: bool,
}

impl Saved {
    /// 界面上显示的结果说明。
    pub fn message(self) -> String {
        let label = self.shortcut.label();
        match (self.confirmed, self.running) {
            (true, _) => format!("已保存 {label}，立即生效。"),
            (false, true) => format!(
                "已保存 {label}。后台这次没有及时回应，设置已直接写入，几秒内自动生效；\
                 如果按下没有反应，可能与其他软件的快捷键冲突，请换一个，或重启 Vakyu。"
            ),
            (false, false) => {
                format!("已保存 {label}。Vakyu 后台当前没有运行，下次启动 Vakyu 时生效。")
            }
        }
    }
}
