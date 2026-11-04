"""Gridded zoom of every texture in a DAT: see-through texels green, mirror seams
cyan, a labelled grid every 8 texels. This is how a layout is read before writing
its brief.

usage: python3 texgrid.py <dat> <out_prefix> <skeleton.dat>
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
import numpy as np
from PIL import Image, ImageDraw
import tex
from mirror import mirror_edges

dat, out, skel = sys.argv[1], sys.argv[2], sys.argv[3]
for name, im in tex.textures(dat):
    H, W = im.shape[:2]; S = max(4, 1024 // max(W, H) * 2)
    rgb = im[..., :3].copy(); rgb[im[..., 3] < 128] = [0, 160, 0]
    o = Image.fromarray(rgb.astype(np.uint8)).resize((W * S, H * S), Image.NEAREST); d = ImageDraw.Draw(o)
    for a, b in mirror_edges(dat, skel, W, H):
        d.line([(a[0] * S, a[1] * S), (b[0] * S, b[1] * S)], fill=(0, 255, 255), width=2)
    for x in range(0, W, 8): d.line([(x * S, 0), (x * S, H * S)], fill=(255, 0, 255)); d.text((x * S + 2, 2), str(x), fill=(255, 255, 0))
    for y in range(0, H, 8): d.line([(0, y * S), (W * S, y * S)], fill=(255, 0, 255)); d.text((2, y * S + 2), str(y), fill=(255, 255, 0))
    key = name.split()[-1]
    o.save(f'{out}_{key}.png'); print(name, W, H, f'{out}_{key}.png')
