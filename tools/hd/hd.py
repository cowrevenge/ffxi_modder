"""HD pass for a gear set: 4x Real-ESRGAN, one shared palette, material detail.

usage: python3 hd.py <out_dir> <dat> [<dat> ...]
Writes <out_dir>/<texname>.rgba (+ .png preview) per texture and prints the
retexture spec for each DAT.
"""
import sys, os, zlib
import numpy as np
from PIL import Image, ImageFilter
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tex, esr

SCALE = 4

# Shared targets (RGB). Leather from the Bounding Boots' saddle brown, cloth and
# plate from the Centurion body, so the worn outfit converges on its own colours.
TARGET = {
    'leather': np.array([92.0, 60.0, 40.0]),
    'cloth':   np.array([40.0, 41.0, 47.0]),
    'plate':   np.array([196.0, 186.0, 166.0]),
}
# How much of each pixel's own deviation from its class mean survives: keeps
# the painted detail while the class mean moves to the target.
DETAIL_KEEP = {'leather': 1.0, 'cloth': 1.1, 'plate': 1.1}

def hsv(img):
    f = img[..., :3] / 255.0
    mx = f.max(-1); mn = f.min(-1); d = mx - mn
    h = np.zeros_like(mx)
    r, g, b = f[..., 0], f[..., 1], f[..., 2]
    m = d > 1e-6
    hr = (mx == r) & m; hg = (mx == g) & m & ~hr; hb = m & ~hr & ~hg
    h[hr] = ((g - b)[hr] / d[hr]) % 6
    h[hg] = (b - r)[hg] / d[hg] + 2
    h[hb] = (r - g)[hb] / d[hb] + 4
    h = h * 60
    s = np.where(mx > 1e-6, d / np.maximum(mx, 1e-6), 0)
    return h, s, mx

def weights(img):
    h, s, v = hsv(img)
    def band(x, lo, hi, soft):
        return np.clip(np.minimum((x - lo) / soft + 1, (hi - x) / soft + 1), 0, 1)
    warm = band(h, 5, 45, 8)
    leather = warm * band(s, 0.32, 1.0, 0.08) * band(v, 0.12, 0.9, 0.06)
    olive = band(h, 45, 100, 8) * band(s, 0.2, 1.0, 0.08)
    plate = band(v, 0.5, 1.0, 0.08) * band(s, 0.0, 0.32, 0.08) * (1 - leather)
    cloth = np.clip(1 - leather - plate, 0, 1) * band(v, 0.0, 0.45, 0.08) * band(s, 0.0, 0.45, 0.1)
    cloth = np.maximum(cloth, olive)
    return {'leather': leather, 'cloth': cloth, 'plate': plate}

def grade(img):
    rgb = img[..., :3].copy()
    w = weights(img)
    out = rgb.copy()
    total = np.zeros(rgb.shape[:2])
    acc = np.zeros_like(rgb)
    for k, wk in w.items():
        if wk.sum() < 4:
            continue
        mean = (rgb * wk[..., None]).sum((0, 1)) / wk.sum()
        moved = TARGET[k] + (rgb - mean) * DETAIL_KEEP[k]
        acc += moved * wk[..., None]
        total += wk
    t = np.clip(total, 0, 1)[..., None]
    blended = acc / np.maximum(total, 1e-6)[..., None]
    out = blended * t + rgb * (1 - t)
    return np.clip(out, 0, 255), w

def fbm(shape, seed, octaves=4, base=8):
    rng = np.random.default_rng(seed)
    h, w = shape
    acc = np.zeros(shape); amp = 1.0; norm = 0.0
    for o in range(octaves):
        n = base * 2 ** o
        g = rng.standard_normal((n, n))
        up = np.array(Image.fromarray(g.astype(np.float32)).resize((w, h), Image.BICUBIC))
        acc += amp * up; norm += amp; amp *= 0.5
    return acc / norm

def detail(rgb, w, seed):
    h, wd = rgb.shape[:2]
    leather = np.array(Image.fromarray((w['leather'] * 255).astype(np.uint8)).resize((wd, h), Image.BILINEAR)) / 255.0
    plate = np.array(Image.fromarray((w['plate'] * 255).astype(np.uint8)).resize((wd, h), Image.BILINEAR)) / 255.0
    grain = fbm((h, wd), seed, octaves=5, base=max(8, wd // 32))
    pores = (np.random.default_rng(seed + 1).random((h, wd)) < 0.015) * -0.08
    brushed = np.array(Image.fromarray(np.random.default_rng(seed + 2).standard_normal((h, wd // 8 or 1)).astype(np.float32)).resize((wd, h), Image.BILINEAR))
    mul = 1 + leather * (0.07 * grain + pores) + plate * 0.035 * brushed
    out = rgb * mul[..., None]
    im = Image.fromarray(np.clip(out, 0, 255).astype(np.uint8))
    im = im.filter(ImageFilter.UnsharpMask(radius=2, percent=70, threshold=2))
    return np.array(im).astype(np.float64)

def process(name, img, seed):
    graded, w = grade(img)
    up = esr.upscale4(graded)
    hd = detail(up, w, seed)
    a = img[..., 3]
    a_up = np.array(Image.fromarray(a.astype(np.uint8)).resize((a.shape[1] * SCALE, a.shape[0] * SCALE), Image.BICUBIC))
    rgba = np.dstack([np.clip(hd, 0, 255), a_up]).astype(np.uint8)
    return rgba

if __name__ == '__main__':
    out_dir = sys.argv[1]; os.makedirs(out_dir, exist_ok=True)
    for path in sys.argv[2:]:
        specs = []
        for i, (name, img) in enumerate(tex.textures(path)):
            key = name.split()[-1]
            rgba = process(key, img, seed=zlib.crc32(key.encode()) & 0xFFFF)
            h, w = rgba.shape[:2]
            raw = os.path.join(out_dir, key + '.rgba')
            rgba.tofile(raw)
            Image.fromarray(rgba[..., :3]).save(os.path.join(out_dir, key + '.png'))
            specs.append(f"{key}={raw}:{w}x{h}")
        print(path, ' '.join(specs))
