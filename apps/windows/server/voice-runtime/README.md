# Windows 本地语音组件

常驻进程加载 X-ASR 中文/英文流式模型，按住 Alt 时采集 Windows 默认输入设备，松键后停止采集并排空已有音频。模型只做语音转写，不调用聊天接口，不保存麦克风录音，不上传音频。

部署布局是 `%LOCALAPPDATA%/Qingjian/voice-runtime/{python,app,models,licenses}`。入口为 `python/python.exe -u app/main.py --models models`，Python 使用官方 3.12.10 x64 embeddable package；运行时不依赖构建机器的 Python。`requirements.txt` 中依赖安装到 `python/Lib/site-packages`；`python312._pth` 包含该目录和 `../app`。

模型来源：

- [X-ASR 项目](https://github.com/Gilgamesh-J/X-ASR)（Apache-2.0）。
- [sherpa-onnx 官方导出包](https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05.tar.bz2)，SHA256 `fa5f63d618e5a01526e275a358bb7772e403f84808a4769fba52cffd8160bf74`。
- 解压后的 `encoder.int8.onnx`、`decoder.onnx`、`joiner.int8.onnx`、`tokens.txt` 放到 `models/streaming`。发布包附上来源、许可证与文件 SHA256 清单。

标准输入是 JSONL：`{"command":"start|stop|cancel|shutdown","generation":1}`。标准输出是 JSONL，`event` 为 `ready`、`started`、`interim`、`final`、`error`、`stopped`、`cancelled`；除 ready 和启动错误外都携带 generation，interim/final 的 text 为整个会话累计文字。

`start` 可携带 `language:"zh"|"en"`。英语学习是中文原话→英文跟读，中文学习是英文原话→中文跟读；普通语音保持中文默认值。语言与提示随 generation 隔离，停止命令不带提示。两种语言共用已安装的中英模型与两条 CPU 推理线程，切换方向不下载模型、不增加后台进程，也不调用显卡。模型输出按原样累计，英语口述不会套中文后处理，中文跟读不会通过英文过滤规则丢字；重复口述和重读不会在识别层被删掉。

跟读的 `start` 可额外携带 `practice_text`；普通口述不携带。当前已校验的 X-ASR 词表可由 `bpe_vocab.py` 生成不含用户内容的 `bpe.vocab`，单一 `modified_beam_search` 模型为每个 stream 独立启用提示：独立英文词 boost 1.0、短专名 1.2，至多 96 个独立词、12 个短语、4096 字热词。提示只在当前轮次内存中存在，不强制解码整个目标句。未知词表、不可写目录或初始化失败回退无提示识别。

中文跟读的热词是单字 boost 0.6 与相邻双字 1.0，可保留句中英文姓名提示，共享上述长度上限；不把长目标句作为一个热词。旧消息省略 language 时，有跟读文本按英语处理、无文本按中文处理，兼容旧组件。

音量另外通过 `{"event":"level","generation":1,"level":0}` 输出，level 为 0–100 整数。PortAudio 回调只根据本块实际 RMS/peak 计算音量，独立线程每 75 毫秒发送最新块，不等待模型解码；新块的音量即使相同也续约，无新块不重复。停止/取消清空待发采样，父进程按代次和活动状态丢弃迟到值，超过 300 毫秒无新值归零。音量不进入 TSF 协议或识别文字，不记录音频。

控制线程与推理线程分离。stop/cancel 先撤销采集意图，关设备后才返回 stopped/cancelled。设备初始化或 start 尚未返回时不并发关闭 PortAudio 句柄；回调检查撤销意图并中止，设备操作返回后安全关闭并确认。Rust 父进程负责设备调用卡住时的超时结束进程。stdin EOF 关闭设备并结束；按住期间不按录音总时长自动结束。

停顿与每段最多 25 秒只触发模型内部断句；已完成段落保留在本轮累计文本中，继续接收后面的声音，只有 stop 才产生 final。短音频块未产生新识别文本时复用已有累计结果，避免长口述及停顿期间重复拼接全文。

模型需要预热，父进程应在 Server 启动时创建 helper。每次录音保留实际输入采样率，松键补静音沿用该采样率，避免 sherpa-onnx 原生重采样器因中途改率而退出。

Windows 下必须先导入 NumPy / sherpa-onnx 原生依赖，再创建阻塞读取标准输入的控制线程。先读匿名管道再首次导入 NumPy 会在原生模块初始化处卡住，离线 `--wave` 分支无法覆盖这个差异。模型构造放在控制线程启动后，保留初始化期间的松键与取消处理；发布验证要额外运行保持 stdin 开启的 Ready → Shutdown 握手。

离线验证（不开麦）：`python/python.exe app/main.py --wave test.wav --models models`；默认设备只读检查：`python/python.exe app/main.py --devices`；控制竞态回归：`python/python.exe app/tests.py -v`。
