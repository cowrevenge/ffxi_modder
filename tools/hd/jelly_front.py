"""Jelly marker: front tentacle red with a yellow smiley, rear tentacle (the tail) green with a
rooster tail, so THF can tell front from back at a glance.

Each tentacle is ONE region: every triangle of it is mapped onto a flat clipart image
(projected straight from the front / straight from behind), so the neck keeps one clean
outline. Triangles of the tentacle that face away, or sit further back than the clipart
can reach, get the clipart's plain background colour.

Texture (2x wide, 2x tall):  left top = original  |  right: smiley, red, rooster, green.
If the jelly is already red the front goes purple; if already green the tail goes purple.

usage: python3 jelly_marker.py <in.DAT> <out_dir>   (writes front.DAT and slm_01.png)
"""
import sys, os, struct, subprocess
import numpy as np, cv2
sys.path.insert(0, '/home/claude/work/ctr'); sys.path.insert(0, '/home/claude/work/hd')
import raster, tex
from PIL import Image

src, out = sys.argv[1], sys.argv[2]; os.makedirs(out, exist_ok=True)
data = bytearray(open(src, 'rb').read())
tris = raster.load(src, [src])
C = np.array([np.mean(t[0], 0) for t in tris])
NF = np.array([np.mean(t[1], 0) for t in tris]); NF /= np.linalg.norm(NF, axis=1, keepdims=True) + 1e-9
top, bot = C[:, 1].min(), C[:, 1].max(); H_ = bot - top       # FFXI y points down: top = min
upper = C[:, 1] < top + 0.46 * H_
SECTOR = 2.2                                                  # x must beat SECTOR*|z|

def region(s):
    """s=+1 front tentacle, -1 tail. returns (all triangles, clipart triangles, projection)"""
    lobe = upper & (s * C[:, 0] > 0.18) & (s * C[:, 0] > SECTOR * np.abs(C[:, 2]))
    d = np.array([s, 0.0, 0.0]); up = np.array([0, -1.0, 0]); rt = np.cross(up, d)
    draw = lobe & (s * NF[:, 0] > 0)
    area = lambda P: 0.5 * abs(np.cross(P[1] - P[0], P[2] - P[0]) @ d)
    A = np.array([area(np.array(tris[k][0])) for k in np.where(draw)[0]])
    c = (C[draw] * A[:, None]).sum(0) / A.sum()
    draw &= s * C[:, 0] > s * c[0] - 0.04            # forward-facing skirt further back: background
    key = lambda p: tuple(np.round(p, 5))
    inner = {key(p) for k in np.where(draw)[0] for p in tris[k][0]}
    edge = [p for k in np.where(~draw)[0] for p in tris[k][0] if key(p) in inner]
    r_in = min(np.hypot((p - c) @ rt, (p - c) @ up) for p in edge)
    print(('front' if s > 0 else 'tail'), 'triangles', lobe.sum(), 'clipart', draw.sum(), 'r_in', round(r_in, 4))
    return lobe, draw, (c, rt, up, r_in)

front, front_draw, front_p = region(+1)
tail, tail_draw, tail_p = region(-1)

(name, img), = [t for t in tex.textures(src) if 'slm_01' in t[0]]
H, W = img.shape[:2]; S = W                                    # clipart images are S x S
hsv = cv2.cvtColor(np.clip(img[..., :3], 0, 255).astype(np.uint8), cv2.COLOR_RGB2HSV)
hue = np.median(hsv[..., 0][hsv[..., 1] > 40]) if (hsv[..., 1] > 40).any() else 0
sat = np.median(hsv[..., 1]) > 60
PURPLE = (185, 80, 230)
RED = PURPLE if (hue < 12 or hue > 165) and sat else (228, 72, 52)
GREEN = PURPLE if 35 <= hue <= 85 and sat else (45, 175, 60)
print('base hue', hue, 'front', RED, 'tail', GREEN)

SMILE_R, ROOSTER_R = 0.36, 0.44                                # clipart radius, image units

def smiley(bg):
    SS = S * 4; f = np.zeros((SS, SS, 3), np.uint8); f[:] = bg
    c = SS // 2; R = int(SS * SMILE_R)
    cv2.circle(f, (c, c), R + SS // 64, (60, 30, 0), -1, cv2.LINE_AA)
    cv2.circle(f, (c, c), R, (255, 232, 40), -1, cv2.LINE_AA)
    for ex in (-1, 1):
        cv2.ellipse(f, (c + ex * int(R * 0.36), c - int(R * 0.28)), (int(R * 0.11), int(R * 0.2)), 0, 0, 360, (25, 15, 5), -1, cv2.LINE_AA)
    cv2.ellipse(f, (c, c + int(R * 0.02)), (int(R * 0.58), int(R * 0.5)), 0, 20, 160, (25, 15, 5), int(R * 0.1), cv2.LINE_AA)
    return cv2.resize(f, (S, S), interpolation=cv2.INTER_AREA)

ROOSTER_SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'rooster_src.png')  # next to this script

def rooster(bg):
    """the user's rooster tail picture: white background cut out (flood fill from the edges,
    then the pale fringe), a dark outline like the smiley's rim, set on the tail colour and
    fitted inside the safe circle"""
    a = np.array(Image.open(ROOSTER_SRC).convert('RGB')).astype(np.float32)
    dark = np.abs(a - [34, 36, 42]).sum(2) < 6                  # screenshot's dark border on the right
    cols = [x for x in range(a.shape[1]) if dark[:, x].mean() > 0.9]
    if cols: a = a[:, :cols[0]]
    UP = 6                                                       # work big so the outline is smooth
    a = cv2.resize(a, (a.shape[1] * UP, a.shape[0] * UP), interpolation=cv2.INTER_CUBIC)
    mn, mxc = a.min(2), a.max(2)
    pale = (mn > 175) & (mxc - mn < 40)                          # white / light grey, not orange
    # edge check: background = pale pixels connected to the picture's border
    n, lab = cv2.connectedComponents(pale.astype(np.uint8), connectivity=4)
    edge_labels = set(np.unique(np.concatenate([lab[0], lab[-1], lab[:, 0], lab[:, -1]]))) - {0}
    bgm = np.isin(lab, list(edge_labels))
    # white pockets between feathers that don't touch the edge: drop the bigger ones too
    for l in range(1, n):
        if l not in edge_labels and (lab == l).sum() > 12 * UP * UP: bgm |= lab == l
    solid = ~bgm
    solid = cv2.morphologyEx(solid.astype(np.uint8), cv2.MORPH_OPEN, np.ones((UP, UP), np.uint8))   # drop specks
    solid = cv2.erode(solid, np.ones((3, 3), np.uint8), iterations=UP // 2)                    # shave the pale fringe
    ys, xs = np.where(solid > 0)
    sl = (slice(ys.min(), ys.max() + 1), slice(xs.min(), xs.max() + 1))
    a, solid = a[sl], solid[sl]
    # outline: a dark band just inside the edge, same colour as the smiley's rim
    pad = np.pad(solid, 1)                                      # pad: the picture's cut-off edge gets outlined too
    ring = (pad - cv2.erode(pad, np.ones((3, 3), np.uint8), iterations=UP * 2))[1:-1, 1:-1]
    a[ring > 0] = (60, 30, 0)
    alpha = cv2.GaussianBlur(solid.astype(np.float32), (0, 0), UP * 0.25)
    SS = S * 4; R = ROOSTER_R * SS
    k = 2 * R / np.hypot(*a.shape[:2]) * 1.3                     # fit inside the circle the tentacle can show
    h, w = int(a.shape[0] * k), int(a.shape[1] * k)
    a = cv2.resize(a, (w, h), interpolation=cv2.INTER_AREA); alpha = cv2.resize(alpha, (w, h), interpolation=cv2.INTER_AREA)[..., None]
    f = np.zeros((SS, SS, 3), np.float32); f[:] = bg
    y0, x0 = (SS - h) // 2, (SS - w) // 2
    f[y0:y0 + h, x0:x0 + w] = a * alpha + f[y0:y0 + h, x0:x0 + w] * (1 - alpha)
    return cv2.resize(np.clip(f, 0, 255).astype(np.uint8), (S, S), interpolation=cv2.INTER_AREA)

new = np.zeros((H * 2, W * 2, 4), np.uint8)
new[:H, :W] = img; new[H:, :W] = img
new[:, W:, 3] = 255
new[0:S, W:, :3] = smiley(RED);       new[S:H, W:, :3] = RED            # rows: smiley, red
new[H:H + S, W:, :3] = rooster(GREEN); new[H + S:, W:, :3] = GREEN      # rows: rooster, green
V0 = {+1: 0.0, -1: 0.5}                                                 # clipart rows (v)
BG = {+1: (0.75, 0.375), -1: (0.75, 0.875)}                             # plain background texel

def clip_uv(p, s):
    c, rt, up, r_in = front_p if s > 0 else tail_p
    fs = 0.92 * r_in / (SMILE_R if s > 0 else ROOSTER_R)
    a = np.clip((p - c) @ rt / fs + 0.5, 0.01, 0.99); b = np.clip(0.5 - (p - c) @ up / fs, 0.01, 0.99)
    return 0.5 + 0.5 * a, V0[s] + 0.25 * b

# UVs: walk the mesh's triangle lists
k = 0; o = 0
while o + 16 <= len(data):
    v = struct.unpack_from('<I', data, o + 4)[0]; kind = v & 0x7f; sz = ((v >> 7) & 0x7ffff) * 16
    if sz < 16: break
    if kind == 0x2a:
        base = o + 16; me = data[base:base + sz - 16]
        c = struct.unpack_from('<I', me, 6)[0] * 2
        while True:
            op, = struct.unpack_from('<H', me, c); c += 2
            if op == 0xffff: break
            if op == 0x8010: c += 0x2c; continue
            if op == 0x8000: c += 16; continue
            n, = struct.unpack_from('<H', me, c); c += 2
            assert op == 0x0054, hex(op)
            for _ in range(n):
                uv = list(struct.unpack_from('<6f', me, c + 6))
                s = +1 if front[k] else -1 if tail[k] else 0
                drawn = front_draw[k] if s > 0 else tail_draw[k] if s < 0 else False
                for j, p in enumerate(tris[k][0]):
                    if drawn: uv[2 * j], uv[2 * j + 1] = clip_uv(p, s)
                    elif s: uv[2 * j], uv[2 * j + 1] = BG[s]
                    else: uv[2 * j], uv[2 * j + 1] = (uv[2 * j] % 1.0) * 0.5, (uv[2 * j + 1] % 1.0) * 0.5
                struct.pack_into('<6f', data, base + c + 6, *uv); c += 30; k += 1
    o += sz
assert k == len(tris), (k, len(tris))
open(f'{out}/uv.DAT', 'wb').write(data)
raw = f'{out}/slm_01.rgba'; new.tofile(raw)
Image.fromarray(new[..., :3]).resize((W * 4, H * 4), Image.NEAREST).save(f'{out}/slm_01.png')
subprocess.run(['/home/claude/ffxi_modder/target/release/retexture', f'{out}/uv.DAT', f'{out}/front.DAT', f'slm_01={raw}:{W*2}x{H*2}'], check=True, stdout=subprocess.DEVNULL)
print('wrote', f'{out}/front.DAT')
