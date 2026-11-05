"""Automatic restyle: grade a texture onto a set's palette (Mod 3 method with the
set's own targets), then the HD pass. Used where a race's layout differs from the
hand-painted Hume male one, so those parts still get the set's colours.
"""
import sys, os
import numpy as np, cv2
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import esr, allgear
from hd import weights

def H(s): return np.array([int(s[i:i + 2], 16) for i in (0, 2, 4)], float)
STYLES = {
    'pride':   {'leather': H('6e472e'), 'cloth': H('2f3570'), 'plate': H('b7c3dc')},
    'bboots':  {'leather': H('7c4626'), 'cloth': H('3e281b'), 'plate': H('c48f5c')},
    'bgloves': {'leather': H('935a30'), 'cloth': H('4a4d2b'), 'plate': H('d0b386')},
}
KEEP = {'leather': 1.0, 'cloth': 1.1, 'plate': 1.1}

def grade(img, targets):
    rgb = img[..., :3].astype(np.float64); w = weights(img)
    acc = np.zeros_like(rgb); total = np.zeros(rgb.shape[:2])
    for k, wk in w.items():
        if wk.sum() < 4: continue
        mean = (rgb * wk[..., None]).sum((0, 1)) / wk.sum()
        acc += (targets[k] + (rgb - mean) * KEEP[k]) * wk[..., None]; total += wk
    t = np.clip(total, 0, 1)[..., None]
    return np.clip(acc / np.maximum(total, 1e-6)[..., None] * t + rgb * (1 - t), 0, 255), w

def restyle(img, style, seed=1):
    clean = img.copy()
    clean[..., :3] = cv2.bilateralFilter(img[..., :3].astype(np.float32), 0, *allgear.DENOISE)
    g, w = grade(clean, STYLES[style])
    w['coloured'] = np.zeros(img.shape[:2])
    up = esr.upscale4(allgear.push(g))
    return np.clip(allgear.detail(up, w, seed), 0, 255)
