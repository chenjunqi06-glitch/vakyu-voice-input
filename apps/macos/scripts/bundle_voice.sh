#!/usr/bin/env bash
# 把原生语音宿主和已校验的本地模型装进输入法包；不读取用户配置。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
APP="${1:?需要目标 app 路径}"
ARCH="${2:?需要目标架构}"
PROFILE="${3:-release}"
BIN_DIR="${4:?需要 Rust 产物目录}"
[[ "$ARCH" == arm64 ]] || { echo "完整语音包目前要求 Apple 芯片；Intel 需单独准备运行时" >&2; exit 1; }
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || {
  echo "请在 Apple 芯片 Mac 上构建原生语音组件" >&2; exit 1;
}
RUNTIME="$ROOT/target/macos-voice-runtime/arm64"
python3 "$ROOT/apps/macos/scripts/prepare_voice_runtime.py" --output "$RUNTIME"
SWIFT_CONFIG=release
[[ "$PROFILE" == release ]] || SWIFT_CONFIG=debug
swift build --package-path "$ROOT/apps/macos/companion" --configuration "$SWIFT_CONFIG" \
  --arch arm64 --jobs "${CARGO_BUILD_JOBS:-2}"
SWIFT_BIN="$(swift build --package-path "$ROOT/apps/macos/companion" --configuration "$SWIFT_CONFIG" \
  --arch arm64 --show-bin-path)"
HELPER="$APP/Contents/Resources/VakyuVoice.app"
RESOURCE="$HELPER/Contents/Resources"
mkdir -p "$HELPER/Contents/MacOS" "$RESOURCE"
cp "$SWIFT_BIN/VakyuVoice" "$HELPER/Contents/MacOS/VakyuVoice"
cp "$ROOT/apps/macos/resources/VoiceInfo.plist" "$HELPER/Contents/Info.plist"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
BUILD="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP/Contents/Info.plist")"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" \
  -c "Set :CFBundleVersion $BUILD" "$HELPER/Contents/Info.plist"
cp "$BIN_DIR/vakyu-voice-service" "$RESOURCE/vakyu-voice-service"
ditto --noextattr --norsrc --noacl "$RUNTIME" "$RESOURCE/voice-runtime"
cp "$ROOT/assets/icon/windows/vakyu-icon.png" "$RESOURCE/vakyu-icon.png"
cp "$ROOT/assets/icon/windows/vakyu-logo.png" "$RESOURCE/vakyu-logo.png"
cp "$APP/Contents/Resources/Vakyu.icns" "$RESOURCE/Vakyu.icns"
cp "$ROOT/apps/macos/resources/VoiceLaunchAgent.plist" "$APP/Contents/Resources/VoiceLaunchAgent.plist"
cp "$ROOT/apps/macos/scripts/install_voice_agent.sh" "$APP/Contents/Resources/install_voice_agent.sh"
chmod +x "$APP/Contents/Resources/install_voice_agent.sh"
printf 'APPL????' > "$HELPER/Contents/PkgInfo"
