"""Draws the app icon (src-tauri/app-icon.png, 1024 px), then: npx tauri icon src-tauri/app-icon.png

The mark: two crop corners around a dot, white on near-black. It reads as "frame + the moment",
stays legible at 16 px, and needs no color to work.

Usage: python -I scripts/make-icon.py   (needs Pillow)
"""

from pathlib import Path

from PIL import Image, ImageDraw

S = 4096  # draw at 4x, downsample for clean edges
OUT = Path(__file__).resolve().parent.parent / "src-tauri" / "app-icon.png"

img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)

# Rounded square, inset so it sits like other apps' icons on Windows, macOS and Linux.
inset = int(S * 0.06)
d.rounded_rectangle((inset, inset, S - inset, S - inset), radius=int(S * 0.2), fill=(22, 22, 22, 255))

white = (245, 245, 244, 255)
stroke = int(S * 0.075)
arm = int(S * 0.24)
lo, hi = int(S * 0.24), int(S * 0.76)
r = stroke // 2

def bar(x0, y0, x1, y1):
    # a rounded bar of width `stroke` along the segment (x0, y0)-(x1, y1), axis-aligned
    box = (min(x0, x1) - r, min(y0, y1) - r, max(x0, x1) + r, max(y0, y1) + r)
    d.rounded_rectangle(box, radius=r, fill=white)

def corner(x, y, dx, dy):
    # an L whose arms go from (x, y) in direction (dx, dy)
    bar(x, y, x + dx * arm, y)
    bar(x, y, x, y + dy * arm)

corner(lo, lo, 1, 1)  # top-left
corner(hi, hi, -1, -1)  # bottom-right

c = S // 2
dot = int(S * 0.105)
d.ellipse((c - dot, c - dot, c + dot, c + dot), fill=white)

img.resize((1024, 1024), Image.LANCZOS).save(OUT)
print(f"wrote {OUT}")
