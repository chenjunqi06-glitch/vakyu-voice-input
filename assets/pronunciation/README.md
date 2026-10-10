# 英语跟读发音数据

`cmudict.dict` 来自 [CMUdict](https://github.com/cmusphinx/cmudict)，提交
`74790861f652b15e4ac49015a90074ad62a27690`，于 2026-10-08 下载。保持上游格式，
未人工增删词条。SHA-256：`81917843c7f44ce2b094ac63873c2c7a4cf802040792c455ba3ca406891c3d22`。

相邻 `LICENSE-CMUdict.txt` 是同一提交的完整许可证；发布含内嵌发音词典的二进制时需同时保留该许可。
CMUdict 允许研究、商业使用与再分发。读音中的重音数字被跟读比较器忽略，词典本身保持原样。

跟读模块用真实音素与有限的发音部位替代比较识别文本，不使用相似拼写或同义词放行。
未登录的中文专名由中文原句确认后，使用 `pinyin` 的多音字数据和现有拼音切分器处理。
这些文字读音匹配用于兼容口音与识别差异，不是声学发音评分。

`pinyin` 使用 crates.io 的 `0.11.0`，上游为
[rust-pinyin](https://github.com/mozillazg/rust-pinyin)，仅启用 `plain`、`heteronym`。
锁定包 SHA-256：`e225f595052d9c46045755be4b8d7950b6d9f3c33e0c0b74ba58f11bbfa8c64b`。
相邻 `LICENSE-pinyin.txt` 复制自此版本的发布包，分发时保留其 MIT 版权许可。
