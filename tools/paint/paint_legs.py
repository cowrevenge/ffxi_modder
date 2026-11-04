"""Centurion's Cuisses (Hume M legs, 128x128 texels), painted new.

Layout read off the original: indigo trousers on top, a leather thigh strap with
two steel buckles, a tasset of overlapping steel lames on the outer thigh with
slate cloth either side, a leather knee strap, a riveted steel knee cop in a
leather surround (mirrored across y=127), and a dark cuff at the bottom.
"""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, CLOTH, LEATHER, mix, rounded_rect
import tex

SLATE = [np.array([0x2c, 0x33, 0x5c], float), np.array([0x45, 0x4f, 0x88], float), np.array([0x63, 0x6f, 0xad], float)]
DEEP = [PAL['cloth'][0] * 0.75, PAL['cloth'][0] * 1.05, PAL['cloth'][1]]

def buckle(c, x0, y0, x1, y1):
    """steel frame buckle with a dark slot and a tongue."""
    m = c.rect(x0, y0, x1, y1)
    c.cast(m, 0.3, 0.7, 0.5)
    c.metal(m, y0, y1, x0, x1, glint=False)
    slot = c.rect(x0 + 1.1, y0 + 1.6, x1 - 1.1, y1 - 1.6)
    c.paint(slot, PAL['leather'][0] * 0.6, LEATHER)
    c.outline(slot, width=0.3)
    tongue = c.rect((x0 + x1) / 2 - 0.4, y0 + 1.6, (x0 + x1) / 2 + 0.4, y1 - 1.6)
    c.paint(tongue, PAL['steel'][1])

def paint(alpha):
    c = Canvas(128, 128, PAL['cloth'][1], base_mat=CLOTH)
    cloth = PAL['cloth']

    # trousers: indigo, long vertical folds, a darker gusset fold (x 64-72, y 16-28)
    c.cloth_panel([(-2, -2), (130, -2), (130, 56), (-2, 56)],
                  folds=[(10, 0, 12, 50), (30, 4, 28, 50), (50, 2, 52, 50), (88, 2, 86, 50), (100, 30, 98, 52), (118, 34, 120, 52)])
    c.paint(c.poly_mask([(63, 14), (65, 14), (73, 30), (71, 30)]), cloth[0], CLOTH)

    # thigh strap with two steel buckles; on the right it runs down toward the knee
    strap = c.poly_mask([(-2, 53), (80, 53), (104, 57.5), (130, 59.5), (130, 66.5), (96, 62.5), (80, 61.5), (-2, 61.5)])
    c.cast(strap, 0, 0.9, 0.55)
    c.shade(strap, PAL['leather'], 53, 62, light=0.22, shadow=0.25, ink=False, mat=LEATHER)
    c.stitch(-1, 54.2, 79, 54.2); c.stitch(-1, 60.3, 79, 60.3)
    c.stitch(81, 54.4, 104, 58.7); c.stitch(104, 58.7, 129, 60.7)
    c.outline(strap, width=0.35)
    buckle(c, 35.2, 52.3, 40.4, 62.2); buckle(c, 56.6, 52.3, 61.8, 62.2)

    # slate cloth panels either side of the tasset
    c.cloth_panel([(-2, 62), (38, 62), (38, 93), (-2, 93)], tones=SLATE, folds=[(12, 64, 12, 92), (28, 64, 29, 92)])
    c.cloth_panel([(78, 63), (96, 63), (130, 67), (130, 84), (78, 84)], tones=SLATE, folds=[(100, 66, 100, 83)])
    # knee strap on the right
    ks = c.strap(80, 83.5, 130, 92.5, cast=True)

    # tasset: dark leather backing, three rows of four overlapping steel lames
    back = c.rect(37, 62, 78, 94)
    c.shade(back, [PAL['leather'][0] * 0.55, PAL['leather'][0] * 0.8, PAL['leather'][0]], 62, 94, ink=False, mat=LEATHER)
    cols = ((39, 48.8), (48.4, 58), (57.6, 67.6), (67.2, 76.5))
    for i, top in enumerate((82.5, 73.5, 64.5)):
        for x0, x1 in reversed(cols):
            c.lame(x0, top, x1, top + 11, curve=1.0, lapped=i < 2)
    for x in (39.8, 75.7):
        c.rivet(x, 63.6, 0.9)

    # strap above the knee
    c.strap(-2, 93, 82, 104.5, cast=True)
    c.strap(82, 92.5, 130, 104.5, tones=DEEP, stitch=False)

    # lower leg: slate cloth around the knee, dark cuff on the right
    c.cloth_panel([(36, 104.5), (82, 104.5), (82, 130), (36, 130)], tones=SLATE, folds=[(46, 106, 44, 128), (58, 106, 60, 128)])
    c.cloth_panel([(82, 104.5), (130, 104.5), (130, 130), (82, 130)], tones=DEEP, folds=[(96, 106, 96, 128), (112, 106, 112, 128)])

    # knee cop: leather surround, steel cop with a rolled rim, mirrored across y=127
    sur = c.poly_mask(rounded_rect(3, 105.5, 36, 140, 4))
    c.cast(sur, 0.3, 0.8, 0.5)
    c.shade(sur, PAL['leather'], 105.5, 127, light=0.15, shadow=0.15, ink=False, mat=LEATHER)
    c.stitch(5, 107.3, 34, 107.3); c.stitch(4.6, 108, 4.6, 129); c.stitch(34.4, 108, 34.4, 129)
    c.outline(sur, width=0.35)
    cop = c.poly_mask(rounded_rect(9.5, 112.5, 29, 142, 3))
    c.cast(cop, 0.3, 0.9, 0.45)
    c.metal(cop, 112.5, 142, 9.5, 29)
    rim = c.poly_mask(rounded_rect(10.2, 113.2, 28.3, 142, 2.6)) & ~c.poly_mask(rounded_rect(11.4, 114.4, 27.1, 142, 2))
    c.paint(rim, mix(PAL['steel'][2], PAL['spec'], 0.4))
    for x in (12.2, 26.3): c.rivet(x, 116.6, 1.0)

    c.cut_outline(alpha)
    return c.result(alpha)

if __name__ == '__main__':
    out_dir, dat = sys.argv[1], sys.argv[2]
    os.makedirs(out_dir, exist_ok=True)
    name, img = tex.textures(dat)[0]
    rgba = paint(img[..., 3])
    key = name.split()[-1]
    raw = os.path.join(out_dir, key + '.rgba'); rgba.tofile(raw)
    from PIL import Image
    Image.fromarray(rgba[..., :3]).save(os.path.join(out_dir, key + '.png'))
    print(dat, f"{key}={raw}:{rgba.shape[1]}x{rgba.shape[0]}")
