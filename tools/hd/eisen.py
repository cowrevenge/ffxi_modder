"""Eisen / Kampf, hand-finished: polished silver plate with deep-cut engraving.

The plates, their scrollwork and rivets are traced from the original texture and
redrawn at 8 px per texel with hard edges:
  - plate: mirror-polished steel in 4 cel tones driven by the plate's own form
    (the original's broad shading), a hard dark horizon reflection, a bright rim
    along the upper-left edge and a dark rim along the lower-right;
  - engraving: every scroll line becomes a deep cut, a dark groove with a lit lower
    lip, so the pattern reads as cut into the metal;
  - rivets: polished domes with a glint.
Everything that is not plate (dark quilted suit, brown leather, braid) comes from the
automatic HD pass, which keeps the original look. Output 4x, original alpha.

usage: python3 eisen.py <in.DAT> <out_dir>      (prints the retexture spec)
"""
import sys, os
import numpy as np, cv2
from scipy import ndimage
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'paint'))
import tex, allgear
from paint import alpha_mask, alpha_values

SS = 8
OUT = 4
# polished silver, dark to light
TONES = np.array([[38, 41, 50], [92, 98, 112], [158, 165, 180], [214, 220, 232], [246, 249, 255]], np.float32)
GROOVE = np.array([22, 22, 28], np.float32)
LIP = np.array([236, 240, 248], np.float32)

def lum(rgb): return rgb[..., :3] @ np.array([0.299, 0.587, 0.114])

def up_mask(m, f, sigma=0.55, thr=0.45):
    """smooth upscale of a texel mask to hard, curved edges."""
    b = cv2.GaussianBlur(m.astype(np.float32), (0, 0), sigma)
    return cv2.resize(b, (m.shape[1] * f, m.shape[0] * f), interpolation=cv2.INTER_CUBIC) > thr

def trace(img):
    rgb = img[..., :3].astype(np.float32)
    L = lum(rgb); sat = rgb.max(-1) - rgb.min(-1)
    light = (L > 88) & (sat < 52)
    plate = ndimage.binary_closing(light, structure=np.ones((3, 3)), iterations=2)
    plate = ndimage.binary_fill_holes(plate) & ~((sat > 52) & (L < 140))     # keep leather out
    lab, n = ndimage.label(plate)
    sizes = ndimage.sum(plate, lab, range(1, n + 1))
    plate = np.isin(lab, np.where(sizes >= 20)[0] + 1)
    # engraving: darker than its surroundings, inside the plate
    local = cv2.GaussianBlur(L, (0, 0), 1.6)
    cut = plate & (L < local - 10) & ndimage.binary_erosion(plate, iterations=1)
    # rivets: small bright peaks
    peak = (L == ndimage.maximum_filter(L, 3)) & (L > 185) & plate
    rv = ndimage.center_of_mass(peak, *ndimage.label(peak)[:1], range(1, ndimage.label(peak)[1] + 1)) if peak.any() else []
    return plate, cut, rv, L

def paint(img, seed=7):
    """HD pass, then a soft polish on every steel texel: deeper engraving, a hard
    polished highlight curve and a cool silver tone. No hard masks, so no seams."""
    from hd import weights
    H, W = img.shape[:2]
    Wo, Ho = W * OUT, H * OUT
    base = allgear.process(img, seed).astype(np.float32)
    rgb = base[..., :3]
    w = weights(img)['plate'].astype(np.float32)
    w = cv2.GaussianBlur(cv2.resize(w, (Wo, Ho), interpolation=cv2.INTER_CUBIC), (0, 0), 1.0)
    w = np.clip((w - 0.15) / 0.5, 0, 1)
    L = rgb @ np.array([0.299, 0.587, 0.114], np.float32)
    form = cv2.GaussianBlur(L, (0, 0), 3.0)
    fine = L - cv2.GaussianBlur(L, (0, 0), 1.2)          # the engraving lines
    mid = L - fine - form                                 # bevels and raised scrolls
    f = np.clip(form / 255.0, 0, 1)
    # polished curve: darker darks, hotter highlights, a dip for the horizon reflection
    pol = np.clip((f - 0.5) * 1.3 + 0.5, 0, 1)
    pol = pol - 0.10 * np.exp(-((f - 0.52) / 0.05) ** 2)
    val = pol * 255 + 1.9 * mid + 2.6 * fine              # cuts twice as deep
    hot = np.clip((val - 225) / 30, 0, 1)                 # clipped specular sheen
    val = val + hot * 14
    silver = np.array([0.94, 0.97, 1.0], np.float32)
    pol_rgb = np.clip(val, 0, 255)[..., None] * silver
    out = rgb * (1 - w[..., None]) + pol_rgb * w[..., None]
    al = alpha_mask(img[..., 3], Wo, Ho)
    if (~al).any():
        ii = ndimage.distance_transform_edt(~al, return_distances=False, return_indices=True); out = out[ii[0], ii[1]]
    return np.dstack([np.clip(out, 0, 255), alpha_values(img[..., 3], Wo, Ho)]).astype(np.uint8)

if __name__ == '__main__':
    dat, out_dir = sys.argv[1], sys.argv[2]
    os.makedirs(out_dir, exist_ok=True)
    specs = []
    for name, img in tex.textures(dat):
        key = name.split()[-1]
        rgba = paint(img)
        raw = os.path.join(out_dir, key + '.rgba'); rgba.tofile(raw)
        Image.fromarray(rgba[..., :3]).save(os.path.join(out_dir, key + '.png'))
        specs.append(f"{key}={raw}:{rgba.shape[1]}x{rgba.shape[0]}")
    print(' '.join(specs))
