"""Scale Mail front plates, redrawn: the plate grids on the front (chest and lower panel, both
on the mirror seam at the texture's left edge), with straight lines, stacked plates and a clean
centre line on the seam (half a line in the texture, the mirror completes it).

Grid lines come from the ORIGINAL texture (rows and the column line); colours from the HD
texture (so it matches the rest of the HD pass). Output replaces the block in the HD texture.

usage: python3 front.py <original.DAT> <hd.DAT> <out_dir>    (prints retexture spec)
"""
import sys, os
import numpy as np, cv2
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tex

S = 4   # HD scale

def lum(a): return a[..., :3].astype(np.float32) @ np.array([0.299, 0.587, 0.114], np.float32)

def peaks(p, thr):
    out = []
    for i in range(len(p)):
        if p[i] >= thr and (i == 0 or p[i] >= p[i - 1]) and (i == len(p) - 1 or p[i] > p[i + 1]):
            if not out or i - out[-1] > 3: out.append(i)
    return out

# front blocks on the seam: (y window searched, y range the column lines are measured over)
# and the lowest y a row line may sit at (below it is the frame)
BLOCKS = [(24, 104, 36, 92, 100),       # chest
          (100, 150, 106, 140, 141)]    # lower front panel

def find_grid(a, block=BLOCKS[0]):
    """rows (y of each line, top frame to bottom frame) and cols (x of the inner column line
    and the right frame line) of a front block."""
    wy0, wy1, cy0, cy1, ymax = block
    L = lum(a); D = (L < cv2.GaussianBlur(L, (0, 0), 2.0) - 5).astype(np.float32)
    # column lines: darkness down the block, x 6..27
    colp = D[cy0:cy1, :28].mean(0)
    inner = 6 + int(np.argmax(colp[6:16])); right = 17 + int(np.argmax(colp[17:27]))
    # row lines: darkness across the first column (clear of the seam and the column line)
    rowp = D[wy0:wy1, 2:inner - 1].mean(1)
    ys = [wy0 + y for y in peaks(rowp, 0.4)]
    # regular run of rows: spacing 8..14
    best = []
    for i in range(len(ys)):
        run = [ys[i]]
        for y in ys[i + 1:]:
            if 8 <= y - run[-1] <= 14: run.append(y)
            elif y - run[-1] > 14: break
        if len(run) > len(best): best = run
    best = [y for y in best if wy0 + 6 <= y <= ymax]
    # the top row's upper line sits under the frame and reads weak: add it one spacing up
    if len(best) >= 2 and best[0] - int(round(np.mean(np.diff(best)))) >= wy0 + 6:
        best = [best[0] - int(round(np.mean(np.diff(best))))] + best
    return best, inner, right

def paint_block(hd, rows, inner, right):
    """hd: HxWx4 HD texture (4x). Redraw x 0..right, y rows[0]..rows[-1]."""
    out = hd.copy().astype(np.float32)
    x1 = right * S; y0, y1 = rows[0] * S, rows[-1] * S
    # plate colour: median of the HD plates (centres of cells), so it keeps the HD grade
    cells = []
    for r0, r1 in zip(rows[:-1], rows[1:]):
        for c0, c1 in [(0, inner), (inner, right)]:
            cy0, cy1 = (r0 + 2) * S, (r1 - 1) * S; cx0, cx1 = (c0 + 2) * S, (c1 - 2) * S
            cells.append(hd[cy0:cy1, cx0:cx1, :3].reshape(-1, 3))
    base = np.median(np.concatenate(cells), 0).astype(np.float32)
    line = base * 0.22
    rng = np.random.default_rng(11)
    grain = cv2.GaussianBlur(rng.standard_normal((y1 - y0, x1)).astype(np.float32), (0, 0), 1.2)
    grain = grain / (grain.std() + 1e-6)
    lw = S                                  # line width: one original texel
    for r0, r1 in zip(rows[:-1], rows[1:]):
        p0, p1 = r0 * S, r1 * S
        h = p1 - p0
        t = (np.arange(h, dtype=np.float32) + 0.5) / h            # 0 top .. 1 bottom
        # stacked plate: lit top lip, body fading darker toward the bottom where it tucks
        # under the next row, plus the next row's cast shadow just under the line
        shade = 1.12 - 0.30 * t
        shade[: max(2, h // 8)] += 0.12                          # bright upper edge
        shade[lw: lw + max(2, h // 10)] -= 0.18                  # shadow from the row above
        for c0, c1 in [(0, inner), (inner, right)]:
            q0, q1 = c0 * S, c1 * S
            blk = base[None, None, :] * shade[:, None, None] * (1 + 0.035 * grain[p0 - y0:p1 - y0, q0:q1, None])
            out[p0:p1, q0:q1, :3] = blk
    # lines: rows (full width), the centre line on the mirror seam (half width at x=0, the
    # mirror makes it whole), inner column, right frame edge
    out[y0:y1, 0:lw // 2, :3] = line
    for r in rows:
        out[r * S - lw // 2: r * S + lw // 2, 0:x1, :3] = line
    out[y0:y1, inner * S - lw // 2: inner * S + lw // 2, :3] = line
    out[y0:y1, right * S - lw // 2: right * S + lw // 2, :3] = line
    return np.clip(out, 0, 255).astype(np.uint8)

def main(orig, hd_dat, out_dir):
    os.makedirs(out_dir, exist_ok=True)
    (n0, a), = list(tex.textures(orig))[:1]
    specs = []
    for k, (name, hd) in enumerate(tex.textures(hd_dat)):
        key = name.split()[-1]
        if k == 0:
            for blk, need in zip(BLOCKS, (4, 3)):
                rows, inner, right = find_grid(a, blk)
                print(f'{orig} block {blk[:2]}: rows {rows} inner {inner} right {right}', file=sys.stderr)
                if len(rows) >= need: hd = paint_block(hd, rows, inner, right)
        hd = np.dstack([hd[..., :3], hd[..., 3]]).astype(np.uint8)
        raw = os.path.join(out_dir, key + '.rgba'); hd.tofile(raw)
        cv2.imwrite(os.path.join(out_dir, key + '.png'), cv2.cvtColor(hd[..., :3], cv2.COLOR_RGB2BGR))
        specs.append(f"{key}={raw}:{hd.shape[1]}x{hd.shape[0]}")
    print(' '.join(specs))

if __name__ == '__main__':
    main(*sys.argv[1:4])
