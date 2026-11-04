import struct, sys, numpy as np
from PIL import Image
import render_gear as rg
import os; sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')); import tex as T

def load(skel, parts):
    G = rg.skeleton(skel)
    out = []
    for path in parts:
        ch = rg.chunks(open(path, 'rb').read())
        tex = {c[2][1:17].decode('latin1'): T.decode(c[2]) for c in ch
               if c[1] == 0x20 and c[2][0] == 0xa1 and c[2][0x39:0x3d] == b'3TXD'}
        for c in ch:
            if c[1] == 0x2a:
                out += tris_uv(c[2], G, tex)
    return out

def tris_uv(me, G, tex):
    # same decode as render_gear.mesh_tris, but keep per-corner UVs and the texture
    u = lambda o, f: struct.unpack_from(f, me, o)
    f3 = me[2]; sym = me[4] == 1; uja = bool(f3 & 0x80); cloth = bool(f3 & 1)
    ja = u(0xc, '<I')[0]*2; nj = u(0x10, '<H')[0]; vc = u(0x12, '<I')[0]*2; vm = u(0x18, '<I')[0]*2; vd = u(0x1e, '<I')[0]*2
    pal = [u(ja+2*i, '<H')[0] for i in range(nj)]
    J = lambda s: pal[s] if uja else s
    s, dc = u(vc, '<HH'); refs = [u(vm+4*i, '<HH') for i in range(s+dc)]
    W = [[], []]; N = [[], []]; o = vd
    for i in range(s):
        p = u(o, '<3f'); n = u(o+12, '<3f') if not cloth else (0, 1, 0); o += 12 if cloth else 24; r = refs[i][0]
        for m in (0, 1):
            if m and not sym: W[1].append(None); N[1].append(None); continue
            j = J(r & 0x7f) if m == 0 else J(r >> 7 & 0x7f)
            pp = np.array(p) if m == 0 else rg.flip(p, r >> 14); nn = np.array(n) if m == 0 else rg.flip(n, r >> 14)
            W[m].append(G[j][0]@pp+G[j][1]); N[m].append(G[j][0]@nn)
    for i in range(dc):
        a = u(o, '<14f') if not cloth else u(o, '<8f')+(0,)*6; o += 32 if cloth else 56
        r0, r1 = refs[s+i]; p0 = [a[0], a[2], a[4]]; p1 = [a[1], a[3], a[5]]; w0, w1 = a[6], a[7]
        n0 = [a[8], a[10], a[12]]; n1 = [a[9], a[11], a[13]]
        for m in (0, 1):
            if m and not sym: W[1].append(None); N[1].append(None); continue
            j0 = J(r0 & 0x7f) if m == 0 else J(r0 >> 7 & 0x7f); j1 = J(r1 & 0x7f) if m == 0 else J(r1 >> 7 & 0x7f)
            q0 = np.array(p0) if m == 0 else rg.flip(p0, r0 >> 14); q1 = np.array(p1) if m == 0 else rg.flip(p1, r1 >> 14)
            m0 = np.array(n0) if m == 0 else rg.flip(n0, r0 >> 14); m1 = np.array(n1) if m == 0 else rg.flip(n1, r1 >> 14)
            W[m].append(G[j0][0]@q0+w0*G[j0][1]+G[j1][0]@q1+w1*G[j1][1])
            N[m].append(w0*(G[j0][0]@m0)+w1*(G[j1][0]@m1))
    c = u(0x6, '<I')[0]*2; tn = ''; out = []
    while True:
        op, = u(c, '<H'); c += 2
        if op == 0xffff: break
        if op == 0x8010: c += 0x2c; continue
        if op == 0x8000: tn = me[c:c+16].decode('latin1'); c += 16; continue
        n, = u(c, '<H'); c += 2; T = []
        if op == 0x0054:
            for t in range(n): i3 = u(c, '<3H'); uv = u(c+6, '<6f'); T.append((i3, [uv[0:2], uv[2:4], uv[4:6]])); c += 30
        elif op == 0x5453:
            i3 = u(c, '<3H'); uv = u(c+6, '<6f'); c += 30; idx = list(i3); uvs = [uv[0:2], uv[2:4], uv[4:6]]
            for t in range(n-1): idx.append(u(c, '<H')[0]); uvs.append(u(c+2, '<2f')); c += 10
            T = [(idx[k:k+3], uvs[k:k+3]) for k in range(n)]
        elif op == 0x0043:
            for t in range(n): T.append((u(c, '<3H'), None)); c += 10
        elif op == 0x4353:
            i3 = u(c, '<3H'); c += 10; idx = list(i3)
            for t in range(n-1): idx.append(u(c, '<H')[0]); c += 2
            T = [(idx[k:k+3], None) for k in range(n)]
        for i3, uv in T:
            for m in (0, 1):
                if W[m][i3[0]] is None: continue
                out.append(([W[m][x] for x in i3], [N[m][x] for x in i3], uv, tex.get(tn)))
    return out

def render(tris, az_deg, size, center, half, flat=False):
    H, Wd = size
    img = np.zeros((H, Wd, 3)); img[:] = (143/255, 163/255, 184/255)
    zb = np.full((H, Wd), np.inf)
    a = np.radians(az_deg)
    # view: model x (forward) and z (side) rotate about y; screen up is -y
    R = np.array([[np.cos(a), 0, np.sin(a)], [0, 1, 0], [-np.sin(a), 0, np.cos(a)]])
    light = np.array([0.4, -0.7, 0.6]); light /= np.linalg.norm(light)
    sc = (H * 0.92) / (2 * half)
    for P, Nn, uv, tex in tris:
        q = [R @ (p - center) for p in P]
        xs = [Wd/2 + v[0]*sc for v in q]; ys = [H/2 + v[1]*sc for v in q]; zs = [v[2] for v in q]
        x0, x1 = int(max(0, np.floor(min(xs)))), int(min(Wd-1, np.ceil(max(xs))))
        y0, y1 = int(max(0, np.floor(min(ys)))), int(min(H-1, np.ceil(max(ys))))
        if x0 > x1 or y0 > y1: continue
        d = (xs[1]-xs[0])*(ys[2]-ys[0]) - (xs[2]-xs[0])*(ys[1]-ys[0])
        if abs(d) < 1e-9: continue
        gx, gy = np.meshgrid(np.arange(x0, x1+1)+0.5, np.arange(y0, y1+1)+0.5)
        w1 = ((gx-xs[0])*(ys[2]-ys[0]) - (xs[2]-xs[0])*(gy-ys[0])) / d
        w2 = ((xs[1]-xs[0])*(gy-ys[0]) - (gx-xs[0])*(ys[1]-ys[0])) / d
        w0 = 1-w1-w2
        m = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
        if not m.any(): continue
        if tex is not None and uv is not None:
            h, w, _ = tex.shape
            uu = (w0*uv[0][0]+w1*uv[1][0]+w2*uv[2][0]) % 1; vv = (w0*uv[0][1]+w1*uv[1][1]+w2*uv[2][1]) % 1
            px = tex[np.minimum((vv*h).astype(int), h-1), np.minimum((uu*w).astype(int), w-1)]
            col = px[..., :3] / 255
            m &= px[..., 3] >= 128          # alpha test, as the game does
        else:
            col = np.full(gx.shape+(3,), 0.6)
        z = w0*zs[0]+w1*zs[1]+w2*zs[2]
        sub = zb[y0:y1+1, x0:x1+1]
        m &= z < sub
        if not m.any(): continue
        sub[m] = z[m]
        nrm = sum(n*wk for n, wk in zip(Nn, [w0[..., None], w1[..., None], w2[..., None]]))
        nrm = nrm / (np.linalg.norm(nrm, axis=-1, keepdims=True)+1e-9)
        lam = np.abs(nrm @ light)
        shade = 1.0 if flat else (0.45 + 0.75*lam)[..., None]
        img[y0:y1+1, x0:x1+1][m] = np.clip(col*shade, 0, 1)[m]
    return img

if __name__ == '__main__':
    out = sys.argv[1]; skel = sys.argv[2]; parts = sys.argv[3:]
    tris = load(skel, parts)
    allp = np.array([p for t in tris for p in t[0]])
    mn, mx = allp.min(0), allp.max(0); center = (mn+mx)/2; half = (mx-mn)[1]/2
    panels = [render(tris, az, (900, 480), center, half) for az in (0, 45, 90, 180)]
    Image.fromarray((np.concatenate(panels, axis=1)*255).astype(np.uint8)).save(out)
