"""Create the iconset consumed by macOS' native ``iconutil`` command."""

from pathlib import Path
import sys

from PIL import Image


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: prepare_macos_icon.py SOURCE.ico OUTPUT.iconset")
    source, output = map(Path, sys.argv[1:])
    output.mkdir(parents=True, exist_ok=True)
    image = Image.open(source).convert("RGBA")
    for points in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            pixels = points * scale
            name = f"icon_{points}x{points}{'@2x' if scale == 2 else ''}.png"
            image.resize((pixels, pixels), Image.Resampling.LANCZOS).save(output / name)


if __name__ == "__main__":
    main()
