//! 形码码表的装载：`[general] scheme` 选了形码时，把码表找出来挂到引擎上。
//!
//! 与本地整句模型同一套找法（见 [`super::find_model`]）：用户目录优先，随包数据兜底。
//! 选形码但码表不在时不静默按拼音跑——配置说五笔、引擎还在拼音是很糟的错位，宁可在日志里喊出来。

use std::path::{Path, PathBuf};

use qingjian_core::Engine;
use qingjian_dictionary::CodeTable;
use qingjian_platform::Scheme;

use super::Router;

/// 用户目录下的码表（用户自己换的那份）。
const USER_TABLE: &str = "wubi/wubi86.tsv";

/// 随包数据里的码表。走 `assets/` 而不是 `data/`：这张表随 git 跟踪（`data/` 是给生成物的，
/// 装机时从数据包解出来），与 emoji / levels 一样在仓库与安装目录里是同一个相对路径，
/// 所以 `cargo run` 的开发布局也找得到。
const BUNDLED_TABLE: &str = "assets/wubi/wubi86.tsv";

/// 找码表：用户目录优先，否则随包数据；都没有为 `None`。
pub fn find_code_table(user_dir: Option<&Path>, bundled_root: &Path) -> Option<PathBuf> {
    user_dir
        .map(|dir| dir.join(USER_TABLE))
        .filter(|path| path.is_file())
        .or_else(|| {
            let bundled = bundled_root.join(BUNDLED_TABLE);
            bundled.is_file().then_some(bundled)
        })
}

/// 按配置的两条轴装配引擎：拼音侧（全拼 / 双拼 / 注音 / 关）与形码侧（五笔）。
/// 两边都开就是混输——编码打全的形码候选在前，见 `Engine::query_mixed`。
///
/// 没开形码时把码表卸掉；开了但码表不在就警告并退回只用拼音（配置说五笔、引擎一个字都打不出
/// 更糟）。`table` 是启动时用 [`find_code_table`] 找好的路径（与本地模型一样，热加载时不重新找）。
fn apply_scheme(engine: &mut Engine, pinyin: Scheme, wubi: bool, table: Option<&Path>) {
    engine.set_shuangpin(pinyin.shuangpin());
    engine.set_zhuyin_mode(pinyin == Scheme::Zhuyin);
    // 拼音侧关掉且形码开着才是「只用形码」；两边都关着时留拼音兜底（否则一个候选都没有）
    engine.set_phonetic(pinyin.is_on() || !wubi);
    if !wubi {
        engine.set_code_table(None);
        return;
    }
    let Some(path) = table else {
        tracing::warn!(
            table = BUNDLED_TABLE,
            "选了形码方案但找不到码表，仍按拼音输入；随包数据里应当带一份",
        );
        engine.set_code_table(None);
        return;
    };
    match CodeTable::from_path(path) {
        Ok(table) => {
            tracing::info!(table = %path.display(), entries = table.len(), "形码码表已载入");
            engine.set_code_table(Some(table));
        }
        Err(error) => {
            tracing::error!(%error, table = %path.display(), "形码码表读不了，仍按拼音输入");
            engine.set_code_table(None);
        }
    }
}

impl Router {
    /// 启动时把找好的码表路径交给路由器，并按当前配置装配一次。
    pub fn configure_code_table(&mut self, table: Option<PathBuf>) {
        self.code_table = table;
        apply_scheme(
            &mut self.engine,
            self.config.scheme,
            self.config.wubi,
            self.code_table.as_deref(),
        );
    }

    /// 热加载：方案变了就按同一个路径重新装配（不重新找文件，与本地模型一致）。
    pub(super) fn reload_code_table(&mut self, pinyin: Scheme, wubi: bool) {
        apply_scheme(&mut self.engine, pinyin, wubi, self.code_table.as_deref());
    }
}
