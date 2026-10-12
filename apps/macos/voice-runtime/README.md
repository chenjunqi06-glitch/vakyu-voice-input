# Apple 芯片本地语音组件

macOS 13 及以上，Apple Silicon 原生 CPU 推理。模型与 Windows 当前版本一致：X-ASR 480 ms zh/en int8，约 170 MB 模型权重；只保留一份模型实例，两个 CPU 推理线程，不使用本地显卡。原生 AVAudioEngine 采音，Python 不申请麦克风权限，也不依赖 sounddevice。

`external_main.py --models <目录>` 用 UTF-8 JSON 行接收 Float32LE 单声道 base64 PCM。具体协议见 `apps/voice-service/PROTOCOL.md`。单包最多 256 KiB、最多一秒音频，推荐 20–100 ms；录音没有总时长上限。识别慢于采集时，推理线程把队列中紧随其后、同一代次同一采样率的音频块合并后一次送入模型（单次最多 1 秒），不丢样本、不越过停止或取消；仍跟不上导致队列满时明确报错，不会静默丢掉音频。录音和识别结果不保存到磁盘。

Windows 与 macOS 复用 `recognition.py`、`practice_context.py`、`bpe_vocab.py`、`audio_meter.py`，打包时从单一实现复制。取消代次立即失效，旧识别不能填入下一轮。松键停止收音，依次处理已收到的音频，再返回最终文字。

下载仅发生在构建机。`sources.json` 固定 CPython 独立分发、sherpa-onnx 1.13.8、NumPy 2.2.6 和模型的 URL 与 SHA256。模型来自 [sherpa-onnx 官方发布](https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models)，上游 [X-ASR](https://github.com/Gilgamesh-J/X-ASR)，Apache-2.0；Python、依赖包的许可证保留在分发目录中。最终用户安装后无需另行下载 Python 或模型。

构建前运行 `python3 apps/macos/scripts/prepare_voice_runtime.py` 可单独准备。脚本只接受 Apple 芯片 Mac，会加载一次模型检查可用并预生成热词词表。不会读取用户 config、API 密钥或个人语料。

在装有 NumPy 的 Python 上运行 `python apps/macos/voice-runtime/tests.py`，覆盖 PCM 验证、取消、停止排空、长录音、过载及音量。真实识别仍使用共享实现已有的中文、英文、静音回放验收。
