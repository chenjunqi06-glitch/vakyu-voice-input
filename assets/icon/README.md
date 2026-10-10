# 图标

- `windows/vakyu.ico`：用户提供的方形小鸡图，Windows 托盘、任务栏、设置窗口图标、TSF 注册与安装包共用；包含 16 至 256 像素的九档尺寸。
- `windows/vakyu-logo.png`：设置侧栏完整字母与小鸡标志，832×303 等比 PNG，不裁成方形。两张原图分别保存在 `vakyu-icon-source.png` 与 `vakyu-wordmark-source.png`；使用 `python windows/create-vakyu-icons.py <任务栏原图> <完整字标原图>` 重新生成。
- `windows/miracle-logo.png`：Miracle 彩虹标志，用于 README、设置程序、macOS bundle 与 Linux Fcitx 图标。
- `windows/miracle-candidate-mark.png`：用户提供的完整字母与吉祥物标志，保留全部内容及原始宽高比；Windows 高 DPI 时按窗口缩放。原始完整图保存为 windows/miracle-candidate-source.png。
- `windows/miracle.ico`：上一版 Windows 图标留档；当前 Windows 产品使用 `vakyu.ico`。
- `legacy-mark.svg`：旧版 README 竹简图标留档，不再用于产品界面。
- `legacy-logo.png`：旧版竹简应用图标，仅留档，不再参与打包。
- `menu.svg`：macOS 输入法图标源文件，黑色键帽镂空四片竹简（模板图，系统只取 alpha）。`menu.pdf` 是它导出的
  22×16pt 矢量版，打包时拷成 `qingjian-menu.pdf`，Info.plist 的图标键都指向它。为什么是这个形式和尺寸见
  `docs/design/architecture.md`「Info.plist 约定」。改了 svg 重新导出：

  ```sh
  rsvg-convert -f pdf --page-width 22pt --page-height 16pt -w 22pt -h 16pt assets/icon/menu.svg -o assets/icon/menu.pdf
  ```
- `windows/mode-zh.svg` / `mode-en.svg` / `mode-caps.svg`：Windows 任务栏的中 / 英 / A 图标源文件（16×16 画布，单色）。
  `windows/render-mode-icons.sh` 用 rsvg-convert + magick 栅格化成 16 / 20 / 24 / 32 四档的 8 位 alpha 蒙版，
  写到 `apps/windows/tsf/resources/mode/`，DLL 用 `include_bytes!` 嵌入、运行时按任务栏深浅色填色（`com/mode/icon.rs`）。
  改了 svg 重跑脚本，生成物随仓库提交。
