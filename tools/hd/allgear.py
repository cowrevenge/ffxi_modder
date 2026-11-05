"""HD pass for ANY gear DAT: the original design and colours, brighter and more
contrasty, 4x upscaled with material detail. Same pipeline as the Centurion HD mod,
minus its one-set shared palette (every item keeps its own colours).

usage: python3 allgear.py <out_dir> <dat>          (prints the retexture spec)
"""
import sys, os, zlib
import numpy as np
from PIL import Image
from scipy import ndimage
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tex, esr, skin
from hd import weights, hsv

# Mod 3's grade, made relative so every item keeps its own colours. Measured on the
# Centurion set (original class mean -> Mod 3 target, in LCh):
#   leather  L +2.7, chroma x1.55, hue kept
#   cloth    L same, a grey turns faintly cool (hue 284, chroma 4)
#   plate    L +16 (x1.27), chroma and hue kept
# The same moves apply to each item's own class means, and to saturated colour of
# any hue (dyed cloth, red leather), so a red Centurion gets the Centurion HD look in red.
SAT = 1.08        # light final push, after the grade
CONTRAST = 1.06
DENOISE = (14, 0.9)   # bilateral sigma colour / space (texels), before upscaling

import cv2

def to_lch(rgb):
    lab = cv2.cvtColor(np.clip(rgb, 0, 255).astype(np.uint8)[None], cv2.COLOR_RGB2LAB)[0].astype(np.float64)
    L = lab[..., 0] * 100 / 255; a = lab[..., 1] - 128; b = lab[..., 2] - 128
    return L, np.hypot(a, b), np.degrees(np.arctan2(b, a)) % 360

def from_lch(L, C, h):
    a = C * np.cos(np.radians(h)); b = C * np.sin(np.radians(h))
    lab = np.stack([np.clip(L * 255 / 100, 0, 255), np.clip(a + 128, 0, 255), np.clip(b + 128, 0, 255)], -1)
    return cv2.cvtColor(lab.astype(np.uint8), cv2.COLOR_LAB2RGB).astype(np.float64)

def classes(img):
    w = weights(img)
    h, s, v = hsv(img)
    taken = np.clip(w['leather'] + w['cloth'] + w['plate'], 0, 1)
    coloured = np.clip((s - 0.3) / 0.1, 0, 1) * np.clip((v - 0.12) / 0.06, 0, 1) * (1 - taken)
    w['coloured'] = coloured
    # bare skin keeps its own colour: take it out of every material class
    sk = skin.skin_weight(img, os.environ.get('RACE', ''), to_lch)
    if sk.any():
        for k in w: w[k] = w[k] * (1 - sk)
    return w

def target(kind, mean):
    L, C, hh = to_lch(mean[None])
    L, C, hh = float(L[0]), float(C[0]), float(hh[0])
    if kind in ('leather', 'coloured'):
        L, C = L + 2.7, min(C * 1.55, 70)
    elif kind == 'cloth':
        if C < 6: C, hh = 4.0, 284.0
        else: C = C * 1.3
    elif kind == 'plate':
        L = min(L * 1.27, 88)
    return from_lch(np.array([[L]]), np.array([[C]]), np.array([[hh]]))[0, 0]

KEEP = {'leather': 1.0, 'cloth': 1.1, 'plate': 1.1, 'coloured': 1.0}

def grade(img):
    rgb = img[..., :3].astype(np.float64)
    w = classes(img)
    acc = np.zeros_like(rgb); total = np.zeros(rgb.shape[:2])
    for k, wk in w.items():
        if wk.sum() < 4: continue
        mean = (rgb * wk[..., None]).sum((0, 1)) / wk.sum()
        acc += (target(k, mean) + (rgb - mean) * KEEP[k]) * wk[..., None]
        total += wk
    t = np.clip(total, 0, 1)[..., None]
    out = acc / np.maximum(total, 1e-6)[..., None] * t + rgb * (1 - t)
    return np.clip(out, 0, 255), w

def push(rgb):
    img = np.dstack([rgb, np.full(rgb.shape[:2], 255.0)])
    h, s, v = hsv(img)
    s = np.where(s < 0.08, s, np.clip(s * SAT, 0, 1))
    v = np.clip((v - 0.5) * CONTRAST + 0.5, 0, 1)
    return hsv_to_rgb(h, s, v)

def hsv_to_rgb(h, s, v):
    c = v * s; hp = (h / 60.0) % 6; x = c * (1 - np.abs(hp % 2 - 1)); m = v - c
    z = np.zeros_like(h)
    conds = [(hp < 1), (hp < 2), (hp < 3), (hp < 4), (hp < 5), (hp >= 5)]
    rgb = [(c, x, z), (x, c, z), (z, c, x), (z, x, c), (x, z, c), (c, z, x)]
    r = np.select(conds, [t[0] for t in rgb]); g = np.select(conds, [t[1] for t in rgb]); b = np.select(conds, [t[2] for t in rgb])
    return np.dstack([r + m, g + m, b + m]) * 255

def detail(rgb, w, seed):
    """light material detail: soft leather grain, faint brushing on plate. No pores."""
    from hd import fbm
    from PIL import ImageFilter
    h, wd = rgb.shape[:2]
    up = lambda m: np.array(Image.fromarray((m * 255).astype(np.uint8)).resize((wd, h), Image.BILINEAR)) / 255.0
    leather = np.clip(up(w['leather']) + up(w['coloured']) * 0.5, 0, 1); plate = up(w['plate'])
    grain = fbm((h, wd), seed, octaves=4, base=max(8, wd // 32))
    brushed = np.array(Image.fromarray(np.random.default_rng(seed + 2).standard_normal((h, wd // 8 or 1)).astype(np.float32)).resize((wd, h), Image.BILINEAR))
    out = rgb * (1 + leather * 0.035 * grain + plate * 0.02 * brushed)[..., None]
    im = Image.fromarray(np.clip(out, 0, 255).astype(np.uint8)).filter(ImageFilter.UnsharpMask(radius=2, percent=60, threshold=3))
    return np.array(im).astype(np.float64)

def process(img, seed):
    clean = img.copy()
    clean[..., :3] = cv2.bilateralFilter(img[..., :3].astype(np.float32), 0, DENOISE[0], DENOISE[1])
    graded, w = grade(clean)
    sk = skin.skin_weight(clean, os.environ.get('RACE', ''), to_lch)[..., None]
    up = esr.upscale4(push(graded) * (1 - sk) + graded * sk)     # skin skips the final push too
    hd = detail(up, w, seed)
    H, W = img.shape[:2]
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'paint')); from paint import alpha_mask, alpha_values
    a = alpha_mask(img[..., 3], W * 4, H * 4)
    if (~a).any():
        idx = ndimage.distance_transform_edt(~a, return_distances=False, return_indices=True)
        hd = hd[idx[0], idx[1]]
    return np.dstack([np.clip(hd, 0, 255), alpha_values(img[..., 3], W * 4, H * 4)]).astype(np.uint8)

if __name__ == '__main__':
    out_dir, dat = sys.argv[1], sys.argv[2]
    os.makedirs(out_dir, exist_ok=True)
    specs = []
    for name, img in tex.textures(dat):
        key = name.split()[-1]
        rgba = process(img, zlib.crc32(key.encode()) & 0xFFFF)
        raw = os.path.join(out_dir, key + '.rgba'); rgba.tofile(raw)
        Image.fromarray(rgba[..., :3]).save(os.path.join(out_dir, key + '.png'))
        specs.append(f"{key}={raw}:{rgba.shape[1]}x{rgba.shape[0]}")
    print(' '.join(specs))
