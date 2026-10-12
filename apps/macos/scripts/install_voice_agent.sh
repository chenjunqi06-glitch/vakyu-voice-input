#!/bin/sh
# 以登录用户身份安装全局语音启动项，不覆盖输入法配置和学习资料。
set -eu
APP="${1:?需要已安装的 Vakyu.app 路径}"
HELPER="$APP/Contents/Resources/VakyuVoice.app/Contents/MacOS/VakyuVoice"
[ -x "$HELPER" ] || { echo "未找到 Vakyu 语音组件" >&2; exit 1; }
[ "$(id -u)" != 0 ] || { echo "语音启动项必须由登录用户安装" >&2; exit 1; }
AGENT_DIR="$HOME/Library/LaunchAgents"
AGENT="$AGENT_DIR/app.vakyu.voice.plist"
mkdir -p "$AGENT_DIR" "$HOME/Library/Logs/Qingjian"
TEMP="$AGENT.tmp"
cp "$APP/Contents/Resources/VoiceLaunchAgent.plist" "$TEMP"
/usr/libexec/PlistBuddy -c "Set :ProgramArguments:0 $HELPER" \
  -c "Set :StandardErrorPath $HOME/Library/Logs/Qingjian/voice-helper.log" "$TEMP"
plutil -lint "$TEMP" >/dev/null
chmod 600 "$TEMP"
mv -f "$TEMP" "$AGENT"
DOMAIN="gui/$(id -u)"
SERVICE="$DOMAIN/app.vakyu.voice"
launchctl bootout "$SERVICE" >/dev/null 2>&1 || true
# bootout 返回时旧进程和服务条目可能仍在退出；立即 bootstrap 会得到 EIO。
attempt=0
while launchctl print "$SERVICE" >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    [ "$attempt" -le 10 ] || { echo "旧语音服务未能退出，停止升级。" >&2; exit 1; }
    sleep 1
done
attempt=0
until launchctl bootstrap "$DOMAIN" "$AGENT"; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 3 ] || { echo "语音服务注册失败，停止升级。" >&2; exit 1; }
    # 部分系统版本在服务条目消失后仍需短暂清理；不卸载已成功启动的服务。
    if launchctl print "$SERVICE" >/dev/null 2>&1; then break; fi
    sleep 1
done
launchctl kickstart "$SERVICE"
launchctl print "$SERVICE" >/dev/null
echo "Vakyu 全局语音已启动；首次使用请按语音设置中的引导完成系统授权。"
