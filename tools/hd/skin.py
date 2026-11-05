"""Bare skin painted into gear textures (thighs, midriff, fingers): find it so the HD
grade leaves its colour alone. Skin is the race's own tone (hue and chroma-per-lightness
measured from bare skin in each race's gear) and is smooth shading, unlike leather of the
same colour, which carries seams, grain and stitching."""
import numpy as np, cv2

# race: (hue in LCh degrees, chroma / lightness, lightness centre, lightness half-width),
# measured on bare legs (Hume F, Elvaan F, Mithra) and bare fingers (Galka).
REF = {'HumeF': (59.8, 0.46, 56, 9), 'ElvaanF': (60.3, 0.57, 43, 8),
       'Mithra': (63.0, 0.73, 43, 8), 'Galka': (113.2, 0.18, 46, 14),
       # measured on Legionnaire's bare thighs. Their tone is brown leather's, so a patch
       # only counts as skin when it is a real stretch of it (see MIN_CORE)
       'HumeM': (60.3, 0.54, 47, 8), 'ElvaanM': (59.9, 0.53, 38, 7),
       'TaruM': (62.5, 0.67, 44, 6), 'TaruF': (62.5, 0.67, 44, 6)}
# smallest smooth core (texels) that counts as skin; leather of skin tone comes in small patches
MIN_CORE = {'HumeM': 300, 'ElvaanM': 300, 'TaruM': 200, 'TaruF': 200}

def band(x, lo, hi, soft):
    return np.clip(np.minimum((x - lo) / soft + 1, (hi - x) / soft + 1), 0, 1)

def skin_weight(img, race, to_lch):
    if race not in REF: return np.zeros(img.shape[:2])
    h0, r0, lc, lw = REF[race]
    H, W = img.shape[:2]
    L, C, h = (x.reshape(H, W) for x in to_lch(img[..., :3].reshape(-1, 3).astype(np.float64)))
    dh = np.abs((h - h0 + 180) % 360 - 180)
    r = C / np.maximum(L, 1)
    colour = band(dh, -1, 10, 3) * band(r / r0, 0.8, 1.22, 0.08) * band(L, lc - lw, lc + lw, 4)
    # smooth: little fine detail around the texel (skin is soft shading)
    fine = np.abs(cv2.Laplacian(cv2.GaussianBlur(L.astype(np.float32), (0, 0), 0.6), cv2.CV_32F, ksize=3))
    rough = cv2.GaussianBlur(fine, (0, 0), 1.5)
    smooth = band(rough, -10, 2.2, 1.0)
    w = colour * smooth
    # skin comes in patches: drop specks, then soften the edge
    m = cv2.morphologyEx((w > 0.5).astype(np.uint8), cv2.MORPH_OPEN, np.ones((3, 3), np.uint8))
    n, lab, st, _ = cv2.connectedComponentsWithStats(m)
    keep = np.isin(lab, [i for i in range(1, n) if st[i, 4] >= MIN_CORE.get(race, 24)])
    # grow each confirmed patch over the whole skin-tone area it sits in: the smoothness
    # test fails within a few texels of a seam, and a thin band of skin is all seam
    tone = (colour > 0.5).astype(np.uint8)
    n2, lab2 = cv2.connectedComponents(tone)
    hit = np.unique(lab2[keep & (tone > 0)]); hit = hit[hit > 0]
    grown = np.isin(lab2, hit)
    w = np.maximum(keep * w, grown * colour)
    return cv2.GaussianBlur(w.astype(np.float32), (0, 0), 0.8).astype(np.float64) * (img[..., 3] > 32)
