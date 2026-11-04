"""Mirror seams of a DAT's mesh, in texel coordinates.

A mirror seam is a 3D edge shared by two triangles that carry the identical UV
triangle: a triangle and its mirrored copy. Interior edges also share a 3D edge,
but their two triangles have different UV triangles, so they never match.

usage: python3 mirror.py <dat> <skeleton.dat> <W> <H> <out.png>
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
import numpy as np
from collections import defaultdict
import raster

def mirror_edges(dat, skel, W, H):
    tris = raster.load(skel, [dat])
    E = defaultdict(list)
    k = lambda p: tuple(np.round(p, 3))
    for P, N, uv, tx in tris:
        if uv is None: continue
        U = [(round(u[0] * W, 2), round(u[1] * H, 2)) for u in uv]
        tri = tuple(sorted(U))
        for i, j in ((0, 1), (1, 2), (2, 0)):
            a, b = k(P[i]), k(P[j]); ua, ub = U[i], U[j]
            if a > b: a, b, ua, ub = b, a, ub, ua
            E[(a, b)].append(((ua, ub), tri))
    return [l[0][0] for l in E.values() if len(l) == 2 and l[0] == l[1]]

if __name__ == '__main__':
    from PIL import Image, ImageDraw
    import tex as T
    dat, skel, W, H, png = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
    m = mirror_edges(dat, skel, W, H); print(len(m), 'mirror edges')
    im = Image.fromarray(T.textures(dat)[0][1][..., :3].astype(np.uint8)).resize((W * 4, H * 4), Image.NEAREST)
    d = ImageDraw.Draw(im)
    for a, b in m: d.line([(a[0] * 4, a[1] * 4), (b[0] * 4, b[1] * 4)], fill=(0, 255, 255), width=3)
    im.save(png)
