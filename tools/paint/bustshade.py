"""Paper the new bust shape into a texture: bake how much each texel's surface now faces
up or down compared with the stock mesh, and lighten / darken the texture by that change
(smooth, so it reads as round). Used after bust_shape on gear whose texture is the HD
pass rather than hand-painted.

usage: python3 bustshade.py <skeleton> <stock.DAT> <shaped.DAT> <out_dir>   (prints spec)
"""
import sys, os
import numpy as np, cv2
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tex
from chest_female import bake_tilt

GAIN = 1.4          # brightness change per unit of tilt change
SOFT = 2.5          # texels of smoothing on the baked change

def main(skel, stock, shaped, out_dir):
    os.makedirs(out_dir, exist_ok=True)
    specs = []
    for (name, img) in tex.textures(shaped):
        H, W = img.shape[:2]
        t0, _, bs = bake_tilt(skel, stock, W, H)
        t1, _, _ = bake_tilt(skel, shaped, W, H)
        d = cv2.GaussianBlur((t1 - t0).astype(np.float32), (0, 0), SOFT * bs)
        d = cv2.resize(d, (W, H), interpolation=cv2.INTER_AREA)
        f = np.clip(1.0 + GAIN * d, 0.55, 1.35)
        # change lightness only: scaling RGB would push saturation too (skin turns orange)
        lab = cv2.cvtColor(np.clip(img[..., :3], 0, 255).astype(np.uint8), cv2.COLOR_RGB2LAB).astype(np.float32)
        lab[..., 0] = np.clip(lab[..., 0] * f, 0, 255)
        rgb = cv2.cvtColor(lab.astype(np.uint8), cv2.COLOR_LAB2RGB).astype(np.float32)
        rgba = np.dstack([rgb, img[..., 3]]).astype(np.uint8)
        key = name.split()[-1]
        raw = os.path.join(out_dir, key + '.rgba'); rgba.tofile(raw)
        Image.fromarray(rgba[..., :3]).save(os.path.join(out_dir, key + '.png'))
        specs.append(f"{key}={raw}:{W}x{H}")
    print(' '.join(specs))

if __name__ == '__main__':
    main(*sys.argv[1:5])
