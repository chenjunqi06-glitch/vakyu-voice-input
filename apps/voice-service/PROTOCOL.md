# Vakyu 原生语音协议

服务参数：`--config PATH --data PATH --python PATH --runtime PATH --models PATH`。
所有进程使用 UTF-8，每行一个 JSON。stdout 只输出协议；日志写 stderr，不打印配置/密钥/录音正文。

## 原生宿主 → 服务

顶层 `command`，snake_case：

- `press`：`mode:"dictation"|"practice"`。宿主已锁定可输入、非密码的目标，并在快捷键保持配置的 hold_ms 后发送。单轮只启动一次，模式锁定到取消/提交成功。dictation 即使跟读总开关开启也只本机识别原话、不翻译；practice 必须总开关已开启，关闭时不可回退普通口述。为兼容旧宿主，无 mode 时按原配置总开关选择。新原生宿主必须明确传 mode，并在跟读开关关闭时不显示浮窗、不发送 practice press。
- `release`：结束原话口述；跟读期间不影响进度。
- `cancel`：Esc 显式取消；失焦不能当作取消，提交时检查原目标。
- `audio`：`generation:u64, sample_rate:u32, pcm:string`。pcm 是 **base64 编码的小端 float32 单声道**，非 JSON 数字数组；建议 16 kHz、每包 20–100 ms。最大 256 KiB base64/包。旧代次丢弃。
- `capture_failed`：`generation:u64,message:string`。原生开麦权限失败或当前目标不安全时回报，保留练习窗供重试，不填入。
- `speech_finished`：`id:u64,ok:bool`，TTS 实际结束后确认；即使失败也应回报。
- `action`：`action:"retry"|"skip"|"accept"|"lookup"|"close_card"|"speak_word"`，lookup 另带 `index:usize`（显示 words 的索引）。
- `commit_result`：`id:u64,ok:bool,message?:string`。只填入原目标，**不发送**；宿主检查目标身份、可编辑、非密码、安全输入。成功才确认；相同 id 必须幂等。同一轮提交失败或确认超时，显式 action retry 始终复用相同 id、原话，不自动重发；普通口述也支持该重试。失败保留窗口，已成功填入的 id 返回成功即可，不能再插一次。
- `set`：`key:string,value:JSON scalar`。只支持 shortcut 节白名单：voice_input、voice_english_practice、voice_practice_learn_english、voice_practice_send_translation、voice_practice_demo、voice_practice_shortcut、voice_paste_fallback、voice_hold_ms、voice_practice_similarity、voice_practice_name_similarity。快捷键必须宿主注册成功才保存；冲突不发送 set。
- `translated`：`id:u64,ok:bool,text:string`。回应 `translate_request`；ok 时 text 是系统翻译的译文（服务按 Core 规则校验整理），否则是失败原因，本轮显示该原因且不填入。
- `reload`：立即重读配置；服务另外每秒检测 mtime。
- `shutdown`：释放 ASR 子进程并退出。

快捷键配置沿用跨平台名称：普通口述 dictation_key 默认 left_alt=左 Option；跟读 shortcut 默认 right_ctrl+right_alt=右 Control+右 Option。right_alt=右 Option，ctrl=Control，alt=Option，win=Command。语言值只有 en / zh。两个快捷键入口独立，普通口述的本机识别路由不受学习方向/跟读开关影响。

## 服务 → 原生宿主

顶层 `event`，snake_case：

- `ready`：ASR 模型已就绪。
- `config`：`voice_input,practice,learn_english,send_translation,demo,paste_fallback` bool；`shortcut,dictation_key` string；`hold_ms,similarity,name_similarity` integer。dictation_key 来自 shortcut.voice_key（alt/left_alt/right_alt）；shortcut 来自 voice_practice_shortcut。paste_fallback 对应 `[shortcut].voice_paste_fallback`，缺省 true；仅白名单，不含 API 凭据。
- `capture`：`active:bool,generation:u64,language:"en"|"zh",practice_text?:string`。active=true 开麦，false 立即关麦；宿主仅向相同 generation 发送音频。
- `dictation`：`text:string,recording:bool,level:float`（0–1）。预览原话并更新真实音量动画。
- `practice`：`view:PracticeView`，完整快照见下。
- `speak`：`id:u64,text:string,language:"en"|"zh",rate:i32`。rate 为慢速级别 -2/-1/0，原生可映射 AVSpeechUtterance 速度；按语言选系统声音。播放期间服务关闭 capture，回报 speech_finished 后约 400 ms 重开，避免把示范当作用户。
- `cancel_speech`：立刻停止 TTS，随后返回的旧 speech_finished 无效。
- `commit`：`id:u64,text:string`。跟读完成才产生；内容默认是原话，`send_translation` 开启时是刚读完的译文；收到成功确认后才累积 XP 和 hide。等待结果时暂不受理 retry，避免同时插入；失败回执或 15 秒确认超时后允许显式重试。
- `hide`：清除本轮浮窗。
- `error`：`message:string`，可读提示，不能关闭未完成的跟读。
- `translate_request`：`id:u64,text:string,target:"en"|"zh"`。跟读不调用任何网络接口，请宿主用系统翻译（macOS Translation 框架，本机）翻译本轮原话，以 `translated` 回报；id 即轮次，过期回报丢弃。
- `setting_result`：`key:string,ok:bool,message:string`；失败保留原配置，用 key 关联原生设置回执。

PracticeView 字段：`learn_english:bool, pronunciations:[string], original:string, words:[string], matched_words:usize, level_label:string, level_progress:string, recognized:string, feedback:string, actions_enabled:bool, recording:bool, word_card:WordCard|null, status:string`。
WordCard：`word:string,meaning:string|null,pronunciation:string|null,hint:string|null`。中文 pronunciation 为带调拼音，英文为 IPA。pronunciations 与中文 words 一一对应，英文时为空。

只允许 Esc 或成功填入后关闭练习窗。点击词卡不改变计时/进度，播放发音暂停收音但保留已读前缀。学习英语开：说中文、读英文、填中文；关：说英文、读中文、填英文。send_translation 开启时改为填入译文：说中文、读英文、填英文（学中文方向：说英文、读中文、填中文）。

Mac 宿主受限粘贴发送一次 Cmd+V 后无法读回时，回报 `commit_result(ok:false)` 并显示“已粘贴，无法核对”。同轮所有后续提交均拒绝再次注入；不累计已确认成功经验，保留原话与复制入口。终态浮窗按现有计时器收起，菜单可恢复结果。

## 服务 ↔ external_main.py

启动 `python <runtime>/external_main.py --models <models>`。
stdin 顶层 `command`：`start{generation,language,practice_text?}`、`audio{generation,sample_rate,pcm}`（同上 base64 float32）、`stop{generation}`、`cancel{generation}`、`shutdown`。
stdout 顶层 `event`：`ready`、`started{generation}`、`interim{generation,text}`、`final{generation,text}`、`level{generation,level}`、`error{generation?:u64,message}`。
cancel 后旧代次所有事件丢弃；录音和跟读没有总时长截止，只有模型准备/停止收尾的 watchdog。子进程失败时回报错误，不能假装完成或释放原文。

## 数据与费用

配置使用现有 TOML Config API，只更新指定键。缺省配置首次创建时使用右 Control + 右 Option；已有配置不覆盖。每秒检测 TOML 的修改时间。服务不使用 API 凭据，也不发网络请求。方向或总开关变化停止旧录音，保留窗口及原话，明确提示按 Esc 再开始。宿主翻译失败时本轮保留原话并允许点 Retry。相似度、延时、快捷键、示范设置变更不打断当前进度。失败阶段 words 为空且 actions_enabled 为 true 时 Retry 表示重新翻译，Skip/Accept 应禁用。

模型已就绪且没有活动轮次时服务阻塞等待命令最多 1 秒；press 会立即唤醒，不额外增加录音延迟。活动轮次和模型准备阶段使用 20 ms 轮询。

学习经验分别保存在 data 下 english-practice-progress.json / chinese-practice-progress.json。不保存音频或句子。普通口述和跟读全程本机：跟读只请宿主用系统翻译原话最终文本，不发任何网络请求，不调用云联想或云查词。
