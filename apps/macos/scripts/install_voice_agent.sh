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
launchctl bootout "$DOMAIN/app.vakyu.voice" >/dev/null 2>&1 || true
launchctl bootstrap "$DOMAIN" "$AGENT"
launchctl kickstart "$DOMAIN/app.vakyu.voice"
echo "Vakyu 全局语音已启动；首次使用请允许麦克风、辅助功能和输入监控。"
