# 五笔码表

`wubi86.tsv`：`词\t编码\t词频`，随包发给形码（五笔）方案用（`qingjian-dictionary::CodeTable` 读，按编码前缀查）。

| 文件 | 来源 | 许可 |
| --- | --- | --- |
| `wubi86_jidian.dict.yaml`（上游源文件，8.9 万条，不进仓库） | 86 五笔极点码表，<https://github.com/sxjudya/rime-wubi86-jidian>（`version: "4.3"`） | Apache-2.0，原文见同目录 `LICENSE` |
| `wubi86.tsv`（产品数据） | 由上面那份转换而来 | 编码来自上游（Apache-2.0）；词频是青简自己统计的 |

上游还有 `wubi86_jidian_extra.dict.yaml`（扩展词）与 `wubi86_jidian_extra_district.dict.yaml`（行政区域），**暂未并入**：
极点方案的 `import_tables` 收了前者，以后按需要再加。

## 词频不是码表自带的权重

码表第二列（Rime 的 `weight`）是码表顺序，不是语料词频。直接拿来排序，同一个词在五笔下和在拼音下会排得不一样，
释义兜底与生词识别也对不上。所以转换时**按词面**从青简词库回填词频：词库里有的用语料词频（与拼音方案同一把尺子），
没有的给 `UNKNOWN_FREQUENCY = 1`。

## 重新生成

先从上游仓库下载 `wubi86_jidian.dict.yaml`（放哪都行，下面以 `data/raw/` 为例），再跑（`--frequency` 建议给基础词库 + 全部领域词库，覆盖率更高）：

```bash
cargo run --release -p qingjian-dict-convert -- --out-dir assets/wubi wubi \
    data/raw/wubi86_jidian.dict.yaml \
    --frequency assets/lexicon/dict.tsv assets/lexicon/dicts/*.tsv
```

命中率（2026-09-16，基础词库 + 领域词库）：8.9 万条里 5.67 万条拿到语料词频，3.25 万条是兜底。
兜底的那部分主要是《通用规范汉字表》之外的罕见字（菚 / 匞 / 葚 这类）与青简词库没收的成语、词组；
它们在候选里排在已知词后面，只有在同编码没有别的选择时才出现。
