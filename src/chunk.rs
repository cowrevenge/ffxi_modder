use crate::{DatError, Result};

pub const CHUNK_KIND_MASK: u32 = 0x7F;
const CHUNK_SIZE_UNITS_MASK: u32 = 0x7FFFF;

#[derive(Debug, Clone, Copy)]
pub struct Chunk<'a> {
    pub name: [u8; 4],

    pub kind: u8,

    pub data: &'a [u8],

    pub offset: usize,
}

impl<'a> Chunk<'a> {
    pub fn name_str(&self) -> String {
        self.name
            .iter()
            .map(|&b| if b == 0 { '.' } else { b as char })
            .collect()
    }
}

#[derive(Debug)]
pub struct ChunkWalker<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ChunkWalker<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    pub fn position(&self) -> usize {
        self.cursor
    }
}

impl<'a> Iterator for ChunkWalker<'a> {
    type Item = Result<Chunk<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.bytes.len() {
            return None;
        }

        let header_off = self.cursor;
        let Some(header) = self.bytes.get(header_off..header_off + 16) else {
            self.cursor = self.bytes.len();
            return Some(Err(DatError::TruncatedChunk {
                offset: header_off,
                needed: 16,
                available: self.bytes.len() - header_off,
            }));
        };

        let name = [header[0], header[1], header[2], header[3]];
        let value = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        // Retail chunk walker: 7-bit kind, 19-bit size in 16-byte units
        // (FFXiMain.dll horizonxi-2023 RVA 0x72B80 / retail-2026-09 RVA 0x73750: shr 7 /
        // and 0x7FFFF). Not 20 bits -- bit 26 is is_shadow; xim's 20-bit walk is a latent bug.
        let kind = (value & CHUNK_KIND_MASK) as u8;
        let size_units = (value >> 7) & CHUNK_SIZE_UNITS_MASK;
        let total_bytes = (size_units as usize).saturating_mul(16);

        if total_bytes < 16 {
            self.cursor = self.bytes.len();
            return None;
        }

        let body_bytes = total_bytes - 16;
        let body_start = header_off + 16;
        let body_end = body_start + body_bytes;

        if body_end > self.bytes.len() {
            self.cursor = self.bytes.len();
            return Some(Err(DatError::TruncatedChunk {
                offset: header_off,
                needed: total_bytes,
                available: self.bytes.len() - header_off,
            }));
        }

        let chunk = Chunk {
            name,
            kind,
            data: &self.bytes[body_start..body_end],
            offset: header_off,
        };
        self.cursor = header_off + total_bytes;
        Some(Ok(chunk))
    }
}

pub fn walk(bytes: &[u8]) -> ChunkWalker<'_> {
    ChunkWalker::new(bytes)
}

pub const CHUNK_HEADER_BYTES: usize = 16;
const CHUNK_SIZE_SHIFT: u32 = 7;
const CHUNK_SIZE_FIELD_MASK: u32 = CHUNK_SIZE_UNITS_MASK << CHUNK_SIZE_SHIFT;

/// A whole chunk: header plus `body` zero-padded to the 16-byte unit. The
/// header's reserved bytes are zero and no flag bit is set.
pub fn encode(name: [u8; 4], kind: u8, body: &[u8]) -> Vec<u8> {
    let mut header = [0u8; CHUNK_HEADER_BYTES];
    header[..4].copy_from_slice(&name);
    header[4..8].copy_from_slice(&(kind as u32 & CHUNK_KIND_MASK).to_le_bytes());
    encode_with_header(&header, body)
}

/// `chunk` re-encoded around `body`, keeping its name, kind, flag bits above
/// the size field and the header's reserved bytes.
pub fn reencode(source: &[u8], chunk: &Chunk<'_>, body: &[u8]) -> Vec<u8> {
    let mut header = [0u8; CHUNK_HEADER_BYTES];
    header.copy_from_slice(&source[chunk.offset..chunk.offset + CHUNK_HEADER_BYTES]);
    encode_with_header(&header, body)
}

fn encode_with_header(header: &[u8; CHUNK_HEADER_BYTES], body: &[u8]) -> Vec<u8> {
    let total = (CHUNK_HEADER_BYTES + body.len()).next_multiple_of(CHUNK_HEADER_BYTES);
    let units = (total / CHUNK_HEADER_BYTES) as u32;
    assert!(
        units <= CHUNK_SIZE_UNITS_MASK,
        "chunk of {total} bytes overflows the size field"
    );
    let value = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    let value = (value & !CHUNK_SIZE_FIELD_MASK) | (units << CHUNK_SIZE_SHIFT);
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&header[..4]);
    out.extend_from_slice(&value.to_le_bytes());
    out.extend_from_slice(&header[8..]);
    out.extend_from_slice(body);
    out.resize(total, 0);
    out
}

#[derive(Debug, Clone)]
pub struct ChunkNode<'a> {
    pub chunk: Chunk<'a>,
    pub children: Vec<ChunkNode<'a>>,
}

pub fn walk_tree<'a>(bytes: &'a [u8]) -> ChunkNode<'a> {
    let synthetic_root = Chunk {
        name: [0; 4],
        kind: 0xFF,
        data: &[],
        offset: 0,
    };
    let mut root = ChunkNode {
        chunk: synthetic_root,
        children: Vec::new(),
    };

    let mut path: Vec<usize> = Vec::new();

    fn at_path<'r, 'a>(root: &'r mut ChunkNode<'a>, path: &[usize]) -> &'r mut ChunkNode<'a> {
        let mut cur = root;
        for &i in path {
            cur = &mut cur.children[i];
        }
        cur
    }

    for result in walk(bytes) {
        let Ok(chunk) = result else { continue };
        match chunk.kind {
            0x00 => {
                path.pop();
            }
            0x01 => {
                let cur = at_path(&mut root, &path);
                let idx = cur.children.len();
                cur.children.push(ChunkNode {
                    chunk,
                    children: Vec::new(),
                });
                path.push(idx);
            }
            _ => {
                let cur = at_path(&mut root, &path);
                cur.children.push(ChunkNode {
                    chunk,
                    children: Vec::new(),
                });
            }
        }
    }

    root
}
