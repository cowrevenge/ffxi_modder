"""Bounding Boots (Hume M feet, 128x64 texels), painted new: leather boots.

Saddle-leather shaft with a dark top rim, a stitched horizontal seam and four
rivets; a light tan toe cap and heel; a tan knee tab with two rivets; a steel
ladder buckle on tan leather; an indigo lining strip; dark soles.
"""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, LEATHER, CLOTH, mix, rounded_rect
import tex

SADDLE = [np.array(v, float) for v in ((0x4a, 0x27, 0x16), (0x7c, 0x46, 0x26), (0xab, 0x6c, 0x3e))]
TAN = [np.array(v, float) for v in ((0x8a, 0x5c, 0x36), (0xc4, 0x8f, 0x5c), (0xe8, 0xbe, 0x86))]
DARK = [SADDLE[0] * 0.45, SADDLE[0] * 0.75, SADDLE[0]]
SOLE = [np.array(v, float) for v in ((0x1a, 0x12, 0x10), (0x2e, 0x22, 0x1c), (0x45, 0x34, 0x2a))]

def welt(c, x0, y, x1, tones=SADDLE):
    """raised stitched seam across the leather."""
    m = c.rect(x0, y - 0.8, x1, y + 0.8)
    c.cast(m, 0, 0.6, 0.55)
    c.paint(m, tones[1], LEATHER)
    c.paint(c.rect(x0, y - 0.8, x1, y - 0.25), tones[2], LEATHER)
    c.paint(c.rect(x0, y + 0.45, x1, y + 0.8), tones[0], LEATHER)
    c.stitch(x0 + 0.5, y, x1 - 0.5, y)

def paint(alpha):
    c = Canvas(128, 64, SADDLE[1], base_mat=LEATHER)

    # shaft
    shaft = c.rect(-2, -2, 88, 57)
    c.shade(shaft, SADDLE, -2, 57, light=0.16, shadow=0.12, ink=False, mat=LEATHER)
    for x in (34, 58, 72):                         # creases down the shaft
        cr = c.poly_mask([(x - 0.3, 6), (x + 0.3, 6), (x + 1.2, 34), (x + 0.6, 34)])
        c.paint(cr, SADDLE[0], LEATHER)
    rim = c.rect(26, -2, 86, 4)
    c.shade(rim, DARK, -2, 4, light=0.3, shadow=0.3, ink=False, mat=LEATHER)
    c.stitch(27, 4.9, 85, 4.9)
    welt(c, 27, 36.6, 86.5)
    c.stitch(86.6, 38, 86.6, 56.5)
    for x, y in ((44.6, 29.8), (52.8, 29.8), (78.4, 30.6), (86.8, 32)):
        c.rivet(x, y, 1.2)

    # sole and heel area on the left
    c.shade(c.rect(-2, 22, 16, 34.6), DARK, 22, 34.6, light=0.15, shadow=0.2, ink=False, mat=LEATHER)   # instep
    c.shade(c.rect(-2, 53, 36, 66), SOLE, 53, 66, light=0.15, shadow=0.2, ink=False, mat=LEATHER)
    heel = c.poly_mask([(-2, 34.6), (9, 34.6), (12, 38), (16, 43.8), (22, 44.4), (30, 45.8), (34, 49.5), (35.5, 53.4), (-2, 53.4)])
    c.cast(heel, 0.3, 0.8, 0.5)
    c.shade(heel, TAN, 34.6, 53.4, light=0.2, shadow=0.2, ink=False, mat=LEATHER)
    c.outline(heel, width=0.35)
    c.stitch(0, 51.6, 33.5, 51.6)
    c.rivet(3.6, 38.2, 0.9)
    # toe cap
    toe = c.poly_mask([(-2, -2), (17.5, -2), (17.5, 15), (12.5, 18.4), (4, 21.6), (-2, 23)])
    c.cast(toe, 0.3, 0.9, 0.5)
    c.shade(toe, TAN, -2, 23, light=0.25, shadow=0.15, ink=False, mat=LEATHER)
    c.outline(toe, width=0.35)
    c.stitch(16.2, 0, 16.2, 14.3); c.stitch(16.2, 14.3, 11.8, 17.3); c.stitch(11.8, 17.3, 3.8, 20.2)
    # dark sole underside along the bottom (mirrored across the chevron seam)
    c.shade(c.rect(28, 55.5, 80, 66), SOLE, 55.5, 66, ink=False, mat=LEATHER)

    # knee tab: tan leather, rounded bottom, two rivets
    tab = c.poly_mask(rounded_rect(85, -6, 104, 24.5, 4))
    c.cast(tab, 0.4, 0.9, 0.5)
    c.shade(tab, TAN, -2, 24.5, light=0.2, shadow=0.18, ink=False, mat=LEATHER)
    c.stitch(86.6, 0, 86.6, 20); c.stitch(102.4, 0, 102.4, 20); c.stitch(89, 23, 100, 23)
    c.outline(tab, width=0.35)
    c.rivet(92, 17.2, 1.3); c.rivet(100, 17.2, 1.3)

    # lining strip and dark leather behind the buckle
    # wrap band: dark leather strips wound round the leg (no cloth on these boots)
    wrap = c.rect(96, 24.5, 104.5, 66)
    c.shade(wrap, DARK, 24.5, 66, light=0.1, shadow=0.1, ink=False, mat=LEATHER)
    for x in (98.6, 101.4):
        c.paint(c.poly_mask([(x - 0.3, 24.5), (x + 0.3, 24.5), (x + 0.3, 66), (x - 0.3, 66)]), SOLE[0], LEATHER)
        c.paint(c.poly_mask([(x + 0.3, 24.5), (x + 0.8, 24.5), (x + 0.8, 66), (x + 0.3, 66)]), DARK[2], LEATHER)
    c.shade(c.rect(104.5, -2, 109, 66), DARK, -2, 66, ink=False, mat=LEATHER)
    c.shade(c.rect(109, 36, 125, 66), DARK, 36, 66, light=0.1, ink=False, mat=LEATHER)
    c.paint(c.rect(124.6, -2, 130, 66), SOLE[0])

    # buckle: tan leather backing with a rounded foot and a rivet, steel ladder frame
    back = c.poly_mask(rounded_rect(109, -6, 124.6, 37, 4))
    c.cast(back, 0.4, 0.9, 0.5)
    c.shade(back, TAN, -2, 37, light=0.15, shadow=0.15, ink=False, mat=LEATHER)
    c.outline(back, width=0.35)
    frame = c.rect(110.2, -2, 123.4, 24.2)
    c.metal(frame, -2, 24.2, 110.2, 123.4, glint=False, ink=False)
    for y0, y1 in ((5.2, 12.8), (15.2, 22.2)):
        slot = c.rect(112.2, y0, 121.4, y1)
        c.paint(slot, SOLE[1], LEATHER)
        c.outline(slot, width=0.35)
        c.paint(c.rect(112.2, y1 - 1.1, 121.4, y1), TAN[0], LEATHER)       # strap seen through
    c.outline(frame, width=0.35)
    c.rivet(117.8, 30.2, 1.4)

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
