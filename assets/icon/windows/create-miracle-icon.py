"""将用户提供的透明彩虹原图裁边，生成 Windows 品牌 PNG 与多尺寸图标。"""

import argparse
from pathlib import Path

from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    original = Image.open(args.source).convert("RGBA")
    # 原图边缘有肉眼不可见的 alpha=1 像素；不让它们压缩托盘中的有效图案。
    bounds = original.getchannel("A").point(lambda value: 255 if value > 1 else 0).getbbox()
    if bounds is None:
        raise ValueError("The source image is fully transparent")
    cropped = original.crop(bounds)
    cropped.thumbnail((464, 464), Image.Resampling.LANCZOS)
    logo = Image.new("RGBA", (512, 512))
    logo.alpha_composite(cropped, ((512 - cropped.width) // 2, (512 - cropped.height) // 2))
    original.save(root / "miracle-source.png", optimize=True)
    logo.save(root / "miracle-logo.png", optimize=True)
    logo.save(root / "miracle.ico", sizes=[(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
    print(f"Source {original.size}; visible bounds {bounds}; generated PNG and 9 icon sizes")


if __name__ == "__main__":
    main()
