#!/usr/bin/env bash
# 面向其他用户的发行包必须配置固定开发者身份与公证；本机开发构建另走 --pkg。
set -euo pipefail
case "${QINGJIAN_SIGN_IDENTITY:-}" in
  "Developer ID Application: "*) ;;
  *) echo "正式发行需要 QINGJIAN_SIGN_IDENTITY 指定 Developer ID Application 证书；不能使用临时开发签名。" >&2; exit 1 ;;
esac
case "${QINGJIAN_INSTALLER_IDENTITY:-}" in
  "Developer ID Installer: "*) ;;
  *) echo "正式发行需要 QINGJIAN_INSTALLER_IDENTITY 指定 Developer ID Installer 证书。" >&2; exit 1 ;;
esac
[[ -n "${QINGJIAN_NOTARY_PROFILE:-}" ]] || {
  echo "正式发行需要 QINGJIAN_NOTARY_PROFILE 指定钥匙串中的公证凭据。" >&2
  exit 1
}
