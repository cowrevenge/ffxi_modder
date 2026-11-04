"""Centurion's Scale Mail (Hume M body, 128x256 texels), painted new from BRIEF.md.

All plating is stacked: rows are drawn bottom-up so every row laps over the top
of the row below it and throws a shadow onto it; scales sit in an offset brick
pattern; lames are held by riveted leather straps laid over them.
"""
import sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paint import Canvas, PAL, DARK_STEEL, CLOTH, rounded_rect
import tex

BRASS = [np.array(v, float) for v in ((0x6b, 0x4a, 0x12), (0xc8, 0x96, 0x2e), (0xf4, 0xd2, 0x7a))]
IVORY = np.array((0xf3, 0xe8, 0xd0), float)

def emblem(c, cx, cy, r=4.4):
    """small brass medallion with a raised cow's head, symmetric about cx (the
    mirror seam), so the mirrored halves make one whole emblem."""
    from paint import mix
    k = r / 4.4
    disk = c.ellipse_mask(cx, cy, r, r)
    c.cast(disk, 0.35, 0.6, 0.45)
    c.metal(disk, cy - r, cy + r, cx - r, cx + r, tones=BRASS, glint=False)
    c.mat[disk] = 0      # polished: no scratches over the relief
    inner = c.ellipse_mask(cx, cy, r * 0.8, r * 0.8)
    c.paint(disk & ~inner & c.poly_mask([(cx - k * 9, cy + k * 9), (cx + k * 9, cy - k * 9), (cx + k * 9, cy + k * 9)]), BRASS[0])
    c.outline(inner, width=0.18, col=BRASS[0])
    ink = PAL['ink']
    # horns: ivory crescents curling up and out
    for s_ in (-1, 1):
        horn = c.poly_mask([(cx + s_ * k * 1.1, cy - k * 1.7), (cx + s_ * k * 2.2, cy - k * 2.4), (cx + s_ * k * 2.9, cy - k * 3.2),
                            (cx + s_ * k * 3.05, cy - k * 2.5), (cx + s_ * k * 2.4, cy - k * 1.6), (cx + s_ * k * 1.4, cy - k * 1.1)])
        c.paint(horn, IVORY); c.outline(horn, width=0.15)
        ear = c.poly_mask([(cx + s_ * k * 1.3, cy - k * 1.0), (cx + s_ * k * 2.7, cy - k * 1.1), (cx + s_ * k * 2.5, cy - k * 0.3), (cx + s_ * k * 1.3, cy - k * 0.2)])
        c.paint(ear, BRASS[1]); c.outline(ear, width=0.15)
    # head: broad brow narrowing to the muzzle
    head = c.poly_mask([(cx - k * 1.5, cy - k * 1.9), (cx + k * 1.5, cy - k * 1.9), (cx + k * 1.6, cy - k * 0.4), (cx + k * 1.15, cy + k * 1.6),
                        (cx - k * 1.15, cy + k * 1.6), (cx - k * 1.6, cy - k * 0.4)])
    c.paint(head, BRASS[2]); c.paint(head & c.rect(cx - k * 3, cy + k * 0.2, cx + k * 3, cy + k * 2), BRASS[1])
    c.outline(head, width=0.15)
    spot = c.poly_mask([(cx, cy - k * 1.7), (cx + k * 0.6, cy - k * 1.0), (cx, cy - k * 0.3), (cx - k * 0.6, cy - k * 1.0)])
    c.paint(spot, BRASS[0])
    muzzle = c.ellipse_mask(cx, cy + k * 1.75, k * 1.45, k * 0.95)
    c.paint(muzzle, IVORY); c.outline(muzzle, width=0.15)
    for s_ in (-1, 1):
        c.paint(c.ellipse_mask(cx + s_ * k * 0.55, cy + k * 1.85, k * 0.22, k * 0.28), ink)    # nostrils
        c.paint(c.ellipse_mask(cx + s_ * k * 0.8, cy - k * 0.5, k * 0.24, k * 0.24), ink)      # eyes
        c.paint(c.ellipse_mask(cx + s_ * k * 0.75, cy - k * 0.58, k * 0.08, k * 0.08), PAL['spec'])

def paint(alpha):
    c = Canvas(128, 256, PAL['cloth'][1], base_mat=CLOTH)
    cloth = PAL['cloth']

    # collar roll: indigo cloth rolled over a curved neckline. Its lower edge is
    # the arch A(x) (high over the middle, lower at the seams), shaded along the
    # curve so it reads as a round roll, not a flat band.
    A = lambda x: 12 + 3.5 * ((x - 64) / 64) ** 2           # top of the leather neck band
    B = lambda x: 20 + 2.5 * ((x - 64) / 64) ** 2           # bottom of the leather neck band
    xs = np.linspace(-2, 130, 67)
    roll = c.poly_mask([(-2, 0), (130, 0)] + [(x, A(x) + 0.5) for x in xs[::-1]])
    Y = np.broadcast_to(c.rows(), roll.shape); X = np.broadcast_to(c.cols(), roll.shape)
    t = Y / (12 + 3.5 * ((X - 64) / 64) ** 2)
    c.paint(roll, cloth[1], CLOTH)
    c.paint(roll & (t > 0.18) & (t < 0.42), cloth[2], CLOTH)    # lit crown of the roll
    c.paint(roll & (t > 0.78), cloth[0], CLOTH)                  # tucked under the band
    for fx in (14, 40, 88, 114):                                 # soft folds bending with the curve
        c.paint(c.poly_mask([(fx - 0.3, 1), (fx + 0.3, 1), (fx + 1.6 * np.sign(fx - 64) + 0.3, A(fx) - 1), (fx + 1.6 * np.sign(fx - 64) - 0.3, A(fx) - 1)]) & roll, cloth[0], CLOTH)

    # neck ring (texels x 0-18, y 0-9 wrap around the neck: x runs from the outer
    # rim to the inside of the ring, y runs around it): a rolled leather collar,
    # light on its crown, stitched round, dark inside the opening
    L_ = PAL['leather']
    ring = c.rect(-2, -1, 19, 9.6)
    for x0, x1, col in ((-2, 2, L_[0]), (2, 5.5, L_[2]), (5.5, 8.5, L_[1]), (8.5, 10.2, L_[0]),
                        (10.2, 19, L_[0] * 0.45)):
        c.paint(ring & c.rect(x0, -1, x1, 9.6), col, 2)
    c.stitch(3.1, -1, 3.1, 9.6); c.stitch(7.4, -1, 7.4, 9.6)

    # centre chest: arch of indigo cloth with two piped panels and a leather ladder
    # neckline dome: the chest panel's top follows the round neck opening
    ang = np.linspace(np.pi, 2 * np.pi, 40)
    dome = [(min(max(66 + 36 * np.cos(a), 38), 98), 52 + 32 * np.sin(a)) for a in ang]
    arch = [(38, 94), (38, 40)] + [p for p in dome if p[1] < 40] + [(98, 40), (98, 94)]
    archm = c.cloth_panel(arch)
    for x0, x1 in ((40, 53), (82, 96)):
        p = c.poly_mask(rounded_rect(x0, 30, x1, 91, 3))
        c.shade(p, cloth, 30, 91, light=0.1, shadow=0.18, ink=False, mat=CLOTH)
        c.outline(p, width=0.9, col=PAL['leather'][1])
        c.outline(p, width=0.35, col=PAL['ink'])
    c.strap(55, 30, 59, 92, stitch=False); c.strap(75, 30, 79, 92, stitch=False)
    for y0, y1 in ((44, 63), (67, 77), (81, 90)):
        slot = c.rect(60, y0, 74, y1)
        c.paint(slot, cloth[0], CLOTH)
        c.outline(slot, width=0.35)
    for y in (63, 77):
        c.strap(59, y, 75, y + 4, stitch=False, cast=True)

    # chest scales: six rows, offset brick pattern, each row lapping the one below.
    # mirror seams: front centre x=1, back centre x=127 (measured from the mesh).
    clipL = c.rect(-2, 28, 30, 98); clipR = c.rect(104, 28, 130, 98)
    for row in reversed(range(6)):
        top = 30 + row * 10
        if row % 2 == 0:
            for cx in (1, 20): c.scale_plate(cx, top, 9.5, 15, clip=clipL)
            for cx in (127, 112): c.scale_plate(cx, top, 7.5, 15, clip=clipR)
        else:
            for cx in (10.5, 29.5): c.scale_plate(cx, top, 9.5, 15, clip=clipL)
            for cx in (119.5, 104.5): c.scale_plate(cx, top, 7.5, 15, clip=clipR)
    # side straps hold the scale columns (laid over them)
    c.strap(30, 30, 37, 97, cast=True); c.strap(97, 30, 104, 97, cast=True)
    for y in (36, 56, 76): c.rivet(33.5, y); c.rivet(100.5, y)
    # leather yoke under the shoulder straps, either side of the neck opening
    for x0, x1 in ((22, 46), (84, 106)):
        yoke = c.rect(x0, 18, x1, 31) & ~archm
        c.shade(yoke, PAL['leather'], 18, 31, light=0.15, shadow=0.2, ink=False, mat=2)
        c.outline(yoke, width=0.35)
    # shoulder straps, over the collar; the front one carries the cow emblem on the seam
    c.strap(-2, 20, 30, 30, cast=True)
    c.strap(100, 20, 130, 30, cast=True); c.rivet(125.5, 25.5)
    # curved leather neck band over everything: follows the neckline arch, stitched
    # along both curved edges, studs where it meets the shoulder straps
    band = c.poly_mask([(x, A(x)) for x in xs] + [(x, B(x)) for x in xs[::-1]])
    c.cast(band, 0.3, 0.9, 0.55)
    Yb = np.broadcast_to(c.rows(), band.shape); Xb = np.broadcast_to(c.cols(), band.shape)
    tb = (Yb - (12 + 3.5 * ((Xb - 64) / 64) ** 2)) / 8.0
    L_ = PAL['leather']
    c.paint(band, L_[1], 2); c.paint(band & (tb < 0.25), L_[2], 2); c.paint(band & (tb > 0.78), L_[0], 2)
    for off in (0.9, -0.9):
        pts = [(x, (A(x) if off > 0 else B(x)) + off) for x in xs]
        for (x0, y0), (x1, y1) in zip(pts[:-1], pts[1:]):
            c.stitch(x0, y0, x1, y1)
    c.outline(band, width=0.35)
    for x in (16, 112): c.rivet(x, (A(x) + B(x)) / 2, 1.3)
    emblem(c, 1, 25.6, 5.6)

    # abdomen: three overlapping lames each side in a leather frame. The lames end
    # on the mirror seam (front x=1, back x=125), so the seam keeps a thin centre line.
    for fx0, fx1, lx0, lx1 in ((-2, 35, 1, 26.5), (91, 130, 100.5, 125)):
        frame = c.rect(fx0, 96, fx1, 151)
        c.shade(frame, PAL['leather'], 96, 151, light=0.06, shadow=0.08, ink=False, mat=2)
        for i, top in enumerate((125, 113, 101)):
            c.lame(lx0, top, lx1, top + 16, lapped=i < 2)
    c.strap(30, 96, 35, 151, stitch=False, cast=True); c.strap(91, 96, 96, 151, stitch=False, cast=True)
    # lower front (mostly cut out): strap and a steel lame inside
    tab = c.rect(35, 96, 91, 151)
    c.shade(tab, [PAL['leather'][0] * 0.6, PAL['leather'][0], PAL['leather'][1] * 0.85], 96, 151, light=0.08, shadow=0.15, ink=False, mat=2)
    c.lame(44, 137, 83, 145.5, curve=0.8, rivets=(47.5, 79.5))
    c.strap(50, 110, 80, 118, cast=True)

    # waist band with leather welt
    c.cloth_panel([(-2, 151), (82, 151), (82, 158), (-2, 158)], tones=[cloth[0] * 0.8, cloth[0], cloth[1]])
    c.strap(-2, 157, 82, 160, stitch=False)

    # back skirt: indigo cloth, five stacked lames held by riveted vertical straps
    c.cloth_panel([(-2, 160), (82, 160), (82, 225), (-2, 225)], folds=[(6, 162, 6, 224), (74, 162, 74, 224)])
    tops = (164, 176, 188, 200, 212)
    for top in reversed(tops):
        c.lame(13, top, 69, top + 14)
    for x0, x1 in ((13, 16.5), (28, 31.5), (50.5, 54), (65.5, 69)):
        c.strap(x0, 160, x1, 226, stitch=False, cast=True)
        for top in tops: c.rivet((x0 + x1) / 2, top + 4, 1.1)

    # belt: one solid leather band (no cloth showing through), stitched blocks, rivets
    dark = [PAL['leather'][0] * 0.7, PAL['leather'][0], PAL['leather'][1]]
    belt = c.rect(-2, 225, 82, 258)
    c.shade(belt, dark, 225, 258, light=0.1, shadow=0.1, ink=False, mat=2)
    c.cast(c.rect(-2, 225, 82, 226), 0, 0.8, 0.6)
    for x0, x1 in ((0, 13), (14, 29), (31, 55), (57, 71), (72, 82)):
        c.strap(x0, 228.3, x1, 240.7)
    for x0, x1 in ((-2, 13), (14, 33), (35, 50), (52, 64), (65, 82)):
        c.strap(x0, 243.3, x1, 258)
    for x, y in ((7.5, 236), (76.5, 236.5), (8.5, 250.5), (70.5, 251)):
        c.rivet(x, y, 1.6)

    # pauldrons: the plate outlines are the original cut-outs, so the plates are
    # painted as full strips and the alpha gives each one its point.
    am = c.alpha(alpha)
    c.strap(76, 150, 81, 256, stitch=False)              # dark hinge strap left of both
    # A: two hanging plates (points x 88.75, 106) + steel side, bar on top. seam x=81
    side = c.rect(114.5, 170, 130, 200) & am
    c.metal(side, 170, 191, 114.5, 128, tones=DARK_STEEL, glint=False)
    for x0, x1 in ((97.5, 114.5), (81, 97.5)):
        m = c.rect(x0, 169, x1, 200) & am
        c.cast(m, 0.6, 0.8, 0.5); c.metal(m, 169, 194.5, x0, x1)
    bar = c.rect(81, 159.5, 130, 171) & am
    c.cast(bar, 0.3, 1.0, 0.45)
    c.metal(bar, 159.5, 171, 81, 128, tones=DARK_STEEL, glint=False)
    c.rivet(119.5, 163.5, 1.6)
    # B: three hanging plates, the first a half plate on the mirror seam x=83.5
    for x0, x1 in ((108, 124.5), (92, 108), (81, 92)):
        m = c.rect(x0, 221, x1, 256) & am
        c.cast(m, 0.6, 0.8, 0.5); c.metal(m, 221, 247.5, x0 if x0 > 81 else 75, x1)
    bar = c.rect(81, 208.5, 125, 222.5) & am
    c.cast(bar, 0.3, 1.0, 0.45)
    c.metal(bar, 208.5, 222.5, 81, 125, tones=DARK_STEEL, glint=False)
    c.rivet(116.5, 213.5, 1.6)

    # ink along every cut-out edge
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
