"""Centurion's Greaves (Hume M feet, 128x64 texels), painted new from BRIEF.md."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, LEATHER, CLOTH, mix, rounded_rect

PAD = [PAL['cloth'][0] * 0.6, PAL['cloth'][0] * 0.88, PAL['cloth'][1] * 0.72]
FOOT = [PAL['leather'][0] * 0.4, PAL['leather'][0] * 0.7, PAL['leather'][0] * 1.0]
CALF = [np.array(v, float) for v in ((0x4a, 0x27, 0x16), (0x7c, 0x46, 0x26), (0xab, 0x6c, 0x3e))]

def panel(c, x0, y0, x1, y1, r=1.2, tones=PAD, mat=CLOTH):
    """dark padded panel: quilted with a lit upper edge, sitting under the straps."""
    m = c.poly_mask(rounded_rect(x0, y0, x1, y1, r))
    c.shade(m, tones, y0, y1, light=0.12, shadow=0.18, ink=False, mat=mat)
    for y in np.arange(y0 + 4.5, y1 - 1, 5):
        c.paint(m & c.rect(x0, y, x1, y + 0.45), tones[0], mat)
        c.paint(m & c.rect(x0, y + 0.45, x1, y + 0.8), tones[2], mat)
    c.outline(m, width=0.35)

def paint(alpha):
    c = Canvas(128, 64, PAL['leather'][1], base_mat=LEATHER)
    am = c.alpha(alpha)
    c.shade(c.rect(-2, -2, 130, 66), PAL['leather'], -2, 66, light=0.05, shadow=0.05, ink=False, mat=LEATHER)
    # calf: plain creased brown leather at the back
    calf = c.rect(83, 23.6, 130, 66)
    c.shade(calf, CALF, 23.6, 66, light=0.12, shadow=0.15, ink=False, mat=LEATHER)
    for x in (92, 103, 116):
        c.paint(c.poly_mask([(x - 0.3, 26), (x + 0.3, 26), (x + 1.3, 62), (x + 0.7, 62)]), CALF[0], LEATHER)
    # padded panels between the straps
    for x0, x1 in ((9, 16), (33, 48), (52, 60), (66, 80), (89, 110), (113, 119)):
        panel(c, x0, -2, x1, 16.6)
    panel(c, 8.6, 25, 24, 66, r=2.2, tones=FOOT, mat=LEATHER)      # the foot: dark leather
    for x0, x1 in ((31, 48), (53, 61), (67, 80)):
        panel(c, x0, 25, x1, 66)
    # vertical straps (laid over the panel edges)
    for x0, x1, y0, y1 in ((-2, 8.8, -2, 66), (16, 24, -2, 17), (30, 33, -2, 17), (48, 52, -2, 66), (60, 66, -2, 17),
                           (61, 67, 23.6, 66), (80, 89, -2, 17), (80, 83.4, 23.6, 66), (110, 113, -2, 17), (119, 130, -2, 17), (24, 31, 23.6, 66)):
        c.strap(x0, y0, x1, y1, stitch=(x1 - x0) > 4, cast=True)
    # broad strap round the top of the shin, steel buckle and a rivet
    top = c.rect(-2, 16.4, 130, 23.6)
    c.cast(top, 0, 0.9, 0.5)
    c.shade(top, PAL['leather'], 16.4, 23.6, light=0.22, shadow=0.22, ink=False, mat=LEATHER)
    c.stitch(-1, 17.5, 129, 17.5); c.stitch(-1, 22.5, 129, 22.5); c.outline(top, width=0.35)
    fr = c.rect(97, 10.4, 106.4, 20.6); c.cast(fr, 0.3, 0.8, 0.5); c.metal(fr, 10.4, 20.6, 97, 106.4, glint=False)
    slot = c.rect(98.8, 12.4, 104.6, 18.6); c.paint(slot, PAL['leather'][0], LEATHER); c.outline(slot, width=0.3)
    c.paint(c.rect(101.3, 12.4, 102.1, 18.6), PAL['steel'][1])
    c.rivet(85, 19.8, 1.2)
    # shin plate: winged steel, dark steel top rail with two rivets; the cut-outs give the wings
    plate = c.rect(27, 46.6, 57, 66) & am
    c.cast(plate, 0.4, 0.9, 0.45); c.metal(plate, 46.6, 66, 27, 57)
    rail = c.rect(27, 46.6, 57, 50.4) & am
    c.metal(rail, 46.6, 50.4, 27, 57, tones=DARK_STEEL, glint=False)
    c.rivet(30.6, 48.5, 1.0); c.rivet(49.4, 48.6, 1.0)
    c.cut_outline(alpha)
    return c.result(alpha)
