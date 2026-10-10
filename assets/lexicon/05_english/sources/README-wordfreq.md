# wordfreq 来源与许可

`assets/lexicon/english-frequency.tsv` 是本仓库已有的英文词频数据，作者来源为
[wordfreq / Robyn Speer](https://github.com/rspeer/wordfreq)。`tools/corpus/english_frequency.py`
对项目英文词表逐词调用 `zipf_frequency(word, "en")`，将结果乘以 1000 后取整数，
写出「编码、词频」两列；`assets/lexicon/english.tsv` 的词频列使用这些数值。
词形筛选来自项目英文词表，字段转换与整数化由本项目执行。这里没有收录原始社交媒体文本。

生成脚本声明 `wordfreq>=3.1`，最初生成这些文件时使用的精确 wordfreq 版本未记录。
下列固定提交用于本次许可文档取证，取回日期为 2026-10-10。

## 许可与署名

wordfreq 上游版权声明为 **Copyright 2022 Robyn Speer**。代码采用 **Apache-2.0**；
上游声明其数据文件可按 **CC BY-SA 4.0** 再分发。本项目保留这一数据许可及署名，
对词频进行筛选、字段转换、整数化的修改也按 CC BY-SA 4.0 提供。
英文词形的 ESDB、CSpell 等来源继续适用各自许可证，程序代码继续适用仓库的 GPL-3.0-or-later。

wordfreq 的数据来源及完整声明保存在 [wordfreq_NOTICE.md](wordfreq_NOTICE.md)，
引用文献保存在 [wordfreq_README.md](wordfreq_README.md) 的最后部分。上游列出的来源包括
Google Books Ngrams / Syntactic Ngrams、Leeds Internet Corpus、Wikipedia、ParaCrawl、
OPUS OpenSubtitles 2018 / OpenSubtitles，以及 Marc Brysbaert 等作者的 SUBTLEX 系列。
SUBTLEX 是免费提供的数据；各作者及文献引用按原文保留。

上游 README 的 “Can I convert wordfreq to a more convenient form for my purposes, like a CSV file?”
一节对 CSV 导出再分发明确回答 “No”，理由包括署名和许可易被剥离，以及结果依赖库的归一化与分词。
本仓库目前已有 TSV 导出，上述原文一并保留供核查；本次补齐许可文档没有取得作者对该导出形式的单独确认。
分发这些词频文件或含其词频的词库时，应同时保留此说明、上游署名与数据许可。

## 原文快照

以下三个 wordfreq 文件原样取自提交
[`912caf64b657478d1dff1138efdc078947d54bb1`](https://github.com/rspeer/wordfreq/tree/912caf64b657478d1dff1138efdc078947d54bb1)。
存档原文中的相对链接以其上游仓库目录为基准。

| 本地文件 | 官方原文 |
| --- | --- |
| [wordfreq_LICENSE.txt](wordfreq_LICENSE.txt) | [wordfreq LICENSE.txt](https://github.com/rspeer/wordfreq/blob/912caf64b657478d1dff1138efdc078947d54bb1/LICENSE.txt) |
| [wordfreq_NOTICE.md](wordfreq_NOTICE.md) | [wordfreq NOTICE.md](https://github.com/rspeer/wordfreq/blob/912caf64b657478d1dff1138efdc078947d54bb1/NOTICE.md) |
| [wordfreq_README.md](wordfreq_README.md) | [wordfreq README.md](https://github.com/rspeer/wordfreq/blob/912caf64b657478d1dff1138efdc078947d54bb1/README.md) |
| [LICENSE-Apache-2.0.txt](LICENSE-Apache-2.0.txt) | [Apache License 2.0 全文](https://www.apache.org/licenses/LICENSE-2.0.txt) |
| [LICENSE-CC-BY-SA-4.0.txt](LICENSE-CC-BY-SA-4.0.txt) | [Creative Commons BY-SA 4.0 全文](https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt) |
