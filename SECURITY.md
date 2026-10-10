# 安全政策 / Security policy

Vakyu 目前处于公开测试阶段，安全修复优先面向最新发布版本。

请通过本仓库的 **Security → Report a vulnerability** 私密报告漏洞。不要把 API 密钥、录音、个人输入记录或尚未修复的漏洞细节放进公开 Issue。项目由社区维护，不承诺固定响应时间。

重点关注：输入内容或密钥泄露、密码框和系统安全桌面的边界、安装包完整性、恶意词库或模型数据引发的异常、语音停止后麦克风仍被占用。

普通语音识别在本机完成；开启翻译或云联想后，相关文本会直接发给用户自行配置的 API 服务商。服务商账号、额度及数据政策由该服务商管理。不要为了使用本项目关闭 Windows 防护或给陌生证书增加信任。

## English

Vakyu is a public beta. Report vulnerabilities privately through **Security → Report a vulnerability** in this repository. Do not publish API keys, recordings, personal input history, or unpatched exploit details in public issues. Maintainers cannot promise a fixed response time.

Security-sensitive areas include text/key leakage, protected input fields, installer integrity, malformed dictionaries/models, and microphone lifecycle. Optional translation and cloud prediction send relevant text directly to the provider configured by the user; offline dictation does not upload audio.
