#!/bin/sh
# 所有复制、清理和签名完成后，登记最终安装路径并用新进程复核。
set -eu
APP="${1:?需要最终安装的 Vakyu.app 路径}"
[ "$(id -u)" != 0 ] || { echo "输入源登记必须由登录用户完成" >&2; exit 1; }
APP="$(cd "$APP" && pwd -P)"
case "$APP" in
    "$HOME/Library/Input Methods/Vakyu.app"|/Library/Input\ Methods/Vakyu.app) ;;
    *) echo "拒绝登记临时构建或备份目录：$APP" >&2; exit 1 ;;
esac
BIN="$APP/Contents/MacOS/qingjian-macos"
VERIFY="$APP/Contents/Resources/verify_input_source"
[ -x "$BIN" ] && [ -x "$VERIFY" ] || { echo "安装包缺少输入源登记/核验程序" >&2; exit 1; }
/usr/bin/codesign --verify --deep --strict "$APP"
REGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
# --register 自身已经在子进程等待 TIS 更新；再复核路径，并容忍一次后续系统重扫。
for attempt in 1 2; do
    if "$REGISTER" -f "$APP" && "$BIN" --register && "$VERIFY" "$APP"; then
        sleep 3
        if "$VERIFY" "$APP"; then
            exit 0
        fi
    fi
    echo "输入源核验尚未稳定（第 $attempt 次），重新登记最终安装包" >&2
    sleep 1
done
echo "Vakyu 输入源登记未通过核验，安装尚未完成。请保留安装日志。" >&2
exit 1
