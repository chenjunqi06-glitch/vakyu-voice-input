# Windows 本地语音组件

常驻进程加载 X-ASR 中文/英文流式模型，按住 Alt 时采集 Windows 默认输入设备，松键后停止采集并排空已有音频。模型只做语音转写，不调用聊天接口，不保存麦克风录音，不上传音频。

完整安装包部署到 Server 可执行文件旁的 `voice-runtime/{python,app,models,licenses}`，不依赖当前用户名和构建机器的 Python。先检查此目录，再兼容 `%LOCALAPPDATA%/Qingjian/voice-runtime`；如果两处均不完整，保留损坏目录并明确报修复安装，避免静默切换到其他识别模型。预检包括 Python、应用脚本、主要原生库和非空模型文件。

入口为 `python/pythonw.exe -I -B -u -X utf8 app/main.py --models models`，使用官方 3.12.10 x64 embeddable package；GUI 解释器不创建控制台，仍通过显式 stdin/stdout 管道通信，并保留 `CREATE_NO_WINDOW`。`-I` 隔离系统 Python 环境、`-B` 禁止在只读程序目录写入 `__pycache__`。`requirements.txt` 中依赖安装到 `python/Lib/site-packages`；`python312._pth` 包含该目录和 `../app`。发行包仅从固定依赖、公开模型及仓库应用源码生成，不能复制用户配置、录音、日志或个人学习资料。

模型来源：

- [X-ASR 项目](https://github.com/Gilgamesh-J/X-ASR)（Apache-2.0）。
- [sherpa-onnx 官方导出包](https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05.tar.bz2)，SHA256 `fa5f63d618e5a01526e275a358bb7772e403f84808a4769fba52cffd8160bf74`。
- 解压后的 `encoder.int8.onnx`、`decoder.onnx`、`joiner.int8.onnx`、`tokens.txt` 放到 `models/streaming`。发布包附上来源、许可证与文件 SHA256 清单。

标准输入是 JSONL：`{"command":"start|stop|cancel|shutdown","generation":1}`。标准输出是 JSONL，`event` 为 `ready`、`started`、`interim`、`final`、`error`、`stopped`、`cancelled`；除 ready 和启动错误外都携带 generation，interim/final 的 text 为整个会话累计文字。

`start` 可携带 `language:"zh"|"en"`。英语学习是中文原话→英文跟读，中文学习是英文原话→中文跟读；普通语音默认 `zh`，但它不把中英模型限制为只识别中文。语言与提示随 generation 隔离，停止命令不带提示。两种语言共用已安装的中英模型与两条 CPU 推理线程，切换方向不下载模型、不增加后台进程，也不调用显卡。模型输出按原样累计，英语口述不会套中文后处理，中文跟读不会通过英文过滤规则丢字；重复口述和重读不会在识别层被删掉。

普通口述的 `start` 可携带 `dictation_vocabulary` 英文词汇字符串，例如 `"deadline Codex"`。`dictation_context.py` 为声学解码提供弱词汇提示，降低中文句子夹英文时丢词或选中音近汉字的概率，不把已有中文结果替换成英文。父进程提供常用软件/工作词汇，并在开启个人学习时附加已提交的常用英文词；本进程不读取输入框、剪贴板或个人文件，不访问网络。仅保留长 2–48 字母的英文词（允许词内撇号），按小写去重，排除单字母；输入最多 8192 字且丢弃截断半词，最多 96 个独立词、4096 字热词，每词及大小写变体 boost 固定为 0.8。若大小写变体超过预算，先保留每个词的原始形式，再用余量补变体，避免基础词变体挤掉个人专名。提示只属于本轮 stream，不累加重复词的权重；若存在 `practice_text` 则完全忽略口述词表，保持原跟读提示。旧消息不带该字段、空/无效词表或模型不支持提示时，均使用无提示识别。此方式不增加模型、副本推理或 API 消耗，也不能保证所有口音及未知专名都识别正确。

跟读的 `start` 可额外携带 `practice_text`；普通口述不携带。当前已校验的 X-ASR 词表可由 `bpe_vocab.py` 生成不含用户内容的 `bpe.vocab`，发行包预生成后随模型安装，运行仅复用。若旧安装缺表且模型目录只读，写入 `%LOCALAPPDATA%/Qingjian/voice-cache/<tokens SHA256>/`，不要求管理员权限；macOS 使用本用户 `~/Library/Caches/Vakyu/voice/`。单一 `modified_beam_search` 模型为每个 stream 独立启用提示：独立英文词 boost 1.0、短专名 1.2，至多 96 个独立词、12 个短语、4096 字热词。提示只在当前轮次内存中存在，不强制解码整个目标句。未知词表、缓存不可写或初始化失败回退无提示识别。

中文跟读的热词是单字 boost 0.6 与相邻双字 1.0，可保留句中英文姓名提示，共享上述长度上限；不把长目标句作为一个热词。旧消息省略 language 时，有跟读文本按英语处理、无文本按中文处理，兼容旧组件。

音量另外通过 `{"event":"level","generation":1,"level":0}` 输出，level 为 0–100 整数。PortAudio 回调只根据本块实际 RMS/peak 计算音量，独立线程每 75 毫秒发送最新块，不等待模型解码；新块的音量即使相同也续约，无新块不重复。停止/取消清空待发采样，父进程按代次和活动状态丢弃迟到值，超过 300 毫秒无新值归零。音量不进入 TSF 协议或识别文字，不记录音频。

控制线程与推理线程分离。stop/cancel 先撤销采集意图，关设备后才返回 stopped/cancelled。设备初始化或 start 尚未返回时不并发关闭 PortAudio 句柄；回调检查撤销意图并中止，设备操作返回后安全关闭并确认。Rust 父进程负责设备调用卡住时的超时结束进程。stdin EOF 关闭设备并结束；按住期间不按录音总时长自动结束。

停顿与每段最多 25 秒只触发模型内部断句；已完成段落保留在本轮累计文本中，继续接收后面的声音，只有 stop 才产生 final。短音频块未产生新识别文本时复用已有累计结果，避免长口述及停顿期间重复拼接全文。

模型需要预热，父进程应在 Server 启动时创建 helper。每次录音保留实际输入采样率，松键补静音沿用该采样率，避免 sherpa-onnx 原生重采样器因中途改率而退出。

Windows 下必须先导入 NumPy / sherpa-onnx 原生依赖，再创建阻塞读取标准输入的控制线程。先读匿名管道再首次导入 NumPy 会在原生模块初始化处卡住，离线 `--wave` 分支无法覆盖这个差异。模型构造放在控制线程启动后，保留初始化期间的松键与取消处理；发布验证要额外运行保持 stdin 开启的 Ready → Shutdown 握手。

分发包仅使用标准 Windows PortAudio 后端，不包含 ASIO 变体。录音模块清除子进程继承的 `SD_ENABLE_ASIO`，避免其他音频软件的环境变量使新安装无法加载音频库；不会修改系统环境设置。

离线验证（不开麦）：`python/python.exe app/main.py --wave test.wav --models models`；默认设备只读检查：`python/python.exe app/main.py --devices`；控制竞态回归：`python/python.exe app/tests.py -v`，包括 `dictation_tests.py` 的混说提示边界、普通口述/跟读隔离、取消隔离和失败回退。真实准确率应另用固定 WAV 做有/无提示对比，并保留纯中文、纯英文及静音回归；合成语音的通过不代表真实用户口音已验收。
