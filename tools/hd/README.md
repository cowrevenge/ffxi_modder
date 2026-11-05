# HD gear pass

4x textures on one shared palette, for any gear DAT. Geometry is left as authored.

```
pip install torch --index-url https://download.pytorch.org/whl/cpu
pip install numpy pillow
curl -L -o tools/hd/realesr-general-x4v3.pth https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/realesr-general-x4v3.pth

python3 tools/hd/hd.py tex in.DAT                                                   # prints a retexture spec line
cargo run --release --bin retexture -- in.DAT out.DAT <spec from that line>
cargo run --release -- out.DAT --check                                              # must say byte-identical
```

**Textures (`hd.py`).**

1. **Palette.** Pixels are softly classed as leather (warm, saturated), cloth (dark, low
   saturation; olive counts as cloth) or plate (light, low saturation). Each class's mean
   moves to one shared target (`TARGET`) while each pixel keeps its own deviation, so the
   painted detail survives and every piece lands on the same browns, charcoals and steels.
2. **4x upscale** with Real-ESRGAN `realesr-general-x4v3` (BSD-3), loaded straight into a
   small torch module in `esr.py`. Edges wrap so tiling UVs stay seamless.
3. **Material detail:** leather grain and pores, a light brushed streak on plate, unsharp
   mask.
4. **Alpha** is the original's, upscaled, so cutouts keep their shape.

`retexture` re-encodes each texture as DXT3 at the new size (PCA block endpoints), copies
every other chunk byte for byte and re-reads what it wrote.

Retail draws gear with back-face culling off, so outline tricks that rely on an inverted
hull mesh do not work on FFXI gear; outlines have to be painted into the texture.

## All-gear HD pass (`allgear.py`)

Keeps every item's own colours. It applies the Centurion HD set's clean-up as *relative*
moves per material (leather a little lighter and richer, grey cloth faintly cool, plate
brighter, dyed colours treated like leather), so a red set gets the same treatment in red.
Speckle is smoothed before the 4x upscale; original alpha values are kept.
Bare skin painted into gear (thighs, midriffs, arms) keeps its colour: `skin.py` finds the
race's own skin tone in smooth patches and takes it out of the grade (set `RACE`, e.g.
`RACE=Mithra`; the batch sets it from the table). Hume F, Elvaan F, Mithra and Galka; the
other races' skin is the same colour as brown leather and their gear shows little of it.

```bash
python3 tools/hd/allgear.py out/ in.DAT            # prints the retexture spec
python3 tools/hd/allgear_batch.py --ffxi "<FINAL FANTASY XI>" --table gear_table_paths.csv \
    --out "<Pivot DATs>\AllGearHD" --retexture target/release/retexture.exe [--skip skip.txt]
```

`allgear_batch.py` resolves every DAT in the table, writes one Pivot overlay folder,
uses all cores, is resumable and logs failures. `--skip` lists DAT ids a hand-made mod
already covers.

`scale_front.py`: Scale Mail's front plate grids (chest and lower panel, on the mirror seam)
redrawn after the pass: straight lines from the original's grid, stacked plates in the HD
colours, and half a centre line at the seam so the mirror makes it whole.
`python3 tools/hd/scale_front.py <original.DAT> <hd.DAT> out/` prints the retexture spec.

`eisen.py`: the same pass plus a polished finish on the steel (deeper engraving, polished
highlight curve, cool silver). Used for Eisen/Kampf.
