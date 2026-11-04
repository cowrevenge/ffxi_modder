# paint: new gear textures from a written brief

Each piece of gear is first described in words (`BRIEF.md`), then painted new from that
description on the piece's locked UV layout. The old texture is only used for where
things sit, where the see-through cut-outs are (its alpha) and where the mirror seams run.

- `paint.py`: the painter. Shapes are given in source texels, drawn at 16 px per texel and
  downsampled to 4x, so edges are hard with no stair-steps. Every shape records its
  material; after the downsample each material gets its surface (brushed, scratched,
  dented steel; pebbled, creased, scuffed leather; woven cloth and canvas). Stacked plates
  (`lame`), shield scales, hanging plates, straps, stitching, rivets, piping.
- `paint_<piece>.py`: one per piece, written straight from its section of `BRIEF.md`.
- `runpaint.py <module> <out_dir> <dat>`: paints every texture in the DAT and prints the
  spec for `retexture`.

```bash
python3 tools/paint/runpaint.py paint_body out/ ROM/28/33.DAT
# -> hm_b61_3=out/hm_b61_3.rgba:512x1024
cargo run --release --bin retexture -- ROM/28/33.DAT out/33.DAT hm_b61_3=out/hm_b61_3.rgba:512x1024
```

Needs numpy, scipy and Pillow. These scripts made the Bastok Pride, Bounding Boots HD
and Battle Gloves HD mods in `ffxi_mod_files`.
