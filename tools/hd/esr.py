import os, torch, torch.nn as nn, torch.nn.functional as F, numpy as np

class SRVGGNetCompact(nn.Module):
    def __init__(self, num_feat=64, num_conv=32, upscale=4):
        super().__init__()
        self.upscale = upscale
        body = [nn.Conv2d(3, num_feat, 3, 1, 1), nn.PReLU(num_parameters=num_feat)]
        for _ in range(num_conv):
            body += [nn.Conv2d(num_feat, num_feat, 3, 1, 1), nn.PReLU(num_parameters=num_feat)]
        body += [nn.Conv2d(num_feat, 3 * upscale * upscale, 3, 1, 1)]
        self.body = nn.ModuleList(body)
        self.up = nn.PixelShuffle(upscale)
    def forward(self, x):
        out = x
        for m in self.body:
            out = m(out)
        return self.up(out) + F.interpolate(x, scale_factor=self.upscale, mode='nearest')

_model = None
def model():
    global _model
    if _model is None:
        m = SRVGGNetCompact()
        sd = torch.load(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'realesr-general-x4v3.pth'), map_location='cpu', weights_only=True)
        sd = sd.get('params', sd.get('params_ema', sd))
        m.body.load_state_dict({k[len('body.'):]: v for k, v in sd.items()})
        _model = m.eval()
    return _model

def upscale4(rgb):
    """rgb: HxWx3 float 0..255 -> 4Hx4Wx3. Edges padded by wrap so tiling UVs stay seamless."""
    pad = 4
    x = np.pad(rgb, ((pad, pad), (pad, pad), (0, 0)), mode='wrap') / 255.0
    t = torch.from_numpy(x.transpose(2, 0, 1)).float()[None]
    with torch.no_grad():
        y = model()(t)[0].clamp(0, 1).numpy().transpose(1, 2, 0)
    return y[pad * 4:-pad * 4, pad * 4:-pad * 4] * 255.0
