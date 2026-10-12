#!/bin/bash
# 用户级安装采用目录交换；失败恢复原包，最后始终重建最终路径的输入源登记。
set -euo pipefail
SOURCE="${1:?需要构建完成的 Vakyu.app 路径}"
DEST="$HOME/Library/Input Methods/Vakyu.app"
IDENTITY=app.qingjian.inputmethod
[ "$(id -u)" != 0 ] || { echo "请以登录用户安装，勿使用 sudo" >&2; exit 1; }
for app in "/Library/Input Methods/Vakyu.app" "/Library/Input Methods/Qingjian.app" "$HOME/Library/Input Methods/Qingjian.app"; do
  if [[ -d "$app" ]] && [[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist" 2>/dev/null)" == "$IDENTITY" ]]; then
    echo "发现另一个安装位置：$app；请先卸载该副本，再进行用户级安装。" >&2
    exit 1
  fi
done
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$SOURCE/Contents/Info.plist")" == "$IDENTITY" ]]
[[ -x "$SOURCE/Contents/Resources/verify_input_source" ]]
[[ ! -L "$DEST" ]] || { echo "安装位置不能是符号链接：$DEST" >&2; exit 1; }
if [[ -e "$DEST" ]]; then
  [[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$DEST/Contents/Info.plist")" == "$IDENTITY" ]]
fi
[[ "$(cd "$SOURCE" && pwd -P)" != "$DEST" ]] || { echo "构建源不能是已安装应用" >&2; exit 1; }
/usr/bin/codesign --verify --deep --strict "$SOURCE"
mkdir -p "$(dirname "$DEST")"
STAGING="$(mktemp -d "$(dirname "$DEST")/.vakyu-install.XXXXXX")"
NEW="$STAGING/new"
OLD="$STAGING/previous"
AGENT="$HOME/Library/LaunchAgents/app.vakyu.voice.plist"
REPLACED=0
VOICE_STARTED=0
restore_registration() {
  if [[ -f "$DEST/Contents/Resources/register_input_source.sh" ]]; then
    /bin/sh "$DEST/Contents/Resources/register_input_source.sh" "$DEST"
  else
    # 兼容升级前没有独立核验器的安装包；使用本次构建的只读核验器复查原包。
    "$DEST/Contents/MacOS/qingjian-macos" --register &&
      "$SOURCE/Contents/Resources/verify_input_source" "$DEST"
  fi
}
rollback() {
  status=$?
  trap - EXIT INT TERM
  set +e
  if [[ "$VOICE_STARTED" == 1 ]]; then
    /bin/launchctl bootout "gui/$(id -u)/app.vakyu.voice" >/dev/null 2>&1 || true
    if [[ -f "$STAGING/voice-agent.plist" ]]; then
      cp -p "$STAGING/voice-agent.plist" "$AGENT"
    elif [[ -f "$AGENT" ]] && [[ "$(/usr/libexec/PlistBuddy -c 'Print :Label' "$AGENT" 2>/dev/null)" == app.vakyu.voice ]]; then
      rm -f "$AGENT"
    fi
  fi
  if [[ "$REPLACED" == 1 || -d "$OLD" ]]; then
    /usr/bin/pkill -x qingjian-macos 2>/dev/null || true
    if [[ ! -d "$OLD" && -d "$DEST" ]]; then
      # 首次安装失败要先恢复原输入源，不能删除仍被选中的唯一输入法包。
      previous="$(cat "$STAGING/source-before" 2>/dev/null)"
      if [[ -n "$previous" ]] && "$DEST/Contents/MacOS/qingjian-macos" --finish-register "$previous"; then
        /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -u "$DEST" || true
      else
        echo "首次安装失败且原输入源未恢复，保留应用和诊断目录：$STAGING" >&2
        exit "$status"
      fi
    fi
    if [[ "$REPLACED" == 1 ]]; then rm -rf "$DEST"; fi
    if [[ -d "$OLD" ]]; then
      mv "$OLD" "$DEST"
      if restore_registration && /usr/bin/open -g "$DEST"; then
        echo "已恢复原安装及输入源登记" >&2
      else
        echo "原安装已恢复，但输入源仍未核验成功：$DEST" >&2
      fi
      if [[ "$VOICE_STARTED" == 1 && -f "$STAGING/voice-agent.plist" ]]; then
        /bin/launchctl bootstrap "gui/$(id -u)" "$AGENT" ||
          echo "原语音组件已恢复，但自动启动尚未恢复" >&2
      fi
    fi
  fi
  if [[ -d "$OLD" ]]; then
    echo "回滚未完全成功；保留原包备份：$OLD" >&2
  else
    rm -rf "$STAGING"
  fi
  exit "$status"
}
trap rollback EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
if [[ -f "$AGENT" ]]; then cp -p "$AGENT" "$STAGING/voice-agent.plist"; fi
"$SOURCE/Contents/Resources/verify_input_source" --current-source > "$STAGING/source-before"
/usr/bin/ditto --noextattr --norsrc --noacl "$SOURCE" "$NEW"
/usr/bin/codesign --verify --deep --strict "$NEW"
/usr/bin/pkill -x qingjian-macos 2>/dev/null || true
if [[ -d "$DEST" ]]; then mv "$DEST" "$OLD"; fi
REPLACED=1
mv "$NEW" "$DEST"
/bin/sh "$DEST/Contents/Resources/register_input_source.sh" "$DEST"
/usr/bin/open -g "$DEST"
if [[ -f "$DEST/Contents/Resources/install_voice_agent.sh" ]]; then
  VOICE_STARTED=1
  /bin/sh "$DEST/Contents/Resources/install_voice_agent.sh" "$DEST"
fi
# 最后再次核验后才释放备份；隐藏的无 .app 后缀备份从未向 LaunchServices 登记。
"$DEST/Contents/Resources/verify_input_source" "$DEST"
trap - EXIT INT TERM
rm -rf "$STAGING"
echo "安装及输入源核验完成：$DEST"
