"""Centurion's Axe (64x64 texels), painted new from BRIEF.md."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, LEATHER, STEEL, mix

BLUED = [np.array(v, float) for v in ((0x22, 0x26, 0x4e), (0x3e, 0x47, 0x86), (0x7d, 0x88, 0xc8))]
GOLD = [np.array(v, float) for v in ((0x6b, 0x4a, 0x12), (0xc8, 0x96, 0x2e), (0xf4, 0xd2, 0x7a))]
HAFT = [np.array(v, float) for v in ((0x16, 0x12, 0x10), (0x2a, 0x22, 0x1c), (0x44, 0x38, 0x2c))]
BLACK = [np.array(v, float) for v in ((0x10, 0x10, 0x16), (0x24, 0x25, 0x30), (0x4a, 0x4c, 0x5c))]
GEM = [np.array(v, float) for v in ((0x2a, 0x10, 0x4a), (0x6a, 0x30, 0xa8), (0xc8, 0xa0, 0xf0))]

def scroll(c, pts, w=0.8):
    """gold filigree: a raised gold line with a lit upper side and an ink edge."""
    for (x0, y0), (x1, y1) in zip(pts[:-1], pts[1:]):
        dx, dy = x1 - x0, y1 - y0; L = np.hypot(dx, dy) or 1
        nx, ny = -dy / L * w, dx / L * w
        m = c.poly_mask([(x0 + nx, y0 + ny), (x1 + nx, y1 + ny), (x1 - nx, y1 - ny), (x0 - nx, y0 - ny)])
        c.cast(m, 0.25, 0.4, 0.5)
        c.paint(m, GOLD[1], STEEL)
        hi = c.poly_mask([(x0 + nx * 0.1, y0 - abs(ny) * 0.9), (x1 + nx * 0.1, y1 - abs(ny) * 0.9), (x1, y1), (x0, y0)])
        c.paint(m & hi, GOLD[2], STEEL)
    for p in pts:
        c.paint(c.ellipse_mask(p[0], p[1], w, w), GOLD[1], STEEL)

def arc(cx, cy, rx, ry, a0, a1, n=14):
    a = np.radians(np.linspace(a0, a1, n)); return list(zip(cx + rx * np.cos(a), cy + ry * np.sin(a)))

def paint(alpha):
    c = Canvas(64, 64, HAFT[1], base_mat=LEATHER)
    c.metal(c.rect(-2, -2, 66, 10.4), -2, 10.4, -2, 66, tones=BLACK, glint=False, ink=False)
    # blued head face with a bright ground edge along the top
    face = c.poly_mask([(-2, 12), (10, 10.4), (30, 9.6), (50, 10), (66, 12), (66, 24.6), (-2, 24.6)])
    c.metal(face, 9.6, 24.6, -2, 66, tones=BLUED)
    c.paint(face & c.rect(-2, 9, 66, 11.8), mix(BLUED[2], PAL['spec'], 0.55))
    # ornament plate: dark steel with gold scrollwork round a gem
    c.metal(c.rect(-2, 24.6, 49.6, 46.4), 24.6, 46.4, -2, 49.6, tones=BLACK, glint=False)
    scroll(c, arc(34.4, 35.8, 5.8, 5.4, 0, 360, 28), 0.75)
    for s in (-1, 1):
        scroll(c, arc(34.4 + s * 13, 33, 7.6, 6.2, 180 if s > 0 else 0, 300 if s > 0 else -120))
        scroll(c, arc(34.4 + s * 9, 40.5, 3.4, 3.2, 90, 90 + s * 300))
    scroll(c, [(4, 27.6), (26, 27.6)]); scroll(c, [(43, 27.6), (49, 27.6)])
    gem = c.ellipse_mask(34.4, 35.8, 2.6, 2.6)
    c.metal(gem, 33.2, 38.4, 31.8, 37, tones=GEM, glint=False)
    c.paint(c.ellipse_mask(33.5, 34.8, 0.7, 0.7), PAL['spec'])
    # gold spike: faceted diamond
    cx, cy = 57, 44.2
    d = c.poly_mask([(cx, 33), (cx + 6.4, cy), (cx, 55.4), (cx - 6.4, cy)])
    c.metal(d, 33, 55.4, cx - 6.4, cx + 6.4, tones=GOLD, glint=False)
    c.paint(c.poly_mask([(cx, 33), (cx + 6.4, cy), (cx, cy)]) & d, GOLD[2], STEEL)
    c.paint(c.poly_mask([(cx, cy), (cx - 6.4, cy), (cx, 55.4)]) & d, GOLD[0], STEEL)
    c.outline(d)
    # haft: dark wrapped wood with gold bands
    haft = c.rect(-2, 46.4, 50, 66)
    c.shade(haft, HAFT, 46.4, 66, light=0.2, shadow=0.25, ink=False, mat=LEATHER)
    for x in np.arange(2, 50, 3):
        c.paint(c.poly_mask([(x, 46.4), (x + 0.5, 46.4), (x + 2.2, 66), (x + 1.7, 66)]) & haft, HAFT[0], LEATHER)
    for x0, x1 in ((-2, 12), (49, 53.4)):
        b = c.rect(x0, 53.6, x1, 66); c.metal(b, 53.6, 66, x0, x1, tones=GOLD, glint=False)
    return c.result(alpha)
