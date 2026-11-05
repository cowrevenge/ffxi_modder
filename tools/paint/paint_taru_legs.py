"""Centurion's Cuisses, Tarutaru (128x64 texels), Bastok Pride. Same design as the Hume
cuisses (indigo trousers with folds, stitched thigh strap with two steel buckles, slate
cloth panels, a tasset of overlapping steel lames, knee strap, steel knee cop in a
leather surround, dark cuff) on the Taru layout, read off the Taru original."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, CLOTH, LEATHER, mix, use_cool_leather
use_cool_leather()
from paint_legs import SLATE, DEEP, buckle

def paint(alpha):
    c = Canvas(128, 64, PAL['cloth'][1], base_mat=CLOTH)
    cloth = PAL['cloth']
    # trousers
    c.cloth_panel([(-2, -2), (130, -2), (130, 27), (-2, 27)],
                  folds=[(10, 0, 12, 24), (30, 2, 29, 24), (50, 1, 52, 24), (59, 4, 72, 13), (90, 1, 88, 24), (112, 2, 114, 24)])
    # slate panels either side of the tasset, and the cuff side
    c.cloth_panel([(-2, 31), (38.5, 31), (38.5, 47.5), (-2, 47.5)], tones=SLATE, folds=[(12, 32, 12, 47), (26, 32, 27, 47)])
    c.cloth_panel([(76, 31), (130, 33), (130, 41), (76, 41)], tones=SLATE, folds=[(100, 33, 100, 40)])
    # thigh strap with two buckles, sloping down on the outside
    strap = c.poly_mask([(-2, 26.2), (82, 26.2), (130, 28.2), (130, 33), (82, 31.2), (-2, 31.2)])
    c.cast(strap, 0, 0.8, 0.55)
    c.shade(strap, PAL['leather'], 26.2, 31.2, light=0.22, shadow=0.25, ink=False, mat=LEATHER)
    c.stitch(-1, 27.1, 81, 27.1); c.stitch(-1, 30.3, 81, 30.3); c.stitch(83, 27.3, 129, 29.2)
    c.outline(strap, width=0.3)
    buckle(c, 34.6, 25.4, 39.4, 31.6); buckle(c, 56.4, 25.4, 61.2, 31.6)
    # tasset: dark leather backing, two rows of four overlapping steel lames
    c.shade(c.rect(37.5, 31.2, 77, 47.5), [PAL['leather'][0] * 0.55, PAL['leather'][0] * 0.8, PAL['leather'][0]], 31.2, 47.5, ink=False, mat=LEATHER)
    cols = ((38.6, 48.4), (48.4, 57.4), (57.6, 67.6), (67.6, 76.4))
    for i, top in enumerate((39.2, 32.4)):
        for x0, x1 in reversed(cols):
            c.lame(x0, top, x1, top + 8.4, curve=0.8, lapped=i == 0)
    # knee strap on the outside, strap above the knee
    c.strap(82, 40.6, 130, 47.4, cast=True)
    c.strap(-2, 47.4, 84, 50.6, stitch=False, cast=True)
    # lower leg: slate around the knee, dark cuff
    c.cloth_panel([(36, 50.6), (66, 50.6), (66, 66), (36, 66)], tones=SLATE, folds=[(44, 52, 43, 64), (56, 52, 57, 64)])
    c.cloth_panel([(66, 47.4), (130, 47.4), (130, 66), (66, 66)], tones=DEEP, folds=[(84, 50, 84, 64), (104, 50, 104, 64)])
    # knee cop: hexagonal (mirrored at the bottom edge), leather surround, two rivets
    sur = c.poly_mask([(6, 52.6), (33, 52.6), (36.5, 56.5), (36.5, 66), (2.5, 66), (2.5, 56.5)])
    c.cast(sur, 0.3, 0.7, 0.5)
    c.shade(sur, PAL['leather'], 52.6, 64, light=0.15, shadow=0.15, ink=False, mat=LEATHER)
    c.stitch(7, 54, 32, 54); c.outline(sur, width=0.3)
    cop = c.poly_mask([(11.5, 57.2), (26.5, 57.2), (29.5, 60.4), (29.5, 66), (8.5, 66), (8.5, 60.4)])
    c.cast(cop, 0.3, 0.8, 0.45)
    c.metal(cop, 57.2, 70, 8.5, 29.5)
    for x in (11.6, 26.4): c.rivet(x, 55.4, 0.8)
    c.cut_outline(alpha)
    return c.result(alpha)
