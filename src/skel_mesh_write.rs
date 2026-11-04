//! Lossless model of a skinned-mesh (0x2A) chunk body, the inverse of
//! [`crate::skel_mesh::parse`]. [`SkelMeshImage::parse`] keeps every byte the
//! reader discards, so [`SkelMeshImage::encode`] of an unedited image is
//! byte-identical to its source. Field layout as read by `skel_mesh::parse`;
//! the 64-byte header and the trailing end-offset copy at 0x2E follow
//! research/xi-tools/docs/mesh/format.md (community, writer-verified).

use crate::vos2::VOS2_HEADER_LEN;

const OFF_FLAGS3: usize = 0x02;
const OFF_INSTRUCTION: usize = 0x06;
const OFF_JOINT_ARRAY: usize = 0x0C;
const OFF_NUM_JOINTS: usize = 0x10;
const OFF_VERTEX_COUNTS: usize = 0x12;
const OFF_NUM_VERTEX_COUNTS: usize = 0x16;
const OFF_JOINT_MAPPING: usize = 0x18;
const OFF_JOINT_MAPPING_COUNT: usize = 0x1C;
const OFF_VERTEX_DATA: usize = 0x1E;
const OFF_VERTEX_DATA_WORDS: usize = 0x22;
const OFF_END: usize = 0x24;
const OFF_END_COPY: usize = 0x2E;

const FLAG3_CLOTH_EFFECT: u8 = 0x01;
const VERTEX_COUNT_KINDS: u16 = 2;
const JOINT_REFS_PER_VERTEX: usize = 2;
const BODY_ALIGN: usize = 16;

pub const OP_END: u16 = 0xFFFF;
pub const OP_RENDER_PROPERTIES: u16 = crate::vos2::POLY_BLOCK_SPECULAR;
pub const OP_TEXTURE: u16 = 0x8000;
pub const OP_TRI_STRIP: u16 = 0x5453;
pub const OP_TRI_MESH: u16 = 0x0054;
pub const OP_UNTEXTURED_TRI_MESH: u16 = 0x0043;
pub const OP_UNTEXTURED_TRI_STRIP: u16 = 0x4353;

/// Bytes `skel_mesh::read_render_properties` consumes after the opcode.
pub const RENDER_PROPERTIES_BYTES: usize = 0x2C;
pub const TEXTURE_NAME_BYTES: usize = 0x10;

/// Triangles one draw op may carry; overflow crashes the retail client
/// (research/xi-tools/docs/mesh/format.md "Engine constraints").
pub const MAX_TRIS_PER_DRAW: usize = 128;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SkelMeshWriteError {
    #[error("body shorter than the {0}-byte header")]
    HeaderTooSmall(usize),
    #[error("cloth-effect meshes are not supported")]
    Cloth,
    #[error("expected {VERTEX_COUNT_KINDS} vertex counts, header says {0}")]
    VertexCountKinds(u16),
    #[error("{section} at {found:#x}, expected {expected:#x} for a contiguous layout")]
    Layout {
        section: &'static str,
        found: usize,
        expected: usize,
    },
    #[error("read past end of body at {0:#x}")]
    Truncated(usize),
    #[error("unknown opcode {op:#06x} at {at:#x}")]
    UnknownOpcode { op: u16, at: usize },
    #[error("{0} overflows its u16 header field")]
    FieldOverflow(&'static str),
}

type Result<T> = std::result::Result<T, SkelMeshWriteError>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexturedCorner {
    pub index: u16,
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    RenderProperties([u8; RENDER_PROPERTIES_BYTES]),
    Texture([u8; TEXTURE_NAME_BYTES]),
    TriMesh(Vec<[TexturedCorner; 3]>),
    TriStrip {
        head: [TexturedCorner; 3],
        tail: Vec<TexturedCorner>,
    },
    UntexturedTriMesh(Vec<([u16; 3], [u8; 4])>),
    UntexturedTriStrip {
        head: [u16; 3],
        bgra: [u8; 4],
        tail: Vec<u16>,
    },
}

impl Instruction {
    /// Vertex indices of each triangle, in stored corner order. Strip
    /// triangles are emitted as the window `[k, k+1, k+2]` with no parity
    /// swap, the order `skel_mesh::parse` hands the renderer.
    pub fn triangles(&self) -> Vec<[u16; 3]> {
        match self {
            Instruction::TriMesh(tris) => tris
                .iter()
                .map(|t| [t[0].index, t[1].index, t[2].index])
                .collect(),
            Instruction::UntexturedTriMesh(tris) => tris.iter().map(|(t, _)| *t).collect(),
            Instruction::TriStrip { head, tail } => {
                let idx: Vec<u16> = head.iter().chain(tail).map(|c| c.index).collect();
                idx.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
            }
            Instruction::UntexturedTriStrip { head, tail, .. } => {
                let idx: Vec<u16> = head.iter().chain(tail).copied().collect();
                idx.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
            }
            Instruction::RenderProperties(_) | Instruction::Texture(_) => Vec::new(),
        }
    }

    pub fn map_indices(&mut self, f: impl Fn(u16) -> u16) {
        match self {
            Instruction::TriMesh(tris) => {
                tris.iter_mut().flatten().for_each(|c| c.index = f(c.index))
            }
            Instruction::UntexturedTriMesh(tris) => tris
                .iter_mut()
                .flat_map(|(t, _)| t)
                .for_each(|i| *i = f(*i)),
            Instruction::TriStrip { head, tail } => head
                .iter_mut()
                .chain(tail.iter_mut())
                .for_each(|c| c.index = f(c.index)),
            Instruction::UntexturedTriStrip { head, tail, .. } => head
                .iter_mut()
                .chain(tail.iter_mut())
                .for_each(|i| *i = f(*i)),
            Instruction::RenderProperties(_) | Instruction::Texture(_) => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SingleVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// Positions are stored pre-multiplied by their weight; normals unweighted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoubleVertex {
    pub position: [[f32; 3]; 2],
    pub weight: [f32; 2],
    pub normal: [[f32; 3]; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkelMeshImage {
    /// `body[..instruction offset]`; the offset/count fields are rewritten on
    /// encode, every other byte is kept.
    pub header: Vec<u8>,
    pub instructions: Vec<Instruction>,
    pub joint_array: Vec<u16>,
    /// Two packed joint refs per vertex, single-jointed vertices first.
    pub joint_refs: Vec<[u16; 2]>,
    pub singles: Vec<SingleVertex>,
    pub doubles: Vec<DoubleVertex>,
    /// Bytes from the end of the vertex data to the 16-byte boundary. Retail
    /// leaves non-zero garbage here; it is rewritten when the padded length
    /// is unchanged and zero-filled otherwise.
    pub padding: Vec<u8>,
}

pub fn pack_joint_ref(index: u16, flipped_index: u16, flip_axis: u16) -> u16 {
    use crate::vos2::{BONE_INDEX_MASK, MIRROR_AXIS_MASK};
    const FLIPPED_SHIFT: u16 = 7;
    const AXIS_SHIFT: u16 = 14;
    (index & BONE_INDEX_MASK)
        | ((flipped_index & BONE_INDEX_MASK) << FLIPPED_SHIFT)
        | ((flip_axis & MIRROR_AXIS_MASK) << AXIS_SHIFT)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let s = self
            .data
            .get(self.pos..self.pos + N)
            .ok_or(SkelMeshWriteError::Truncated(self.pos))?;
        self.pos += N;
        Ok(s.try_into().expect("slice length is N"))
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn vec3(&mut self) -> Result<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    fn corner(&mut self, index: u16) -> Result<TexturedCorner> {
        Ok(TexturedCorner {
            index,
            uv: [self.f32()?, self.f32()?],
        })
    }
    fn expect_at(&self, section: &'static str, found: usize) -> Result<()> {
        if found == self.pos {
            Ok(())
        } else {
            Err(SkelMeshWriteError::Layout {
                section,
                found,
                expected: self.pos,
            })
        }
    }
}

fn header_u16(h: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([h[off], h[off + 1]])
}

fn header_offset(h: &[u8], off: usize) -> usize {
    2 * u32::from_le_bytes([h[off], h[off + 1], h[off + 2], h[off + 3]]) as usize
}

impl SkelMeshImage {
    pub fn has_normals(&self) -> bool {
        self.header[OFF_FLAGS3] & FLAG3_CLOTH_EFFECT == 0
    }

    pub fn parse(body: &[u8]) -> Result<Self> {
        if body.len() < VOS2_HEADER_LEN {
            return Err(SkelMeshWriteError::HeaderTooSmall(VOS2_HEADER_LEN));
        }
        if body[OFF_FLAGS3] & FLAG3_CLOTH_EFFECT != 0 {
            return Err(SkelMeshWriteError::Cloth);
        }
        let kinds = header_u16(body, OFF_NUM_VERTEX_COUNTS);
        if kinds != VERTEX_COUNT_KINDS {
            return Err(SkelMeshWriteError::VertexCountKinds(kinds));
        }
        let mut r = Reader {
            data: body,
            pos: VOS2_HEADER_LEN,
        };
        r.expect_at("instructions", header_offset(body, OFF_INSTRUCTION))?;

        let mut instructions = Vec::new();
        loop {
            let at = r.pos;
            let op = r.u16()?;
            let ins = match op {
                OP_END => break,
                OP_RENDER_PROPERTIES => Instruction::RenderProperties(r.take()?),
                OP_TEXTURE => Instruction::Texture(r.take()?),
                OP_TRI_MESH => {
                    let n = r.u16()? as usize;
                    let mut tris = Vec::with_capacity(n);
                    for _ in 0..n {
                        let i = [r.u16()?, r.u16()?, r.u16()?];
                        tris.push([r.corner(i[0])?, r.corner(i[1])?, r.corner(i[2])?]);
                    }
                    Instruction::TriMesh(tris)
                }
                OP_TRI_STRIP => {
                    let n = r.u16()? as usize;
                    let i = [r.u16()?, r.u16()?, r.u16()?];
                    let head = [r.corner(i[0])?, r.corner(i[1])?, r.corner(i[2])?];
                    let mut tail = Vec::with_capacity(n.saturating_sub(1));
                    for _ in 1..n {
                        let idx = r.u16()?;
                        tail.push(r.corner(idx)?);
                    }
                    Instruction::TriStrip { head, tail }
                }
                OP_UNTEXTURED_TRI_MESH => {
                    let n = r.u16()? as usize;
                    let mut tris = Vec::with_capacity(n);
                    for _ in 0..n {
                        let i = [r.u16()?, r.u16()?, r.u16()?];
                        tris.push((i, r.take()?));
                    }
                    Instruction::UntexturedTriMesh(tris)
                }
                OP_UNTEXTURED_TRI_STRIP => {
                    let n = r.u16()? as usize;
                    let head = [r.u16()?, r.u16()?, r.u16()?];
                    let bgra = r.take()?;
                    let mut tail = Vec::with_capacity(n.saturating_sub(1));
                    for _ in 1..n {
                        tail.push(r.u16()?);
                    }
                    Instruction::UntexturedTriStrip { head, bgra, tail }
                }
                op => return Err(SkelMeshWriteError::UnknownOpcode { op, at }),
            };
            instructions.push(ins);
        }

        r.expect_at("joint array", header_offset(body, OFF_JOINT_ARRAY))?;
        let num_joints = header_u16(body, OFF_NUM_JOINTS) as usize;
        let joint_array = (0..num_joints)
            .map(|_| r.u16())
            .collect::<Result<Vec<_>>>()?;

        r.expect_at("vertex counts", header_offset(body, OFF_VERTEX_COUNTS))?;
        let single_count = r.u16()? as usize;
        let double_count = r.u16()? as usize;
        let total = single_count + double_count;

        r.expect_at("joint mapping", header_offset(body, OFF_JOINT_MAPPING))?;
        let joint_refs = (0..total)
            .map(|_| Ok([r.u16()?, r.u16()?]))
            .collect::<Result<Vec<_>>>()?;

        r.expect_at("vertex data", header_offset(body, OFF_VERTEX_DATA))?;
        let has_normals = body[OFF_FLAGS3] & FLAG3_CLOTH_EFFECT == 0;
        let mut singles = Vec::with_capacity(single_count);
        for _ in 0..single_count {
            let position = r.vec3()?;
            let normal = if has_normals { r.vec3()? } else { [0.0; 3] };
            singles.push(SingleVertex { position, normal });
        }
        let mut doubles = Vec::with_capacity(double_count);
        for _ in 0..double_count {
            let mut d = DoubleVertex {
                position: [[0.0; 3]; 2],
                weight: [0.0; 2],
                normal: [[0.0; 3]; 2],
            };
            for axis in 0..3 {
                d.position[0][axis] = r.f32()?;
                d.position[1][axis] = r.f32()?;
            }
            d.weight = [r.f32()?, r.f32()?];
            if has_normals {
                for axis in 0..3 {
                    d.normal[0][axis] = r.f32()?;
                    d.normal[1][axis] = r.f32()?;
                }
            }
            doubles.push(d);
        }
        r.expect_at("end", header_offset(body, OFF_END))?;
        if body.len() != r.pos.next_multiple_of(BODY_ALIGN) {
            return Err(SkelMeshWriteError::Layout {
                section: "body end",
                found: body.len(),
                expected: r.pos.next_multiple_of(BODY_ALIGN),
            });
        }

        Ok(SkelMeshImage {
            header: body[..VOS2_HEADER_LEN].to_vec(),
            instructions,
            joint_array,
            joint_refs,
            singles,
            doubles,
            padding: body[r.pos..].to_vec(),
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let has_normals = self.has_normals();
        let mut out = self.header.clone();
        let put = |out: &mut Vec<u8>, b: &[u8]| out.extend_from_slice(b);
        let put16 = |out: &mut Vec<u8>, v: u16| out.extend_from_slice(&v.to_le_bytes());
        let putf = |out: &mut Vec<u8>, v: f32| out.extend_from_slice(&v.to_bits().to_le_bytes());

        let instruction_at = out.len();
        for ins in &self.instructions {
            match ins {
                Instruction::RenderProperties(b) => {
                    put16(&mut out, OP_RENDER_PROPERTIES);
                    put(&mut out, b);
                }
                Instruction::Texture(name) => {
                    put16(&mut out, OP_TEXTURE);
                    put(&mut out, name);
                }
                Instruction::TriMesh(tris) => {
                    put16(&mut out, OP_TRI_MESH);
                    put16(&mut out, count16(tris.len(), "tri mesh count")?);
                    for t in tris {
                        t.iter().for_each(|c| put16(&mut out, c.index));
                        t.iter().flat_map(|c| c.uv).for_each(|f| putf(&mut out, f));
                    }
                }
                Instruction::TriStrip { head, tail } => {
                    put16(&mut out, OP_TRI_STRIP);
                    put16(&mut out, count16(tail.len() + 1, "tri strip count")?);
                    head.iter().for_each(|c| put16(&mut out, c.index));
                    head.iter()
                        .flat_map(|c| c.uv)
                        .for_each(|f| putf(&mut out, f));
                    for c in tail {
                        put16(&mut out, c.index);
                        c.uv.iter().for_each(|&f| putf(&mut out, f));
                    }
                }
                Instruction::UntexturedTriMesh(tris) => {
                    put16(&mut out, OP_UNTEXTURED_TRI_MESH);
                    put16(&mut out, count16(tris.len(), "untextured mesh count")?);
                    for (i, bgra) in tris {
                        i.iter().for_each(|&v| put16(&mut out, v));
                        put(&mut out, bgra);
                    }
                }
                Instruction::UntexturedTriStrip { head, bgra, tail } => {
                    put16(&mut out, OP_UNTEXTURED_TRI_STRIP);
                    put16(&mut out, count16(tail.len() + 1, "untextured strip count")?);
                    head.iter().for_each(|&v| put16(&mut out, v));
                    put(&mut out, bgra);
                    tail.iter().for_each(|&v| put16(&mut out, v));
                }
            }
        }
        put16(&mut out, OP_END);

        let joint_array_at = out.len();
        self.joint_array.iter().for_each(|&j| put16(&mut out, j));

        let counts_at = out.len();
        put16(
            &mut out,
            count16(self.singles.len(), "single vertex count")?,
        );
        put16(
            &mut out,
            count16(self.doubles.len(), "double vertex count")?,
        );

        let mapping_at = out.len();
        self.joint_refs
            .iter()
            .flatten()
            .for_each(|&j| put16(&mut out, j));

        let vertex_at = out.len();
        for v in &self.singles {
            v.position.iter().for_each(|&f| putf(&mut out, f));
            if has_normals {
                v.normal.iter().for_each(|&f| putf(&mut out, f));
            }
        }
        for v in &self.doubles {
            for axis in 0..3 {
                putf(&mut out, v.position[0][axis]);
                putf(&mut out, v.position[1][axis]);
            }
            v.weight.iter().for_each(|&f| putf(&mut out, f));
            if has_normals {
                for axis in 0..3 {
                    putf(&mut out, v.normal[0][axis]);
                    putf(&mut out, v.normal[1][axis]);
                }
            }
        }
        let end_at = out.len();

        let h = &mut out[..VOS2_HEADER_LEN];
        set_offset(h, OFF_INSTRUCTION, instruction_at);
        set_offset(h, OFF_JOINT_ARRAY, joint_array_at);
        set_u16(
            h,
            OFF_NUM_JOINTS,
            count16(self.joint_array.len(), "joint count")?,
        );
        set_offset(h, OFF_VERTEX_COUNTS, counts_at);
        set_u16(h, OFF_NUM_VERTEX_COUNTS, VERTEX_COUNT_KINDS);
        set_offset(h, OFF_JOINT_MAPPING, mapping_at);
        set_u16(
            h,
            OFF_JOINT_MAPPING_COUNT,
            count16(
                self.joint_refs.len() * JOINT_REFS_PER_VERTEX,
                "joint mapping count",
            )?,
        );
        set_offset(h, OFF_VERTEX_DATA, vertex_at);
        set_u16(
            h,
            OFF_VERTEX_DATA_WORDS,
            count16((end_at - vertex_at) / 2, "vertex data size")?,
        );
        set_offset(h, OFF_END, end_at);
        set_offset(h, OFF_END_COPY, end_at);

        let aligned = end_at.next_multiple_of(BODY_ALIGN);
        if self.padding.len() == aligned - end_at {
            out.extend_from_slice(&self.padding);
        } else {
            out.resize(aligned, 0);
        }
        Ok(out)
    }
}

fn count16(n: usize, field: &'static str) -> Result<u16> {
    u16::try_from(n).map_err(|_| SkelMeshWriteError::FieldOverflow(field))
}

fn set_u16(h: &mut [u8], off: usize, v: u16) {
    h[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn set_offset(h: &mut [u8], off: usize, byte_offset: usize) {
    h[off..off + 4].copy_from_slice(&((byte_offset / 2) as u32).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datid::DatId;

    fn corner(index: u16, u: f32) -> TexturedCorner {
        TexturedCorner {
            index,
            uv: [u, 1.0 - u],
        }
    }

    fn sample() -> SkelMeshImage {
        let mut header = vec![0u8; VOS2_HEADER_LEN];
        header[0] = 1;
        header[4] = 1;
        let mut props = [0u8; RENDER_PROPERTIES_BYTES];
        props[..4].copy_from_slice(&[0x80; 4]);
        let mut name = [b' '; TEXTURE_NAME_BYTES];
        name[..3].copy_from_slice(b"tim");
        SkelMeshImage {
            header,
            instructions: vec![
                Instruction::RenderProperties(props),
                Instruction::Texture(name),
                Instruction::TriMesh(vec![[corner(0, 0.1), corner(1, 0.2), corner(2, 0.3)]]),
                Instruction::TriStrip {
                    head: [corner(0, 0.4), corner(1, 0.5), corner(2, 0.6)],
                    tail: vec![corner(1, 0.7)],
                },
                Instruction::UntexturedTriMesh(vec![([2, 1, 0], [1, 2, 3, 4])]),
                Instruction::UntexturedTriStrip {
                    head: [0, 1, 2],
                    bgra: [5, 6, 7, 8],
                    tail: vec![0],
                },
            ],
            joint_array: vec![0, 3],
            joint_refs: vec![[pack_joint_ref(1, 1, 3), 0], [1, 0], [0, 1]],
            singles: vec![
                SingleVertex {
                    position: [1.0, 2.0, 3.0],
                    normal: [0.0, 1.0, 0.0],
                },
                SingleVertex {
                    position: [4.0, 5.0, 6.0],
                    normal: [1.0, 0.0, 0.0],
                },
            ],
            doubles: vec![DoubleVertex {
                position: [[0.6, 1.2, 1.8], [0.4, 0.8, 1.2]],
                weight: [0.6, 0.4],
                normal: [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
            }],
            padding: Vec::new(),
        }
    }

    #[test]
    fn garbage_padding_after_the_vertex_data_round_trips() {
        let mut bytes = sample().encode().unwrap();
        let end = header_offset(&bytes, OFF_END);
        assert!(end < bytes.len(), "sample must leave padding to fill");
        bytes[end..].iter_mut().for_each(|b| *b = 0xC2);
        let back = SkelMeshImage::parse(&bytes).unwrap();
        assert_eq!(back.encode().unwrap(), bytes);
    }

    #[test]
    fn encode_then_parse_is_identity_and_reencodes_byte_identical() {
        let image = sample();
        let bytes = image.encode().unwrap();
        assert_eq!(bytes.len() % BODY_ALIGN, 0);
        let back = SkelMeshImage::parse(&bytes).unwrap();
        assert_eq!(back.instructions, image.instructions);
        assert_eq!(back.joint_refs, image.joint_refs);
        assert_eq!(back.singles, image.singles);
        assert_eq!(back.doubles, image.doubles);
        assert_eq!(back.encode().unwrap(), bytes);
    }

    #[test]
    fn the_reader_sees_what_the_writer_wrote() {
        let bytes = sample().encode().unwrap();
        let mesh = crate::skel_mesh::parse(DatId::from_str("0000"), &bytes);
        let mirrored_buffers_per_draw = 2;
        assert_eq!(mesh.meshes.len(), 4 * mirrored_buffers_per_draw);
        let first = &mesh.meshes[0].vertices;
        assert_eq!(first[0].p0, [1.0, 2.0, 3.0]);
        assert_eq!(first[0].u, 0.1);
        assert_eq!(first[2].p0, [0.6, 1.2, 1.8]);
        assert_eq!(first[2].joint1_weight, 0.4);
        let mirror = &mesh.meshes[1].vertices;
        assert_eq!(mirror[0].p0, [1.0, 2.0, -3.0]);
    }

    #[test]
    fn a_gap_between_sections_is_refused_not_silently_closed() {
        let mut bytes = sample().encode().unwrap();
        let joint_at = header_offset(&bytes, OFF_JOINT_ARRAY);
        set_offset(&mut bytes, OFF_JOINT_ARRAY, joint_at + 2);
        assert!(matches!(
            SkelMeshImage::parse(&bytes),
            Err(SkelMeshWriteError::Layout {
                section: "joint array",
                ..
            })
        ));
    }

    #[test]
    fn strip_triangles_are_the_sliding_window() {
        let ins = &sample().instructions[3];
        assert_eq!(ins.triangles(), vec![[0, 1, 2], [1, 2, 1]]);
    }
}
