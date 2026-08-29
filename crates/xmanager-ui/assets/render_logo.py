"""Render the XManager app mark to SVG, PNG, and multi-size ICO."""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 1024
CORNER = 0.22
INSET = 0.30
STROKE = 0.11
TOP = (37, 99, 235, 255)  # #2563EB
BOTTOM = (59, 130, 246, 255)  # #3B82F6
MARK = (248, 250, 255, 255)  # #F8FAFF
ICO_SIZES = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]


def lerp(a: int, b: int, t: float) -> int:
    return int(round(a + (b - a) * t))


def rounded_mask(size: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, size - 1, size - 1], radius=radius, fill=255
    )
    return mask


def paint_gradient(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size))
    pixels = img.load()
    last = size - 1
    for y in range(size):
        t = y / last
        color = (
            lerp(TOP[0], BOTTOM[0], t),
            lerp(TOP[1], BOTTOM[1], t),
            lerp(TOP[2], BOTTOM[2], t),
            255,
        )
        for x in range(size):
            pixels[x, y] = color
    img.putalpha(rounded_mask(size, int(size * CORNER)))
    return img


def rounded_line(
    draw: ImageDraw.ImageDraw,
    start: tuple[int, int],
    end: tuple[int, int],
    width: int,
    fill: tuple[int, int, int, int],
) -> None:
    draw.line([start, end], fill=fill, width=width)
    radius = max(width // 2, 1)
    for x, y in (start, end):
        draw.ellipse(
            [x - radius, y - radius, x + radius, y + radius],
            fill=fill,
        )


def paint_mark(size: int) -> Image.Image:
    overlay = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)
    pad = int(size * INSET)
    width = max(int(size * STROKE), 2)
    rounded_line(draw, (pad, pad), (size - pad, size - pad), width, MARK)
    rounded_line(draw, (size - pad, pad), (pad, size - pad), width, MARK)
    return overlay


def render_master() -> Image.Image:
    return Image.alpha_composite(paint_gradient(SIZE), paint_mark(SIZE))


def write_svg(path: Path) -> None:
    radius = int(512 * CORNER)
    pad = int(512 * INSET)
    stroke = int(512 * STROKE)
    path.write_text(
        f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
  <defs>
    <linearGradient id="g" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#2563EB"/>
      <stop offset="100%" stop-color="#3B82F6"/>
    </linearGradient>
  </defs>
  <rect width="512" height="512" rx="{radius}" fill="url(#g)"/>
  <g fill="none" stroke="#F8FAFF" stroke-width="{stroke}" stroke-linecap="round">
    <line x1="{pad}" y1="{pad}" x2="{512 - pad}" y2="{512 - pad}"/>
    <line x1="{512 - pad}" y1="{pad}" x2="{pad}" y2="{512 - pad}"/>
  </g>
</svg>
""",
        encoding="utf-8",
    )


def main() -> None:
    out = Path(__file__).resolve().parent
    master = render_master()
    write_svg(out / "logo.svg")
    master.resize((256, 256), Image.Resampling.LANCZOS).save(out / "logo.png")
    master.save(out / "logo.ico", format="ICO", sizes=ICO_SIZES)
    print(f"wrote {out / 'logo.svg'}")
    print(f"wrote {out / 'logo.png'}")
    print(f"wrote {out / 'logo.ico'}")


if __name__ == "__main__":
    main()
