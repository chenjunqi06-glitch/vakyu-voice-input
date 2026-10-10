#!/usr/bin/env bash
# 从内到外签名独立 Python、扩展库、语音宿主和 IMK 包；不依赖 --deep 猜测。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
APP="${1:?需要 app 路径}"
IDENTITY="${QINGJIAN_SIGN_IDENTITY:--}"
SIGN_ARGS=(--force --sign "$IDENTITY")
if [[ "$IDENTITY" != - ]]; then SIGN_ARGS+=(--options runtime --timestamp); fi
xattr -cr "$APP"
# Python 分发含 symlink；只签实体 Mach-O，链接路径不会重复覆盖签名。
while IFS= read -r -d '' candidate; do
  if file -b "$candidate" | grep -q 'Mach-O'; then
    codesign "${SIGN_ARGS[@]}" "$candidate"
  fi
done < <(find "$APP/Contents/Resources" -type f -print0)
HELPER="$APP/Contents/Resources/VakyuVoice.app"
if [[ -d "$HELPER" ]]; then
  codesign "${SIGN_ARGS[@]}" --entitlements "$ROOT/apps/macos/resources/Voice.entitlements" "$HELPER"
fi
codesign "${SIGN_ARGS[@]}" "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
