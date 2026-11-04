//! Lossless writer for FFXI skinned-mesh DATs, plus the readers it is built on.
//! The reader modules are copied from kuluu-ffxi's `ffxi-dat` crate
//! (https://github.com/cowrevenge/kuluu-ffxi, GPL-3.0-or-later).

pub mod chunk;
pub mod datid;
pub mod kind;
pub mod skel;
pub mod skel_anim;
pub mod skel_anim_write;
pub mod skel_mesh;
pub mod skel_mesh_write;
pub mod texture;
pub mod texture_write;
pub mod vos2;

#[derive(Debug, thiserror::Error)]
pub enum DatError {
    #[error("truncated chunk at offset {offset}: needed {needed} bytes, only {available} remain")]
    TruncatedChunk {
        offset: usize,
        needed: usize,
        available: usize,
    },
    #[error("MMB error: {0}")]
    Mmb(String),
}

pub type Result<T> = std::result::Result<T, DatError>;
