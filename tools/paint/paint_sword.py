"""Centurion's Sword (64x64 texels), painted new from BRIEF.md."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, LEATHER, STEEL, mix

DARKL = [PAL['leather'][0] * 0.4, PAL['leather'][0] * 0.7, PAL['leather'][0] * 1.0]
FULLER = [PAL['steel'][0] * 0.75, PAL['steel'][0] * 1.15, PAL['steel'][1] * 0.95]

def paint(alpha):
    c = Canvas(64, 64, DARK_STEEL[1], base_mat=STEEL)
    # grip: dark leather wrap between steel end bands
    grip = c.rect(-2, -2, 66, 8)
    c.shade(grip, DARKL, -2, 8, light=0.25, shadow=0.25, ink=False, mat=LEATHER)
    for x in np.arange(6, 57, 3.2):
        c.paint(c.poly_mask([(x, -2), (x + 0.6, -2), (x + 2.4, 8), (x + 1.8, 8)]) & grip, DARKL[0], LEATHER)
    for x0, x1 in ((-2, 6.6), (55.6, 66)):
        m = c.rect(x0, -2, x1, 8); c.metal(m, 1, 7, x0, x1, glint=False)
    # hilt block: dark steel with bright raised pieces and engraved channels
    c.metal(c.rect(15, 8, 66, 33.4), 8, 33.4, 15, 66, tones=DARK_STEEL, glint=False, ink=False)
    for pts in ([(16, 16.6), (34, 16.6), (34, 26.2), (16, 26.2)],
                [(36, 26), (55, 26), (55, 29.8), (36, 29.8)],
                [(40.8, 18.2), (49.2, 18.2), (49.2, 21), (46.3, 21), (46.3, 26), (43.7, 26), (43.7, 21), (40.8, 21)],
                [(55.2, 8.6), (57.4, 8.6), (57.4, 33), (55.2, 33)],
                [(17, 9.4), (52, 9.4), (52, 13.6), (17, 13.6)]):
        m = c.poly_mask(pts); c.cast(m, 0.3, 0.6, 0.5)
        ys = [p[1] for p in pts]; xs = [p[0] for p in pts]
        c.metal(m, min(ys), max(ys), min(xs), max(xs), glint=False)
    groove = c.rect(18.4, 20.6, 31.6, 22.2); c.paint(groove, PAL['ink'])
    # pommel: faceted diamond
    cx, cy = 7.6, 21.6
    d = c.poly_mask([(cx, 9.6), (cx + 7.6, cy), (cx, 33.4), (cx - 7.6, cy)])
    c.metal(d, 9.6, 33.4, 0, 15.2)
    for tri, col in (([(cx, 9.6), (cx + 7.6, cy), (cx, cy)], mix(PAL['steel'][2], PAL['spec'], 0.4)),
                     ([(cx, cy), (cx + 7.6, cy), (cx, 33.4)], PAL['steel'][0])):
        c.paint(c.poly_mask(tri) & d, col, STEEL)
    c.outline(d)
    # blade: darker fuller down the flat, polished bevelled edge, point at the right
    blade = c.poly_mask([(-2, 34.4), (56, 34.4), (63.6, 49), (56, 63.6), (-2, 63.6)])
    flat = blade & c.rect(-2, 34.4, 66, 48.8)
    c.metal(blade, 34.4, 63.6, -2, 63.6, ink=False)
    c.metal(flat, 34.4, 48.8, -2, 60, tones=FULLER, ink=False)
    c.paint(blade & c.poly_mask([(-2, 48.6), (59.4, 48.6), (59.9, 49.6), (-2, 49.6)]), mix(PAL['steel'][2], PAL['spec'], 0.7))
    c.outline(blade)
    return c.result(alpha)
