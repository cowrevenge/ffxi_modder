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
