"""Battle Gloves (Hume M hands, 64x64 texels), painted new: cloth gloves.

Brown wrapped canvas hand with raised, stitched finger seams; a deep olive
quilted canvas cuff with vertical gathers, bound at its curved lower edge by a
light canvas trim; a light welt and two stud tabs at the wrist; dark inside.
"""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, CANVAS, LEATHER, NONE, mix
import tex

HAND = [np.array(v, float) for v in ((0x5e, 0x36, 0x1c), (0x93, 0x5a, 0x30), (0xc7, 0x8d, 0x55))]
OLIVE = [np.array(v, float) for v in ((0x2c, 0x2e, 0x1a), (0x4a, 0x4d, 0x2b), (0x6d, 0x71, 0x42))]
TRIM = [np.array(v, float) for v in ((0x9c, 0x80, 0x5c), (0xd0, 0xb3, 0x86), (0xf0, 0xdc, 0xb2))]
SEAM = PAL['seam']
INSIDE = np.array((0x16, 0x10, 0x14), float)

def piping(c, pts, w=0.75, tones=TRIM):
    """raised canvas piping along a polyline: light core, shadow side, stitch holes."""
    for (x0, y0), (x1, y1) in zip(pts[:-1], pts[1:]):
        dx, dy = x1 - x0, y1 - y0; L = np.hypot(dx, dy) or 1
        nx, ny = -dy / L * w, dx / L * w
        m = c.poly_mask([(x0 + nx, y0 + ny), (x1 + nx, y1 + ny), (x1 - nx, y1 - ny), (x0 - nx, y0 - ny)])
        sh = c.poly_mask([(x0 + nx * 1.9 + 0.2, y0 + ny * 1.9 + 0.35), (x1 + nx * 1.9 + 0.2, y1 + ny * 1.9 + 0.35),
                          (x1 + 0.2, y1 + 0.35), (x0 + 0.2, y0 + 0.35)])
        c.img[sh & ~m] *= 0.6
        c.paint(m, tones[1], CANVAS)
        hi = c.poly_mask([(x0 + nx * 0.2, y0 + ny * 0.2 - 0.25), (x1 + nx * 0.2, y1 + ny * 0.2 - 0.25),
                          (x1, y1), (x0, y0)])
        c.paint(hi & m, tones[2], CANVAS)
        c.stitch(x0, y0, x1, y1, col=SEAM, dash=0.6, gap=0.7)

def paint(alpha):
    c = Canvas(64, 64, HAND[1], base_mat=CANVAS)

    # hand: wrapped canvas, banded by the wraps (light top of each wrap, dark underside)
    hand = c.rect(-2, 32, 66, 66)
    c.shade(hand, HAND, 32, 66, light=0.1, shadow=0.1, ink=False, mat=CANVAS)
    for y in range(36, 64, 5):                     # wrap folds
        f = c.poly_mask([(-2, y), (66, y + 1.2), (66, y + 1.9), (-2, y + 0.7)])
        c.paint(f & hand, HAND[0], CANVAS)
        f2 = c.poly_mask([(-2, y + 0.7), (66, y + 1.9), (66, y + 2.6), (-2, y + 1.4)])
        c.paint(f2 & hand, HAND[2], CANVAS)

    # top row: wrist leather, the stud tabs and the dark inside of the cuff
    top = c.rect(-2, -2, 25, 16)
    c.shade(top, PAL['leather'], -2, 16, light=0.15, shadow=0.2, ink=False, mat=LEATHER)
    c.paint(c.rect(8.4, -2, 10.4, 12), TRIM[1], CANVAS); c.outline(c.rect(8.4, -2, 10.4, 12), width=0.3)
    c.paint(c.rect(24.6, -2, 66, 16), INSIDE)
    c.paint(c.rect(25, -2, 66, 1.8), TRIM[1], CANVAS)
    for x0, x1 in ((30.8, 36.6), (39.6, 44.2)):
        tab = c.rect(x0, 1.8, x1, 10.2)
        c.cast(tab, 0.3, 0.8, 0.5)
        c.shade(tab, PAL['leather'], 1.8, 10.2, light=0.25, shadow=0.3, ink=False, mat=LEATHER)
        c.outline(tab, width=0.35)
        c.rivet((x0 + x1) / 2, 7.4, 1.0)
    c.paint(c.rect(44.2, 1.8, 66, 8), PAL['leather'][0], LEATHER)

    # welt across the wrist
    piping(c, [(-2, 17), (66, 17)], w=1.1)

    # cuff: olive quilted canvas with gathers, curved lower edge
    edge = [(-2, 33), (8, 33.4), (10.5, 36), (13.5, 39), (17, 41.3), (21, 42.8), (26, 43.3), (31, 43.2),
            (36, 42.4), (40, 41.2), (42.6, 38.6), (44.4, 35.6), (46, 34.1), (56, 34), (60, 33.2), (66, 33)]
    cuff = c.poly_mask([(-2, 18.2), (66, 18.2)] + edge[::-1])
    c.shade(cuff, OLIVE, 18.2, 42, light=0.14, shadow=0.16, ink=False, mat=CANVAS)
    for x in np.arange(2, 64, 4.3):                # gathers: shadow crease + lit ridge
        cr = c.poly_mask([(x - 0.35, 18.5), (x + 0.35, 18.5), (x + 0.6, 44), (x - 0.1, 44)]) & cuff
        c.paint(cr, OLIVE[0], CANVAS)
        rd = c.poly_mask([(x + 1.4, 18.5), (x + 2.0, 18.5), (x + 2.2, 44), (x + 1.6, 44)]) & cuff
        c.paint(rd, OLIVE[2], CANVAS)
    c.cast(cuff, 0.2, 0.9, 0.55)
    piping(c, edge)

    # finger seams: raised stitched canvas, U-shaped, as on the original
    piping(c, [(10.5, 43.5), (10.5, 56.5), (14, 59), (18.5, 61.6), (25.5, 61.8)], w=0.6)
    piping(c, [(25.5, 46.5), (25.5, 61.8)], w=0.6)
    piping(c, [(25.5, 61.8), (35.5, 61.8)], w=0.6)
    piping(c, [(35.5, 46.5), (35.5, 61.8)], w=0.6)
    piping(c, [(46.5, 44.5), (46.5, 59.5), (42, 61.8), (35.5, 61.8)], w=0.6)

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
