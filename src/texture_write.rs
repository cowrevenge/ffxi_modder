//! DXT3 texture-chunk (0x20, type 0xA1) bodies. Header fields per
//! research/xi-tools/docs/mesh/format.md "Texture section"; every field the
//! writer does not own is copied from a retail template so unknown header
//! words keep their retail values.

use crate::texture::FLG_DXT;

pub const TEXTURE_NAME_BYTES: usize = 0x10;
const OFF_NAME: usize = 0x01;
const OFF_WIDTH: usize = 0x15;
const OFF_HEIGHT: usize = 0x19;
const OFF_FOURCC: usize = 0x39;
const OFF_DATA_SIZE: usize = 0x3D;
const OFF_ROW_BYTES: usize = 0x41;
const DXT3_HEADER_BYTES: usize = 0x45;
const DXT3_FOURCC: &[u8; 4] = b"3TXD";

const DXT_BLOCK_TEXELS: u32 = 4;
const DXT3_BLOCK_BYTES: usize = 16;
const DXT3_ALPHA_BYTES: usize = 8;
/// FFXI stores alpha at half scale: nibble 8 decodes to 0x88, which
/// `texture::ffxi_alpha_remap` doubles to fully opaque.
const OPAQUE_ALPHA_PAIR: u8 = 0x88;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TextureWriteError {
    #[error("template is not a DXT3 texture body")]
    NotDxt3Template,
    #[error("{width}x{height} is not a positive multiple of 4")]
    Dimensions { width: u32, height: u32 },
}

fn rgb565(rgb: [u8; 3]) -> u16 {
    (u16::from(rgb[0] >> 3) << 11) | (u16::from(rgb[1] >> 2) << 5) | u16::from(rgb[2] >> 3)
}

/// A solid, opaque `rgb` DXT3 texture named `name`, sized `width` x `height`,
/// with its remaining header copied from `template` (a retail DXT3 body).
pub fn solid_dxt3(
    template: &[u8],
    name: [u8; TEXTURE_NAME_BYTES],
    width: u32,
    height: u32,
    rgb: [u8; 3],
) -> Result<Vec<u8>, TextureWriteError> {
    if template.len() < DXT3_HEADER_BYTES
        || template[0] != FLG_DXT
        || &template[OFF_FOURCC..OFF_FOURCC + 4] != DXT3_FOURCC
    {
        return Err(TextureWriteError::NotDxt3Template);
    }
    if width == 0
        || height == 0
        || !width.is_multiple_of(DXT_BLOCK_TEXELS)
        || !height.is_multiple_of(DXT_BLOCK_TEXELS)
    {
        return Err(TextureWriteError::Dimensions { width, height });
    }
    let blocks = ((width / DXT_BLOCK_TEXELS) * (height / DXT_BLOCK_TEXELS)) as usize;
    let data_size = (blocks * DXT3_BLOCK_BYTES) as u32;
    let row_bytes = (width / DXT_BLOCK_TEXELS) * DXT3_BLOCK_BYTES as u32;

    let mut out = template[..DXT3_HEADER_BYTES].to_vec();
    out[OFF_NAME..OFF_NAME + TEXTURE_NAME_BYTES].copy_from_slice(&name);
    out[OFF_WIDTH..OFF_WIDTH + 4].copy_from_slice(&width.to_le_bytes());
    out[OFF_HEIGHT..OFF_HEIGHT + 4].copy_from_slice(&height.to_le_bytes());
    out[OFF_DATA_SIZE..OFF_DATA_SIZE + 4].copy_from_slice(&data_size.to_le_bytes());
    out[OFF_ROW_BYTES..OFF_ROW_BYTES + 4].copy_from_slice(&row_bytes.to_le_bytes());

    let color = rgb565(rgb).to_le_bytes();
    let mut block = [0u8; DXT3_BLOCK_BYTES];
    block[..DXT3_ALPHA_BYTES].fill(OPAQUE_ALPHA_PAIR);
    block[DXT3_ALPHA_BYTES..DXT3_ALPHA_BYTES + 2].copy_from_slice(&color);
    block[DXT3_ALPHA_BYTES + 2..DXT3_ALPHA_BYTES + 4].copy_from_slice(&color);
    for _ in 0..blocks {
        out.extend_from_slice(&block);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> Vec<u8> {
        let mut t = vec![0u8; DXT3_HEADER_BYTES];
        t[0] = FLG_DXT;
        t[OFF_FOURCC..OFF_FOURCC + 4].copy_from_slice(DXT3_FOURCC);
        t[0x11] = 0x28;
        t
    }

    #[test]
    fn solid_texture_decodes_to_its_colour_fully_opaque() {
        let name = *b"tim     purple  ";
        let body = solid_dxt3(&template(), name, 8, 8, [0xFF, 0x00, 0xFF]).unwrap();
        assert_eq!(body.len(), DXT3_HEADER_BYTES + 4 * DXT3_BLOCK_BYTES);
        assert_eq!(&body[OFF_NAME..OFF_NAME + TEXTURE_NAME_BYTES], &name);
        assert_eq!(body[0x11], 0x28);
        let tex = crate::texture::decode_texture(&body).unwrap();
        assert_eq!((tex.width, tex.height), (8, 8));
        assert!(tex
            .rgba
            .chunks(4)
            .all(|p| p[..3] == [0xFF, 0x00, 0xFF]
                && crate::texture::ffxi_alpha_remap(p[3]) == u8::MAX));
    }

    #[test]
    fn rejects_a_non_dxt3_template_and_odd_sizes() {
        assert_eq!(
            solid_dxt3(&[0u8; 4], [b' '; 16], 8, 8, [0; 3]),
            Err(TextureWriteError::NotDxt3Template)
        );
        assert_eq!(
            solid_dxt3(&template(), [b' '; 16], 6, 8, [0; 3]),
            Err(TextureWriteError::Dimensions {
                width: 6,
                height: 8
            })
        );
    }
}

const DXT_COLOR_BYTES: usize = 8;
const INDEX_BITS: u32 = 2;

fn expand565(c: u16) -> [f32; 3] {
    let r = ((c >> 11) & 0x1F) as f32 * 255.0 / 31.0;
    let g = ((c >> 5) & 0x3F) as f32 * 255.0 / 63.0;
    let b = (c & 0x1F) as f32 * 255.0 / 31.0;
    [r, g, b]
}

fn luma(p: &[u8]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// One DXT color block (4-color mode) for 16 RGBA texels: endpoints are the
/// brightest and darkest texel, each texel takes the nearest palette entry.
pub fn encode_color_block(texels: &[[u8; 4]; 16]) -> [u8; DXT_COLOR_BYTES] {
    let hi = texels.iter().max_by(|a, b| luma(*a).total_cmp(&luma(*b))).expect("16 texels");
    let lo = texels.iter().min_by(|a, b| luma(*a).total_cmp(&luma(*b))).expect("16 texels");
    let (mut c0, mut c1) = (rgb565([hi[0], hi[1], hi[2]]), rgb565([lo[0], lo[1], lo[2]]));
    if c0 < c1 {
        std::mem::swap(&mut c0, &mut c1);
    }
    let (e0, e1) = (expand565(c0), expand565(c1));
    let mix = |a: f32, b: f32, w: f32| a * w + b * (1.0 - w);
    let palette: [[f32; 3]; 4] = [
        e0,
        e1,
        [0, 1, 2].map(|i| mix(e0[i], e1[i], 2.0 / 3.0)),
        [0, 1, 2].map(|i| mix(e0[i], e1[i], 1.0 / 3.0)),
    ];
    let mut indices = 0u32;
    if c0 != c1 {
        for (i, t) in texels.iter().enumerate() {
            let best = (0..4)
                .min_by(|&a, &b| {
                    let d = |k: usize| (0..3).map(|c| (palette[k][c] - t[c] as f32).powi(2)).sum::<f32>();
                    d(a).total_cmp(&d(b))
                })
                .expect("palette has entries");
            indices |= (best as u32) << (INDEX_BITS * i as u32);
        }
    }
    let mut out = [0u8; DXT_COLOR_BYTES];
    out[..2].copy_from_slice(&c0.to_le_bytes());
    out[2..4].copy_from_slice(&c1.to_le_bytes());
    out[4..].copy_from_slice(&indices.to_le_bytes());
    out
}

/// A DXT3 body with new colors from `rgba` and the alpha blocks of `source`
/// (a DXT3 body of the same size) kept byte for byte.
pub fn recolor_dxt3(source: &[u8], rgba: &[u8]) -> Result<Vec<u8>, TextureWriteError> {
    if source.len() < DXT3_HEADER_BYTES || source[0] != FLG_DXT || &source[OFF_FOURCC..OFF_FOURCC + 4] != DXT3_FOURCC {
        return Err(TextureWriteError::NotDxt3Template);
    }
    let rd = |o: usize| u32::from_le_bytes(source[o..o + 4].try_into().expect("4 bytes"));
    let (width, height) = (rd(OFF_WIDTH), rd(OFF_HEIGHT));
    let (bw, bh) = ((width / DXT_BLOCK_TEXELS) as usize, (height / DXT_BLOCK_TEXELS) as usize);
    let mut out = source.to_vec();
    for by in 0..bh {
        for bx in 0..bw {
            let mut texels = [[0u8; 4]; 16];
            for (i, t) in texels.iter_mut().enumerate() {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                let o = (y * width as usize + x) * 4;
                t.copy_from_slice(&rgba[o..o + 4]);
            }
            let block = DXT3_HEADER_BYTES + (by * bw + bx) * DXT3_BLOCK_BYTES;
            out[block + DXT3_ALPHA_BYTES..block + DXT3_BLOCK_BYTES].copy_from_slice(&encode_color_block(&texels));
        }
    }
    Ok(out)
}

const ALPHA_NIBBLE_MAX: u8 = 15;
const ALPHA_NIBBLE_STEP: u16 = 17;
const PCA_ITERATIONS: usize = 8;

/// One DXT color block with endpoints on the block's principal colour axis
/// (power iteration on the covariance), so gradients that are not grey keep
/// their hue; falls back to the luma pair when the block is flat.
pub fn encode_color_block_pca(texels: &[[u8; 4]; 16]) -> [u8; DXT_COLOR_BYTES] {
    let px: Vec<[f32; 3]> = texels.iter().map(|t| [t[0] as f32, t[1] as f32, t[2] as f32]).collect();
    let mean = px.iter().fold([0.0f32; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]).map(|v| v / 16.0);
    let mut cov = [[0.0f32; 3]; 3];
    for p in &px {
        let d = [p[0] - mean[0], p[1] - mean[1], p[2] - mean[2]];
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += d[i] * d[j];
            }
        }
    }
    let mut axis = [1.0f32, 1.0, 1.0];
    for _ in 0..PCA_ITERATIONS {
        let n = [0, 1, 2].map(|i| cov[i][0] * axis[0] + cov[i][1] * axis[1] + cov[i][2] * axis[2]);
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len < f32::EPSILON {
            return encode_color_block(texels);
        }
        axis = n.map(|v| v / len);
    }
    let proj = |p: &[f32; 3]| (p[0] - mean[0]) * axis[0] + (p[1] - mean[1]) * axis[1] + (p[2] - mean[2]) * axis[2];
    let (lo, hi) = px.iter().map(proj).fold((f32::MAX, f32::MIN), |(a, b), t| (a.min(t), b.max(t)));
    let at = |t: f32| [0, 1, 2].map(|i| (mean[i] + axis[i] * t).clamp(0.0, 255.0) as u8);
    let mut block = *texels;
    block[0][..3].copy_from_slice(&at(hi));
    block[1][..3].copy_from_slice(&at(lo));
    let pair = encode_color_block(&[block[0], block[1], block[0], block[1], block[0], block[1], block[0], block[1], block[0], block[1], block[0], block[1], block[0], block[1], block[0], block[1]]);
    let (c0, c1) = (u16::from_le_bytes([pair[0], pair[1]]), u16::from_le_bytes([pair[2], pair[3]]));
    if c0 == c1 {
        return pair;
    }
    let (e0, e1) = (expand565(c0), expand565(c1));
    let palette: [[f32; 3]; 4] = [
        e0,
        e1,
        [0, 1, 2].map(|i| e0[i] * 2.0 / 3.0 + e1[i] / 3.0),
        [0, 1, 2].map(|i| e0[i] / 3.0 + e1[i] * 2.0 / 3.0),
    ];
    let mut indices = 0u32;
    for (i, p) in px.iter().enumerate() {
        let best = (0..4)
            .min_by(|&a, &b| {
                let d = |k: usize| (0..3).map(|c| (palette[k][c] - p[c]).powi(2)).sum::<f32>();
                d(a).total_cmp(&d(b))
            })
            .expect("palette has entries");
        indices |= (best as u32) << (INDEX_BITS * i as u32);
    }
    let mut out = [0u8; DXT_COLOR_BYTES];
    out[..2].copy_from_slice(&c0.to_le_bytes());
    out[2..4].copy_from_slice(&c1.to_le_bytes());
    out[4..].copy_from_slice(&indices.to_le_bytes());
    out
}

/// A full DXT3 body from `rgba` (`width` x `height`, alpha as the texture
/// stores it), header copied from `template` (a retail DXT3 body) with the
/// name, size and data-size fields rewritten.
pub fn encode_dxt3(
    template: &[u8],
    name: [u8; TEXTURE_NAME_BYTES],
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Vec<u8>, TextureWriteError> {
    let solid = solid_dxt3(template, name, width, height, [0; 3])?;
    if rgba.len() != (width * height * 4) as usize {
        return Err(TextureWriteError::Dimensions { width, height });
    }
    let (bw, bh) = ((width / DXT_BLOCK_TEXELS) as usize, (height / DXT_BLOCK_TEXELS) as usize);
    let mut out = solid[..DXT3_HEADER_BYTES].to_vec();
    for by in 0..bh {
        for bx in 0..bw {
            let mut texels = [[0u8; 4]; 16];
            for (i, t) in texels.iter_mut().enumerate() {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                let o = (y * width as usize + x) * 4;
                t.copy_from_slice(&rgba[o..o + 4]);
            }
            let mut alpha = [0u8; DXT3_ALPHA_BYTES];
            for (i, t) in texels.iter().enumerate() {
                let nib = ((u16::from(t[3]) + ALPHA_NIBBLE_STEP / 2) / ALPHA_NIBBLE_STEP).min(u16::from(ALPHA_NIBBLE_MAX)) as u8;
                alpha[i / 2] |= nib << (4 * (i % 2));
            }
            out.extend_from_slice(&alpha);
            out.extend_from_slice(&encode_color_block_pca(&texels));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod encode_tests {
    use super::*;

    #[test]
    fn full_encode_round_trips_colour_and_alpha() {
        let mut t = vec![0u8; DXT3_HEADER_BYTES];
        t[0] = FLG_DXT;
        t[OFF_FOURCC..OFF_FOURCC + 4].copy_from_slice(DXT3_FOURCC);
        let (w, h) = (8u32, 4u32);
        let mut rgba = Vec::new();
        for i in 0..w * h {
            let a = if i % 2 == 0 { 255 } else { 0 };
            rgba.extend_from_slice(&[200, 120, 60, a]);
        }
        let body = encode_dxt3(&t, *b"tim     hdtest  ", w, h, &rgba).unwrap();
        let dec = crate::texture::decode_texture(&body).unwrap();
        assert_eq!((dec.width, dec.height), (w, h));
        for (i, p) in dec.rgba.chunks(4).enumerate() {
            for c in 0..3 {
                assert!((p[c] as i32 - rgba[i * 4 + c] as i32).abs() <= 8, "texel {i} {p:?}");
            }
        }
    }
}
