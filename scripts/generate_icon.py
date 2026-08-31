import math
from PIL import Image, ImageDraw, ImageFilter

def create_hollow_canvas_icon(size):
    # Create image with RGBA
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    scale = size / 512.0

    # 1. Background rounded rectangle
    margin = 20 * scale
    r = 100 * scale
    box = [margin, margin, size - margin, size - margin]
    draw.rounded_rectangle(box, radius=r, fill=(10, 15, 30, 255), outline=(35, 48, 80, 255), width=int(max(1, 5 * scale)))

    # 2. Glowing background rings (Mandala / Studio motif)
    cx, cy = size / 2, size / 2
    r_outer = 160 * scale
    draw.ellipse([cx - r_outer, cy - r_outer, cx + r_outer, cy + r_outer], outline=(108, 92, 231, 100), width=int(max(1, 3 * scale)))

    r_mid = 120 * scale
    draw.ellipse([cx - r_mid, cy - r_mid, cx + r_mid, cy + r_mid], outline=(70, 197, 189, 130), width=int(max(1, 2 * scale)))

    # 3. Dynamic Paint Spline Swoosh
    swoosh = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    s_draw = ImageDraw.Draw(swoosh)

    # Cyan Paint Curve
    points_cyan = []
    for t in range(0, 100):
        frac = t / 100.0
        angle = math.pi * 0.2 + frac * math.pi * 1.2
        radius = (90 + math.sin(frac * math.pi) * 60) * scale
        px = cx + math.cos(angle) * radius
        py = cy + math.sin(angle) * radius
        points_cyan.append((px, py))

    if len(points_cyan) > 1:
        for i in range(len(points_cyan) - 1):
            w = int(max(2, (18 - abs(i - 50) * 0.25) * scale))
            s_draw.line([points_cyan[i], points_cyan[i+1]], fill=(70, 235, 220, 220), width=w)

    img = Image.alpha_composite(img, swoosh)

    # 4. Stylus Pen angled at -45 deg
    pen_layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    p_draw = ImageDraw.Draw(pen_layer)

    # Calculate rotated coordinates
    def rot(x, y, angle_rad):
        cos_a = math.cos(angle_rad)
        sin_a = math.sin(angle_rad)
        dx = x - cx
        dy = y - cy
        return (cx + dx * cos_a - dy * sin_a, cy + dx * sin_a + dy * cos_a)

    angle = -math.pi / 4.0

    # Pen Body
    body_poly = [
        rot(cx - 12 * scale, cy - 140 * scale, angle),
        rot(cx + 12 * scale, cy - 140 * scale, angle),
        rot(cx + 10 * scale, cy + 80 * scale, angle),
        rot(cx - 10 * scale, cy + 80 * scale, angle),
    ]
    p_draw.polygon(body_poly, fill=(28, 36, 60, 255), outline=(60, 80, 125, 255))

    # Grip & Gold Ring
    ring_poly = [
        rot(cx - 13 * scale, cy + 30 * scale, angle),
        rot(cx + 13 * scale, cy + 30 * scale, angle),
        rot(cx + 13 * scale, cy + 38 * scale, angle),
        rot(cx - 13 * scale, cy + 38 * scale, angle),
    ]
    p_draw.polygon(ring_poly, fill=(255, 215, 110, 255))

    grip_poly = [
        rot(cx - 13 * scale, cy + 40 * scale, angle),
        rot(cx + 13 * scale, cy + 40 * scale, angle),
        rot(cx + 11 * scale, cy + 90 * scale, angle),
        rot(cx - 11 * scale, cy + 90 * scale, angle),
    ]
    p_draw.polygon(grip_poly, fill=(15, 20, 35, 255), outline=(70, 235, 220, 200))

    # Nib Cone
    nib_cone = [
        rot(cx - 10 * scale, cy + 90 * scale, angle),
        rot(cx + 10 * scale, cy + 90 * scale, angle),
        rot(cx, cy + 145 * scale, angle),
    ]
    p_draw.polygon(nib_cone, fill=(50, 65, 100, 255))

    # Glowing Cyan Nib Tip
    tip_cone = [
        rot(cx - 5 * scale, cy + 125 * scale, angle),
        rot(cx + 5 * scale, cy + 125 * scale, angle),
        rot(cx, cy + 152 * scale, angle),
    ]
    p_draw.polygon(tip_cone, fill=(120, 255, 245, 255))

    # Sparkle at nib
    sparkle_pt = rot(cx, cy + 156 * scale, angle)
    sr = max(1.5, 5 * scale)
    p_draw.ellipse([sparkle_pt[0] - sr, sparkle_pt[1] - sr, sparkle_pt[0] + sr, sparkle_pt[1] + sr], fill=(255, 255, 255, 255))

    img = Image.alpha_composite(img, pen_layer)
    return img

sizes = [16, 32, 48, 64, 128, 256]
images = [create_hollow_canvas_icon(s) for s in sizes]

# Save high-res master PNG
images[-1].save("assets/icon.png", format="PNG")
print("Saved assets/icon.png")

# Save multi-res Windows ICO
images[0].save("assets/icon.ico", format="ICO", sizes=[(s, s) for s in sizes], append_images=images[1:])
print("Saved assets/icon.ico")
