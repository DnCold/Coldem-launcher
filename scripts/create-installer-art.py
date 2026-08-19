from pathlib import Path

from PIL import Image, ImageDraw, ImageFont, ImageFilter


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "src" / "assets"
OUT = ROOT / "src-tauri" / "windows"
OUT.mkdir(parents=True, exist_ok=True)


def font(size: int, bold: bool = False):
    candidates = (
        [
            r"C:\Windows\Fonts\segoeuib.ttf",
            r"C:\Windows\Fonts\arialbd.ttf",
        ]
        if bold
        else [r"C:\Windows\Fonts\segoeui.ttf", r"C:\Windows\Fonts\arial.ttf"]
    )
    for candidate in candidates:
        if Path(candidate).exists():
            return ImageFont.truetype(candidate, size)
    return ImageFont.load_default()


def gradient(size, top, bottom):
    width, height = size
    image = Image.new("RGB", size)
    pixels = image.load()
    for y in range(height):
        amount = y / max(height - 1, 1)
        color = tuple(int(top[i] * (1 - amount) + bottom[i] * amount) for i in range(3))
        for x in range(width):
            pixels[x, y] = color
    return image.convert("RGBA")


def runner(size):
    source = Image.open(ASSETS / "runner-head.png").convert("RGBA")
    source.thumbnail((size, size), Image.Resampling.LANCZOS)
    return source


def neon_line(layer, points, fill, width=2):
    glow = Image.new("RGBA", layer.size, (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    glow_draw.line(points, fill=(*fill, 110), width=width * 5, joint="curve")
    glow = glow.filter(ImageFilter.GaussianBlur(width * 3))
    layer.alpha_composite(glow)
    ImageDraw.Draw(layer).line(points, fill=(*fill, 220), width=width, joint="curve")


def make_header():
    scale = 4
    size = (150 * scale, 57 * scale)
    image = gradient(size, (10, 11, 27), (33, 12, 55))
    draw = ImageDraw.Draw(image)
    neon_line(image, [(0, 188), (600, 30)], (146, 61, 255), 2 * scale // 2)
    neon_line(image, [(360, 228), (600, 92)], (173, 241, 66), 1 * scale // 2)
    draw.rectangle((0, 0, size[0] - 1, size[1] - 1), outline=(110, 72, 175, 220), width=2)

    mark = runner(43 * scale)
    image.alpha_composite(mark, (10 * scale, 7 * scale))

    title_font = font(18 * scale, bold=True)
    small_font = font(6 * scale, bold=True)
    draw.text((57 * scale, 12 * scale), "COLDEM", fill=(247, 244, 255), font=title_font)
    draw.text((58 * scale, 35 * scale), "STAY COLD  •  PLAY HARD", fill=(181, 238, 72), font=small_font)

    image.resize((150, 57), Image.Resampling.LANCZOS).convert("RGB").save(OUT / "installer-header.bmp")


def make_sidebar():
    scale = 3
    size = (164 * scale, 314 * scale)
    image = gradient(size, (8, 9, 24), (37, 10, 57))
    overlay = Image.new("RGBA", size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)

    # Subtle technical grid and sticker-like accents.
    for x in range(0, size[0], 18 * scale):
        draw.line((x, 0, x, size[1]), fill=(86, 55, 122, 35), width=1)
    for y in range(0, size[1], 18 * scale):
        draw.line((0, y, size[0], y), fill=(86, 55, 122, 35), width=1)
    neon_line(overlay, [(0, 52 * scale), (164 * scale, 18 * scale)], (142, 58, 255), 1 * scale)
    neon_line(overlay, [(0, 255 * scale), (164 * scale, 303 * scale)], (174, 240, 65), 1 * scale)
    overlay.alpha_composite(runner(106 * scale), (28 * scale, 20 * scale))
    image.alpha_composite(overlay)

    draw = ImageDraw.Draw(image)
    title_font = font(21 * scale, bold=True)
    body_font = font(8 * scale)
    body_bold = font(8 * scale, bold=True)
    draw.text((15 * scale, 143 * scale), "COLDEM", fill=(248, 245, 255), font=title_font)
    draw.text((16 * scale, 169 * scale), "THE DAN COLD GAME HUB", fill=(179, 240, 65), font=body_bold)
    draw.line((16 * scale, 190 * scale, 148 * scale, 190 * scale), fill=(130, 73, 185), width=1 * scale)
    draw.text((16 * scale, 203 * scale), "Browse. Install. Play.", fill=(227, 224, 239), font=body_font)
    draw.text((16 * scale, 220 * scale), "Your worlds, always ready.", fill=(227, 224, 239), font=body_font)
    draw.text((16 * scale, 270 * scale), "DANCOLD  /  2026", fill=(156, 139, 185), font=body_font)

    image.resize((164, 314), Image.Resampling.LANCZOS).convert("RGB").save(OUT / "installer-sidebar.bmp")


if __name__ == "__main__":
    make_header()
    make_sidebar()
