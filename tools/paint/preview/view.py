"""Render one or more DATs on the skeleton, alpha-tested like the game, four
angles per row (one row per argument group).

usage: python3 view.py <skeleton.dat> <out.png> <size> <dat> [<dat> ...]
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from PIL import Image
import raster
skel, out, size, dats = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4:]
rows = []
for d in dats:
    tris = raster.load(skel, [d])
    allp = np.array([p for t in tris for p in t[0]]); mn, mx = allp.min(0), allp.max(0)
    c = (mn + mx) / 2; h = (mx - mn).max() / 2
    rows.append(np.concatenate([raster.render(tris, az, (size, size), c, h) for az in (90, 0, 270, 180)], 1))
Image.fromarray((np.concatenate(rows, 0) * 255).astype(np.uint8)).save(out)
