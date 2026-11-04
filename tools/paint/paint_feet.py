"""Centurion's Greaves (Hume M feet, 128x64 texels), painted new from BRIEF.md."""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, LEATHER, CLOTH, mix, rounded_rect, use_cool_leather, CHARCOAL
use_cool_leather()

PAD = PAL['cloth']                                  # same indigo as the trousers
FOOT = PAL['cloth']                                 # shoe: trouser indigo too
CALF = PAL['leather']

def panel(c, x0, y0, x1, y1, r=1.2, tones=PAD, mat=CLOTH, quilt=True):
    """dark padded panel: quilted with a lit upper edge, sitting under the straps."""
    m = c.poly_mask(rounded_rect(x0, y0, x1, y1, r))
    c.shade(m, tones, y0, y1, light=0.12, shadow=0.18, ink=False, mat=mat)
    for y in (np.arange(y0 + 4.5, y1 - 1, 5) if quilt else ()):
        c.paint(m & c.rect(x0, y, x1, y + 0.45), tones[0], mat)
        c.paint(m & c.rect(x0, y + 0.45, x1, y + 0.8), tones[2], mat)
    c.outline(m, width=0.35)

def paint(alpha):
    c = Canvas(128, 64, PAL['leather'][1], base_mat=LEATHER)
    am = c.alpha(alpha)
    c.shade(c.rect(-2, -2, 130, 66), PAL['leather'], -2, 66, light=0.05, shadow=0.05, ink=False, mat=LEATHER)
    # calf: plain creased brown leather at the back
    calf = c.rect(84.4, 23.2, 130, 66)
    c.shade(calf, CALF, 23.2, 66, light=0.12, shadow=0.15, ink=False, mat=LEATHER)
    for x in (92, 103, 116):
        c.paint(c.poly_mask([(x - 0.3, 26), (x + 0.3, 26), (x + 1.3, 62), (x + 0.7, 62)]), CALF[0], LEATHER)
    # padded panels between the straps (strap positions and widths as on the original)
    panel(c, 7.2, -2, 25, 17.4, quilt=False)                    # shoe upper
    for x0, x1 in ((33, 49.5), (52.6, 63.6), (66.6, 82), (90, 119)):
        panel(c, x0, -2, x1, 17.4)
    panel(c, 7.2, 23.2, 25, 66, r=2.2, quilt=False)             # shoe
    c.paint(c.rect(19.4, 10.4, 22.6, 13.6), PAL['leather'][0] * 0.7, LEATHER)   # underside of the instep (one small triangle samples here)
    heel = c.rect(17.6, 38, 25, 66)                             # heel and sole block: leather, as on the original
    c.shade(heel, PAL['leather'], 38, 66, light=0.12, shadow=0.15, ink=False, mat=LEATHER)
    c.outline(heel, width=0.35); c.stitch(18.6, 39, 18.6, 65)
    for x0, x1 in ((31, 49.5), (52.6, 63.6), (66.6, 82)):
        panel(c, x0, 23.2, x1, 66)
    # vertical straps (laid over the panel edges)
    for x0, x1, y0, y1 in ((-2, 7.2, -2, 66), (25, 33, -2, 17.4), (25, 31, 23.2, 66), (49.5, 52.6, -2, 66),
                           (63.6, 66.6, -2, 66), (82, 90, -2, 17.4), (82, 84.4, 23.2, 66), (119, 130, -2, 17.4)):
        c.strap(x0, y0, x1, y1, stitch=(x1 - x0) > 4, cast=True)
    # broad strap round the top of the shin, steel buckle and a rivet
    top = c.rect(-2, 17.4, 130, 23.2)
    c.cast(top, 0, 0.9, 0.5)
    c.shade(top, PAL['leather'], 17.4, 23.2, light=0.22, shadow=0.22, ink=False, mat=LEATHER)
    c.stitch(-1, 18.4, 129, 18.4); c.stitch(-1, 22.2, 129, 22.2); c.outline(top, width=0.35)
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
