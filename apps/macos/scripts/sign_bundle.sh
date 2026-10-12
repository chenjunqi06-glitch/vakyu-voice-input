#!/usr/bin/env bash
# 从内到外签名独立 Python、扩展库、语音宿主和 IMK 包；不依赖 --deep 猜测。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
APP="${1:?需要 app 路径}"
IDENTITY="${QINGJIAN_SIGN_IDENTITY:-${QINGJIAN_LOCAL_SIGN_IDENTITY:-}}"
if [[ -z "$IDENTITY" ]]; then
  # 只在唯一候选时自动选本机开发证书，避免多团队机器每次选中不同身份。
  LOCAL_IDENTITIES="$(/usr/bin/security find-identity -v -p codesigning 2>/dev/null |
    /usr/bin/awk '/"Apple Development:|"Vakyu Local Code Signing"/ {print $2}')"
  if [[ -n "$LOCAL_IDENTITIES" && "$(printf '%s\n' "$LOCAL_IDENTITIES" | wc -l | tr -d ' ')" == 1 ]]; then
    IDENTITY="$LOCAL_IDENTITIES"
  else
    IDENTITY=-
  fi
fi
SIGN_ARGS=(--force --sign "$IDENTITY")
if [[ "$IDENTITY" == - ]]; then
  echo "未配置固定签名身份，使用临时签名；重新编译后可能需要重新添加语音系统权限。" >&2
else
  SIGN_ARGS+=(--options runtime)
  # 本机自签/开发证书不依赖联网时间戳；正式 Developer ID 仍默认启用。
  if [[ "$IDENTITY" == "Developer ID Application:"* || "${QINGJIAN_SIGN_TIMESTAMP:-0}" == 1 ]]; then
    SIGN_ARGS+=(--timestamp)
  fi
fi
xattr -cr "$APP"
# Python 分发含 symlink；只签实体 Mach-O，链接路径不会重复覆盖签名。
while IFS= read -r -d '' candidate; do
  if file -b "$candidate" | grep -q 'Mach-O'; then
    codesign "${SIGN_ARGS[@]}" "$candidate"
  fi
done < <(find "$APP/Contents/Resources" -type f -print0)
HELPER="$APP/Contents/Resources/VakyuVoice.app"
if [[ -d "$HELPER" ]]; then
  python3 "$ROOT/apps/macos/scripts/runtime_manifest.py" "$HELPER/Contents/Resources/voice-runtime"
  codesign "${SIGN_ARGS[@]}" --entitlements "$ROOT/apps/macos/resources/Voice.entitlements" "$HELPER"
fi
codesign "${SIGN_ARGS[@]}" "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
