//! 发行包随程序安装语音组件；完整的用户目录组件保留为旧安装兼容。

use std::path::{Path, PathBuf};

const REQUIRED_FILES: &[&str] = &[
    "python/python.exe",
    "python/pythonw.exe",
    "python/python312.dll",
    "python/python312.zip",
    "python/python312._pth",
    "python/Lib/site-packages/numpy/__init__.py",
    "python/Lib/site-packages/sherpa_onnx/__init__.py",
    "python/Lib/site-packages/sherpa_onnx/lib/onnxruntime.dll",
    "python/Lib/site-packages/sherpa_onnx/lib/_sherpa_onnx.cp312-win_amd64.pyd",
    "python/Lib/site-packages/sounddevice.py",
    "python/Lib/site-packages/_sounddevice_data/portaudio-binaries/libportaudio64bit.dll",
    "python/Lib/site-packages/_cffi_backend.cp312-win_amd64.pyd",
    "app/main.py",
    "app/worker.py",
    "app/capture.py",
    "app/recognition.py",
    "app/audio_meter.py",
    "app/bpe_vocab.py",
    "app/practice_context.py",
    "models/streaming/tokens.txt",
];

pub(super) fn select_root(executable: Option<&Path>, local_data: Option<&Path>) -> Option<PathBuf> {
    let installed = executable
        .and_then(Path::parent)
        .map(|path| path.join("voice-runtime"));
    let legacy = local_data.map(|path| path.join("Qingjian/voice-runtime"));
    let candidates = [installed, legacy];
    candidates
        .iter()
        .flatten()
        .find(|root| is_complete(root))
        .cloned()
        .or_else(|| {
            // 存在但损坏的组件交给启动预检报错，不能悄悄退到精度较低的 SAPI。
            candidates.into_iter().flatten().find(|root| root.exists())
        })
}

pub(super) fn is_complete(root: &Path) -> bool {
    REQUIRED_FILES
        .iter()
        .all(|file| nonempty_file(&root.join(file)))
        && ["encoder", "decoder", "joiner"].iter().all(|name| {
            let models = root.join("models/streaming");
            nonempty_file(&models.join(format!("{name}.int8.onnx")))
                || nonempty_file(&models.join(format!("{name}.onnx")))
        })
}

fn nonempty_file(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|file| file.is_file() && file.len() > 0)
}

#[cfg(test)]
pub(super) fn write_fixture(root: &Path) {
    for file in REQUIRED_FILES.iter().copied().chain([
        "models/streaming/encoder.int8.onnx",
        "models/streaming/decoder.onnx",
        "models/streaming/joiner.int8.onnx",
    ]) {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture").unwrap();
    }
}
