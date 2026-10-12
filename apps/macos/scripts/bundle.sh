#!/usr/bin/env bash
# 把 qingjian-macos 打包成 Vakyu.app。
#
#   scripts/bundle.sh            # 只打包到 target/macos-bundle.noindex/Vakyu.app
#   scripts/bundle.sh --install  # 打包并安装到 ~/Library/Input Methods/，杀掉旧进程（开发用）
#   scripts/bundle.sh --pkg      # 打包并做成 target/pkg/vakyu-<版本>-macos-<arm64|x86_64>.pkg（分发给测试者）
#   scripts/bundle.sh --release-pkg # 正式发行：必须具有 Developer ID 应用/安装包签名与公证配置
#
# 架构：缺省编译本机架构；QINGJIAN_TARGET=x86_64-apple-darwin（或 aarch64-apple-darwin）交叉编译另一种，
# 先 `rustup target add` 一次。CI 在 Apple Silicon runner 上两个都打（.github/workflows/release.yml）。
# 产品数据：data/generated/ 里有 dict.qj 就用自建词库（TSV 比 .qj 新会重打），没有就退回 assets/sample/ 样例。
#
# 签名与公证都由环境变量决定，没设就 ad-hoc 签名、pkg 不签（本机自用够了，分发给别人会被 Gatekeeper 拦，
# 对方要在「系统设置 → 隐私与安全性」里点「仍要打开」）：
#   QINGJIAN_SIGN_IDENTITY       "Developer ID Application: …"   给 .app 签名（hardened runtime）
#   QINGJIAN_INSTALLER_IDENTITY  "Developer ID Installer: …"     给 .pkg 签名
#   QINGJIAN_NOTARY_PROFILE      notarytool store-credentials 存的 keychain profile 名，设了就公证并钉票据
#
# --install 与 pkg 均从最终安装路径登记，并由独立进程核验输入源。
set -euo pipefail
[[ "$(uname -s)" == Darwin ]] || { echo "必须在 macOS 上构建安装包" >&2; exit 1; }
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export MACOSX_DEPLOYMENT_TARGET=13.0

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
if [[ "${1:-}" == "--release-pkg" ]]; then
  bash "$ROOT/apps/macos/scripts/check_release_signing.sh"
  set -- --pkg
fi
APP_NAME="Vakyu"
BIN_NAME="qingjian-macos"
PROFILE="${PROFILE:-release}"
WITH_VOICE="${VAKYU_WITH_VOICE:-1}"
APP="$ROOT/target/macos-bundle.noindex/$APP_NAME.app"
INSTALL_DIR="$HOME/Library/Input Methods"
# 目标三元组为空就是本机；架构名按 pkg 文件名与 distribution.xml 的 hostArchitectures 用的写法（arm64 / x86_64）
TARGET="${QINGJIAN_TARGET:-}"
case "${TARGET:-$(uname -m)}" in
  aarch64-apple-darwin|arm64) ARCH="arm64" ;;
  x86_64-apple-darwin|x86_64) ARCH="x86_64" ;;
  *) echo "不认识的架构: ${TARGET:-$(uname -m)}" >&2; exit 1 ;;
esac

cd "$ROOT"
# 完整语音版不能把样例词库作为产品数据交付。
if [[ "$WITH_VOICE" == 1 ]]; then
  for required in data/generated/dict.qj data/generated/lm.qj \
    "${QINGJIAN_MODEL_DIR:-data/models/hanzhang-zhiwei}/hanzhang-zhiwei-small.qjm" \
    "${QINGJIAN_P2C_MODEL_DIR:-data/models/hanzhang-tongbian}/hanzhang-tongbian-small.qjm"; do
    [[ -s "$required" ]] || {
      echo "完整 Vakyu 包缺少产品数据：$required；请先运行 bash tools/release/data-fetch.sh。" >&2
      exit 1
    }
  done
fi
# 源码压缩包没有 .git，仍可构建；也不能误用解压目录上级的仓库信息。
GIT_REV=unknown
BUILD_NUMBER=1
if [[ -e "$ROOT/.git" ]] && command -v git >/dev/null 2>&1; then
  GIT_REV="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
  BUILD_NUMBER="$(git rev-list --count HEAD 2>/dev/null || echo 1)"
  if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then GIT_REV="${GIT_REV}+"; fi
else
  # 源码归档反复打包也需不同构建号，避免 LaunchServices 沿用旧图标与应用元数据。
  mkdir -p "$ROOT/target"
  BUILD_COUNTER="$ROOT/target/macos-local-build-number"
  PREVIOUS_BUILD="$(cat "$BUILD_COUNTER" 2>/dev/null || echo 1)"
  [[ "$PREVIOUS_BUILD" =~ ^[0-9]+$ ]] || { echo "本地构建号无效" >&2; exit 1; }
  BUILD_NUMBER=$((10#$PREVIOUS_BUILD + 1))
  printf '%s\n' "$BUILD_NUMBER" > "$BUILD_COUNTER"
fi
export QINGJIAN_BUILD="${GIT_REV} · $(date +%Y-%m-%d)"
BUILD_ARGS=(-p "$BIN_NAME" --locked)
if [[ "$WITH_VOICE" == 1 ]]; then
  [[ "$ARCH" == arm64 ]] || { echo "完整语音构建需要 Apple 芯片；仅构建旧输入法可显式设 VAKYU_WITH_VOICE=0" >&2; exit 1; }
  BUILD_ARGS+=(-p vakyu-voice-service)
fi
[[ "$PROFILE" == "release" ]] && BUILD_ARGS+=(--release)
BIN_DIR="target/$PROFILE"
if [[ -n "$TARGET" ]]; then
  BUILD_ARGS+=(--target "$TARGET")
  BIN_DIR="target/$TARGET/$PROFILE"
fi
cargo build "${BUILD_ARGS[@]}"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN_DIR/$BIN_NAME" "$APP/Contents/MacOS/$BIN_NAME"
cp apps/macos/Info.plist "$APP/Contents/Info.plist"
# 版本号来自 apps/macos/Cargo.toml（各平台壳版本号独立，不跟 workspace 走），构建号用提交数（单调递增，pkg 升级判断靠它）。
# 发版之间版本号带 -dev（0.1.2-dev）：本地与 CI 中间构建一眼能与线上包区分；发版提交去掉 -dev 再打标签（docs/notes/release.md）。
# 开发版再接上 git 短哈希（0.1.3-dev-1a2b3c4，工作区有改动加 +），测试时一眼知道装的是哪个提交；Cargo.toml 里仍只写 -dev。
# pkgbuild / distribution 的 version 只认数字点号，去掉预发布后缀；Info.plist 与 pkg 文件名保留完整版本
VERSION="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' apps/macos/Cargo.toml | head -1)"
if [[ "$VERSION" == *-dev ]]; then VERSION="${VERSION}-${GIT_REV}"; fi
PKG_VERSION="${VERSION%%-*}"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" \
  -c "Set :CFBundleVersion $BUILD_NUMBER" "$APP/Contents/Info.plist"
# 卸载脚本随包，装了 pkg 的用户从 Resources 里运行
cp apps/macos/scripts/uninstall.sh "$APP/Contents/Resources/uninstall.sh"
cp apps/macos/scripts/register_input_source.sh "$APP/Contents/Resources/register_input_source.sh"
# 安装端不依赖 Xcode/Swift 编译器，独立核验程序在构建时生成并随包签名。
xcrun clang -arch "$ARCH" -fobjc-arc -Wall -Wextra -Werror -mmacosx-version-min=13.0 \
  -framework AppKit -framework Carbon apps/macos/scripts/verify_input_source.m \
  -o "$APP/Contents/Resources/verify_input_source"
# 输入源名称在各系统语言下统一显示 Vakyu。
cp -R apps/macos/resources/*.lproj "$APP/Contents/Resources/"
# 词库与释义表打进 Resources。data/generated/ 里有生成好的产品数据（自建词库 + 语言模型 + LLM 释义表）就用它，
# 否则用 assets/sample/ 的样例。没有数据管道的机器跑 tools/release/data-fetch.sh 按 tools/release/data.lock 下载。
cp assets/sample/*.tsv "$APP/Contents/Resources/"
# emoji 表（Unicode CLDR，可发布）
cp assets/emoji/*.tsv "$APP/Contents/Resources/"
# 词汇等级表（CEFR-J / Octanove / JLPT，见 assets/levels/README.md），「统计」页按级数词汇
cp assets/levels/levels-*.tsv "$APP/Contents/Resources/"
# 五笔码表（输入方案选五笔时用，见 assets/wubi/README.md；极点 86 码表，Apache-2.0）
mkdir -p "$APP/Contents/Resources/wubi"
cp assets/wubi/wubi86.tsv "$APP/Contents/Resources/wubi/"
if [[ -f data/generated/dict.tsv || -f data/generated/dict.qj ]]; then
  # 词库与语言模型打成 .qj（mmap 直接用），TSV 比 .qj 新时重新打包；只有 .qj（CI 从数据包解出来的）就直接用
  if [[ -f data/generated/dict.tsv && ( ! -f data/generated/dict.qj || data/generated/dict.tsv -nt data/generated/dict.qj ) ]]; then
    cargo run --release -q -p qingjian-dict-convert -- pack dict --name "Vakyu 基础词库" \
      --license "MIT AND Unicode-3.0" --attribution "通用规范汉字表；现代汉语常用词表（liuxilu 校对版）；THUOCL（清华大学自然语言处理实验室，MIT）；读音 Unihan（Unicode）" \
      --source https://github.com/qingjian-team/qingjian/tree/main/assets/lexicon
  fi
  if [[ -f data/generated/lm-bigram.tsv && ( ! -f data/generated/lm.qj || data/generated/lm-bigram.tsv -nt data/generated/lm.qj ) ]]; then
    cargo run --release -q -p qingjian-dict-convert -- pack lm --name "Vakyu 语言模型（中文维基 + LCCC，Vakyu 词库分词）" \
      --license "CC-BY-SA-4.0 AND MIT" --attribution "中文维基百科（CC BY-SA 4.0）；LCCC（清华大学 CoAI，MIT）"
  fi
  cp data/generated/dict.qj "$APP/Contents/Resources/"
  # 领域词库（lexicon 拆出的 dicts/*.qj）随包放 Resources/dicts/，缺省只开成语，偏好设置「词库」页可勾选
  if ls data/generated/dicts/*.qj >/dev/null 2>&1; then
    mkdir -p "$APP/Contents/Resources/dicts"
    cp data/generated/dicts/*.qj "$APP/Contents/Resources/dicts/"
  fi
  [[ -f data/generated/lm.qj ]] && cp data/generated/lm.qj "$APP/Contents/Resources/"
  # 含章·知微（字级 Transformer）：三件套与单文件放 data/models/hanzhang-zhiwei/。
  model_dir="${QINGJIAN_MODEL_DIR:-data/models/hanzhang-zhiwei}"
  if [[ -f "$model_dir/model.safetensors" ]]; then
    QINGJIAN_MODEL_DIR="$model_dir" tools/release/pack-model.sh
  fi
  if [[ -f "$model_dir/hanzhang-zhiwei-small.qjm" ]]; then
    mkdir -p "$APP/Contents/Resources/models/hanzhang-zhiwei"
    cp "$model_dir/hanzhang-zhiwei-small.qjm" "$APP/Contents/Resources/models/hanzhang-zhiwei/"
    chmod 644 "$APP/Contents/Resources/models/hanzhang-zhiwei/hanzhang-zhiwei-small.qjm"
    echo "打包含章·知微：$model_dir/hanzhang-zhiwei-small.qjm"
  fi
  # 含章·通变（P2C，带噪拼音 → 汉字）：放 Resources/models/hanzhang-tongbian/。
  p2c_dir="${QINGJIAN_P2C_MODEL_DIR:-data/models/hanzhang-tongbian}"
  if [[ -f "$p2c_dir/model.safetensors" ]]; then
    QINGJIAN_MODEL_DIR="$p2c_dir" tools/release/pack-model.sh
  fi
  if [[ -f "$p2c_dir/hanzhang-tongbian-small.qjm" ]]; then
    mkdir -p "$APP/Contents/Resources/models/hanzhang-tongbian"
    cp "$p2c_dir/hanzhang-tongbian-small.qjm" "$APP/Contents/Resources/models/hanzhang-tongbian/"
    chmod 644 "$APP/Contents/Resources/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm"
    echo "打包含章·通变：$p2c_dir/hanzhang-tongbian-small.qjm"
  fi
  # 释义表打成 .qj（TSV 比 .qj 新时重打），英文词表仍是 TSV。各表来源不同，元数据按表写（见 assets/glossary/README.md）
  for lang in en ja zh es; do
    src="assets/glossary/glossary-$lang.tsv"
    out="data/generated/glossary-$lang.qj"
    [[ -f "$src" ]] || continue
    if [[ "$lang" == es ]]; then
      license="GPL-3.0-or-later"
      attribution="Azure Translator 机器翻译（Tofuzhu，tools/corpus/glossary_es.py）"
    else
      license="MIT"
      attribution="LLM 生成（DeepSeek），qingjian-gloss-gen"
    fi
    if [[ ! -f "$out" || "$src" -nt "$out" ]]; then
      cargo run --release -q -p qingjian-dict-convert -- pack glossary --language "$lang" --input "$src" \
        --name "Vakyu 释义表（${lang}）" --license "$license" --attribution "$attribution"
    fi
    cp "$out" "$APP/Contents/Resources/"
  done
  for f in assets/lexicon/english.tsv data/generated/english.tsv; do
    [[ -f "$f" ]] && cp "$f" "$APP/Contents/Resources/"
  done
  echo "使用 data/generated/ 的产品数据（自建词库）"
fi
printf 'APPL????' > "$APP/Contents/PkgInfo"

# 应用图标与输入法菜单使用小鸡标志，设置和浮窗使用完整字标。
ICONSET="$ROOT/target/Vakyu.iconset"
rm -rf "$ICONSET" && mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size assets/icon/windows/vakyu-icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double assets/icon/windows/vakyu-icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Vakyu.icns"
swift apps/macos/scripts/menu_icon.swift assets/icon/windows/vakyu-icon.png \
  "$APP/Contents/Resources/vakyu-menu.png" "$APP/Contents/Resources/vakyu-input-source-baseline.pdf" \
  "$APP/Contents/Resources/vakyu-input-sourceTemplate.pdf"
cp assets/icon/windows/vakyu-icon.png "$APP/Contents/Resources/vakyu-icon.png"
cp assets/icon/windows/vakyu-logo.png "$APP/Contents/Resources/vakyu-logo.png"
if [[ "$WITH_VOICE" == 1 ]]; then
  bash apps/macos/scripts/bundle_voice.sh "$APP" "$ARCH" "$PROFILE" "$ROOT/$BIN_DIR"
fi
# 嵌套运行时从内到外签名，语音宿主带麦克风 entitlement。
bash apps/macos/scripts/sign_bundle.sh "$APP"
if [[ "$WITH_VOICE" == 1 ]]; then
  python3 apps/macos/scripts/check_port.py --bundle "$APP"
fi
echo "打包完成: ${APP}（版本 ${VERSION}，构建 ${BUILD_NUMBER}，${ARCH}）"

if [[ "${1:-}" == "--pkg" ]]; then
  # 每个架构一个工作目录，成品都放 target/pkg/，两个架构接着打互不覆盖
  PKG="$ROOT/target/pkg/vakyu-$VERSION-macos-$ARCH.pkg"
  PKG_DIR="$ROOT/target/pkg/$ARCH"
  rm -rf "$PKG_DIR"
  mkdir -p "$PKG_DIR/payload.noindex" "$PKG_DIR/resources"
  # 不带扩展属性复制，否则载荷里全是 ._ 元数据文件
  ditto --noextattr --norsrc --noacl "$APP" "$PKG_DIR/payload.noindex/$APP_NAME.app"
  # 组件描述里关掉 bundle 重定位：否则机器上别处已有同 bundle id 的 .app（比如 ~/Library 下的开发副本）时，
  # 安装器会把新版装到那里而不是 /Library/Input Methods
  pkgbuild --analyze --root "$PKG_DIR/payload.noindex" "$PKG_DIR/component.plist" >/dev/null
  /usr/libexec/PlistBuddy -c "Set :0:BundleIsRelocatable false" "$PKG_DIR/component.plist" 2>/dev/null || \
    /usr/libexec/PlistBuddy -c "Add :0:BundleIsRelocatable bool false" "$PKG_DIR/component.plist"
  mkdir -p "$PKG_DIR/scripts"
  install -m 755 apps/macos/pkg/scripts/preinstall "$PKG_DIR/scripts/preinstall"
  install -m 755 apps/macos/pkg/scripts/postinstall "$PKG_DIR/scripts/postinstall"
  pkgbuild --root "$PKG_DIR/payload.noindex" --component-plist "$PKG_DIR/component.plist" \
    --install-location "/Library/Input Methods" --scripts "$PKG_DIR/scripts" \
    --identifier app.qingjian.inputmethod --version "$PKG_VERSION" "$PKG_DIR/$APP_NAME-component.pkg" >/dev/null
  # 已进入组件包后不再保留可被系统扫描的 payload app 副本。
  rm -rf "$PKG_DIR/payload.noindex"
  cp apps/macos/pkg/resources/*.html "$PKG_DIR/resources/"
  cp LICENSE "$PKG_DIR/resources/license.txt"
  # 二进制只有一种架构，hostArchitectures 限定只在对应机器上装；另一种架构用 QINGJIAN_TARGET 再打一份
  sed -e "s/@VERSION@/$VERSION/g" -e "s/@PKG_VERSION@/$PKG_VERSION/g" -e "s/@ARCH@/$ARCH/g" apps/macos/pkg/distribution.xml > "$PKG_DIR/distribution.xml"
  SIGN_ARGS=()
  if [[ -n "${QINGJIAN_INSTALLER_IDENTITY:-}" ]]; then
    SIGN_ARGS=(--sign "$QINGJIAN_INSTALLER_IDENTITY" --timestamp)
  fi
  # bash 3.2 下空数组展开会撞 set -u，用 ${arr[@]+"${arr[@]}"} 写法
  productbuild --distribution "$PKG_DIR/distribution.xml" --package-path "$PKG_DIR" \
    --resources "$PKG_DIR/resources" ${SIGN_ARGS[@]+"${SIGN_ARGS[@]}"} "$PKG" >/dev/null
  if [[ -n "${QINGJIAN_NOTARY_PROFILE:-}" ]]; then
    xcrun notarytool submit "$PKG" --keychain-profile "$QINGJIAN_NOTARY_PROFILE" --wait
    xcrun stapler staple "$PKG"
    echo "已公证并钉上票据"
  elif [[ -z "${QINGJIAN_INSTALLER_IDENTITY:-}" ]]; then
    echo "注意: pkg 未签名未公证，测试者首次打开要在「系统设置 → 隐私与安全性」里点「仍要打开」"
  fi
  echo "pkg: $PKG"
  shasum -a 256 "$PKG"
fi

if [[ "${1:-}" == "--install" ]]; then
  bash "$ROOT/apps/macos/scripts/install_local.sh" "$APP"
  echo "已安装到: $INSTALL_DIR/$APP_NAME.app"
  echo "日志: ~/Library/Logs/Qingjian/"
fi
