use crate::{DatError, Result};

pub const VOS2_HEADER_LEN: usize = 0x40;
const KIND_TYPE_USE_BONE_TABLE: u16 = 0x80;
pub(crate) const BONE_INDEX_MASK: u16 = 0x7F;
pub(crate) const MIRROR_AXIS_MASK: u16 = 0x03;
const POLY_BLOCK_CLASS_MASK: u16 = 0x80F0;
pub(crate) const POLY_BLOCK_SPECULAR: u16 = 0x8010;
const POLY_BLOCK_TEXTURE_NAME: u16 = 0x8000;

#[derive(Debug, thiserror::Error)]
pub enum Vos2Error {
    #[error("VertexOs2 chunk too small for header: need {needed}, got {got}")]
    HeaderTooSmall { needed: usize, got: usize },
    #[error(
        "VertexOs2 section offset {section} = {byte_offset:#x} out of bounds (body len {body_len})"
    )]
    SectionOob {
        section: &'static str,
        byte_offset: usize,
        body_len: usize,
    },
    #[error("VertexOs2 poly block walked past end of body")]
    PolyOob,
}

impl From<Vos2Error> for DatError {
    fn from(e: Vos2Error) -> Self {
        DatError::Mmb(format!("vos2: {e}"))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vos2Header {
    pub version: u8,
    pub kind_type: u16,
    pub flip: u16,
    pub off_poly_bytes: usize,
    pub off_bone_table_bytes: usize,

    pub bone_table_count: u16,
    pub off_weight_bytes: usize,
    pub off_bone_bytes: usize,

    pub bone_indices_count: u16,
    pub off_vertex_bytes: usize,
    pub off_poly_load_bytes: usize,
    pub poly_lod2_count: u16,
}

impl Vos2Header {
    pub fn parse(body: &[u8]) -> Result<Self> {
        if body.len() < VOS2_HEADER_LEN {
            return Err(Vos2Error::HeaderTooSmall {
                needed: VOS2_HEADER_LEN,
                got: body.len(),
            }
            .into());
        }
        let u16_at = |o: usize| u16::from_le_bytes([body[o], body[o + 1]]);
        let u32_at =
            |o: usize| u32::from_le_bytes([body[o], body[o + 1], body[o + 2], body[o + 3]]);
        Ok(Self {
            version: body[0],
            kind_type: u16_at(0x02),
            flip: u16_at(0x04),
            off_poly_bytes: u32_at(0x06) as usize * 2,
            off_bone_table_bytes: u32_at(0x0C) as usize * 2,
            bone_table_count: u16_at(0x10),
            off_weight_bytes: u32_at(0x12) as usize * 2,
            off_bone_bytes: u32_at(0x18) as usize * 2,
            bone_indices_count: u16_at(0x1C),
            off_vertex_bytes: u32_at(0x1E) as usize * 2,
            off_poly_load_bytes: u32_at(0x24) as usize * 2,
            poly_lod2_count: u16_at(0x32),
        })
    }

    pub fn use_bone_table(&self) -> bool {
        (self.kind_type & KIND_TYPE_USE_BONE_TABLE) != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vos2BoneIndices {
    pub bone_index1: u8,
    pub bone_index2: u8,
    pub mirror_axis: u8,
}

impl Vos2BoneIndices {
    pub fn from_u16(w: u16) -> Self {
        Self {
            bone_index1: (w & BONE_INDEX_MASK) as u8,
            bone_index2: ((w >> 7) & BONE_INDEX_MASK) as u8,
            mirror_axis: ((w >> 14) & MIRROR_AXIS_MASK) as u8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vos2Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct Vos2BoneWeight {
    pub weight1: f32,
    pub weight2: f32,
    pub pos1: [f32; 3],
    pub pos2: [f32; 3],
    pub normal1: [f32; 3],
    pub normal2: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct Vos2Triangle {
    pub indices: [u16; 3],
    pub uvs: [[f32; 2]; 3],
}

#[derive(Debug, Clone)]
pub struct Vos2Group {
    pub texture_name: String,
    pub triangles: Vec<Vos2Triangle>,

    pub specular_exponent: f32,

    pub specular_intensity: f32,
}

#[derive(Debug, Clone)]
pub struct Vos2Mesh {
    pub header: Vos2Header,
    pub vertices: Vec<Vos2Vertex>,
    pub groups: Vec<Vos2Group>,

    pub bone_table: Vec<u16>,

    pub bone_indices: Vec<Vos2BoneIndices>,

    pub bone_weights: Vec<Vos2BoneWeight>,
}

impl Vos2Mesh {
    pub fn skeleton_bone_for(&self, vertex_idx: usize) -> Option<u16> {
        let raw = self.raw_bone_index_for(vertex_idx)?;
        let raw = raw as u16;
        if self.header.use_bone_table() {
            self.bone_table.get(raw as usize).copied()
        } else {
            Some(raw)
        }
    }

    pub fn raw_bone_index_for(&self, vertex_idx: usize) -> Option<u8> {
        let bi_idx = vertex_idx.checked_mul(2)?;
        self.bone_indices.get(bi_idx).map(|b| b.bone_index1)
    }

    pub fn skeleton_bone2_for(&self, vertex_idx: usize) -> Option<u16> {
        let bi_idx = vertex_idx.checked_mul(2)?.checked_add(1)?;
        let raw = self.bone_indices.get(bi_idx)?.bone_index1 as u16;
        if self.header.use_bone_table() {
            self.bone_table.get(raw as usize).copied()
        } else {
            Some(raw)
        }
    }
}

pub fn parse_vos2(body: &[u8]) -> Result<Vos2Mesh> {
    let header = Vos2Header::parse(body)?;

    if header.off_weight_bytes + 4 > body.len() {
        return Err(Vos2Error::SectionOob {
            section: "weight",
            byte_offset: header.off_weight_bytes,
            body_len: body.len(),
        }
        .into());
    }
    let weight1 = i16::from_le_bytes([
        body[header.off_weight_bytes],
        body[header.off_weight_bytes + 1],
    ]) as usize;
    let weight2 = i16::from_le_bytes([
        body[header.off_weight_bytes + 2],
        body[header.off_weight_bytes + 3],
    ]) as usize;

    let vstart = header.off_vertex_bytes;
    const STRIDE1: usize = 24;
    const STRIDE2: usize = 56;
    let v1_bytes = weight1 * STRIDE1;
    let v2_bytes = weight2 * STRIDE2;
    if vstart + v1_bytes + v2_bytes > body.len() {
        return Err(Vos2Error::SectionOob {
            section: "vertex",
            byte_offset: vstart,
            body_len: body.len(),
        }
        .into());
    }

    let mut vertices = Vec::with_capacity(weight1 + weight2);
    for i in 0..weight1 {
        let off = vstart + i * STRIDE1;
        let pos = [
            f32::from_le_bytes(body[off..off + 4].try_into().unwrap()),
            f32::from_le_bytes(body[off + 4..off + 8].try_into().unwrap()),
            f32::from_le_bytes(body[off + 8..off + 12].try_into().unwrap()),
        ];
        let normal = [
            f32::from_le_bytes(body[off + 12..off + 16].try_into().unwrap()),
            f32::from_le_bytes(body[off + 16..off + 20].try_into().unwrap()),
            f32::from_le_bytes(body[off + 20..off + 24].try_into().unwrap()),
        ];
        vertices.push(Vos2Vertex { pos, normal });
    }
    let mut bone_weights: Vec<Vos2BoneWeight> = Vec::with_capacity(weight2);
    for i in 0..weight2 {
        let off = vstart + v1_bytes + i * STRIDE2;
        let read = |k: usize| f32::from_le_bytes(body[off + k..off + k + 4].try_into().unwrap());
        let pos1 = [read(0), read(8), read(16)];
        let pos2 = [read(4), read(12), read(20)];
        let weight1_val = read(24);
        let weight2_val = read(28);
        let normal1 = [read(32), read(40), read(48)];
        let normal2 = [read(36), read(44), read(52)];
        vertices.push(Vos2Vertex {
            pos: pos1,
            normal: normal1,
        });
        bone_weights.push(Vos2BoneWeight {
            weight1: weight1_val,
            weight2: weight2_val,
            pos1,
            pos2,
            normal1,
            normal2,
        });
    }

    let groups = parse_poly_block(body, header.off_poly_bytes)?;

    let mut bone_table = Vec::with_capacity(header.bone_table_count as usize);
    if header.bone_table_count > 0 {
        let bt_end = header
            .off_bone_table_bytes
            .saturating_add(header.bone_table_count as usize * 2);
        if bt_end > body.len() {
            return Err(Vos2Error::SectionOob {
                section: "bone_table",
                byte_offset: header.off_bone_table_bytes,
                body_len: body.len(),
            }
            .into());
        }
        for i in 0..header.bone_table_count as usize {
            let o = header.off_bone_table_bytes + i * 2;
            bone_table.push(u16::from_le_bytes([body[o], body[o + 1]]));
        }
    }

    let mut bone_indices = Vec::with_capacity(header.bone_indices_count as usize);
    if header.bone_indices_count > 0 {
        let bi_end = header
            .off_bone_bytes
            .saturating_add(header.bone_indices_count as usize * 2);
        if bi_end > body.len() {
            return Err(Vos2Error::SectionOob {
                section: "bone_indices",
                byte_offset: header.off_bone_bytes,
                body_len: body.len(),
            }
            .into());
        }
        for i in 0..header.bone_indices_count as usize {
            let o = header.off_bone_bytes + i * 2;
            let w = u16::from_le_bytes([body[o], body[o + 1]]);
            bone_indices.push(Vos2BoneIndices::from_u16(w));
        }
    }

    Ok(Vos2Mesh {
        header,
        vertices,
        groups,
        bone_table,
        bone_indices,
        bone_weights,
    })
}

fn parse_poly_block(body: &[u8], start: usize) -> Result<Vec<Vos2Group>> {
    let mut p = start;
    let mut groups: Vec<Vos2Group> = Vec::new();
    let mut tex_name = String::new();

    let mut specular_exponent: f32 = 0.0;
    let mut specular_intensity: f32 = 0.0;

    while p + 4 <= body.len() {
        let wf = u16::from_le_bytes([body[p], body[p + 1]]);
        let ws = u16::from_le_bytes([body[p + 2], body[p + 3]]) as usize;

        if wf & POLY_BLOCK_CLASS_MASK == POLY_BLOCK_SPECULAR {
            if p + 0x2E > body.len() {
                return Err(Vos2Error::PolyOob.into());
            }

            let exp_off = p + 2 + 36;
            let int_off = p + 2 + 40;
            specular_exponent =
                f32::from_le_bytes(body[exp_off..exp_off + 4].try_into().unwrap_or([0; 4]));
            specular_intensity =
                f32::from_le_bytes(body[int_off..int_off + 4].try_into().unwrap_or([0; 4]));
            p += 0x2E;
            continue;
        }
        if wf & POLY_BLOCK_CLASS_MASK == POLY_BLOCK_TEXTURE_NAME {
            if p + 0x12 > body.len() {
                return Err(Vos2Error::PolyOob.into());
            }
            let name_bytes = &body[p + 2..p + 18];
            tex_name = name_bytes
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect::<String>()
                .trim_end()
                .to_string();
            p += 0x12;
            continue;
        }

        match wf {
            0x5453 => {
                let header_size = 4;
                let strip_bytes = 30 + ws.saturating_sub(1) * 10;
                if p + header_size + strip_bytes > body.len() || ws == 0 {
                    return Err(Vos2Error::PolyOob.into());
                }
                let triangles = parse_strip(&body[p + header_size..], ws)?;
                groups.push(Vos2Group {
                    texture_name: tex_name.clone(),
                    triangles,
                    specular_exponent,
                    specular_intensity,
                });
                p += header_size + strip_bytes;
            }
            0x0054 => {
                let header_size = 4;
                let body_bytes = ws * 30;
                if p + header_size + body_bytes > body.len() {
                    return Err(Vos2Error::PolyOob.into());
                }
                let triangles = parse_tri_list(&body[p + header_size..], ws);
                groups.push(Vos2Group {
                    texture_name: tex_name.clone(),
                    triangles,
                    specular_exponent,
                    specular_intensity,
                });
                p += header_size + body_bytes;
            }
            0x4353 => {
                p += ws * 20 + 0x0C;
            }
            0x0043 => {
                p += ws * 10 + 0x04;
            }
            _ => break,
        }
    }

    Ok(groups)
}

fn parse_tri_list(buf: &[u8], count: usize) -> Vec<Vos2Triangle> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let off = i * 30;
        if off + 30 > buf.len() {
            break;
        }
        let i0 = u16::from_le_bytes([buf[off], buf[off + 1]]);
        let i1 = u16::from_le_bytes([buf[off + 2], buf[off + 3]]);
        let i2 = u16::from_le_bytes([buf[off + 4], buf[off + 5]]);
        let read_uv = |k: usize| -> [f32; 2] {
            [
                f32::from_le_bytes(buf[off + k..off + k + 4].try_into().unwrap()),
                f32::from_le_bytes(buf[off + k + 4..off + k + 8].try_into().unwrap()),
            ]
        };
        out.push(Vos2Triangle {
            indices: [i0, i1, i2],
            uvs: [read_uv(6), read_uv(14), read_uv(22)],
        });
    }
    out
}

fn parse_strip(buf: &[u8], corner_count: usize) -> Result<Vec<Vos2Triangle>> {
    if corner_count == 0 || buf.len() < 30 {
        return Ok(Vec::new());
    }

    let i0 = u16::from_le_bytes([buf[0], buf[1]]);
    let i1 = u16::from_le_bytes([buf[2], buf[3]]);
    let i2 = u16::from_le_bytes([buf[4], buf[5]]);
    let read_uv = |o: usize| -> [f32; 2] {
        [
            f32::from_le_bytes(buf[o..o + 4].try_into().unwrap()),
            f32::from_le_bytes(buf[o + 4..o + 8].try_into().unwrap()),
        ]
    };
    let uv0 = read_uv(6);
    let uv1 = read_uv(14);
    let uv2 = read_uv(22);

    let mut tris = Vec::with_capacity(corner_count.saturating_sub(2));
    let mut prev2 = (i0, uv0);
    let mut prev1 = (i1, uv1);
    let mut cur = (i2, uv2);
    tris.push(Vos2Triangle {
        indices: [prev2.0, prev1.0, cur.0],
        uvs: [prev2.1, prev1.1, cur.1],
    });

    let mut flip = false;
    let mut p = 30;
    for _ in 0..corner_count.saturating_sub(3) {
        if p + 10 > buf.len() {
            break;
        }
        let idx = u16::from_le_bytes([buf[p], buf[p + 1]]);
        let uv = read_uv(p + 2);
        prev2 = prev1;
        prev1 = cur;
        cur = (idx, uv);
        let tri = if flip {
            Vos2Triangle {
                indices: [prev1.0, prev2.0, cur.0],
                uvs: [prev1.1, prev2.1, cur.1],
            }
        } else {
            Vos2Triangle {
                indices: [prev2.0, prev1.0, cur.0],
                uvs: [prev2.1, prev1.1, cur.1],
            }
        };
        tris.push(tri);
        flip = !flip;
        p += 10;
    }
    Ok(tris)
}
