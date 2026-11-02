"""Renders the Aurelia application icon (PNG master + multi-size ICO).

Brand: golden-hour "A" prism on warm ink. Run from the repo root:

    python scripts/generate_icon.py
"""

import math

from PIL import Image, ImageChops, ImageDraw, ImageFilter

INK_TOP = (26, 18, 11)
INK_BOTTOM = (11, 8, 6)
BORDER = (62, 47, 37)
GOLD = (246, 196, 90)
AMBER = (232, 150, 63)
CORAL = (228, 112, 95)
TEAL = (63, 167, 150)
PARCHMENT = (243, 231, 214)


def _background(size, scale):
    """Vertical ink gradient inside a rounded square with a hairline border."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    grad = Image.new("RGBA", (1, size), (0, 0, 0, 0))
    for y in range(size):
        t = y / max(1, size - 1)
        grad.putpixel(
            (0, y),
            (
                round(INK_TOP[0] + (INK_BOTTOM[0] - INK_TOP[0]) * t),
                round(INK_TOP[1] + (INK_BOTTOM[1] - INK_TOP[1]) * t),
                round(INK_TOP[2] + (INK_BOTTOM[2] - INK_TOP[2]) * t),
                255,
            ),
        )
    grad = grad.resize((size, size))

    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [16 * scale, 16 * scale, size - 16 * scale, size - 16 * scale],
        radius=108 * scale,
        fill=255,
    )
    img.paste(grad, (0, 0), mask)
    return img, mask


def create_aurelia_icon(size):
    scale = size / 512.0
    img, mask = _background(size, scale)
    draw = ImageDraw.Draw(img)
    cx = cy = size / 2.0

    # Warm ambient bloom behind the mark.
    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    gdraw = ImageDraw.Draw(glow)
    gdraw.ellipse(
        [cx - 165 * scale, cy - 150 * scale, cx + 165 * scale, cy + 180 * scale],
        fill=(GOLD[0], GOLD[1], GOLD[2], 70),
    )
    glow = glow.filter(ImageFilter.GaussianBlur(48 * scale))
    glow.putalpha(ImageChops.multiply(glow.getchannel("A"), img.getchannel("A")))
    img = Image.alpha_composite(img, glow)
    draw = ImageDraw.Draw(img)

    # Symmetry rings (mandala motif).
    r_outer = 172 * scale
    draw.ellipse(
        [cx - r_outer, cy - r_outer, cx + r_outer, cy + r_outer],
        outline=(GOLD[0], GOLD[1], GOLD[2], 90),
        width=max(1, int(3 * scale)),
    )
    r_inner = 132 * scale
    for deg in range(0, 360, 24):
        draw.arc(
            [cx - r_inner, cy - r_inner, cx + r_inner, cy + r_inner],
            start=deg,
            end=deg + 12,
            fill=(AMBER[0], AMBER[1], AMBER[2], 130),
            width=max(1, int(2 * scale)),
        )

    # The "A" prism: two legs + crossbar.
    apex = (cx, cy - 118 * scale)
    left = (cx - 108 * scale, cy + 104 * scale)
    right = (cx + 108 * scale, cy + 104 * scale)
    leg_w = max(2, int(36 * scale))
    draw.line([left, apex, right], fill=GOLD + (255,), width=leg_w, joint="curve")
    draw.line(
        [(cx - 46 * scale, cy + 26 * scale), (cx + 46 * scale, cy + 26 * scale)],
        fill=AMBER + (255,),
        width=max(2, int(24 * scale)),
    )

    # Diagonal stylus across the mark, parchment barrel with a gold grip.
    pen = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    pdraw = ImageDraw.Draw(pen)
    angle = math.radians(45)
    ca, sa = math.cos(angle), math.sin(angle)

    def rot(x, y):
        dx, dy = x - cx, y - cy
        return (cx + dx * ca - dy * sa, cy + dx * sa + dy * ca)

    pdraw.line(
        [rot(cx - 150 * scale, cy + 150 * scale), rot(cx + 96 * scale, cy - 40 * scale)],
        fill=(PARCHMENT[0], PARCHMENT[1], PARCHMENT[2], 235),
        width=max(2, int(15 * scale)),
    )
    grip_a, grip_b = rot(cx + 62 * scale, cy - 6 * scale), rot(cx + 108 * scale, cy - 46 * scale)
    pdraw.line([grip_a, grip_b], fill=GOLD + (255,), width=max(2, int(21 * scale)))
    nib = rot(cx - 178 * scale, cy + 176 * scale)
    pdraw.polygon(
        [
            (nib[0] - 7 * scale, nib[1] - 7 * scale),
            (nib[0] + 7 * scale, nib[1] + 7 * scale),
            nib,
        ],
        fill=CORAL + (255,),
    )
    img = Image.alpha_composite(img, pen)
    draw = ImageDraw.Draw(img)

    # Sparkles (parchment + teal) as four-point stars.
    def sparkle(px, py, r, color):
        draw.polygon(
            [
                (px, py - r),
                (px + r * 0.34, py - r * 0.34),
                (px + r, py),
                (px + r * 0.34, py + r * 0.34),
                (px, py + r),
                (px - r * 0.34, py + r * 0.34),
                (px - r, py),
                (px - r * 0.34, py - r * 0.34),
            ],
            fill=color,
        )

    sparkle(cx + 128 * scale, cy - 150 * scale, 26 * scale, PARCHMENT + (255,))
    sparkle(cx - 150 * scale, cy - 96 * scale, 17 * scale, GOLD + (230,))
    sparkle(cx + 150 * scale, cy + 118 * scale, 12 * scale, TEAL + (220,))

    # Hairline border on top of everything.
    draw.rounded_rectangle(
        [16 * scale, 16 * scale, size - 16 * scale, size - 16 * scale],
        radius=108 * scale,
        outline=BORDER + (255,),
        width=max(1, int(5 * scale)),
    )
    return img


def main():
    sizes = [16, 32, 48, 64, 128, 256]
    master = create_aurelia_icon(sizes[-1])

    master.save("assets/icon.png", format="PNG")
    print("Saved assets/icon.png (256x256)")

    # Pillow derives every ICO entry from the master; `sizes` must be explicit or
    # only a single 16x16 image is written.
    master.save("assets/icon.ico", format="ICO", sizes=[(s, s) for s in sizes])
    print(f"Saved assets/icon.ico ({len(sizes)} sizes)")


if __name__ == "__main__":
    main()
