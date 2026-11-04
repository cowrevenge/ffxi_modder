# How the gear texture upgrades were made

This is the full process behind the Centurion set, Bastok Pride, Bounding Boots HD and
Battle Gloves HD mods in [`ffxi_mod_files`](https://github.com/cowrevenge/ffxi_mod_files),
with every pitfall we hit along the way. The scripts live in [`tools/paint`](../tools/paint).

The short version: **don't filter the old texture. Describe the gear in words, then paint
it new from those words** on the original UV layout. Everything else is about reading
that layout correctly and checking the result the way the game will draw it.

---

## 1. Find the files

1. Get the item's model id for the slot, then resolve it to a DAT id with the client's own
   table (`MainDll::equipment_model_index(race, slot, model)`, read from FFXiMain.dll's
   `.data`). Slots: 1 head, 2 body, 3 hands, 4 legs, 5 feet, 6 main hand.
2. Resolve the DAT id to a `ROM\<dir>\<file>.DAT` path through VTABLE/FTABLE, as in the
   main README. **Do this per install.** The same id can land in a different folder on
   another client.
3. Copy the original DAT and the race's skeleton DAT somewhere to work on. Never edit the
   game's files.

Each gear DAT has one or more DXT3 textures (chunk kind `0x20`, `3TXD`) and the skinned
meshes that use them. `tools/paint/tex.py` decodes the textures to RGBA.

## 2. Read the layout before drawing anything

The UV layout is locked: every object has to be redrawn exactly where it sits now, or the
mesh shows the wrong thing in the wrong place. Three tools in `tools/paint/preview` do
the reading:

- `texgrid.py <dat> <out> <skeleton>`: the texture zoomed with a labelled 8-texel grid,
  see-through texels in green, mirror seams in cyan. You read object positions in texels
  straight off it.
- `view.py <skeleton> <out.png> <size> <dat>...`: the gear on the skeleton from four
  sides, alpha-tested like the game.
- `mirror.py`: finds the mirror seams on its own (see the pitfalls).

**Probe what the model actually samples.** When something on the model looks wrong and
you can't tell which texels it comes from, render the mesh with a fake texture whose
colour *is* the texel coordinate (red = x, green = y), with `flat=True` so lighting
doesn't change the values, and read the pixel:

```python
yy, xx = np.mgrid[0:H, 0:W]
fake = np.dstack([xx * 2, yy * 4, np.zeros_like(xx), real_alpha]).astype(float)  # keep the real alpha!
tris2 = [(P, N, uv, fake if tx is not None else None) for P, N, uv, tx in tris]
img = raster.render(tris2, az, size, center, half, flat=True) * 255
# texel at screen (x, y) = (img[y, x, 0] / 2, img[y, x, 1] / 4)
```

This is how we found that the round neck collar is a tiny 18x9 texel patch in a corner,
that the "blue under the heel" was one small triangle sampling texel (21, 12), and that
the blue on the shoulders was the collar roll.

## 3. Write the brief

For every piece, write down in plain words what each object is, what it's made of and
where it sits, in texels (see `tools/paint/BRIEF.md`). Then write a style section that all
pieces share: palette, tones per material, how plates stack, where ink goes.

This step is not optional. Every time we skipped it and "just fixed" the old texture, the
result was worse.

## 4. Paint

`tools/paint/paint.py` is the painter; `paint_<piece>.py` is one per piece, written
straight from the brief.

- **Supersampled vector shapes.** Everything is a polygon or ellipse in source texels,
  drawn at 16 px per texel and downsampled (LANCZOS) to 4x. Edges come out hard and
  clean, with no stair-steps and no blur.
- **Cel colour, 3 tones per material** (shadow, base, light) plus a white glint.
- **Ink** is drawn by eroding a shape's mask, and only inside shapes.
- **A material map.** Every shape records its material (steel, leather, cloth, canvas);
  ink, stitching and glints record none. After the downsample each material gets its
  surface, scaled down on the brightest tones:
  - steel: brushed grain, fine scratches (drawn at 2x), small dents, a faint temper mottle;
  - leather: pebbled grain, creases, scuffed lighter edges;
  - cloth: twill weave and threads; canvas: a coarse weave.
- **Metal shading** (`metal()`): hot top band, base, a hard tilted horizon reflection,
  bounce light, dark lower edge, a lit top-left bevel and dark bottom-right bevel, one
  diagonal glint.
- **Stacked plates** (`lame()`, `scale_plate()`): see the pitfalls; rows are drawn bottom
  up with cast shadows.
- **Small parts**: `strap`, `stitch` (with a shadowed hole under each dash), `rivet`,
  piping, buckles.

`runpaint.py <module> <out_dir> <dat>` paints every texture in the DAT and prints the
spec line for `retexture`.

## 5. Build the DAT

```bash
cargo run --release --bin retexture -- original.DAT new.DAT hm_b61_3=out/hm_b61_3.rgba:512x1024
```

`retexture` swaps the texture chunk for the new size and pixels and keeps every other
chunk byte for byte. The texture is re-encoded as DXT3, so alpha is stored at 4 bits.

## 6. Check

1. **Render with the alpha test on** (`view.py`, or `raster.render`). A preview that
   ignores alpha shows parts the game never draws. That cost us a full round.
2. **Compare side by side** with the original and with any other version of the set
   (we compared Bastok Pride against the Centurion HD set on every pass).
3. **Look at the whole outfit at once**, not just one piece. Colour balance problems
   (see below) only show up together.
4. **Then check in game.** The game found two issues the preview missed: cloth showing
   between the bottom scale points, and blue on the shoulder tops.

## 7. Ship

- One Pivot overlay folder per mod: `<Mod>\ROM\<dir>\<file>.DAT`, using the paths
  resolved in step 1. Hume male paths are only right for Hume male, and only on clients
  whose VTABLE maps the items the same way.
- Mods that don't share any files can run together; for mods that do, the higher one in
  `/pivot` wins.
- Record a SHA256 for every shipped DAT.

---

## Pitfalls and catches

### Layout and seams

- **Mirror seams are not at the texture border.** On the body the front centre seam is
  at x = 1 and the back seam at x = 127, not 0 and 128. Being off by one or two texels
  makes the mirrored halves of a shape not line up (lopsided centre scale, broken brick
  pattern). Measure seams; never assume them.
- **How to find a mirror seam:** a 3D edge shared by two triangles that carry the
  *identical* UV triangle (a triangle and its mirrored copy). Interior edges also share a
  3D edge but their two triangles have different UV triangles. Counting "UV edges used
  once" does not work: that also catches every island edge.
- **No ink on a seam, but keep the lines the original had there.** An outline drawn along
  a seam gets doubled or broken by the mirror. Shapes that cross a seam are drawn past it
  so they mirror into one whole shape. But where the original had a deliberate centre
  line (the skirt plates), leaving it out looks wrong: end the plates on the seam with
  their ink edge so the mirror makes one thin centre line. A thick riveted strap there
  was too much.
- **Symmetric details on a seam must be symmetric about the seam.** The cow emblem sits
  on x = 1, so each mirrored half only shows one side of it.
- **Small regions you'd never guess.** The neck collar, the instep under the heel and the
  shoulder tops each sample a few texels in places that look unrelated. When a colour
  shows up where it shouldn't, probe it (step 2) instead of guessing.

### Transparency

- **Keep the original alpha.** The cut-outs (pauldron points, the gauntlet finger points,
  the shin plate wings, the visor) are in the alpha channel. Upscale it bicubic and then
  threshold it, so edges are smooth curves but strictly in or out.
- **Let the alpha shape the plate, don't fight it.** Drawing pointed plates on a guessed
  grid left visible cloth between your points and the real cut-out. Paint the plates as
  full strips with the dividers at the original notches, and the alpha gives each one
  its point. Then ink along the cut edge.
- **Fill transparent texels with the nearest visible colour**, never with the base cloth.
  Texture filtering samples them at the cut edge and bleeds that colour in.
- **The preview must alpha-test.** See step 6.

### Plates

- **Plating stacks; it is not a grid of boxes.** Draw rows bottom-up so every row laps
  over the top of the row below. Cast a shadow from each plate before drawing it. Shade
  the strip just under the lap darker, and give the lower edge a rolled, lit rim.
  Without these it reads as separate tiles even when the shapes overlap.
- **Scale mail is a brick pattern.** Alternate rows are offset by half a scale.
- **Fill behind the plates with metal.** Wherever points or gaps leave space, whatever is
  underneath shows. Put a dark steel backing under the scales and tuck an extra row under
  the bottom one, or cloth shows between the points.

### Surfaces

- **Clean flat colour with no surface reads as cartoon, not metal.** Bright colour is
  fine, but every material needs a surface: steel needs grain, scratches and a hard
  reflection; leather needs grain and wear; cloth needs weave.
- **Apply surfaces after the downsample, per material.** If you put noise on the 16x
  canvas, the downsample averages it away. If you put it on everything, it lands on ink
  and glints and dirties them.
- **Polished metal has no scratches.** The brass emblem reads better as a clean relief.

### Colour across a set

- **Leather overpowers easily.** Bright orange-tan straps, boots and gloves pulled the eye
  away from the steel and cloth. The fix was a cooler, darker, less saturated brown,
  taken from the HD set (shadow `#3e281b`, base `#6e472e`, light `#9c6d48`, stitching
  `#c9ad84`).
- **Big areas decide the look, small accents don't.** Shoes in brown leather made the
  whole lower body read brown. Making the greave shoes and padded panels the same indigo
  as the trousers fixed the set's balance at once. Keep leather for straps, heels and
  trim.
- **Match the original's proportions.** Straps drawn a few texels wider than the original
  looked huge on the model. Measure strap widths on the original texture, column by
  column, and copy them.
- **Don't carry one set's colour into gear from another set.** The Battle Gloves and
  Bounding Boots are cloth and leather. Indigo (the Bastok colour) on them looked wrong.
- **Contrast keeps shapes readable.** A brown shoe upper on a brown sole lost the arch;
  a darker upper brought it back.

---

## What didn't work

All of these were tried on the old texture and dropped:

- **AI upscaling** (Real-ESRGAN realesr-general-x4v3): sharper, but painted lines came out
  soft and blurry.
- **A shock filter on the upscale** to harden lines: the hard lines came out jagged and
  stair-stepped, and it mangled small details like buttons and rivets.
- **Mean-shift / k-means "cel" filters**: posterized gradients into blobs and lost the
  plating.
- **"Anime" restyling filters**: rejected outright.
- **Geometry smoothing** (subdividing the meshes): the original edges are hard on
  purpose; rounding them made the gear worse. Removed.
- **Patching the filtered result** (detecting rivets, redrawing buttons and plates on
  top): every fix uncovered another problem. Starting over from a written brief was
  faster and better.
