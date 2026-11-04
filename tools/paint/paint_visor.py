"""Centurion's Visor (Hume M head, 64x64 texels), painted new from BRIEF.md."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, LEATHER, mix, use_cool_leather, CHARCOAL
use_cool_leather()

DARKL = [PAL['leather'][0] * 0.45, PAL['leather'][0] * 0.75, PAL['leather'][0] * 1.05]

def paint(alpha):
    c = Canvas(64, 64, DARKL[1], base_mat=LEATHER)
    # brow band: dark leather backing
    band = c.rect(-2, -2, 60, 16.6)
    c.shade(band, DARKL, -2, 16.6, light=0.12, shadow=0.15, ink=False, mat=LEATHER)
    c.stitch(24, 15.2, 49, 15.2)
    # outer plate (stepped lower edge) with rivet
    a = c.poly_mask([(-2, -2), (16.4, -2), (16.4, 12.8), (7.2, 12.8), (4.3, 15.4), (-2, 15.4)])
    c.cast(a, 0.4, 0.8, 0.5); c.metal(a, -2, 15.4, -2, 16.4); c.rivet(13.8, 6.3, 1.2)
    # two narrow bars
    for x0, x1 in ((17, 19.6), (20.8, 23.4)):
        m = c.rect(x0, -2, x1, 11.6); c.cast(m, 0.3, 0.7, 0.5); c.metal(m, -2, 11.6, x0, x1, glint=False)
    # centre plate
    b = c.rect(36, 0.8, 44.4, 14.2); c.cast(b, 0.4, 0.8, 0.5); c.metal(b, 0.8, 14.2, 36, 44.4)
    # dark steel end plate with rivet
    e = c.rect(49.2, -2, 59.4, 15.2); c.cast(e, 0.4, 0.8, 0.5)
    c.metal(e, -2, 15.2, 49.2, 59.4, tones=DARK_STEEL, glint=False); c.rivet(54.5, 7.8, 1.2)
    # cheek guard: L-shaped steel plate
    l = c.poly_mask([(28, 16), (35.2, 16), (35.2, 38.6), (9, 38.6), (9, 30.8), (28, 30.8)])
    c.cast(l, 0.4, 0.9, 0.5); c.metal(l, 16, 38.6, 9, 35.2); c.rivet(31.6, 19.5, 1.0)
    # chin strap down the side and under the chin, joined by a stitched seam
    c.strap(44, 15.5, 60, 57, tones=DARKL, cast=True)
    under = c.rect(-2, 45.8, 48.5, 61.6)
    c.cast(under, 0.2, 0.8, 0.55)
    c.shade(under, DARKL, 45.8, 61.6, light=0.2, shadow=0.2, ink=False, mat=LEATHER)
    c.stitch(-1, 47.2, 47.5, 47.2); c.stitch(-1, 60.2, 47.5, 60.2)
    j = c.poly_mask([(42, 46), (43.6, 45.6), (51, 54.8), (49.4, 55.4)])
    c.paint(j, PAL['leather'][2], LEATHER); c.outline(j, width=0.3)
    c.stitch(42.8, 45.9, 50.2, 55.1)
    c.cut_outline(alpha)
    return c.result(alpha)
