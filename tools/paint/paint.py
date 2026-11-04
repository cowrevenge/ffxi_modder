"""Paint new gear textures from the brief (BRIEF.md), on the locked UV layout.

Shapes are given in source texels and drawn at 16 px per texel, then downsampled
to 4x: hard edges, no stair-steps. Every shape also writes its material into a
material map; after the downsample each material gets its surface texture
(brushed and scratched steel, pebbled and worn leather, woven cloth), so the
colours stay bright and cel-banded but the surfaces read as real materials.
Ink, stitching and highlights carry no material, so they stay clean.

Transparency is the original texture's, upscaled, so cut-outs stay exactly where
the mesh expects them. Anything that crosses a mirror seam (the texture border)
is drawn past the border, so no ink line ever lands on a seam.
"""
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

SS = 16          # canvas px per source texel
OUT = 4          # output px per source texel

def hexc(s):
    s = s.lstrip('#'); return np.array([int(s[i:i + 2], 16) for i in (0, 2, 4)], np.float64)

PAL = {
    'steel': [hexc('5c6788'), hexc('b7c3dc'), hexc('e8eef9')],
    'leather': [hexc('5a2f1c'), hexc('9a5a32'), hexc('c98a54')],
    'stitch': hexc('e7c58e'),
    'cloth': [hexc('1c1f45'), hexc('2f3570'), hexc('4a539c')],
    'canvas': [hexc('6b4a30'), hexc('a57a52'), hexc('d0a878')],
    'seam': hexc('3a2414'),
    'ink': hexc('140e24'),
    'spec': hexc('ffffff'),
}
DARK_STEEL = [PAL['steel'][0] * 0.7, PAL['steel'][0] * 1.1, PAL['steel'][0] * 1.5]
# cooler, less saturated leather (the HD set's browns) for the Bastok Pride set
COOL_LEATHER = [hexc('3e281b'), hexc('6e472e'), hexc('9c6d48')]
COOL_STITCH = hexc('c9ad84')
CHARCOAL = [hexc('121418'), hexc('25282e'), hexc('444850')]

def use_cool_leather():
    PAL['leather'][:] = COOL_LEATHER
    PAL['stitch'][:] = COOL_STITCH

INK_W = 0.42     # texels
STITCH_W = 0.22

NONE, STEEL, LEATHER, CLOTH, CANVAS = 0, 1, 2, 3, 4

def mix(a, b, t):
    return a * (1 - t) + b * t

class Canvas:
    def __init__(self, w, h, base, base_mat=CLOTH, seed=7):
        self.w, self.h = w, h
        self.img = np.zeros((h * SS, w * SS, 3), np.float64)
        self.img[:] = base
        self.mat = np.full((h * SS, w * SS), base_mat, np.uint8)
        self.rng = np.random.default_rng(seed)

    # -- masks ---------------------------------------------------------------
    def poly_mask(self, pts):
        m = Image.new('L', (self.w * SS, self.h * SS), 0)
        ImageDraw.Draw(m).polygon([(x * SS, y * SS) for x, y in pts], fill=255)
        return np.array(m) > 0

    def ellipse_mask(self, cx, cy, rx, ry):
        m = Image.new('L', (self.w * SS, self.h * SS), 0)
        ImageDraw.Draw(m).ellipse([(cx - rx) * SS, (cy - ry) * SS, (cx + rx) * SS, (cy + ry) * SS], fill=255)
        return np.array(m) > 0

    def rect(self, x0, y0, x1, y1):
        return self.poly_mask([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])

    def paint(self, mask, col, mat=NONE):
        self.img[mask] = col; self.mat[mask] = mat

    def rows(self):
        return (np.arange(self.h * SS) + 0.5)[:, None] / SS

    def cols(self):
        return (np.arange(self.w * SS) + 0.5)[None, :] / SS

    # -- cel-shaded shape ------------------------------------------------------
    def shade(self, mask, tones, y0, y1, light=0.22, shadow=0.28, ink=True, spec=None, mat=NONE, outline_skip=None):
        """mask filled with 3 cel tones banded top (light) to bottom (shadow)."""
        Y = np.broadcast_to(self.rows(), mask.shape)
        t = (Y - y0) / max(y1 - y0, 1e-6)
        self.img[mask] = tones[1]
        self.img[mask & (t < light)] = tones[2]
        self.img[mask & (t > 1 - shadow)] = tones[0]
        self.mat[mask] = mat
        if spec is not None:
            self.tick(mask, *spec)
        if ink:
            self.outline(mask, skip=outline_skip)

    def tick(self, mask, sx, sy, sl):
        m = self.poly_mask([(sx, sy), (sx + sl, sy - sl * 0.15), (sx + sl, sy + 0.45), (sx, sy + 0.6)]) & mask
        self.paint(m, PAL['spec'])

    def outline(self, mask, width=INK_W, col=None, skip=None):
        r = max(1, int(round(width * SS)))
        inner = ndimage.binary_erosion(mask, iterations=r, border_value=1)
        edge = mask & ~inner
        if skip is not None:
            edge &= ~skip
        self.paint(edge, PAL['ink'] if col is None else col)

    def cast(self, mask, dx=0.5, dy=0.9, k=0.5):
        """drop shadow of a shape onto whatever is already painted below/right of it."""
        sh = ndimage.shift(mask.astype(np.uint8), (dy * SS, dx * SS), order=0, cval=0) > 0
        sh &= ~mask
        self.img[sh] *= k

    def stitch(self, x0, y0, x1, y1, col=None, dash=1.2, gap=0.8):
        """dashed stitch line between two points (texels)."""
        n = np.hypot(x1 - x0, y1 - y0)
        steps = int(n / (dash + gap)) + 1
        for i in range(steps):
            a = i * (dash + gap) / max(n, 1e-6); b = min(1, a + dash / max(n, 1e-6))
            if a >= 1: break
            ax, ay = x0 + (x1 - x0) * a, y0 + (y1 - y0) * a
            bx, by = x0 + (x1 - x0) * b, y0 + (y1 - y0) * b
            dx, dy = -(by - ay), (bx - ax); L = np.hypot(dx, dy) or 1
            dx, dy = dx / L * STITCH_W, dy / L * STITCH_W
            m = self.poly_mask([(ax + dx, ay + dy), (bx + dx, by + dy), (bx - dx, by - dy), (ax - dx, ay - dy)])
            # stitch hole shadow first, then the thread
            hole = ndimage.shift(m.astype(np.uint8), (0.25 * SS, 0.2 * SS), order=0) > 0
            self.img[hole & ~m] *= 0.55
            self.paint(m, PAL['stitch'] if col is None else col)

    def rivet(self, cx, cy, r=1.4):
        self.cast(self.ellipse_mask(cx, cy, r, r), 0.35, 0.45, 0.5)
        self.paint(self.ellipse_mask(cx, cy, r, r), PAL['ink'])
        self.paint(self.ellipse_mask(cx, cy, r * 0.78, r * 0.78), PAL['steel'][0])
        self.paint(self.ellipse_mask(cx - r * 0.08, cy - r * 0.08, r * 0.62, r * 0.62), PAL['steel'][1])
        self.paint(self.ellipse_mask(cx - r * 0.3, cy - r * 0.3, r * 0.26, r * 0.26), PAL['spec'])

    # -- metal -----------------------------------------------------------------
    def metal(self, mask, y0, y1, x0=None, x1=None, tones=None, glint=True, ink=True):
        """bright anime steel: hot top band, base, a hard tilted horizon reflection,
        bounce light, dark lower edge, bevel rims and a diagonal glint."""
        tones = PAL['steel'] if tones is None else tones
        sh, base, lt = tones
        if x0 is None:
            xs = np.where(mask.any(0))[0]; x0, x1 = xs[0] / SS, (xs[-1] + 1) / SS
        Y = np.broadcast_to(self.rows(), mask.shape); X = np.broadcast_to(self.cols(), mask.shape)
        w = max(x1 - x0, 1e-6); h = max(y1 - y0, 1e-6)
        t = (Y - y0) / h + 0.12 * (X - (x0 + x1) / 2) / w      # slight tilt
        horizon = mix(base, sh, 0.55); bounce = mix(base, sh, 0.18)
        self.img[mask] = base
        self.img[mask & (t < 0.2)] = lt
        self.img[mask & (t >= 0.47) & (t < 0.58)] = horizon
        self.img[mask & (t >= 0.58)] = bounce
        self.img[mask & (t > 0.8)] = sh
        self.mat[mask] = STEEL
        # bevel: lit rim on the top and left, dark rim on the bottom and right
        r = max(1, int(0.55 * SS))
        inner = ndimage.binary_erosion(mask, iterations=r, border_value=1)
        rim = mask & ~inner
        up = ndimage.shift(mask.astype(np.uint8), (r, r), order=0) > 0
        self.img[rim & ~up] = mix(lt, PAL['spec'], 0.5)
        dn = ndimage.shift(mask.astype(np.uint8), (-r, -r), order=0) > 0
        self.img[rim & ~dn & up] = mix(sh, PAL['ink'], 0.3)
        if glint and w > 6:
            d = (X - x0) + 0.55 * (Y - y0)
            g0 = w * 0.55
            band = mask & inner & (((d > g0) & (d < g0 + w * 0.12)) | ((d > g0 + w * 0.17) & (d < g0 + w * 0.2)))
            self.img[band] = mix(self.img[band], PAL['spec'], 0.35)
        if ink:
            self.outline(mask)

    # -- objects ---------------------------------------------------------------
    def strap(self, x0, y0, x1, y1, tones=None, stitch=True, vertical=None, cast=False):
        tones = PAL['leather'] if tones is None else tones
        m = self.rect(x0, y0, x1, y1)
        if cast: self.cast(m, 0.35, 0.5, 0.6)
        vertical = (y1 - y0) > (x1 - x0) if vertical is None else vertical
        if vertical:
            X = np.broadcast_to(self.cols(), m.shape); t = (X - x0) / max(x1 - x0, 1e-6)
            self.img[m] = tones[1]; self.img[m & (t < 0.22)] = tones[2]; self.img[m & (t > 0.78)] = tones[0]
            self.mat[m] = LEATHER
            if stitch and x1 - x0 > 3:
                self.stitch(x0 + 0.9, y0 + 0.5, x0 + 0.9, y1 - 0.5); self.stitch(x1 - 0.9, y0 + 0.5, x1 - 0.9, y1 - 0.5)
        else:
            self.shade(m, tones, y0, y1, ink=False, mat=LEATHER)
            if stitch and y1 - y0 > 3:
                self.stitch(x0 + 0.5, y0 + 0.9, x1 - 0.5, y0 + 0.9); self.stitch(x0 + 0.5, y1 - 0.9, x1 - 0.5, y1 - 0.9)
        return m

    def scale_plate(self, cx, top, half, height, point=0.4, clip=None):
        """shield scale: straight top, sides, chevron point at the bottom centre."""
        bot = top + height
        side = top + height * (1 - point)
        m = self.poly_mask([(cx - half, top), (cx + half, top), (cx + half, side), (cx, bot), (cx - half, side)])
        if clip is not None: m &= clip
        if not m.any(): return m
        self.cast(m)
        self.metal(m, top, bot, cx - half, cx + half)
        return m

    def lame(self, x0, y0, x1, y1, curve=1.8, clip=None, tones=None, rivets=(), lapped=True):
        """one lame of a stacked row (shingles): straight top that slides under the
        lame above, a shadowed strip just below that lap, a bright body and a
        rolled lower edge that catches the light and laps over the lame below."""
        tones = PAL['steel'] if tones is None else tones
        n = 24
        bot = lambda x: y1 - curve + curve * (1 - ((x - (x0 + x1) / 2) / ((x1 - x0) / 2)) ** 2)
        pts = [(x0, y0), (x1, y0)] + [(x1 - (x1 - x0) * i / n, bot(x1 - (x1 - x0) * i / n)) for i in range(n + 1)]
        m = self.poly_mask(pts)
        if clip is not None: m &= clip
        self.cast(m, 0.25, 1.3, 0.4)
        self.metal(m, y0, y1, x0, x1, tones=tones, ink=False)
        Y = np.broadcast_to(self.rows(), m.shape); X = np.broadcast_to(self.cols(), m.shape)
        d = bot(X) - Y                                            # texels above the lower edge
        t = (Y - y0) / (y1 - y0)
        sh, base, lt = tones
        if lapped:
            self.img[m & (t < 0.3)] = mix(base, sh, 0.55)          # in the shadow of the lap
            self.img[m & (t < 0.12)] = mix(base, sh, 0.8)
        self.img[m & (d < 1.6) & (d >= 0.9)] = mix(lt, PAL['spec'], 0.6)   # rolled edge
        self.img[m & (d < 2.6) & (d >= 1.6)] = mix(base, sh, 0.3)
        self.outline(m)
        for rx in rivets:
            self.rivet(rx, y0 + (y1 - y0) * 0.45, 1.1)
        return m

    def hanging_plate(self, x0, x1, top, bot, point=0.35, clip=None):
        cx = (x0 + x1) / 2
        side = top + (bot - top) * (1 - point)
        m = self.poly_mask([(x0, top), (x1, top), (x1, side), (cx, bot), (x0, side)])
        if clip is not None: m &= clip
        self.cast(m, 0.6, 0.8, 0.5)
        self.metal(m, top, bot, x0, x1)
        return m

    def square_plate(self, x0, y0, x1, y1):
        r = min(x1 - x0, y1 - y0) * 0.18
        m = self.poly_mask(rounded_rect(x0, y0, x1, y1, r))
        self.cast(m)
        self.metal(m, y0, y1, x0, x1)
        return m

    def cloth_panel(self, pts, tones=None, folds=(), mat=CLOTH):
        tones = PAL['cloth'] if tones is None else tones
        m = self.poly_mask(pts)
        ys = [p[1] for p in pts]
        self.shade(m, tones, min(ys), max(ys), light=0.12, shadow=0.2, ink=False, mat=mat)
        for (fx0, fy0, fx1, fy1) in folds:
            fm = self.poly_mask([(fx0 - 0.35, fy0), (fx0 + 0.35, fy0), (fx1 + 0.35, fy1), (fx1 - 0.35, fy1)]) & m
            self.img[fm] = tones[0]
        return m

    # -- output ----------------------------------------------------------------
    def result(self, alpha_src):
        W, H = self.w * OUT, self.h * OUT
        im = Image.fromarray(np.clip(self.img, 0, 255).astype(np.uint8)).resize((W, H), Image.LANCZOS)
        rgb = np.array(im).astype(np.float64)
        f = SS // OUT
        frac = lambda k: (self.mat == k).reshape(H, f, W, f).mean((1, 3))
        rng = self.rng
        lum = rgb.mean(-1, keepdims=True)
        def add(delta, k):
            nonlocal rgb
            # texture follows the local tone: weaker on the hottest highlights
            rgb += (delta * frac(k))[..., None] * (1.15 - 0.6 * lum / 255)
        add(steel_texture(H, W, rng), STEEL)
        add(leather_texture(H, W, rng, frac(LEATHER)), LEATHER)
        add(cloth_texture(H, W, rng, 4.0), CLOTH)
        add(cloth_texture(H, W, rng, 7.0, coarse=True), CANVAS)
        # alpha: the original cut-outs, upscaled smooth and then made hard (in or out)
        a = alpha_mask(alpha_src, W, H)
        # transparent texels take the nearest visible colour, so filtering never
        # bleeds a stray colour into the cut edges
        idx = ndimage.distance_transform_edt(~a, return_distances=False, return_indices=True)
        rgb = rgb[idx[0], idx[1]]
        return np.dstack([np.clip(rgb, 0, 255), a * 255]).astype(np.uint8)

    def alpha(self, alpha_src):
        """the visible area at canvas resolution."""
        return alpha_mask(alpha_src, self.w * SS, self.h * SS)

    def cut_outline(self, alpha_src):
        """ink along every cut-out edge, on the visible side."""
        a = self.alpha(alpha_src)
        self.outline(a)

def alpha_mask(alpha_src, W, H):
    a = Image.fromarray(alpha_src.astype(np.uint8)).resize((W, H), Image.BICUBIC)
    return np.array(a) >= 128

# -- surface textures (output resolution, values are luminance offsets) ----------
def steel_texture(H, W, rng):
    brushed = ndimage.gaussian_filter(rng.standard_normal((H, W)), (0.45, 9), mode='wrap')
    brushed *= 7 / brushed.std()
    grain = ndimage.gaussian_filter(rng.standard_normal((H, W)), 0.5, mode='wrap') * 9
    # scratches, drawn at 2x and boxed down: thin bright cut with a dark lip below it
    S = 2
    hi = Image.new('L', (W * S, H * S), 0); lo = Image.new('L', (W * S, H * S), 0)
    dh, dl = ImageDraw.Draw(hi), ImageDraw.Draw(lo)
    for _ in range(H * W // 110):
        x, y = rng.uniform(0, W * S), rng.uniform(0, H * S)
        ang = rng.normal(-0.35, 0.45) if rng.random() < 0.8 else rng.uniform(0, np.pi)
        L = rng.uniform(3, 14) * S
        x2, y2 = x + L * np.cos(ang), y + L * np.sin(ang)
        v = int(rng.uniform(120, 255))
        dh.line([(x, y), (x2, y2)], fill=v, width=1)
        dl.line([(x, y + 1.5), (x2, y2 + 1.5)], fill=v, width=1)
    box = lambda im: np.array(im, np.float64).reshape(H, S, W, S).mean((1, 3)) / 255
    scratch = 40 * box(hi) - 26 * box(lo)
    # small dents / pits
    pits = np.zeros((H, W)); n = H * W // 900
    pits[rng.integers(0, H, n), rng.integers(0, W, n)] = 1
    pits = ndimage.gaussian_filter(pits, 0.6) * -110
    # faint large-scale tempering mottle
    mottle = ndimage.gaussian_filter(rng.standard_normal((H, W)), 10, mode='wrap'); mottle *= 6 / mottle.std()
    return brushed + grain + scratch + pits + mottle

def leather_texture(H, W, rng, frac):
    n = ndimage.gaussian_filter(rng.standard_normal((H, W)), 1.1, mode='wrap'); n /= n.std()
    crease = np.exp(-(n / 0.28) ** 2) * -17                # pebble grain: dark creases between bumps
    pebble = np.clip(n, 0, None) * 6
    mottle = ndimage.gaussian_filter(rng.standard_normal((H, W)), 7, mode='wrap'); mottle *= 11 / mottle.std()
    fine = rng.standard_normal((H, W)) * 3
    # scuffed edges: lighter, broken wear along every leather border
    edge = (frac > 0.5) & (ndimage.uniform_filter(frac, 7) < 0.86)
    wear_n = ndimage.gaussian_filter(rng.standard_normal((H, W)), 1.5); wear_n /= wear_n.std()
    wear = ndimage.gaussian_filter((edge & (wear_n > -0.2)).astype(np.float64), 0.7) * 26
    # creases across the strap
    cr = np.zeros((H, W))
    for _ in range(H * W // 2500):
        y, x = rng.integers(0, H), rng.integers(0, W); L = rng.integers(4, 12)
        cr[y, max(0, x - L // 2):x + L // 2] = 1
    cr = ndimage.gaussian_filter(cr, (0.6, 1.0)) * -40
    return crease + pebble + mottle + fine + wear + cr

def cloth_texture(H, W, rng, amp, coarse=False):
    yy, xx = np.mgrid[0:H, 0:W]
    if coarse:
        weave = np.sin(xx * np.pi / 1.5) * np.sin(yy * np.pi / 1.5) * amp
    else:
        weave = (((xx + yy) % 4) < 2) * amp - amp / 2       # twill diagonal
    threads = ndimage.gaussian_filter(rng.standard_normal((H, W)), (6, 0.4), mode='wrap'); threads *= amp / threads.std()
    slub = ndimage.gaussian_filter(rng.standard_normal((H, W)), (0.4, 5), mode='wrap'); slub *= amp * 0.6 / slub.std()
    mottle = ndimage.gaussian_filter(rng.standard_normal((H, W)), 9, mode='wrap'); mottle *= amp * 1.2 / mottle.std()
    return weave + threads + slub + mottle

def rounded_rect(x0, y0, x1, y1, r, n=6):
    pts = []
    for cx, cy, a0 in ((x1 - r, y0 + r, -90), (x1 - r, y1 - r, 0), (x0 + r, y1 - r, 90), (x0 + r, y0 + r, 180)):
        for i in range(n + 1):
            a = np.radians(a0 + 90 * i / n)
            pts.append((cx + r * np.cos(a), cy + r * np.sin(a)))
    return pts
