#!/bin/sh
# 卸载 Vakyu 的输入法和全局语音；只有 --purge 才删除本用户资料。
set -u
purge=0
[ "${1:-}" = "--purge" ] && purge=1
launchctl bootout "gui/$(id -u)/app.vakyu.voice" >/dev/null 2>&1 || true
rm -f "$HOME/Library/LaunchAgents/app.vakyu.voice.plist"
pkill -x VakyuVoice 2>/dev/null || true
pkill -x qingjian-macos 2>/dev/null || true
for name in Vakyu Qingjian; do
    for directory in "/Library/Input Methods" "$HOME/Library/Input Methods"; do
        app="$directory/$name.app"
        [ -d "$app" ] || continue
        identity="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist" 2>/dev/null || true)"
        [ "$identity" = app.qingjian.inputmethod ] || continue
        if [ "$directory" = "/Library/Input Methods" ]; then
            sudo rm -rf "$app"
            sudo pkgutil --forget app.qingjian.inputmethod >/dev/null 2>&1 || true
        else
            rm -rf "$app"
        fi
    done
done
if [ "$purge" = 1 ]; then
    rm -rf "$HOME/Library/Application Support/Qingjian" "$HOME/Library/Logs/Qingjian"
    echo "学习数据、配置与日志已删除"
else
    echo "已保留 ~/Library/Application Support/Qingjian 下的配置和学习资料"
fi
echo "已卸载 Vakyu。输入法列表若未刷新，请注销后重新登录。"
