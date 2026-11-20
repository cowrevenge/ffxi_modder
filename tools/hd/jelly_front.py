"""Jelly front marker: the tentacle (lobe) on the jelly's front gets its own copy of the
texture, reshaded red (purple if the base is already red), so you can tell its front from
its back. The texture is doubled in height: the top half is the original, the bottom half
the reshaded copy; front-lobe triangles have their V moved into the bottom half, every
other triangle into the top half. Same mesh, same triangle count, file size grows only by
the larger texture.

usage: python3 front_tint.py <in.DAT> <out_dir>   (writes front.DAT and preview pngs)
"""
import sys, os, struct, subprocess
import numpy as np, cv2
sys.path.insert(0, '/home/claude/work/ctr'); sys.path.insert(0, '/home/claude/work/hd')
import raster, render_gear as rg, tex
from PIL import Image

src, out = sys.argv[1], sys.argv[2]; os.makedirs(out, exist_ok=True)
data = bytearray(open(src, 'rb').read())
tris = raster.load(src, [src])
C = np.array([np.mean(t[0], 0) for t in tris])
top, bot = C[:, 1].min(), C[:, 1].max()           # FFXI y points down: top = min
H_ = bot - top
# front lobe: upper part of the jelly, in the +x (forward) sector
lobe = (C[:, 1] < top + 0.46 * H_) & (C[:, 0] > 0.18) & (C[:, 0] > 1.4 * np.abs(C[:, 2]))
print('front lobe triangles', lobe.sum(), 'of', len(tris))

# texture
(name, img), = [t for t in tex.textures(src) if 'slm_01' in t[0]]
H, W = img.shape[:2]
rgb = img[..., :3].astype(np.float32)
# how far up the lobe each texel sits (fade the tint in from the lobe base to its tip)
height = np.zeros((H, W), np.float32); cnt = np.zeros((H, W), np.float32)
for t, f in zip(tris, lobe):
    if not f: continue
    q = np.array([[u[0] * W, u[1] * H] for u in t[2]], np.float32)
    m = np.zeros((H, W), np.uint8); cv2.fillConvexPoly(m, np.round(q * 4).astype(np.int32), 1, shift=2)
    hgt = (top + 0.46 * H_ - np.mean([p[1] for p in t[0]])) / (0.46 * H_)   # 0 at base, 1 at tip
    height += m * hgt; cnt += m
w = np.clip((np.where(cnt > 0, height / np.maximum(cnt, 1), 0) - 0.10) * 3.6, 0, 1)
w = cv2.GaussianBlur(w, (0, 0), 1.5)
# base colour: red already? then purple
hsv = cv2.cvtColor(np.clip(rgb, 0, 255).astype(np.uint8)[None].reshape(H, W, 3), cv2.COLOR_RGB2HSV)
hue = np.median(hsv[..., 0][hsv[..., 1] > 40]) if (hsv[..., 1] > 40).any() else 0
reddish = (hue < 12 or hue > 165) and np.median(hsv[..., 1]) > 60
dark, lit = (np.array([165, 40, 220]), np.array([245, 190, 255])) if reddish else (np.array([215, 30, 25]), np.array([255, 170, 140]))
print('base hue', hue, '-> marker', 'purple' if reddish else 'red')
L = rgb @ np.array([0.299, 0.587, 0.114], np.float32)
lo, hi = np.percentile(L, 5), np.percentile(L, 99)
t = np.clip((L - lo) / max(hi - lo, 1), 0, 1) ** 0.6
shade = dark[None, None] * (1 - t[..., None]) + lit[None, None] * t[..., None]
marked = rgb * (1 - w[..., None]) + shade * w[..., None]
new = np.zeros((H * 2, W, 4), np.uint8)
new[:H, :, :3] = np.clip(rgb, 0, 255); new[H:, :, :3] = np.clip(marked, 0, 255)
new[:H, :, 3] = img[..., 3]; new[H:, :, 3] = img[..., 3]

# UVs: walk the mesh's triangle lists and squeeze every V into its half
k = 0
for (hdr, kind, me, off), _ in [((c[0], c[1], c[2], o), o) for c, o in zip(rg.chunks(data), [0] * 999)] if False else []: pass
o = 0
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
                f = lobe[k]
                for j in (1, 3, 5): uv[j] = (uv[j] % 1.0) * 0.5 + (0.5 if f else 0.0)
                struct.pack_into('<6f', data, base + c + 6, *uv); c += 30; k += 1
    o += sz
assert k == len(tris), (k, len(tris))
open(f'{out}/uv.DAT', 'wb').write(data)
raw = f'{out}/slm_01.rgba'; new.tofile(raw)
Image.fromarray(new[..., :3]).resize((W * 4, H * 8), Image.NEAREST).save(f'{out}/slm_01.png')
subprocess.run(['/home/claude/ffxi_modder/target/release/retexture', f'{out}/uv.DAT', f'{out}/front.DAT', f'slm_01={raw}:{W}x{H*2}'], check=True, stdout=subprocess.DEVNULL)
print('wrote', f'{out}/front.DAT')
