# 各 crate 的实现要点

CLAUDE.md 只保留目录地图与规则，每个 crate / app / tool 的实现细节收在这里：入口类型、数据文件、常数、生成命令。
改了实现要同步改这里；与代码冲突时以代码为准。

## crates/qingjian-dictionary

词库（TSV 解析或 `.qj` mmap），键按字节序排好，查询逐音节位置二分收窄（简拼位置按音节块跳扫），
`lookup_pattern`（≥ 模式长度）与 `lookup_exact`（正好等长）同一套实现。词库键以 `v` 表示 ü，
TSV 解析、查询与生成工具把 `lue` / `nue` 统一成 `lve` / `nve`。
旧 `.qj` 含这些键时，加载器建立规范化的内存词库。新 `.qj` 继续使用 mmap。
辅码码表（`aux_code_table/`，`AuxCodeTable` = `Kind::AuxCodeTable` 的 `.qj`）是另一套存储：`TEXT` 词 arena + `CODE` 码 arena +
`ENTR` 条目（12 字节，词字节序升序、同词相邻）+ `HASH` 词 → 条目区间起点（`qingjian_format::hash`）。查询只有
`AuxCodeLookup::code_with_prefix` 一个方法：候选词逐个问「有没有以当前码段开头的码」。导入 `import_aux_code_table`
解析 Rime `.dict.yaml` 的 `columns`（缺省 text / code / weight）与 `import_tables`（相对主文件目录合表），
「有词无码」与非法码进统计返回、不静默跳过；一个可用条目都没有时报 `NoCodeEntries`。

`CodeTable` 是形码码表（五笔），与词库并列的另一类查表：键是编码本身（`ggll`），不是拼音音节序列，
格式 `词\t编码\t词频`（`assets/wubi/wubi86.tsv`，8.9 万条），按 `(编码, 词频降序)` 排好、`lookup` 二分定位前缀区间。
编码打全的词（`exact`）排在同前缀的更长编码词前面，这就是一级 / 二级简码的取法，不需要另做简码表。
与词库的一处关键差别：码表没有 `.qj` 容器，只读 TSV。

## crates/qingjian-core

模块：`composition`（缓冲区与光标；中文模式下 Shift+字母按小写进 `buffer` 参与匹配、大写记在 `shifted`，`typed_text` 还原后用于原样上屏）/ `parser` / `correction`（拼写纠错：整段一处编辑的候选纠正 + `typo` 音节级敲错变体表，后者进整句词图当带代价的边）/
`candidate` / `ranking` / `shortcut` / `sentence` / `fuzzy` / `shuangpin`（双拼：七套方案键位表、键 → 全拼解码与消耗换算）/ `zhuyin`（大千注音：键 → 注音符号 → 拼音，`[general] zhuyin` 开关，声调只判音节完整不进查询）/ `emoji` /
`english`（英文模式候选）/ `engine`（`query::EnglishTail`：句末英文词并入整句，`woxiangxuehaorust` → 我想学好rust，尾段也像拼音时按分数与拼音读法比）。
辅码（`engine/aux_code.rs`）：`Engine.aux_code: Option<String>` 是码段（`None` 拼音态，`Some("")` 刚触发或删空停在辅码态——`Engine.aux_keep_empty`，配置 `[general] aux_code_keep_empty` 缺省开），
不进 `Composition`；`aux_trigger`（配的触发键 + 光标在段尾 + 作用域能完整切分 + 双拼韵母键优先）、`enter_aux`、
`push_aux_code`（只收 a-z）、`clear_aux`；退格在辅码态内部分派（删码；删空按 `aux_keep_empty` 停在辅码态或回拼音态，空码段再退格退出），`commit_with` / `take_raw` /
`punctuate` / `clear` 都收尾清码。查询在 `rank` 之后**反向过滤**：候选词逐个 `AuxCodeLookup::code_with_prefix`
（码表在 `Engine.aux_codes`，`set_aux_codes` 注入），不命中的隐藏，命中的按「完全匹配码 > 码长降序 > 原词频序」
重排（stable sort 保住原序），命中码（没在筛码时是词的首条码）写进 `Candidate.aux_code`；码段非空时跳过整句 / 英文 / 快捷 / emoji / 自定义短语
与云联想。preedit 分段多出 [触发键 `Typed`][码段 `MarkedKind::AuxCode`]，见 `Query::marked_segments`。

形码（五笔）在 `engine::query::code`。`Engine` 上有两个开关：`set_code_table`（码表）与 `set_phonetic`（拼音侧参不参与），
在 `query_inner` 进切分之前按这两个分派——只有拼音 / 只有形码（`query_code`）/ **两边都开（`query_mixed`，混输）**。
编码按前缀查表，`CandidateKind::Code` 的候选 `syllables` 为空、上屏吃掉整段作用域（`whole_scope`）。
混输是「拼音那条 Query 前面插上形码候选」：拼音读不出来时（`ggll`）整个按形码走，两边都空才算错；
按文本去重，编码打全的形码词在前、拼音居中、只命中前缀的形码词垫后（一律形码在前的话 `kai` 的首选会变成编码 `kaik` 的词）；混输下模式键同双拼换成大写，四码以内不做拼写纠错，且五笔码最长 4 位、第 5 个字母起自然只剩拼音。
拼音那套在纯形码下全部不适用，靠 `modes()` 返回 `ModeKeys::LETTERLESS` 与 `active_correction` 直接返回 `None` 关掉；
译词标注、生词记录、输入日志、用户选择学习与个人 n-gram 仍照常工作。
`Engine` 是对外唯一门面，`Translator` / `Learner` trait 在 `engine` 模块；词库是「主词库 + 附加词库（`set_extra_dictionaries`）+ 用户词」的列表；繁体输出（`traditional` 开关与 `traditional_map` 映射）依赖 `ferrous-opencc`（`s2tw`）在出候选与上屏边界转换，内部保持简体。
- 中英混输的英文词位置：`Engine::set_chinese_first`（配置 `[general] chinese_first`，缺省关）关着时拼音不像话的输入英文排第一（`extras::insert_english`，
  用户老选中文词时仍让中文在前），开着时整句先插、英文词紧随其后排第二（`query_inner` 里两步的先后按开关掉转）；句末英文词并入整句（`EnglishTail`）不受它影响。
  缺省关是回放定的（9241 词 / 269 条英文上屏：缺省开英文首选 82.5% → 7.1%）。
- `custom_phrase::merge_replacements` 把平台给的「输入码 → 短语」表（macOS 系统文本替换）并进配置里的自定义短语：每条占该码最靠前的空位（1–9），
  输入码不是小写字母、已有同码同文本、九位都满的跳过；Core 不管数据从哪来。
- 拼写纠错（`correction`）：整段一处编辑的变体里相邻换位允许末尾音节没敲完（`loose_segmentation`，`mignt` → `ming t…`；
  有「每个音节都完整、末尾不是落单单字母」的变体时残尾变体不参与，免得 `keyyi` 的 可以 被 `ke yi y…` 抢走），
  纠正之间比较时换位减 `TypoCosts::correction_transpose_discount`（1 nat，CLI `--tune correction-transpose=`），与原样比不减；
  2026-09-15 回放 283 词 / 23 句：词首选 89.0%、整句 82.6%，与改前持平（折扣若也用在与原样比，`zhongwne` 会误纠成 中文，整句掉一条）。

`EngineSession` 保存可挂起的组句、标点、历史与学习链，`Engine::swap_session` 在同一个引擎里交换输入状态，共用词库与落盘服务。切换上下文时清除查询及异步预测缓存，并由平台恢复各自私密状态。

`Engine::raw_preedit()`（`engine/raw/`）只读返回 `RawPreedit { text, cursor_bytes }`：完整未上屏组合及 UTF-8 字节光标，与随后 `take_raw()` 共用文本生成，保留大小写、显式分隔符及光标后的剩余内容，不包含待处理辅码；不运行候选查询、不学习、不记日志、统计、历史或展示回报。
注音继续输出符号；光标按解码单元内按键前缀产生的符号数映射到完整单元的同序字符边界，轻声先敲时采用逻辑位置，一声空格不占显示字符。未知键仍按原解码规则聚到尾串，其光标跟随输出位置而可能不单调；首位固定 0，末位固定完整文本长度。`take_raw()` 的提交和清理顺序不变。

`Engine::discard_input` / `EngineSession::discard_input` 用于隐私能力变化时无痕清理输入，包括透传缓冲、学习链和暂存词汇曝光；`set_private` 只切换写入开关，保留已输入的组句。

## crates/qingjian-translate

`Glossary`，本地 TSV 释义表（词性 + 译文）；`LevelTable`，词汇等级表（`assets/levels/levels-{en,ja}.tsv`，CEFR A1–C2 / JLPT N5–N1，
`uv run tools/corpus/levels.py` 从 `data/levels/` 的原始 CSV 生成，来源与许可见 `assets/levels/README.md`），「统计」页按级数词汇用，不进候选。

## crates/qingjian-learning

- `FrequencyLearner`：用户选择次数（`user.tsv`）、按输入串记的选择（`user-choices.tsv`，词级排序里同输入串选过的优先）、用户词（`user-words.tsv`，主词库同格式，
  Engine 与主词库一起查）、个人英文词（`user-english.tsv`，回车原样上屏的英文词与选过的英文候选，与随包英文词表一起出候选且在前；
  原样上屏的串要像一个词才学：`MIN_ENGLISH_WORD_LETTERS` = 2 到 `MAX_ENGLISH_WORD_LETTERS` = 15 个字母、切不成完整拼音、
  也不能读成「拼音头 + 英文尾」——打不出来时整串上屏的失败拼音学进去会反过来堵住混输那条路，越打不出来学得越多）、
  个人敲错表（`user-typos.tsv`，接受过的 (敲的, 要的) 音节对，词图敲错边与整段纠错的代价按它打折）与个人 n-gram（`user-ngram.tsv`，Core `sentence::UserNgram`，
  二元 + 三元在线计数，整句转换与词级排序里与静态模型插值；Tab 接受的云端整句按 `sentence::segment_text` 切词后也记；
  连着选出的两个词记够次数自动造词进用户词，一段拼音分几次选完的合成词记两次也造）。
- `InputLog`：输入日志（`input-log.jsonl`，每次上屏一行：敲的键、切分、看到的前几个候选、选了第几个、来源、纠错、撤销，
  Core `InputLogger` trait 的落盘实现，`[general] input_log` 缺省开，只写本机，给离线回归评测与个人模型用）。
- `UsageStats`：输入统计（`usage.tsv`，按天记汉字 / 中文词 / 英文词 / 上屏次数，Core `UsageMeter` trait 的实现，Engine 每次上屏 `Usage::of_text` + 按来源定词数，
  整句按 `segment_text` 切词数；与输入日志无关，偏好设置「统计」页显示，`book_scale` 折成几本《某书》）。
- `VocabularyBook`：词汇记录（`user-vocab.tsv`，Core `VocabularyTracker` trait 的实现：学习语言的每条译词看到过几轮 / 上屏过 / ⌥+数字 打出过几次；Core 私密输入统一跳过曝光和提交写入，但仍可读取已有记录用于排序和生词标记；
  Engine `annotate` 据此填 `Sense::fresh`，看到轮次不到 `FRESH_UNTIL` = 3 的译词壳里画橙色；「看到」按上屏那一刻屏幕上那一页算，壳每次画完 `Engine::note_displayed` 告知当前页）。
- 各表落盘走 Core `storage::write_atomic`（临时文件 + fsync + 改名），加载按行容错（坏行警告跳过，真读不了壳退回内存学习），
  壳激活期间每 60 秒 `Engine::flush_learning`；IMK 回调边界 `imk::catch_panic` 拦 panic、缓冲区字母原样上屏（见 architecture.md「崩溃不丢」）。

## crates/qingjian-predict

- `CloudPredictor`：`Predictor` trait 的网络实现（async-openai，OpenAI 兼容接口，默认 DeepSeek），后台线程防抖 / 缓存 / 超时，`submit` / `poll` 非阻塞。网络请求期间有更新输入时取消旧请求并只处理最新请求；缓存命中先于防抖窗口返回。
  `PredictConfig` 是配置的 `[predict]` 分节。只在组句中联想，一次请求给云端词（容错校验后补进候选第一页末尾 `[predict] slots` 格，缺省 2，不预留不占位，
  前面的本地候选不挪；排布在 Core `CandidateLayout`）和整句补全（preedit 右侧，Tab）；上屏后不联想，本地历史不进请求。
  简拼（半数以上音节是缩写）的请求 `max_items = 0`，只求整句补全（`prediction::mostly_abbreviated`）：按声母凑出来的词大多是生造词，
  拼音校验又按首字母序列匹配放行缩写，拦不住；问字模式的答案不受这条限制。
- `CloudGlossFiller`：释义兜底（Core `GlossFiller` trait，与 Predictor 分开的线程与通道，攒 1.5 秒 / 8 个词发一次，问过不再问）：
  随包释义表没有的词库词 / 云端词上屏后入队，结果壳每秒 `Engine::poll_glosses` 经 `Translator::learn` 写进 `qingjian-translate::PersonalGlossary`
  （`user-glossary-<语言>.tsv`，`LayeredTranslator` 个人表优先）；随云联想开关一起开。
- 问字键（缺省 `u`）开头是问字模式（`PredictionKind::Question`，答案带读音、不校验拼音），`?` 开头要 `ModeKeys::question_mark` 开着才算（配置 `[shortcut] question_mark`，缺省关，壳用 `Engine::takes_question_mark` 决定空缓冲区的 `?` 是入口还是标点）；`PredictionKind::Translate` 是壳里快捷键触发的「翻译选中文字」
  （双向：汉字为主译成学习语言，外文译回中文，`prediction::translation_target`），译文走结果的 `sentence`。

## crates/qingjian-format

`.qj` 数据容器（`Container` mmap 读、`Writer` 写、`Table<T>` / `Text` 零拷贝视图、`hash` 可落盘哈希索引、`Metadata` 名称 / 许可证 / 署名）。
词库与语言模型都能 `write_qj` / 从 `.qj` 打开，启动 50 ms；`cargo run --release -p qingjian-dict-convert -- pack dict|lm --name … --license …`
生成 `data/generated/{dict,lm}.qj`，`bundle.sh` 在 TSV 更新时自动重打并只把 `.qj` 打进包。设计见 `docs/design/architecture.md`「数据文件：`.qj` 容器」。

## crates/qingjian-neural

candle 加载 Transformer（GPT-2 风格 decoder，导出成
`model.safetensors` + `config.json` + `vocab.json` 三件套）。features `accelerate` / `metal` 换后端，壳用 `metal`。

Core 的 `sentence::SentenceScorer` 有两个实现，同一个 trait 拿到**两种条件**（`context` 光标前文、`keys` 这批路径共同解释的那段按键），各挑自己训练时的那个、忽略另一个：

- `CharScorer`（含章·知微，字级模型）用 `context`：给「前文 + 整句」按字累加 log 概率，前文的每层 K / V 缓存（`PrefixCache`），
  同一段前文只算一次，每个候选只算自己那几个字（64 字前文 × 8 条 28 ms，Metal）。
- `P2cScorer`（含章·通变，P2C 模型）用 `keys`，不看前文。**产品端整句重排用它**：冻结集 8322 句上 42.49% 对含章·知微字级模型的 41.80%
  （配对 McNemar p=0.003，留出验证 λ 未过拟合），中位延迟 18.5 对 22.0 ms，而且同一个模型还能造词。
  字级模型没有下岗，它是移动端「光标联想」那类 `P(下一段 | 前文)` 功能的基础——P2C 的 `<eos>` 明确切断上文，结构上做不了那件事。

Engine 侧在 `engine/rescoring/`：接了打分器就取 Viterbi 前 `RESCORE_PATHS` = 6 条路径按 `路径分 + λ·(神经分 − 静态二元分)` 重排（λ `NEURAL_WEIGHT` 0.5，
个人 n-gram / 用户加分 / 代价不动）。**神经分只决定名次，不写回 `Conversion::score`**：P2C 打的是 `log P(汉字 | 拼音)`，
比静态分高十几二十 nat 且抬升随句长变化，写回去会污染跨读法的比较（拼写纠错的原样 vs 纠正、混输的头段 vs 整段），
那两处的门槛都按静态尺度定；按贝叶斯展开也只在同一串按键下可比。分走「(前文, 按键) + 文本 → 神经分」缓存 `NeuralCache`（两个条件任一变了整张作废）；
按键取的是**当前这批 patterns 覆盖的字母**而不是整个作用域——英文尾巴那条只转换 head，拿整段当条件会让它凭空背上没覆盖的字母；
同步打分器（`with_sentence_scorer`，CLI 评测）当场补分，
异步的（`with_async_sentence_scorer`，后台线程 `RescoreWorker`）查询不等模型：缺分的记下来，壳停键后 `request_rescoring`、`poll_rescoring` 到了再 `query` 一次。
前文优先用壳给的应用光标前文（`set_rescoring_context`），没有用本会话最近 64 个上屏字符。CLI `--neural <导出目录>`（`--neural-weight` / `--neural-context` / `--neural-async`）。

P2C 教师强制打分 `CharScorer::score_p2c(keys, texts)`：前缀 `<eos> + 完整拼音 + <sep>`，只累加候选字符的 log 概率（不计 EOS），
共享拼音前缀 KV，候选批量前向；超过上下文时报错，不截断拼音。CLI 实验入口 `--eval-p2c data/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm --eval-text data/eval/sentences.tsv`，
用 `--neural-weight` 调 λ。评测适配器与产品端 `P2cScorer` 同一条件，只是推理出错时直接终止（不静默回退到基线冒充成功）。
`--eval-details <输出.jsonl>` 为任意整句评测保存逐句首选、整句候选、全部候选与查询耗时，支持配对比较，仍不上屏、不学习。

P2C 自由生成实验：`--eval-text <集> --eval-generate data/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm`（`P2c::convert` 束宽 5，不经词图不经重排），
量的是重排这层架构的上限代价；2081 句上首选 51.9% / 前五 73.5%，对比重排路径的 42.3% / 45.5%，两条路互补（只有生成对 16.5%、只有重排对 6.9%），
代价是 `O(生成字数)` 次串行前向。数字与取舍见 [neural-rescoring.md](neural-rescoring.md)。

**模型直接生成的整句**（`engine/query/generating.rs`，候选类型 `CandidateKind::Generated`）：词图只会把整段按键读成拼音，
中英混输（`yongdockerbushuhenfangbian`）与生词在它那里根本没有路径，出来的只能是把英文段硬读成拼音的废话（用的哦乘客仍不熟很方便）。
词图读不通整段时改问 `SentenceScorer::generate`（P2C 走 `P2c::convert`，字级模型返回空），生成的整句插在词图那几条前面。
「读不通」三条：最优切分里有不完整音节、切分没覆盖到末尾（`woyongvscodexiedaima` 的 `v` 起不了音节）、拼写纠错生效
（`womaileyigeiphone` 的 `phone` 被当成敲错的 `paone`）。拼音干净的输入一条都不触发，常态零成本；
异步打分器下走同一次 `request_rescoring`（`Job.generate`），按键回调不等它。
`GENERATE_BEAM` = 5、`GENERATE_MAX_CHARS` = 32、`GENERATED_CANDIDATES` = 2、`MIN_GENERATED_LETTERS` = 6；双拼 / 注音不走（按键不是模型见过的字母）。
生成的候选没有音节对齐，上屏吃掉整段作用域、不记学习。漏的是「英文词本身就是合法拼音」那种（`zhegeapihenhaoyong` 的 `api` 读成 `a pi`），
光看切分分不出来。

整句候选不止一条：`SENTENCE_CANDIDATES` = 3 条，重排后的前几条路径都进候选表，四个音节
（`ALTERNATE_MIN_SYLLABLES`）以下不给备选（那几格留给词级候选）。冻结集上前三 42.4% → 45.5%，无模型时 38.1% → 43.2%。
`Engine::sentence_paths` 的 `want` 参数控制算几条：拼写纠错对每个纠正候选都要转一次，那条路仍然只算一条。

冷启动字词实验：`--eval-cold <样本.jsonl> --cold-output <新结果.jsonl>` 强制用缺省配置与内存学习器，
与 `--config` / `--user-dict` / `--predict` / `--replay` 等冲突，不加载或删除个人数据。
每行样本字段 `id/text/keys/source/category`；按完整拼音查候选，不给上文、不上屏，解析失败也记录在分母中。
领域词库用 `--extra-dict` 显式指定，`--neural` 可测字级重排；`--cold-model` 可同时测 P2C beam 5、固定上限 16 字的生成。
实验合并策略保留原首选，将生成的新增汉字候选插在其后，再接原候选；保存全部候选、词库文本/读音覆盖及耗时。
该策略只量候选覆盖和排序代价，没有改产品行为。生成搜索不读取目标字数，候选输出拒绝覆盖已有文件。

## crates/qingjian-lm

`BigramModel`，Core `sentence::LanguageModel` trait 的实现，从 `data/generated/lm.qj`（或 `lm-unigram.tsv` / `lm-bigram.tsv`）加载
（没有这两个文件就退化为一元词频整句）。数据由 `tools/corpus/parquet_to_text.py`（uv 脚本，HF parquet → 简体纯文本）加
`cargo run --release -p qingjian-dict-convert -- bigram --phrases assets/lexicon/phrases.tsv --phrases assets/lexicon/domain_words.tsv --brand assets/lexicon/brand.tsv --brand assets/lexicon/mixed_words.tsv data/corpus/*.txt` 生成；语料在 `data/corpus/`（gitignore）。
短语层不当 token 统计（分词时摘掉、统计完按成分合成一元 / 二元，短语得分等于原来两个词的路径，见 `bigram.rs` 模块注释），品牌词按给定次数写进一元与句首二元。

## crates/qingjian-platform

输入方案是**两条独立的轴**：`Scheme`（拼音侧：全拼 / 双拼七套 / 大千注音 / 关，`[general] scheme`）
与 `[general] wubi`（形码侧：空为关 / `wubi86`）。两边都开就是**混输**（`GeneralConfig::mixed`）。
`scheme_label(pinyin, wubi)` 是状态条显示的方案名（形码在前），做成自由函数而不是存进 `RouterConfig`——
存了会与那两项冗余、手搓配置的地方就漂移（状态条那条测试正是这么发现的）。

演进：2026-09-16 之前是 `shuangpin` + `zhuyin` 两个键表达同一个维度，先并成单选的 `scheme`，
同一天又拆成两条轴（单选的 `scheme` 表达不了混输）。旧键（`shuangpin` / `zhuyin` / `scheme = "wubi86"`）
都还在读、文件不自动改写。


`Config`（TOML 配置文件，`[general]` / `[shortcut]` / `[fuzzy]` / `[dictionaries]` / `[apps]` / `[predict]` 分节，首次运行写模板，
`set_value` 用 toml_edit 原地改键保留注释；`[model] enabled` 本地整句模型开关，`LocalModelConfig`；
中英模式两项：`[shortcut] switch_mode`（`SwitchKeys`：勾选 shift / control / ctrl+alt+space，可多选，老配置的单个字符串照读）与 `[general] english_mode`（内置英文模式总开关））；
`extra_dictionaries` 列出 / 加载随包领域词库与用户 `dicts/`
（mac 壳与 Windows Server 共用，同名 `.qj` 优先于 `.tsv`）；`code_tables` 同构地列出 / 加载随包根 `codes/` 与用户 `codes/` 的码表
（`[aux_code] disabled` 是黑名单，`[general] aux_code_key` 缺省 `;` 且校验后退回缺省、`aux_code_show` 是显示码开关）；
`protocol` 模块是 Windows Server ↔ TSF DLL 的 IPC 协议类型
（`ClientMessage` / `ServerMessage` / `Frame` / `PreeditSegment`，全 serde，两端共用，见 `docs/design/architecture.md`「Windows：TSF」；
`PROTOCOL_VERSION` = 8（v7 加任务栏图标右键菜单的 `Indicator`），`PreeditKind::AuxCode` 对应 Core 的 `MarkedKind::AuxCode`，`Frame.aux_code_show` 随帧下发显示码开关）。

## crates/qingjian-render

横排矩阵：`Frame::columns` 不为 0 时 `Layout::Horizontal` 走 `renderer/matrix.rs`（列宽用帧里的 `column_ems`，Core `Grid::column_ems` 按整份候选估、
单格封顶 `MAX_CELL_EMS` = 4 字宽，滚动时窗口不跳；网格下固定一行信息，放不下的截断）；视口与高亮移动在 Core `candidate::layout::Grid`（`GRID_ROWS` = 6），
预览示例里有 `matrix-horizontal` 场景。mac 壳里这套按键由 `[general] horizontal_grid`（缺省关）加横排两个条件一起开（`Host::grid_keys`）。

自绘渲染器：候选窗一帧 + 主题 → 预乘 RGBA 位图，tiny-skia 栅格 + cosmic-text 文字（fontdb 按平台清单只加载几个字体文件、不扫系统），
自己解析 `trak` 字距表、按主题 gamma 查表加深笔画（直接对 swash 字形遮罩应用 LUT，不为每个字形另分配校正后的缓冲）；cosmic-text 打了 `opsz` 光学字号补丁（qingjian-team/cosmic-text 分支 `qingjian-opsz`，workspace `[patch.crates-io]` 钉 rev）。
阴影位于 `shadow/`：`parameters` 保存外观参数，`key` 精确匹配几何，`scratch` 在线程局部复用全尺寸 RGBA 遮罩、alpha mask 与 `blur` 三遍盒式模糊工作区；最近一项模糊 mask 按画布尺寸、内容矩形、圆角、缩放和阴影几何参数缓存，同几何重绘跳过遮罩栅格与模糊。颜色只在混合时应用，改色不重建 mask；几何变化与缓存命中均有像素一致性回归。
垂直模糊维护整行 `u32` 列和，按行连续读写到复用输出 mask，每遍结束交换输入输出；舍入和边界零填充保持逐列算法，确定性随机尺寸/半径与三遍组合做逐像素对照，避免宽窗口逐列跨步访问。
`cargo run --release -p qingjian-render --example benchmark -- --scale 1.25 --report target/perf-render-report.md` 测冷帧、同尺寸高亮轮换、逐键增长至 51/160 键的紫色渐变竖排场景，输出平均/p95/max 与位图尺寸范围。只计 `Renderer::render`；不含字体初始化、线程启动、查询、网络或平台贴图；冷帧用独立线程和新 Renderer，系统文件缓存不清空。`--no-shadow` 可核对同构建无阴影场景；不得把不同构建方式或未测旧版当成加速基线。
`examples/preview.rs` 出 PNG 与真机截图并排比、`--measure` 与 AppKit 对宽度。mac 壳 `candidates/bitmap/` 贴位图，`[general] renderer = "system"` 切回 AppKit 绘制
（过渡期退路，偏好设置「候选窗口」页可选）；`[general] font` 是候选窗字族名（空为系统字体，`bitmap/font_files.rs` 用 CoreText 按字族名找文件只加载那几个，没装就回系统字体；
设置页 `preferences/font_picker/` 是搜索框 + 列表）。设计与验收见 `docs/design/rendering.md`。

## crates/qingjian-update

检查更新（设计见 `docs/design/update.md`）：`index/` 是索引的类型、下载（`fetch.rs`，复用 workspace 的 reqwest + 单线程 tokio，20 秒超时、2 MB 上限）与验签
（`signature.rs`，`PUBLIC_KEYS` 列表，`verify_strict`）；`checker/` 是调度（`Checker::poll` 由壳的每秒定时器调，到点起一次性线程）、落盘状态 `UpdateState`（`update.json`，先写临时文件再改名）
与查到的结果 `Available`。`Version` 自己实现语义化版本比较，不引 semver。`[update]` 配置与 `UpdateChannel` 在 `qingjian-platform`。
`examples/check.rs` 手动走一遍；`tools/release-sign` 是发版侧的 keygen / sign / verify。

## apps/cli

测试工具，`cargo run -p qingjian-cli -- kaifa`。

- `--predict` 强制开云联想并等结果打印，交互模式下上屏后也联想。
- `--wubi <码表>` 用形码码表（`词\t编码\t词频` 的 TSV）替代拼音：按键当编码按前缀查表，候选不带音节，上屏吃掉整段编码。
- `--typing` 逐键计时（性能测试用 release 构建跑，目标每键 10 ms 以内）。
- `--chinese-first` 打开中文优先（`[general] chinese_first = true` 的排法），配合 `--replay` 比两种英文词位置。
- `--replay <input-log.jsonl>` 回放评测：把日志里每次上屏的键重新喂给引擎，按来源算首选 / 前五命中率、平均名次、不在候选的条数，打印没命中的例子（`--misses N`）；
  只在内存里学习不写文件，加 `--user-dict` 可带上现有学习数据。
  **按日志记的方案逐条切换**（`replay/scheme.rs` 的 `SchemeSwitcher`，读每条的 `scheme` 字段）：整份日志通常只有一套方案，
  所以只在方案串变化时才换码表（`Engine::set_code_table` 收所有权，换一次要克隆 8.9 万条）。日志里是形码但没给 `--wubi` 时，
  那部分只计数——拿拼音的读法喂形码的键会算出看着像真的、实则无意义的命中率。
  含触发键的日志行按壳那样逐键重喂（进辅码态、码段进码段），
  并多打一行「辅码选词 N 条，其中同拼音纯输入首选命中 M 条」（学习闭环的尺子）。
- `--aux-table <路径>`（可多次）装辅码码表（`.qj` 或 `词<Tab>码` TSV）；交互模式与查询模式都按壳的方式逐键喂入，
  配的触发键进辅码态、之后的字母进码段，候选行会带上命中的码。
- `--tune 名=值`（逗号分隔）覆盖个人 n-gram 插值与敲错代价的常数扫网格（名字见 `apps/cli/src/tuning.rs`，Core 侧是 `Engine::set_interpolation` / `set_typo_costs`，壳只用缺省值）。
- `--eval-text <文本>...` 装了码表（`--aux-table`）时多打一行码表覆盖率：词频前 10,000 与全库两段命中比例（与导入统计同源）。
- `--eval-text <文本>...` 整句评测：把用户自己写的中文文本按标点切句、按词库读音转成全拼，冷启动喂给引擎看整句能不能还原原句
  （首选命中率 / 字准确率 / 查询耗时；不依赖日志里当时选了什么，给整句排序与语言模型的改动当尺子），`--eval-save` 冻结成 `句子\t拼音\t上文` 三列文件，
  之后直接 `--eval-text` 它保证比的是同一份句子（本机的在 `data/eval/sentences.tsv`）。排序、整句、纠错的改动先跑它们再合。

## apps/macos

IMK 输入法，源码按 `app / host / imk / candidates / menubar / preferences` 分目录。

- 输入法菜单（状态项 + 系统输入源菜单）与偏好设置窗口都是配置文件的前端：只写 `config.toml`，`Host::apply_config` 一条通路热加载，激活期间每秒看一次文件 mtime。
  输入方案（`[general] scheme`）也在这里装配：双拼 / 注音设给引擎，形码额外按 `paths::code_table_path()` 挂码表
  （用户目录 `wubi/wubi86.tsv` 优先，包里 `Resources/wubi/` 兜底；找不到只警告并按拼音跑）。
- `apps/macos/scripts/bundle.sh --install` 打包安装到 `~/Library/Input Methods/`（开发用），`--pkg` 做分发用的 pkg（装 `/Library/Input Methods/`，postinstall 跑 `qingjian-macos --register`
  注册、启用并切成当前输入源；签名 / 公证靠 `QINGJIAN_SIGN_IDENTITY` / `QINGJIAN_INSTALLER_IDENTITY` / `QINGJIAN_NOTARY_PROFILE`，没设就 ad-hoc；`QINGJIAN_TARGET` 指定架构，
  成品 `target/pkg/qingjian-<版本>-macos-<arm64|x86_64>.pkg`）；`scripts/uninstall.sh` 卸载。
- 日志在 `~/Library/Logs/Qingjian/`（按天分文件留 7 天，删了会重建），用户数据与配置在 `~/Library/Application Support/Qingjian/`。
- 配置项：云联想 `[predict]`（偏好设置「云服务」页有「测试连接」按钮：`qingjian_predict::ConnectionTest` 起线程发一条最小请求，`Host` 用独立定时器 `CloudTestMonitor` 轮询结果；按钮旁保留独立状态并同步到底部，配置刷新不清空测试结果，测试期间禁用按钮且 Host 拒绝重复请求；
  `reasoning_effort` 缺省 `none`，DeepSeek V4 默认思考，不关正文为空）；模糊音 `[fuzzy]` 默认都关；`[general]` 学习语言（`off` 不显示译文）/ 每页候选数 / 翻页键 / 外观 / 竖排横排 / 拼音显示位置 /
  英文模式候选开关 / 中文优先 `chinese_first` / 双拼方案 `shuangpin`（小鹤 / 自然码 / 微软 / 搜狗 / 智能ABC / 小浪 / 首道，空为全拼）/ 日志级别 `log_level`（缺省 info 不含敲的内容，debug 逐键记，热切换）/ 输入日志 `input_log`；
  `[shortcut]` 模式键 v / u、`question_mark`（缺省关，开了空缓冲区敲 `?` 进问字）、上屏第一 / 第二个译词的修饰键 `translation` / `translation_second`、删候选 `delete_candidate`（缺省 shift，用户词整删、词库词清学习）、翻译选中文字 `translate_selection`；
  `[apps] english_candidates_off` 按 bundle identifier 列出英文模式不给候选的应用（缺省终端 / 编辑器 / IDE，`*` 前缀匹配）；
  `[dictionaries] domains` 打开随包的领域词库（`Resources/dicts/` 11 本，缺省只开 `idioms`），`disabled` 关掉用户目录 `dicts/` 里的某本导入词库；
  偏好设置「词库」页随包的可开关、导入的可开关 / 移除，可导入 TSV / Rime yaml / .qj。
- 系统文本替换（系统设置「键盘 → 文本替换」）：`host/config/text_replacements.rs` 从 `NSUserDefaults` 全局域读 `NSUserDictionaryReplacementItems`
  （每条 `{ on, replace, with }`），激活输入法时重读，变了就经 Core `merge_replacements` 并进配置里的自定义短语再 `set_custom_phrases`；
  `[general] system_text_replacements` 开关（缺省开，「自定义短语」页勾选框），内容可能含证件号、地址，日志只记条数。
- 输入法进程由 launchd 拉起，看不到 shell 的环境变量：密钥写进配置同目录的 `.env`（`QINGJIAN_API_KEY=...`，输入法启动时 dotenvy 读入）或 `config.toml` 的 `api_key`。
- 本地整句模型：`bundle.sh` 把 `data/models/hanzhang-tongbian/`（或 `QINGJIAN_P2C_MODEL_DIR`）打进 `Resources/models/hanzhang-tongbian/`，通变优先；知微放 `Resources/models/hanzhang-zhiwei/` 作回退，用户目录的对应模型优先于随包同类模型，旧用户目录仍可读取。`host/model/mod.rs` 在后台线程加载并预热（首次 Metal 编译）后
  `set_async_sentence_scorer` 接上，`refresh` 每键先读应用光标前 64 字给 Engine 当前文、查询后 `schedule_rescoring`，`RescoreMonitor` 停键 80 ms 请求、20 ms 轮询，
  结果到了重查一次只重画当前页（翻过页 / 动过高亮不动）；「云服务」页有开关（`[model] enabled`）。
- 端到端验证可用 `osascript` 的 System Events 往 TextEdit 发按键再读回文本（终端需要辅助功能权限；输入法得在中文模式）。
- `Info.plist` 的 `app.qingjian.inputmethod.Hans` 模式使用 `tsInputModeScriptKey=smSimpChinese`，模式 repertoire 仅 `Hans`；顶层 repertoire 仍包含 `Hans`、`Latn`。这是对旧模式被 TIS 标为 ASCII-capable 的受控声明修正，是否修复新 TextEdit 选中后仍输出英文、无 IMK 会话的问题，须以安装后 A/B 验证为准。端到端验收同时核对当前模式、Vakyu 会话、候选和中文上屏，不能只凭注册命令成功。

## apps/windows

候选渲染的详细日志只记录耗时达到 5 ms 的帧，分别标记渲染（布局、阴影与文字）、RGBA → DIB 转换（含 DIB 分配）和 `UpdateLayeredWindow` 提交，不记录候选正文；快帧不逐条写日志。Windows 的本机性能验证使用 Release 构建，Debug 的软件栅格与逐像素循环成本不能代表交付版性能。

候选窗与悬浮状态条共用 UI 线程上的系统深浅色查询缓存，1 秒过期，避免逐键显示候选时重复打开注册表；系统外观变化最迟在缓存过期后应用。

语音浮窗位置由 `ui/voice::place` 按输入目标所在显示器的 `rcWork` 底部居中计算，保留阴影和 16 个逻辑像素的底部间距；任务栏自动避让，多屏负坐标和 DPI 由现有 monitor/layered 处理。光标只用于选择屏幕，浮窗不跟随输入框移动；转写行数增加时保持底边不动、向上展开。显示时机仍由按住生命周期控制。

长语音不再设置会话总时长截止：全局控制器、神经语音线程、SAPI 回退及 Python 采集都仅由松键或取消关闭采集；模型的断句只累积文字与重置解码段，不结束本轮。松键后的神经收尾看门狗仍为 12 秒，全局兜底为 15 秒，避免抢先丢弃识别线程的结果或错误；失联、焦点变化和取消的关麦保护不变。全局日志只记录结束字数、取消原因及阶段时长，不记录正文；浮窗的尾部 256 字显示裁剪不影响最终全文。

中英跟读是用户主动开启的例外流程：原 `[shortcut] voice_english_practice=false` 保留为“启用跟读练习”总开关，默认关闭；新增 `voice_practice_learn_english=true` 独立选择方向，默认中文口述→英文跟读，`false` 为英文口述→中文跟读。Windows 快捷键页可热切换，两方向共用 `voice_practice_shortcut`，方向变化取消未完成轮次，关闭总开关恢复普通中文语音。口述 Final 后先关闭本轮采集并锁住原文；`VoicePracticeTranslator::request_to(id, source, Language)` 使用独立异步请求翻译本轮文字，原 `request` 兼容默认英文目标；仅支持 Chinese/English，其他目标返回 `UnsupportedLanguage`。不读取光标上下文，也不输出原文/译文日志。它复用 `[predict]` 的端点、模型和密钥，但不要求开启预测联想；每段仍为一次文字请求，最多 4000 个原文字符、8192 输出 token、120 秒超时。提示要求忠实翻译、不续写，按目标读取 `english` 或 `chinese` JSON 字段并校验文字；中文允许拉丁专名，但纯英文不能通过。残缺或无效回复整条失败，不能截成可跟读的短句。请求通道只保留最新项，方向随独立代次一起替换，取消会销毁在途网络 future，复用公开请求 ID 也不交付旧方向结果。

跟读诊断记录实际应用的开关、口述收尾、翻译提交与结果、再次开麦及取消阶段。取消来源只保留键盘、鼠标、外部注入、前台变化、焦点通知等粗分类，不能代替活动代次判断；不记录具体按键、窗口对象、正文或凭据。目标捕获与复核失败用固定错误枚举记录，便于区分录音未启动、翻译失败和焦点误取消。

`qingjian-learning::EnglishPractice` 是平台无关的纯文本匹配，正文不落盘。每次按本轮完整累计 ASR 文本重算英文目标的已读前缀，兼容常见缩写、数字等价及有限语气词/重读；临时识别修正时允许高亮回退。累计文本中的错误允许通过实际读出的正确短语重新对齐，局部修正需后续正确上下文，整句/短语重读可恢复旧错误；正确词与语气词的重复不设六词硬限，不能借纠错省略必读内容。`with_level` 在顺序匹配上加入有预算的识别文字容错：英语近音使用内嵌 CMUdict 的音素比较，缓存词典；新手允许有限口音音素差异，进阶收紧距离，挑战仅允许同音。近音必须消耗实际识别词，与弱读省略预算分离，短句和首尾词也可匹配，1 个英文词可对应 1–3 个识别词。`with_context` 只对译句中与中文原句连续汉字读音对应的大写专名启用拼音音节匹配；允许拆合词或 ASR 汉字结果，不能省略音节。`pinyin` 多音字数据只用于本轮匹配，不保存原文。CMUdict 与 pinyin 来源及分发许可证见 `assets/pronunciation/README.md`。弱词省略只允许在完整实词锚点之间，首末词、数字和否定含义不能借容错跳过；它不分析声学音素，也不提供发音质量分数。Server `voice/global/practice` 把临时完整匹配稳定 650 毫秒视为停止采集的条件，只有 Final 再次完整匹配才交付原中文；Final 回改后保留已确认文本并重新打开跟读采集以补读。英语识别依赖已安装的中英神经模型，中文 SAPI 回退不进入跟读。

`qingjian-learning::ChinesePractice` 对中文目标使用独立的纯文本顺序匹配：汉字逐字形成显示单元，拉丁专名保留单词，标点附着前一个单元；目标必须包含汉字。根据累计 ASR 文字匹配汉字或允许的同音读音，中途闲聊、重读和识别回改可以继续尝试，不跳过必读目标字。拼音提示由本地 `pinyin` 生成，同音匹配不是声调或发音标准评分。两方向复用已安装的中英神经识别模型，不增加模型、常驻识别进程或 GPU 依赖；中文 SAPI 回退仅用于普通中文口述，不进入双向跟读。跟读成功最终交付锁定的原文，学英语填原中文、学中文填原英文，不交付译文。

`PracticeProfile` / `PracticeRound` / `PracticeLevel` 管理平台无关的经验与轮次结算：累计 0–119 XP 为新手 Lv.1，120–499 XP 为进阶 Lv.2，500 XP 起为挑战 Lv.3；轮次开始时固定等级，完成后的升级在下一轮生效。Final 确认完整匹配、原目标复核且系统接收输入成功后，按显示单元数奖励，单轮上限 60 XP；同一轮只能领取一次，取消、失败和临时结果均不奖励、不扣分。Windows `voice/global/game` 仅把经验和完成次数原子保存到当前用户的 `%LOCALAPPDATA%\Qingjian\` 下，英语使用 `english-practice-progress.json`，中文使用 `chinese-practice-progress.json`，方向切换不覆盖另一份进度。不使用 IP、句子或录音识别用户；文件损坏时保留原文件，当前运行只在内存累计。

`[shortcut] voice_practice_demo=true` 默认开启示范，但不改变默认关闭的 `voice_english_practice` 总开关或用户已保存的开关值。Windows 快捷键页可单独关闭示范。`voice/global/demo` 的独立 SAPI TTS 工作线程按目标语言选择本机中英文声音，通过代次取消播放；新手/进阶/挑战的语速分别为 -2/-1/0。`Demonstrating` 阶段麦克风保持关闭，播放结束后再等 400 ms 消散尾音，复核原目标和活动代次后才自动开麦；未找到对应语言声音或播放失败时明确显示示范不可用，改为直接跟读，不能直接放行原文。关闭示范时直接进入跟读准备。翻译、示范、录音和取消均沿用原轮次与焦点保护。

跟读在松开快捷键后显示原文及目标译句，后台每拍续约浮窗租约，译句围绕当前显示单元最多显示五行；已读深色、未读浅色，当前位置有下划线和独立提示。中文目标提供本地拼音，英文音标仅从发音词典取得。`PracticeView` 带 `level_label` / `level_progress` 显示当前语言等级徽章与真实经验，底部状态行区分示范播放、跟读录音及收尾，界面不推算经验。原输入框、活动代次和权限在翻译前、再次开麦前与上屏前复核；Esc、切换学习方向、关闭总开关、安全桌面变化或后台更新取消整轮。翻译/识别失败不放行原文，跟读完成仅填入、不发送 Enter。全局启动失败且跟读已开启时禁用旧语音回退，避免绕过门禁；开启跟读或切换方向也会撤销旧轮次及其迟到结果。常规模式仍是按住显示、松开收窗。

全局语音入口在 `server/voice/global`：单实例命名管道取得后，独立按键线程监听左右 Alt、鼠标与前台焦点变化，独立控制线程每 20 ms 推进一份 VoiceManager。宿主是否加载 TSF DLL、是否切到青简都不影响语音入口。短 Alt 与组合键按扫描码恢复；按住达到阈值才开麦。结果只向开始录音时锁定的 HWND/PID/焦点与可编辑控件交付，真实键鼠活动、焦点变化或输入桌面切换立即取消。目标识别支持标准 Edit/RichEdit、UIA Value/Text 与 LegacyIAccessible；密码、只读、未知编辑能力及权限不足目标不注入。上屏前完整复核目标和活动代次，一次发送 Unicode 批次，不使用剪贴板、回车或自动重试部分写入；系统接收批次不等于应用正文已验证。UIA 连接与事务超时各 500 ms，录音中只快速查询 Win32 焦点和输入桌面。

全局所有者成功启动后，Router 的 `InputSettings.voice_input=false` 禁止宿主内的旧语音入口，旧 VoiceStart/Stop/Control 返回终止错误；普通 Poll、会话重连/断线不领取或取消全局结果。真实 VoiceSink 只由全局线程持有，旧路径不操作它。语音触发配置、跟读总开关、学习方向或示范开关变化，以及跟读开启时翻译配置变化，才更新全局配置；配置代次与永久暂停屏障阻止过期启用命令重新开麦或上屏。全局浮窗通过 250 ms 续约门禁判断是否仍按住，不用被钩子暂扣的 Alt 异步位；legacy 路径仍用原检查。下述 TSF 桥保留为全局初始化失败时的兼容回退。

Windows 按住说话：`server/voice` 优先使用 `%LOCALAPPDATA%\Qingjian\voice-runtime` 的独立中文流式识别进程（便携 Python + sherpa-onnx + x-asr int8 模型），启动服务时预热模型但不开麦。Rust 通过有界 JSONL 管道控制，helper 控制线程独立于推理，松开、取消或父进程退出会停止采集。只有未安装运行包时才使用独立 SAPI `SpInprocRecognizer`（中文 `Language=804`）；运行包损坏会明确报错。TSF DLL 不加载音频组件。`VoiceStart`/`VoiceStop` 立即应答，停止回 `Stopping`，后续 `Poll` 的 `Update.voice` 交付累计 `Interim` 或一次 `Final`；`VoiceCancel` 单向取消，代次隔离旧结果。流式断句保存此前文字，松键补静音排空模型右侧上下文；SAPI 尾部处理上限 1500 ms。不写音频文件、不将音频发给文本模型。

语音薄桥由 `InputSettings.voice_bridge_version` 协商；支持 v1 时，DLL 在安全锚点准备好后发送带单调 `sequence` 的 `VoiceControl`（Press/Poll/Release/Cancel），Server 负责长按计时、录音与结果生命周期。Press 只进入 Armed，不开麦；达到 `[shortcut] voice_hold_ms`（默认 180，规范化至 100–1000 ms）后还需当前会话的新 Poll 才启动，避免已松开或已断线的旧请求开麦。过期 sequence 或其他会话的控制消息不能停止、领取或抢占本轮。DLL 连接旧 Server 时回退原 VoiceStart/VoiceStop 路径；新 Armed 不交给旧 DLL。`voice_key`（alt/left_alt/right_alt）随设置热同步，设置页提供开关、触发键和灵敏度。

`tsf/com/service/voice` 保留物理键边沿、焦点、隐私与文档锚点保护；80 ms 轮询补齐宿主漏送的系统 Alt 边沿，按住时遇组合键取消。轮询仅接受连续两拍都在前台焦点线程的新按下，避免 Alt+Tab 切入、同进程后台线程或重复回调误开麦。松开即通知后台停止收窗，最终文字通过异步 TSF 编辑会话写入并再次校验原窗口、输入框与空锚点。生命周期日志不记录正文。`dispatch/voice` 的 2 秒心跳租约保留，管道断开另通知 Router 取消该连接会话；`ui/voice` 为无激活、鼠标穿透的临时浮窗。后台更新断线会取消未完成语音，DLL 自动重连后重新协商，不回放旧结果。

`apps/windows/scripts/update-components.ps1` 用逐组件 SHA256 计划更新，省略 x64/x86 时只替换后台/设置程序；未变化的 DLL 不复制、不注册。需要升级薄桥时用内容哈希命名并行 DLL，核对注册并列出仍载旧版的应用，不强退宿主。替换后台前经 PrepareUpdate 取消语音并落盘学习数据，持安装互斥体防旧 DLL 自动拉起，原权限进程重新启动后台；失败恢复已替换文件与旧注册。仅兼容后台更新可免重开应用，基础 DLL 更新仍需重新加载一次。

兼容回退路径的语音按下边沿及活动会话会查询当前键盘 profile，必须同时匹配青简的 profile 类型、CLSID 与 GUID；线程有焦点本身不能证明青简是当前键盘输入法。其他类别的 profile 激活通知不作为键盘身份依据。独立全局语音不依赖该 profile。

兼容回退路径的最终文字通过 `ITfInsertAtSelection` 写入原空锚点，不能只调 `ITfRange::SetText`：Win32 记事本的过渡上下文可能返回成功却没有把文字送进正文。写入前保存当前选区、临时选中锚点；插入后原光标未移动则跟到末尾，否则恢复保存的选区。已有键盘组句仍等待结束，非空锚点拒绝覆盖；正文插入后的光标错误不重试正文。日志只确认宿主接受请求，最终上屏以真实窗口验证为准。

Windows 神经语音启动顺序：先导入 NumPy / sherpa-onnx 原生依赖，再启动 stdin 控制线程，最后构造模型。实测反过来会使首次原生模块初始化与匿名管道阻塞读取互相卡住，单独 `--wave` 无法覆盖。模型构造仍在控制线程启动后进行，保留冷加载期间松键与取消的保护；发包必须补跑真实管道 `Ready` → `Shutdown` 测试（不发 `start`、不开麦）。

本机无麦克风验证：先用中文 TTS 生成 WAV，设置 `QINGJIAN_VOICE_TEST_WAV`（可另设 `QINGJIAN_VOICE_TEST_EXPECTED` 关键词），以 `QINGJIAN_UIACCESS=0` 运行 `cargo test -p qingjian-windows-server --lib transcribes_chinese_wave_fixture -- --ignored --nocapture`。该测试只读取合成音频。

神经语音 helper 源码在 `apps/windows/server/voice-runtime/`；安装包布局为 `python/python.exe`、`app/main.py` 与 `models/streaming/`。`main.py --wave <16-bit PCM WAV>` 从文件回放，`--devices` 只列设备，两者均不开麦。实时输入保留设备采样率，补尾静音也使用同一采样率，避免 44.1/48 kHz 麦克风在结束时触发底层重采样器错误。模型断句保留累计全文，英语分段保留空格。停止与取消分别回 `stopped` / `cancelled`；Rust 在会话取消后仍保留独立 2 秒关闭确认看门狗，失联则终止子进程。设备原生启动未完成时只撤销采集意图，不并发关闭同一音频句柄，实际关闭后才确认。新 helper 首次预热可能需要约十几秒，之后各段录音共用模型。

一个产品两个 package：`server`（Server 进程：IPC 分派 + Engine + 命名管道 + 自绘候选窗与悬浮状态条）与 `tsf`（TSF 文本服务 DLL，lib 名固定 `qingjian_tsf`），
外加 `settings`（WinUI 3 设置程序，含「辅码」页）与 `installer`（Inno Setup）。
Server 侧辅码接线：`RouterConfig.aux_code_key` / `aux_code_show`（`apply_config` 热加载）、`assembly` 从随包 `codes/` 与用户 `codes/` 装码表
（`[aux_code] disabled` 过滤，`code_tables::load`）、按键在 `dispatch/key/input.rs` 分流（触发键进辅码态、辅码态里字母进码段、
大写（`shift_letter = "compose"`）先清码段回拼音态再进缓冲区、标点先上屏高亮候选再转全角）、候选窗 `ui/candidates/row.rs` 的 `Row.code` 把码用方括号括起来紧跟在候选词后面（不进 annotation），拼音行 `view.rs` 给码段加下划线。
热加载的 `dicts/` 与 `codes/` 目录与启动同款（修过一处传基础目录的错）。不合成一个 crate，因为 DLL 不能带 Engine 的依赖树，见 `apps/windows/README.md`；
协议类型在 `qingjian-platform::protocol`，设计见 `docs/design/architecture.md`「Windows：TSF」。
输入方案由 `[general] scheme` 一处决定，Server 启动与热加载各装配一次；形码的码表用 `dispatch::code::find_code_table` 找
（用户目录 `wubi/wubi86.tsv` 优先，随包 `assets/wubi/wubi86.tsv` 兜底——走 `assets/` 与 emoji / levels 一致，开发布局也对得上），**路径在启动时定下、热加载不重新找**。
选了形码却没有码表文件时只警告并按拼音跑——配置说五笔、引擎还在拼音是静默错位，宁可吵。
中英模式的两项设置（`[shortcut] switch_mode` 切换键：勾选 shift / control / ctrl+alt+space，`[general] english_mode` 内置英文模式开关）
由 Server 经协议下发给 DLL（`InputSettings`，见下文「按键行为设置」），改完在下一拍（约 320 ms）生效；
`ctrl+alt+space` 走 TSF 保留键登记（`com/key/preserved.rs` 的 `GUID_SWITCH_MODE`）。系统的 Ctrl + Space（「输入法/非输入法切换」）不进勾选项但适配它：
它翻的「输入法开 / 关」compartment（`com/mode/sink.rs`）关 = 英文、开 = 中文，我们切模式时把开关写成一致；开关一变也作废被截走 Space 的那次「单击 Ctrl」。
模式全局一份、存在 Server（`Router.english`），DLL 激活 / 得到焦点 / 轮询时 `SyncMode` 取回，用户切了 `ModeChanged` 报上去。会话号用线程 id（`com::session_id`）——TSF 的 client id
各进程都是同样那几个值，拿它当会话号会在 Server 那边撞号。四条切换入口都汇到
`service/mode.rs::set_english_mode` 一处拦住；状态条点击在 Server 侧（`dispatch/status/mod.rs`）按同一项拦，
设置界面在 `settings/src/panel/pages/general.rs`。

TSF 原有数字 / OEM 标点 / 空格键码按当前布局用 `ToUnicodeEx` 解析（bit 2 避免改变键盘状态），
仅接受单个非代理项 UTF-16 单元。字母、小键盘和 AltGr 处理不变，不保证组合音符输入。
拼音显示位置（`[general] preedit`）在 Windows 上分两处落地：Server 把它读进 `RouterConfig.preedit` 并随 `Frame.preedit_mode`
下发给 DLL，DLL（`com/service/key_sink.rs`）按 `inline()` 决定要不要放行内拼音，Server（`ui/candidates/render_data.rs::window_preedit`）
按 `in_window()` 决定候选窗口顶部画不画拼音行；`window` 模式没有组句范围，光标矩形改从 `com/edit/anchor.rs::caret_rect`（当前选区）量。
连不上 Server 时 DLL 自己拉起它（`tsf/src/com/service/launch.rs`）：`ShellExecuteW` 起与 DLL 同目录的 `qingjian-server.exe`
（`uiAccess=true` 的 exe 用 `CreateProcess` 报 740），进程内 5 秒冷却 + 跨进程命名互斥体防止砸出一串 Server；
起完清掉重连退避，下一键就试。Server 只在登录时由「启动」文件夹拉起，中途挂了以前只能等下次登录。

词库导入（设置「词库」页）走 `qingjian-dictionary::import` 转成 `.qj`（空词库拒绝），多选批量、成功的从 `[dictionaries] disabled` 摘掉、页面显示每个文件的结果；
Server 每次轮询比对用户 `dicts` 的路径 / mtime / 长度快照，配置没变也重载新增、同名更新与移除；配置解析失败时词库沿用上次有效的开关（#36）。

中英切换键（`[shortcut] switch_mode`）与内置英文模式开关（`[general] english_mode`）得在**按键到达之前**就知道
（单击判定在 `OnTestKeyUp`、是否登记语言栏按钮），但 DLL 跑在每个应用进程里、拿不到 Server 那份配置，
`%APPDATA%\Qingjian` 对 AppContainer 里的商店应用也读不到（那是给输入日志和 `.env` 用的目录，不该加 ACE）。
所以由 **Server 读配置、经协议下发**（`InputSettings`：`OpenSession` 回包带一次，之后每拍 `SyncMode` 跟着走），
DLL 不读文件、不查 mtime。`SessionOpened` 只回过协议版本对得上的 DLL——老的 `open` 是只写不读，
多回一条会被它当成下一次 `Poll` 的应答而报错，那条连接就废了；老 DLL 从 `ModeSync` 那一拍也能拿到同一份（新字段直接忽略）。

## assets

- `assets/sample/`：手写样例词库与释义表，不是产品数据。
- `assets/wubi/wubi86.tsv`：五笔码表（`dict-convert wubi` 生成，来源与许可见同目录 README，Apache-2.0）。
  `bundle.sh` 拷到 `Resources/wubi/`，Windows 安装器拷到 `{app}\assets\wubi\`（与两边 Server 的找法对齐：macOS `paths::code_table_path`、Windows `dispatch::code::find_code_table`）。
- `assets/emoji/emoji-zh.tsv` / `emoji-en.tsv`：Unicode CLDR 中文 / 英文 annotations 转出的 emoji 表（Unicode License v3，可发布；中文词与英文词各配 emoji，两张表加载时合成一张），
  `cargo run --release -p qingjian-dict-convert -- --out-dir assets/emoji emoji --language zh data/cldr/annotations-zh.json data/cldr/annotationsDerived-zh.json`（en 同理）。
- 英文词表词频：`uv run tools/corpus/english_frequency.py data/generated/english.tsv -o data/generated/english-frequency.tsv`，再 `... english <词表> --frequency <那个文件>`。

## tools/gloss-gen

用 LLM 批量生成释义表：`cargo run --release -p qingjian-gloss-gen -- generate`（密钥读 `QINGJIAN_API_KEY`，结果 JSONL 在 `data/generated/`，不进 git、可续跑，`--limit 80` 试跑）
再 `... export`（写 `glossary-{en,ja}.tsv`，产品数据在 `assets/glossary/`，见那里的 README；格式 `词\t词性. 译词[|假名]`）。CLI 与 bundle.sh 用的就是这两个文件。

## tools/dict-convert

产品数据的生成工具，输出到 `data/generated/`（gitignore）。

- `lexicon`：从 `assets/lexicon/`（自建词库源：规范字 + 常用词 + THUOCL 领域词）加 Unihan 读音（`data/unihan/Unihan_Readings.txt`）、LLM 多音字标注（`gloss-gen pinyin`，
  结果 `data/generated/pinyin-llm.jsonl`，不进 git）、语料词频（`lm-unigram.tsv`）建基础词库 `dict.tsv`（8.7 万条），并把 THUOCL 领域词按语料次数 < 50 拆成
  `dicts/<领域>.tsv` + `.qj`（11 本、13 万条，`--domain-keep-min`），流程见 `assets/lexicon/lexicon-build.md`；`--extra-words` 并入人工挑的领域词 `assets/lexicon/domain_words.tsv`。
- `english`：转 `assets/lexicon/05_english/00_all_words.tsv`；同编码优先保留含大写的专名写法（Windows ≠ windows），
  展示写法补充表 `07_display_forms.tsv` 后置读入；`cedict`：释义表备用来源。中英混杂词源在 `assets/lexicon/mixed_words.tsv`（`lexicon --extra-words`）。
- `wubi`：Rime 形码码表（`.dict.yaml`，极点 86 五笔）→ `词\t编码\t词频`（`wubi.rs`，`--name` 决定文件名，缺省 `wubi86.tsv`）。
  **码表自带的权重不用**——那是码表顺序不是语料词频，用了同一个词在形码下和在拼音下会排得不一样；词频从青简词库按**词面**交叉回填，
  词库里没有的词给 `UNKNOWN_FREQUENCY = 1`。解析复用 `qingjian_dictionary::import::rime`（与用户导入 Rime 词库同一个解析器，
  形码码表的第二列是编码，格式一样）。词库还没收的词不收（码表整张进，不做取码推导，见 `docs/plan/wubi.md`）。
- `bigram`：统计语料；`--phrases` 给短语层、`--brand` 给品牌词（`assets/lexicon/brand.tsv`，青简 210）与中英混杂词（`mixed_words.tsv`，C盘 / B站：合成计数要成分词在语料里，C 不是 token，只能直接给一元，次数对着同音竞争词定），领域词也走合成计数（语料里只有几十次的词当 token 统计会吸走成分词的二元证据）。
- `mine`：从语料挖词库没收的高频词并过滤（`oov_filter.rs`：虚词规则 + 相邻字对 PMI≥3，`--candidates` 只重过滤）。
- `phrases`：挖短语层（两遍扫语料：相邻两词、两段二元都够频的相邻三词，总次数与对话语料次数都 ≥ 2000 + 边界规则，读音由成分词拼出；我的 / 不知道 / 有没有 这类常用词表不收的组合，
  `assets/lexicon/phrases.tsv`；词库已并入过短语时重跑加 `--refresh`）。
- `pack dict|lm|glossary|codes`：打 `.qj`（释义表也进容器；`codes` 是唯一带计算的一种，见下）。
- `stroke`：CNS11643 全字庫筆順（`data/cns/`，官方 Properties.zip / MapingTables.zip 解出，gitignore）+ 大陆序覆盖表
  `assets/stroke/prc-rules.tsv` → `data/generated/codes/stroke.tsv`（随包笔画表的源数据：7,991 字、127 KB，
  1 横 2 竖 3 撇 5 折 n 点捺；首笔按《通用规范汉字笔顺规范》GF 0023—2020 全对：门字头 / 戶→户 两条前缀规则 + 66 行整字覆盖，阝第二笔随规范改竖）；`--verify` 双对照——笔画数按一级字每 12 字取 1（291 字）、首笔按一级字 3,500 全量，白名单
  `assets/stroke/residual-whitelist.tsv`（7 字）与首笔 `assets/stroke/residual-first-strokes.tsv`（408 字，对照源几何假阳性；首笔不按几何近似放行，不符的字都要逐字进白名单）各自口径之外一处不符即退出码非 0；两张对照表（笔画数 / 首笔几何类别）由 `mmh-reference` 子命令从 hanzi-writer-data 生成到 `data/mmh/`（Arphic 许可，不进仓库、不随包；缺席时跳过对照并提示）。
  来源、许可与验收记录见 `assets/stroke/README.md`。
- `pack codes`：把 `codes/stroke.tsv` 与词库（缺省 `data/generated/dict.qj`）算成随包原生码表 `data/generated/codes/stroke.qj`
  （键位 1→h 横 / 2→s 竖 / 3→p 撇 / 5→z 折 / 点捺 n→n；单字「前 4 笔 + 末笔」，不足 5 笔按实际取；词组每字首笔，
  二字 2 码、三字 3 码、四字以上 = 前三字首笔 + 末字首笔；词里有一个字不在笔画表里整词跳过）；
  `--stroke` / `--dict` / `--output` 改路径，元数据缺省「笔画」/ `OFL-1.1` / CNS11643 数位发展部署名（可覆盖），数据版本取笔画表日期。
  2026-09-20 实测：词库 92,821 词条 / 91,904 词，有码 91,773、无码跳过 131，码表 91,773 条 / 3.0 MB。

## apps/linux

`qingjian-linux-server` 为独立产品 `0.1.0-dev`，装配本地 Engine、词库、释义、频率学习、个人 n-gram、词汇记录与可选输入日志，
本地整句模型优先加载用户 `~/.local/share/qingjian/models/hanzhang-tongbian/` 或随包 `data/models/hanzhang-tongbian/`，缺失时回退 `models/hanzhang-zhiwei/`；旧用户目录兼容读取。按 `[model] enabled` 在后台加载、停键 80 ms 后重排，节拍与 Windows Server 的 `dispatch/rescore` 相同；不接云服务。`dispatch/session` 交换每个上下文的 EngineSession；真正的能力变化丢弃输入，普通焦点切换隔离保存。
默认面板插件仅转换事件，Shift 模式、候选点击、分页和失焦提交都由 Server 决定。

Unix socket 用共享长度前缀与 Frame（当前公共版本 8，与 `PROTOCOL_VERSION` 同步，Fcitx5 插件里写死在 `miracle.cpp` 的 OpenSession）；插件复用一条连接，每个上下文独立会话。Linux v3 扩展逐会话握手、确认 Sensitive/Password/Disable 后接受按下/释放、焦点和点击事实。
候选回报绑定连接代次、上下文和服务端帧序号，仅当前聚焦页的有效释义进入 `note_displayed`，不把生成帧算作已展示。
`[general] preedit` 使用已有 `both` / `inline` / `window`；没有新增 Linux 自绘配置。详见 [linux-fcitx5.md](linux-fcitx5.md)。

语音识别提示与音量：`VoiceManager::start_practice` 通过 `StartPractice` 与 helper `start.practice_text` 传本轮目标译句。Python 用单一 `modified_beam_search` 模型和每轮弱热词（独立词 1.0、短专名 1.2，上限 96 词/12 短语/4096 字）；`bpe_vocab.py` 仅对固定 SHA256 的 X-ASR tokens 生成分词表，失败回退普通识别。普通 Start、Stop、Cancel 均不携带上一轮提示。PortAudio 实际 RMS/peak 经底噪与对数映射为 0–100，独立音量线程在录音中最多 30Hz 发 generation-scoped `level`、未录音时阻塞休眠；不进入 VoiceUpdate/TSF 协议。Rust `AudioMeter` 核对录音代次/held，300ms 无新样本归零，SAPI 回退使用 SPEI_SR_AUDIO_LEVEL。控制线程约60ms更新UI音量，浮窗五柱只用实测历史，文字不变仍重绘；停止/隐藏/阶段变化清零。跟读浮窗展示最近识别片段，仅在内存，不写日志；Interim文字变化即刷新，即使匹配进度未改变。

双向词卡与低配内存：`qingjian-translate::offline_english_meaning` 查询随包 `glossary-zh.tsv` 的中文义，`offline_chinese_meaning` 查询 `glossary-en.tsv` 的英文义；直接在已排序 TSV 字节范围二分查找，不构建堆索引、启动后台任务或访问网络。英文查询先精确匹配，再尝试保守的复数、所有格、过去式、进行式与常见不规则词形，必须命中真实条目才显示并标明原形，不随意剥离 `-ly/-er`。中文不将逐字义拼成虚构的整词义，未收录的词或人名明确提示无结果，不能承诺全覆盖。`chinese_pinyin` 提供带调拼音，常见多音词使用短语读音，单字多音字列出备选读音。词卡不增加 API 请求或 token。`qingjian-learning::english_ipa_variants` 和近音匹配复用 CMUdict；首次英语跟读只建立 135,166 条 `u32` 行偏移（540,664 字节），不把全表解析为音素对象，命中后才解析单词读音。示范和词语发音复用同一 SAPI 线程并选择所学语言声音，播放词语会保存已匹配前缀、停采集并在结束后留 400ms 尾音再恢复；点击查词不改变跟读阶段。绘制最多 30fps，无额外模型或 GPU 依赖。

CMU 源文本约 3.6 MB，与约 528 KiB 行索引分别计算；解析读音使用最多 128 个不同词、每键最多 96 字节的 FIFO 缓存，严格/拼写容错两种结果共享 `Arc`，也缓存未命中，避免反复分配。IPA 查询保留词内撇号和连字符，`we'll/well` 与 `co-op/coop` 不混淆。SAPI 线程无播放时阻塞等命令，播放时才每 15ms 检查完成。跟读浮窗不沿用普通录音 250ms 短租约，避免输入框复核或 TSF 确认稍慢时误隐藏；显式取消与填入成功仍清理窗口和进度，普通录音保留失联自动隐藏。

中英跟读共用快捷键：`PracticeShortcut` 将 `[shortcut] voice_practice_shortcut` 保存为 `right_alt`、`right_ctrl+right_alt` 或 `ctrl+alt+e` 等字符串，切换 `voice_practice_learn_english` 不重置快捷键。设置录入只在本进程前台短暂安装实体按键钩子，Esc、失焦、取消或 30 秒超时退出。`SetPracticeShortcut` 复用现有命名管道，现有全局触发线程先 `RegisterHotKey` 新键，再原子保存配置并释放旧键；注册或写入失败保留旧键。持久注册仅在语音和跟读开启时存在；录音仍由现有低层钩子识别按住/松开，不新增常驻进程。组合键拆分的 key-up 共用结束活动代次，避免等待尾音期间误取消。系统注册检测不能覆盖应用内部快捷键与第三方低层钩子。

候选窗完整标志：使用用户提供的2078×757透明完整字母图，运行图832×303，按32逻辑像素高等比缩放；窄窗口限制可用宽度。Miracle/GDI两条绘制路径均以标志高度的2/3位于框内、1/3外露定位，最近DPI及宽度尺寸缓存复用。

候选窗标志间距：有页码的竖排直接复用页脚高度，不额外插入空候选行；只有无页码的竖排保留一行防止标志压住末项。两种绘制路径共享RenderData，完整标志及2/3框内定位不变。

Windows 品牌显示为 Vakyu：托盘、两个 EXE 与 TSF 共用 `assets/icon/windows/vakyu.ico`，设置侧栏独立使用 `vakyu-logo.png`（174×64 逻辑像素容器，Uniform 保持完整宽高比）。`create-vakyu-icons.py` 分别接受方形小鸡和完整字标两份原图，避免相互覆盖。设置窗口标题、单实例查找与圆角窗口匹配均为 `Vakyu 设置`；开始菜单、启动项和卸载列表同步更新。安装目录、配置路径、IPC 与 COM/profile GUID 保留兼容值。

Windows 悬浮状态条：`ui/status/logo.rs` 复用完整 `vakyu-logo.png`，解码时去透明留白、按24逻辑像素高等比缩放，左内缩6、半高外露。原图只解码一次，最近DPI位图在线程内缓存。`render_status_with_inset` 与GDI均预留14逻辑像素顶部空间，按钮与分隔线整体下移；外露区域复用既有阴影画布，无新增窗口或刷新定时器。Placement点击同时检查横纵范围，字标与阴影不触发模式切换，拖动仍记录紫色框内容原点。

全局语音交付日志区分宿主 TSF 确认和系统接收 Unicode 事件，附目标 PID/TID 与投递方式；日志无正文，后者不能代替目标应用真实填入的验收。

跟读纯修饰组合：`PracticeShortcut::RIGHT_CTRL_ALT` 持久化为 `right_ctrl+right_alt`，`right_modifiers` 标记实体右 Ctrl。设置 `hotkey/capture/keys.rs` 在全部按键松开后确认，支持继续追加字母、不支持的纯修饰组合及时报告错误；后台状态机接受两种按下及松开顺序，只发一次结束，拒绝左侧键及 AltGr 左 Ctrl 前缀。系统注册分别检查 Ctrl+右 Alt、Alt+右 Ctrl，任一冲突或写配置失败均撤销本轮注册并保留旧键。

## Windows 搜索图标刷新

组件更新和品牌快捷方式更新后，通过 `apps/windows/scripts/refresh-icons.ps1` 对程序、图标、开始菜单入口发送 Shell 更新通知。未提权的外层更新器调用 `refresh-search-icons.ps1`，从已安装的 `vakyu.ico` 重建当前用户 Windows 10 Search `AppIconCache` 下本产品设置与卸载入口的精确图片。不能只删除缓存：搜索索引可能仍引用原路径，导致默认白框。

图片使用 Search 的带透明度 BMPV5 格式，按数字缩放目录生成；先核验临时图片再原子替换，原缓存备份到 `%LOCALAPPDATA%/Qingjian/icon-cache-backups`，相同内容不反复写入。直接解码 ICO 内的 PNG，避开 Windows PowerShell 5/.NET Framework `Icon.ToBitmap` 的 PNG 图标解码噪点。之后调用 `ie4uinit -show`，不会清理其他程序缓存或重启 Explorer。完整安装器仍以原用户执行常规图标刷新。搜索自身持有旧图片时，可单独重启 SearchApp 后重开搜索，不必退出输入法。


## Apple Silicon 全局语音与共享跟读

`crates/qingjian-practice` 保存平台无关的 PracticeSession、英语/中文 Matcher、Phase、PracticeView 和离线词卡；原 Windows 全局跟读改为重新导出共享实现，窗口动作在各宿主适配。现有匹配、词卡、逐词进度、最终识别确认门禁的测试随会话迁入，避免 Mac 重写近似判断。

`apps/voice-service` 以 JSON 行协议接原生宿主，复用 `VoicePracticeTranslator` 和共享会话。输入原话最后结果只翻译一次，lookup 不请求云端；取消、TTS、录音重试使用 generation，翻译使用 round，提交有独立 id 与宿主成功确认。确认后才落盘方向独立 XP 和隐藏。配置只通过白名单编辑，`.env` 只把选中环境键读入内存，不修改进程环境；配置/密钥热加载会重建翻译器。协议与字段详见 `apps/voice-service/PROTOCOL.md`。

Mac 的 `apps/macos/companion` 使用系统 AVAudioEngine/AVSpeechSynthesizer、CGEventTap/Carbon 和 AX；录音与控件接入属于壳，不放匹配逻辑。Swift 采样转16k mono，以 base64 Float32LE 交给 `apps/macos/voice-runtime/external_main.py`，后者复用现有 Python Recognition。录音代次校验在原生、服务、识别线程三处执行；松键排空已排队采样，显式取消可直接废弃。系统安全输入拒绝交付，AX文本/选区变化不覆盖草稿，粘贴仅面向原进程且核对结果。 松键后立即关闭本地录音提示，迟到的 dictation(recording=true) 不覆盖已释放状态；自动跟读采音仍由 practice 事件驱动。AX 应用对象、全局查询与焦点对象在首次属性访问前设 0.25 秒单次通信超时，避免先查询后限时的空窗；不能视为整轮捕获耗时上限。语音日志记录快捷键入口模式、固定拦截原因码、输入目标捕获耗时、长按阈值前释放、轮次启动、松键、音频排空耗时与填入成功布尔值，不含按键序列、应用正文、音频或凭据。尚未建立会话的失败使用同一非激活浮窗显示五秒，独立于 model.visible；后续会话开始或隐藏会取消提示计时器，代次检查防止旧提示关闭新会话。

`bundle.sh` 默认 Apple Silicon 完整语音包；原有 Intel 维护构建显式 `VAKYU_WITH_VOICE=0`。Python/ONNX模型/轮子由 `sources.json` SHA256固定，嵌套签名从内到外，首次权限从语音设置申请。macOS TCC 会将嵌套宿主的麦克风访问归属外层 IMK app，因此两层 Info.plist 均声明 `NSMicrophoneUsageDescription`，源码与成品检查都会验证。单一登录项让语音不依赖目标程序重新加载IMK；保留历史目录与bundle ID。主输入法与语音设置通过固定分布式通知互开，不传正文或密钥。

开发版 ad-hoc 签名更新会改变 cdhash；即使安装路径与 bundle ID 不变，旧 TCC 授权的 code requirement 也可能不再匹配当前程序。辅助功能、输入监控及归属于外层 app 的麦克风授权都需要以当前版本重新确认，不能把系统设置中的同名旧开关视为有效授权。系统授权只由用户触发，自动刷新不调用权限申请接口，也不修改 TCC 数据库。

按键监听以 `CGEvent.flags` 的通用修饰位及公开 IOKit 左右位作为当前事件快照，`ModifierKeyState` 维护左右状态；普通 keyDown/keyUp 按序维护；每个新事件前只用会话 keyState 清理旧普通键，保留当前键与修饰键事件的优先级，同时补发已丢失的释放转换。Fn63 仅由自身 flagsChanged 更新，不能把 F 键/方向键的 secondaryFn 标志当作 Fn 实体按下。关闭设置时的 idle endRecording 不再使路由器进入阻塞；结束真实录入且所有键已松开时立即解除阻塞，残留键与重连期间仍保留全部松开保护。全局会话 `keyState` 也用于重连初始化与缺侧别时的回退，不能覆盖已收到的松键快照，避免抑制事件后的陈旧状态令录音一直不结束。

语音权限与监听状态每 5 秒进行一次只读检查，手动“刷新权限”即时执行同一检查。监听健康状态核对事件端口有效性和 tap 是否启用；权限允许但端口失效时销毁并重建，权限不足时展示对应状态。“重新启动语音服务”也重建按键监听器，不只重启识别服务，保留原界面与快捷键配置。此处描述恢复机制，不代表开发版更新后的权限及真实录音已完成本机验收。

完整语音构建要求产品词库、语言模型和两类含章模型存在，缺少时先运行 `tools/release/data-fetch.sh`。版本号解析兼容 Windows CRLF；`check_port.py --bundle` 检查完整数据及全部运行时 Mach-O 的 arm64 架构和签名。`runtime_manifest.py` 在内层 Mach-O 签名后更新 `SHA256SUMS.json`，再封存外层 app；验收同时核对清单，避免签名后校验值过期。独立 Python 不带标准库 test/venv 开发模块。

Mac设置新增“语音跟读”页和菜单快捷开关，复用现有配置名；新用户默认左Option普通语音、右Control＋右Option跟读。原生press携带mode，服务按轮锁定，普通语音不被跟读开关送去翻译；跟读键仅在跟读开启时生效。候选渲染使用共享紫色Palette，父视图画完整渐变和外露字标，PNG只加载一次。完整产品说明、真机验收及差异边界见 `macos-vakyu-port.md`。Windows测试不能视为Mac编译/麦克风/目标软件兼容验收。

Mac 跟读键支持右 Command＋右 Option，配置规范值为 `right_win+right_alt`，兼容 `right_cmd` / `right_command` 别名。Swift `ShortcutRecorder` 在纯修饰键全松开后确认，允许继续追加字母；非法组合不退化成单 Option。`VoiceKeyRouter` 区分左右键、完整松开后才接受新轮次，共享 `PracticeShortcut` 和服务持久化保留侧键信息。此组合在 Windows 暂不支持，导入时拒绝注册而不降级成单键。

macOS 26.4.1 的 imklaunchagent 拒绝本包旧 `<bundle id>_Connection`，连接名改为 `<bundle id>.Connection`，plist、BundleInfo fallback 和契约测试一致。NSApplication 在 host 创建候选 NSPanel 前初始化。

IMK 控制器的定向 debug 诊断用进程内递增编号关联初始化、激活、停用与事件分发。初始化记录客户 bundle、client 是否存在及父类初始化是否成功；事件仅记录 event/client 是否存在、事件类型与处理结果，不记录键码、字符或正文。新增的客户信息查询仅在 debug 启用时执行，并独立捕获 Rust panic，不影响原有回调处理。会话诊断使用独立的 `vakyu_imk_session` 日志目标，排查时仅需 `RUST_LOG=info,vakyu_imk_session=debug`；不再通过开启控制器模块的 debug 后逐项排除子模块来避免正文日志，新增子模块也不会意外继承诊断级别。默认 info 不新增逐键输出。系统 CharacterPalette 搜索框能输入不等于微信聊天会话已激活，诊断必须依据客户 bundle 与同一会话编号判断。

Mac 菜单图由 `swift apps/macos/scripts/menu_icon.swift assets/icon/windows/vakyu-icon.png <状态项.png> <彩色输入源.pdf> <单色输入源Template.pdf>` 生成。自有状态项仍用彩色 `vakyu-menu.png`：18×18pt / 36×36px / 144dpi，主体最长边16pt并居中。系统菜单顶层与两个模式菜单键改用 `vakyu-input-sourceTemplate.pdf`，透明底包含原图的外轮廓、眉眼与喙，可被系统着成黑或白而不丢失五官；输入源面板仍用彩色 `vakyu-input-source-baseline.pdf`。两份PDF均为16pt画布、14pt主体、水平居中并上移1pt。模板由原图alpha边缘（半径3.5%）与深色/高亮五官生成，不改应用图、语音状态项或设置字标。2026-10-10 用户照片证明实心白方块更符合整块原图被系统单色渲染，不能继续称为通用应用占位图；离屏深浅背景可见五官，实际菜单仍需目测验证。

Mac 浮窗从首帧按 `entryMode` 和学习方向标示普通语音/英语跟读/中文跟读。`OverlayLayout` 统一半尺寸：录制原话/普通语音360×142.5pt、跟读360×255pt；HostingView取消理想尺寸撑大，长内容滚动、操作栏固定，词卡与超长词不横向越界。启动前错误提示因不可交互，按完整正文扩高。

Mac 文本交付的 `TextDeliveryAccess` 隔离系统AX/剪贴板与状态机，`DeliveryVerification` 用原全文和UTF-16选区构造精确预期；AX返回success不再等于完成。有限遍历祖先中的AXWebArea使网页控件优先粘贴。AX仅尝试一个setter，完全未变经过150ms二次确认及同一焦点/原草稿验证才允许一次回退；部分改动/缺少读回均锁定不确定状态，阻止重复注入。新增固定路径/确认布尔日志，不含正文。粘贴仅发送一次，50ms 首次复核同一焦点/窗口/安全状态并精确读回；完整结果已到达即可确认，尚未到达则保留发送后350ms最终检查。早期上下文失效直接锁为不确定，不因焦点稍后恢复而补确认。剪贴板恢复独立延至发送后至少750ms，并保留changeCount防覆盖；epoch阻止重置后的迟到确认。

`FocusResolution` 固定开始捕获时的前台 PID，仅 system-wide 焦点缺失时向该应用查询焦点，捕获与回填前验证复用同一解析器；安全控件、进程不符或已返回的不支持控件不能被回退掩盖。无焦点、安全输入与安全控件分别提示并记录固定原因码，不记录正文。微信 4.1.13 当前可能不提供聊天控件，其资源包含“读屏优化模式”文案，但入口与是否能解决尚待核实；没有关闭安全输入保护。

Mac 安装脚本注册输入源后通过LaunchServices显式启动IMK：升级时输入源ID未变化，系统可能不重启已退出的普通输入法，仅启动语音会造成“语音可用、键盘输入无效”。启动使用已安装绝对路径、后台方式，pkg在当前桌面用户身份执行。

可读目标仍要求完整原文及有效 UTF-16 选区，沿用精确核对与草稿保护。`[shortcut].voice_paste_fallback` 默认 true，通过 Config 事件的 `paste_fallback` 同步给 Swift；关闭后 `no_focus` / `unverifiable_editor` / `unsupported_editor` 仍拒绝。开启时仅缺失焦点、未提供值/选区或不透明容器可进入受限粘贴：`FrontWindow` 只取前台 PID 的可见 layer-0 CGWindowID，不取标题/图像；`FocusResolution.permitsPasteFallback` 在捕获、采音及发送前复核同一 PID/窗口和安全状态。已知焦点和 AX 窗口也须保持一致，密码、外进程、非编辑控件及权限错误不能兜底。TextDelivery 使用既有 eventMarker 与剪贴板恢复路径，只发一次 Cmd+V，标记 unverified 并保留结果，不发 Enter、不读回不透明控件、不补发；录音中关闭开关也禁止后续兜底粘贴。

macOS 授权引导由 `PermissionSetupView` 提供：缺权限时排在设置页顶部；麦克风与辅助功能为基本授权，活跃 `.defaultTap` 按 AX 授权尝试创建，以实际 `keyboard.available` 决定就绪，不再用 `CGPreflightListenEventAccess` 的被动监听预检硬性阻断。系统未建立钩子时才显示输入监控兼容入口，撤销 AX 时仍停止钩子。AX 请求后直接打开 Accessibility 页，当前 Bundle URL 供拖入权限列表、访达定位与用户主动复制。`--release-pkg` 在构建前要求 Developer ID Application/Installer 身份及公证 profile，实际签名与公证步骤继续负责校验证书和凭据。

本机更新维护脚本清理同 bundle ID 的构建副本后，必须最后重新完成安装位置的 TIS 登记；只做 LaunchServices 登记可能留下白色占位图和失效输入源。2026-10-10 已复现启用列表仅余 ABC，重新登记后 Vakyu.Hans 恢复，用户确认中文恢复。不得把这种登记错误归因于语音权限。

正式构建链路使用 `register_input_source.sh` 从最终路径登记，再由随包编译的 `verify_input_source` 两次独立回读父项、模式、启用/可选状态、图标安装路径与仍存在的重复包；持续失败返回非零。`install_local.sh` 目录交换失败会恢复原包并重新登记；pkg 的 preinstall 保存原包，postinstall 核验失败尝试恢复。构建 app 放在 `target/macos-bundle.noindex/`，pkg 临时 payload 封包后删除。20 项隔离故障注入覆盖登记重试与两种安装回滚，实际安装仍需真机验证；细节见 `macos-install-registration.md`。

仅取消构建副本的登记也不够：Spotlight 会重新发现仍在磁盘上的嵌套 `.app`。本机维护脚本现先逐项核验归档，再移除旧版和构建目录里的可运行副本；保留 `.pkg` 和可恢复归档。异常路径也会独立尝试恢复安装版的 LaunchServices 与 TIS。清理后用 `NSWorkspace.urlsForApplications` 核对只有安装位置；不能仅凭注销命令的退出码认定系统已稳定。

没有 `.git` 的源码归档打包会递增 `target/macos-local-build-number`，外层输入法与语音应用使用相同构建号，避免每次都以构建 1 覆盖而沿用旧的应用元数据。权限页的应用拖入卡片直接使用随包的小鸡图案，不依赖 LaunchServices 图标缓存；拖入数据仍是当前运行应用的完整 URL。

2026-10-10 构建 3 安装后，本机实际日志确认 `mic=true ax=true input=false tap=true ready=true`，左 Option 和右侧跟读组合都产生了对应入口事件。本机因此无需单独恢复输入监控授权；这不代表其他系统版本一定相同，钩子创建失败时仍保留输入监控引导。


2026-10-11 macOS 语音交付失败使用 `CompletionPresentation` 的 3 秒代次计时器收起面板，保留逻辑会话、原目标及 TextDelivery 未确认写入门禁，不把失败当成功，也不重复投递。菜单可恢复失败结果并主动复制最后 commit 原文（仅内存）。下一次快捷键先发送 cancel，等旧 Hide 回包且实体按键仍按住才开始新一轮；取消期间屏蔽旧 error。收起面板不吞宿主 Esc，重试被拒绝时保留失败操作。成功仍由 commit_result(true) → Hide 关闭。协调器替身回归 73 断言通过，原生 AX/录音兼容性另行验收。

2026-10-12 焦点诊断增加 app bundle ID/PID、system/app 来源与 AX 数字错误码，不记录正文。AXIsProcessTrusted 为 true 而焦点读取返回 apiDisabled/cannotComplete 时，PermissionState 标记可能失效并提供系统设置入口；成功读取清除提示，错误不能转成受限粘贴。签名脚本优先显式固定身份，再选唯一开发/本机证书，无证书时才临时签名并警告；不自动修改证书信任或 TCC。

### macOS 构建 9 与本机翻译合并

本机翻译变更以 fd15868 构建 9 为基础，仅合入系统 Translation 框架、宿主 translated 回执、浮窗与译文投递设置；保留 Delivery、权限、键盘和受限粘贴处理。Shared Rust 服务可在 Windows 测试，Swift Translation 代码需要 Xcode 16/macOS 15 SDK 及 Mac 原生构建和真机验证，不据此生成安装包通过结论。
