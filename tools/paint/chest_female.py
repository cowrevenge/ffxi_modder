"""Front chest scales for races with a bust (Hume F, Elvaan F, Mithra).

Shapes: read from the race's own original texture. The light plate texels in the front
chest block are split into individual scales (the dark gaps between them are the lines).
Curve: baked from the race's mesh. Every texel of the block gets the up/down tilt of the
model surface it lands on, so the upper bust (facing up) shades lighter and the
underside (facing down) darker, scale by scale. Stretch: the mesh's vertical stretch per
row sets how tall the light band and lapping shadow are drawn.
"""
import sys, os
import numpy as np, cv2
from scipy import ndimage
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), 'preview'))
import tex
from paint import SS, PAL, STEEL, mix

BLOCK = (0, 28, 31, 99)      # x0, y0, x1, y1 texels: front chest scale block

def segment(img):
    """label each scale of the front chest block: the dark lines between scales are
    found in the original, and every region they enclose is one scale."""
    x0, y0, x1, y1 = BLOCK
    rgb = img[y0:y1, x0:x1, :3].astype(np.float64)
    L = rgb @ [0.299, 0.587, 0.114]
    sat = rgb.max(-1) - rgb.min(-1)
    line = (L < 62) | (sat > 45)          # dark gaps, and the brown leather beyond the block
    line = ndimage.binary_closing(line, structure=np.ones((2, 2)))
    lab, n = ndimage.label(~line)
    sizes = ndimage.sum(np.ones_like(lab), lab, range(1, n + 1))
    keep = np.zeros(n + 1, bool); keep[1:] = sizes >= 12
    lab = np.where(keep[lab], lab, 0)
    # grow scales halfway into the thin lines so neighbours touch under the ink
    idx = ndimage.distance_transform_edt(lab == 0, return_distances=False, return_indices=True)
    d = ndimage.distance_transform_edt(lab == 0)
    grown = np.where((lab == 0) & (d <= 1.0) & ~((sat > 45)), lab[idx[0], idx[1]], lab)
    full = np.zeros(img.shape[:2], np.int32); full[y0:y1, x0:x1] = grown
    return full

def bake_tilt(skel, dat, W, H, scale=4):
    """per texel: the surface's vertical facing (+1 up, -1 down) and vertical stretch."""
    import raster
    tris = raster.load(skel, [dat])
    up = np.zeros((H * scale, W * scale)); wsum = np.zeros_like(up); st = np.zeros_like(up)
    for P, N, uv, tx in tris:
        if uv is None: continue
        q = np.array([[u[0] * W * scale, u[1] * H * scale] for u in uv], np.float32)
        n = sum(N) / 3.0; n = n / (np.linalg.norm(n) + 1e-9)
        # model up is -y; tilt = how much the face points up
        tilt = -n[1]
        # vertical stretch: world height per texel row of this triangle
        P = np.array(P); dv = (q[:, 1].max() - q[:, 1].min()) / scale
        dy = np.ptp(P[:, 1])
        s = dy / dv if dv > 1e-3 else 0
        m = np.zeros(up.shape, np.uint8)
        cv2.fillConvexPoly(m, np.round(q * 4).astype(np.int32), 1, lineType=cv2.LINE_8, shift=2)
        up[m > 0] += tilt; st[m > 0] += s; wsum[m > 0] += 1
    ok = wsum > 0
    up[ok] /= wsum[ok]; st[ok] /= wsum[ok]
    # fill gaps from neighbours
    idx = ndimage.distance_transform_edt(~ok, return_distances=False, return_indices=True)
    up = up[idx[0], idx[1]]; st = st[idx[0], idx[1]]
    return up, st, scale

def make_chest(img, skel, dat):
    """returns CHEST(c, clip): the same stacked brick-pattern scales as Hume male (rows
    lapping the row below), lit by the female model's own curve."""
    H, W = img.shape[:2]
    tilt, stretch, bs = bake_tilt(skel, dat, W, H)
    # the mesh is a few flat triangles; smooth their facing into a rounded curve
    tilt = cv2.GaussianBlur(tilt.astype(np.float32), (0, 0), 3.2 * bs)
    tilt_c = cv2.resize(tilt, (W * SS, H * SS), interpolation=cv2.INTER_CUBIC)

    def chest(c, clip):
        for cx in (10.5, 29.5): c.scale_plate(cx, 88, 9.5, 15, clip=clip)
        for row in reversed(range(7)):
            top = 30 + row * 10
            for cx in ((1, 20) if row % 2 == 0 else (10.5, 29.5)):
                c.scale_plate(cx, top, 9.5, 15, clip=clip)
        # light by where each texel sits on the body, 4 cel steps: top of the bust
        # (facing up) bright, the front plain, the lower curve shaded, under it dark
        m = clip & (c.mat == STEEL)
        t = tilt_c[m]
        f = np.clip(0.86 + 1.5 * t, 0.58, 1.22)      # smooth falloff, no hard band
        c.img[m] = np.clip(c.img[m] * f[:, None], 0, 255)
    return chest
