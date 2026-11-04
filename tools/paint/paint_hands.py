"""Centurion's Finger Gauntlets (Hume M hands, 64x64 texels), painted new from BRIEF.md."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, LEATHER, mix

DARKL = [PAL['leather'][0] * 0.45, PAL['leather'][0] * 0.75, PAL['leather'][0] * 1.05]
PALM = [DARKL[0] * 0.7, DARKL[0], DARKL[1]]

def paint(alpha):
    c = Canvas(64, 64, DARKL[1], base_mat=LEATHER)
    am = c.alpha(alpha)
    # glove: dark leather back, darker palm
    c.shade(c.rect(33, -2, 66, 66), DARKL, -2, 66, light=0.08, shadow=0.1, ink=False, mat=LEATHER)
    c.shade(c.rect(36, 47, 66, 66), PALM, 47, 66, light=0.1, shadow=0.2, ink=False, mat=LEATHER)
    back = c.rect(35, 16.2, 57.4, 29.6)
    c.shade(back, DARKL, 16.2, 29.6, light=0.18, shadow=0.15, ink=False, mat=LEATHER)
    c.outline(back, width=0.35); c.stitch(36, 17.4, 56.4, 17.4); c.stitch(36, 28.4, 56.4, 28.4)
    c.strap(35.5, 30.4, 58, 35.6, cast=True); c.rivet(55.6, 33, 0.9)
    c.strap(39, 0.6, 57.4, 10.2)
    # wrist cuff: leather band with a long steel plate and a rivet
    cuff = c.rect(-2, -2, 36, 15.4)
    c.shade(cuff, PAL['leather'], -2, 15.4, light=0.15, shadow=0.15, ink=False, mat=LEATHER)
    c.stitch(-1, 1.2, 35, 1.2); c.stitch(-1, 14, 35, 14)
    p = c.rect(1, 4.8, 28.6, 12.2); c.cast(p, 0.4, 0.8, 0.5); c.metal(p, 4.8, 12.2, 1, 28.6)
    c.rivet(23.6, 8.2, 1.2)
    # wrist lames: band, upper plate row, band, pointed lower row (each lapping the next)
    for x0, x1 in ((33.4, 39), (20, 33.6), (7, 20.4)):          # lower row first (it sits underneath)
        m = c.rect(x0, 42, x1, 66) & am
        c.cast(m, 0.4, 0.9, 0.5); c.metal(m, 42, 60, x0, x1)
    b2 = c.rect(3, 35, 38.6, 43.4) & am
    c.cast(b2, 0.2, 1.2, 0.45); c.metal(b2, 35, 43.4, 3, 38.6, tones=DARK_STEEL, glint=False)
    for x0, x1 in ((19.6, 33.6), (7, 20.4)):
        m = c.rect(x0, 20.6, x1, 36) & am
        c.cast(m, 0.4, 1.0, 0.45); c.metal(m, 20.6, 36, x0, x1)
    b1 = c.rect(3, 15.2, 34, 21.6) & am
    c.cast(b1, 0.2, 1.2, 0.45); c.metal(b1, 15.2, 21.6, 3, 34, tones=DARK_STEEL, glint=False)
    for x in (10.5, 30): c.rivet(x, 18.4, 0.9); c.rivet(x, 39.2, 0.9)
    c.cut_outline(alpha)
    return c.result(alpha)
