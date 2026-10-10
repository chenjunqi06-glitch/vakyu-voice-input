"""分别生成 Vakyu 任务栏小鸡图标和设置页完整字标，保留原始透明通道。"""

import argparse
from pathlib import Path

from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("icon_source", type=Path)
    parser.add_argument("wordmark_source", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    icon_source = Image.open(args.icon_source).convert("RGBA")
    wordmark = Image.open(args.wordmark_source).convert("RGBA")
    bounds = icon_source.getchannel("A").point(lambda a: 255 if a > 1 else 0).getbbox()
    if bounds is None:
        raise ValueError("The icon source is fully transparent")
    icon = icon_source.crop(bounds)
    icon.thumbnail((480, 480), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (512, 512))
    canvas.alpha_composite(icon, ((512 - icon.width) // 2, (512 - icon.height) // 2))
    icon_source.save(root / "vakyu-icon-source.png", optimize=True)
    canvas.save(root / "vakyu-icon.png", optimize=True)
    sizes = [(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)]
    canvas.save(root / "vakyu.ico", sizes=sizes)
    # 字标不裁成方形；设置侧栏使用 Uniform 显示完整字母和小鸡。
    wordmark.save(root / "vakyu-wordmark-source.png", optimize=True)
    wordmark.thumbnail((832, 832), Image.Resampling.LANCZOS)
    wordmark.save(root / "vakyu-logo.png", optimize=True)
    print(f"Taskbar icon: {icon_source.size}, 9 ICO sizes; settings logo: {wordmark.size}")


if __name__ == "__main__":
    main()
