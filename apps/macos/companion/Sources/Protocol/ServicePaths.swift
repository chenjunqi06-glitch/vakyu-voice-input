// 所有数据留在现有兼容目录；打包脚本负责资源布局，不从 PATH 猜测解释器。
import Foundation

struct ServicePaths {
    let executable: URL

    let arguments: [String]

    init(bundle: Bundle = .main) throws {
        guard let resources = bundle.resourceURL else {
            throw NativeFailure("找不到 Vakyu 语音资源，请重新安装。")
        }
        let data = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Qingjian", isDirectory: true)
        try FileManager.default.createDirectory(at: data, withIntermediateDirectories: true)
        executable = resources.appendingPathComponent("vakyu-voice-service")
        let runtime = resources.appendingPathComponent("voice-runtime", isDirectory: true)
        let python = runtime.appendingPathComponent("python/bin/python3")
        let application = runtime.appendingPathComponent("app", isDirectory: true)
        let models = runtime.appendingPathComponent("models", isDirectory: true)
        for path in [executable, python, application.appendingPathComponent("external_main.py"), models] {
            guard FileManager.default.fileExists(atPath: path.path) else {
                throw NativeFailure("语音组件不完整，请重新安装包含语音模型的 Vakyu。")
            }
        }
        arguments = ["--config", data.appendingPathComponent("config.toml").path,
                     "--data", data.path, "--python", python.path,
                     "--runtime", application.path, "--models", models.path]
    }
}
