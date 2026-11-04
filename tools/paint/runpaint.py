"""shared entry point: python3 runpaint.py <module> <out_dir> <dat>"""
import sys, os, importlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tex
from PIL import Image
mod = importlib.import_module(sys.argv[1]); out_dir, dat = sys.argv[2], sys.argv[3]
os.makedirs(out_dir, exist_ok=True)
specs = []
for name, img in tex.textures(dat):
    key = name.split()[-1]
    rgba = mod.paint(img[..., 3])
    raw = os.path.join(out_dir, key + '.rgba'); rgba.tofile(raw)
    Image.fromarray(rgba[..., :3]).save(os.path.join(out_dir, key + '.png'))
    specs.append(f"{key}={raw}:{rgba.shape[1]}x{rgba.shape[0]}")
print(' '.join(specs))
