"""Render Vibn logo PNGs at multiple sizes (pure stdlib).

Produces: icons/icon.png (256), icons/32x32.png, icons/128x128.png,
icons/128x128@2x.png, plus the full .iconset for `iconutil -c icns`.
"""
import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "icons"
ICONSET = ICONS / "icon.iconset"
ICONS.mkdir(exist_ok=True)
ICONSET.mkdir(exist_ok=True)

# Colors (RGB) — match the SVG gradient stops
VIOLET = (167, 139, 250)
PURPLE = (124, 58, 237)
DEEP   = (76, 29, 149)
WHITE  = (255, 255, 255)


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def gradient(t):
    if t < 0.55:
        return lerp(VIOLET, PURPLE, t / 0.55)
    return lerp(PURPLE, DEEP, (t - 0.55) / 0.45)


def in_rounded_rect(x, y, w, h, r):
    """1.0 inside, 0.0 outside, smoothed at 1-pixel edge."""
    px = max(0, max(r - x, x - (w - 1 - r)))
    py = max(0, max(r - y, y - (h - 1 - r)))
    d = math.hypot(px, py)
    if d <= r - 0.5:
        return 1.0
    if d >= r + 0.5:
        return 0.0
    return r + 0.5 - d  # 0..1


def bars_distance(x, y, size):
    """Min signed distance from (x,y) to the closest pill-shaped bar (0 outside the union).
    Returns a value <= 0 inside any bar (further inside = more negative)."""
    s = size / 64.0
    bar_w = 5.6 * s
    gap = 2.8 * s
    cx = 32 * s
    total_w = bar_w * 5 + gap * 4
    start_x = cx - total_w / 2
    heights = [14, 24, 38, 28, 18]
    cy = 32 * s
    r = bar_w / 2
    best = float("inf")
    for i, hh in enumerate(heights):
        bx = start_x + i * (bar_w + gap) + r  # bar centerline X
        h = hh * s
        # treat bar as a vertical capsule from (bx, cy - h/2 + r) to (bx, cy + h/2 - r) with radius r
        y_min = cy - h / 2 + r
        y_max = cy + h / 2 - r
        dy = 0.0
        if y < y_min:
            dy = y_min - y
        elif y > y_max:
            dy = y - y_max
        dx = abs(x - bx)
        d = math.hypot(dx, dy) - r
        if d < best:
            best = d
    return best


def render(size: int) -> bytes:
    r_corner = 16 * size / 64.0  # rx=16 in 64-unit SVG viewBox

    pixels = bytearray(size * size * 4)
    for y in range(size):
        for x in range(size):
            i = (y * size + x) * 4
            in_bg = in_rounded_rect(x + 0.5, y + 0.5, size, size, r_corner)
            if in_bg <= 0:
                pixels[i:i + 4] = b"\x00\x00\x00\x00"
                continue

            t = (x + y) / (2.0 * size)
            r, g, b = gradient(t)
            a = int(round(255 * in_bg))

            # Bars (white)
            d_bars = bars_distance(x + 0.5, y + 0.5, size)
            fg_alpha = 0.0
            if d_bars <= -0.5:
                fg_alpha = 1.0
            elif d_bars < 0.5:
                fg_alpha = 0.5 - d_bars

            if fg_alpha > 0:
                r = int(r + (255 - r) * fg_alpha)
                g = int(g + (255 - g) * fg_alpha)
                b = int(b + (255 - b) * fg_alpha)

            pixels[i:i + 4] = bytes([r, g, b, a])
    return bytes(pixels)


def write_png(path: Path, size: int, rgba: bytes) -> None:
    sig = b"\x89PNG\r\n\x1a\n"

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    # add filter byte per row (0 = None)
    raw = b"".join(b"\x00" + rgba[y * size * 4:(y + 1) * size * 4] for y in range(size))
    idat = zlib.compress(raw, 9)
    png = sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")
    path.write_bytes(png)


def main() -> None:
    sizes = [16, 32, 64, 128, 256, 512, 1024]
    cache = {}
    for s in sizes:
        print(f"  rendering {s}x{s}")
        cache[s] = render(s)

    # Tauri-expected files
    write_png(ICONS / "icon.png", 256, cache[256])
    write_png(ICONS / "32x32.png", 32, cache[32])
    write_png(ICONS / "128x128.png", 128, cache[128])
    write_png(ICONS / "128x128@2x.png", 256, cache[256])

    # macOS iconset (for iconutil -c icns)
    iconset_map = {
        "icon_16x16.png":      16,
        "icon_16x16@2x.png":   32,
        "icon_32x32.png":      32,
        "icon_32x32@2x.png":   64,
        "icon_128x128.png":    128,
        "icon_128x128@2x.png": 256,
        "icon_256x256.png":    256,
        "icon_256x256@2x.png": 512,
        "icon_512x512.png":    512,
        "icon_512x512@2x.png": 1024,
    }
    for name, s in iconset_map.items():
        write_png(ICONSET / name, s, cache[s])

    # Tray template (just the bars, no background — macOS inverts for menu bar)
    tray = bytearray(32 * 32 * 4)
    for y in range(32):
        for x in range(32):
            i = (y * 32 + x) * 4
            d = bars_distance(x + 0.5, y + 0.5, 32)
            a = 0.0
            if d <= -0.5:
                a = 1.0
            elif d < 0.5:
                a = 0.5 - d
            tray[i:i + 4] = bytes([255, 255, 255, int(round(255 * a))])
    write_png(ICONS / "tray.png", 32, bytes(tray))

    print("done. now run: iconutil -c icns crates/vibn-desktop/icons/icon.iconset -o crates/vibn-desktop/icons/icon.icns")


if __name__ == "__main__":
    main()
