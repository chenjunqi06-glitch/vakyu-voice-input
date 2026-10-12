// 用户级排他锁避免登录启动与手动打开竞态造成两个常驻识别模型。
import Foundation
import Darwin

final class InstanceLease {
    private var descriptor: Int32 = -1

    init() throws {
        let directory = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Qingjian", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent("vakyu-voice.lock")
        descriptor = Darwin.open(file.path, O_CREAT | O_RDWR | O_CLOEXEC, mode_t(0o600))
        guard descriptor >= 0 else { throw NativeFailure("无法建立语音服务的本机锁。") }
        guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
            Darwin.close(descriptor)
            descriptor = -1
            throw NativeFailure("语音服务已经运行。")
        }
    }

    deinit { if descriptor >= 0 { flock(descriptor, LOCK_UN); Darwin.close(descriptor) } }
}
