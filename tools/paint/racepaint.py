"""Paint one piece for any race: the hand-painted Hume male design where the race's
layout matches it, the automatic restyle where it doesn't (blended at the seam).

usage: python3 racepaint.py <piece> <race_dat> <humem_dat> <out_dir>   (prints retexture spec)
"""
import sys, os, importlib
import numpy as np, cv2
from scipy import ndimage
from PIL import Image
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'hd'))
import tex, autostyle

MOD = {'body': 'paint_body', 'legs': 'paint_legs', 'hands': 'paint_hands', 'feet': 'paint_feet', 'visor': 'paint_visor',
       'sword': 'paint_sword', 'axe': 'paint_axe', 'bboots': 'paint_bboots', 'bgloves': 'paint_bgloves'}
STYLE = {'bboots': 'bboots', 'bgloves': 'bgloves'}

def mismatch(a, b):
    """texels where the race's original differs from Hume male's (layout change)."""
    if a.shape != b.shape: return np.ones(b.shape[:2], bool)
    d = np.abs(a[..., :3] - b[..., :3]).mean(-1) > 40
    d |= (a[..., 3] >= 64) != (b[..., 3] >= 64)
    d = ndimage.binary_opening(d)
    lab, n = ndimage.label(ndimage.binary_closing(d, iterations=2))
    keep = np.zeros_like(d)
    for i in range(n):
        m = lab == i + 1
        if m.sum() > 15: keep |= m
    return ndimage.binary_dilation(keep, iterations=2)

piece, race_dat, hume_dat, out_dir = sys.argv[1:5]
os.makedirs(out_dir, exist_ok=True)
TARU = {'feet': 'paint_taru_feet', 'bboots': 'paint_taru_bboots', 'legs': 'paint_taru_legs'}
if os.environ.get('RACE') == 'TaruM' and piece in TARU:
    MOD = dict(MOD, **{piece: TARU[piece]})
mod = importlib.import_module(MOD[piece])
specs = []
for (name, img), (_, himg) in zip(tex.textures(race_dat), tex.textures(hume_dat)):
    key = name.split()[-1]
    M = mismatch(himg, img)
    if os.environ.get('RACE') == 'TaruM' and piece in TARU:
        M = np.zeros(img.shape[:2], bool)          # hand-painted for the Taru layout
    race = os.environ.get('RACE', '')
    if piece == 'body' and M.shape == (256, 128):
        # chest scale blocks: same columns on every race, only the row spacing shifts
        # (bust). Keep the painted scales whole; blending islands there breaks them.
        M[26:100, 0:32] = False; M[26:100, 100:128] = False
    if race == 'Mithra' and piece == 'body':
        # the tail sheath sits here on Mithra (Hume M has abdomen plates here):
        # keep it plain, the original's leather in the set's colours, no armour
        M[108:158, 0:25] = True
    if race in ('Mithra', 'Galka') and piece == 'body':
        # rear waist beside the tail: Hume M's abdomen plate block lives here in the
        # texture; on these races it is the back of the waist, painted from their own
        M[98:146, 96:128] = True
    Hh, Ww = img.shape[:2]
    if (M.all() or himg.shape != img.shape) and not (os.environ.get('RACE') == 'TaruM' and piece in TARU):
        rgb = autostyle.restyle(img, STYLE.get(piece, 'pride'))
        from paint import alpha_mask, alpha_values
        a = alpha_mask(img[..., 3], Ww * 4, Hh * 4)
        if (~a).any():
            idx = ndimage.distance_transform_edt(~a, return_distances=False, return_indices=True); rgb = rgb[idx[0], idx[1]]
        rgba = np.dstack([rgb, alpha_values(img[..., 3], Ww * 4, Hh * 4)]).astype(np.uint8)
    else:
        if piece == 'body' and race in ('HumeF', 'ElvaanF', 'Mithra'):
            import chest_female
            skel = os.environ['SKEL']
            mod.CHEST = chest_female.make_chest(img, skel, race_dat)
        rgba = mod.paint(img[..., 3]).astype(np.float64)
        if piece == 'body': mod.CHEST = None
        if M.any():
            auto = autostyle.restyle(img, STYLE.get(piece, 'pride'))
            m = cv2.GaussianBlur(cv2.resize(M.astype(np.float32), (Ww * 4, Hh * 4), interpolation=cv2.INTER_NEAREST), (0, 0), 1.5)[..., None]
            rgba[..., :3] = rgba[..., :3] * (1 - m) + auto * m
        if race == 'Mithra' and piece == 'body':
            # Mithra's tail (body texture x1-15, y133-157): its natural colour, HD only
            import allgear
            own = allgear.process(img, 7)[..., :3].astype(np.float64)
            T = np.zeros((Hh, Ww), np.float32); T[132:158, 0:16] = 1
            t = cv2.GaussianBlur(cv2.resize(T, (Ww * 4, Hh * 4), interpolation=cv2.INTER_NEAREST), (0, 0), 1.5)[..., None]
            rgba[..., :3] = rgba[..., :3] * (1 - t) + own * t
        if race == 'Galka' and piece == 'legs':
            # the tail is a tail: its own original colours, only the HD pass, no restyle
            import allgear
            own = allgear.process(img, 7)[..., :3].astype(np.float64)
            T = np.zeros((Hh, Ww), np.float32); T[66:127, 97:128] = 1      # the bare tail tip only (the plates are armour)
            t = cv2.GaussianBlur(cv2.resize(T, (Ww * 4, Hh * 4), interpolation=cv2.INTER_NEAREST), (0, 0), 1.5)[..., None]
            rgba[..., :3] = rgba[..., :3] * (1 - t) + own * t
        rgba = rgba.astype(np.uint8)
    raw = os.path.join(out_dir, key + '.rgba'); rgba.tofile(raw)
    Image.fromarray(rgba[..., :3]).save(os.path.join(out_dir, key + '.png'))
    specs.append(f"{key}={raw}:{rgba.shape[1]}x{rgba.shape[0]}  mismatch={M.mean():.2f}")
print(' '.join(s.split()[0] for s in specs)); print('#', ' '.join(s.split()[1] for s in specs), file=sys.stderr)
