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
