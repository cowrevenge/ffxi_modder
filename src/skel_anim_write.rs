//! In-place edits to an animation (0x2B) chunk body, the inverse of the
//! layout `skel_anim::parse` reads: a 10-byte header (u16, u16 joint count,
//! u16 frame count, f32 frame duration), then one 84-byte record per joint
//! (u32 joint index; per channel group an i32 offset per channel, then an f32
//! constant per channel, for rotation x4, translation x3, scale x3). A
//! non-zero offset points at `frame count` floats at `POOL_START + 4 * offset`.
//!
//! Edits never overwrite existing pool data (sequences may be shared between
//! channels): a new sequence is appended to the end of the body and the
//! channel's offset is pointed at it. Everything not edited stays byte for byte.

const POOL_START: usize = 10;
const OFF_NUM_JOINTS: usize = 2;
const OFF_NUM_FRAMES: usize = 4;
const RECORD_BYTES: usize = 84;
const ROTATION_CHANNELS: usize = 4;
const FLOAT_BYTES: usize = 4;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnimWriteError {
    #[error("animation body shorter than its header or records")]
    Truncated,
    #[error("joint {0} has no record in this animation")]
    NoRecord(u32),
    #[error("joint {0}'s record is disabled (negative offset)")]
    Disabled(u32),
    #[error("expected {expected} frames, got {got}")]
    FrameCount { expected: usize, got: usize },
    #[error("pool offset overflows i32")]
    PoolOverflow,
}

fn u16_at(d: &[u8], o: usize) -> usize {
    u16::from_le_bytes([d[o], d[o + 1]]) as usize
}

fn i32_at(d: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(d[o..o + 4].try_into().expect("4 bytes"))
}

pub fn frame_count(body: &[u8]) -> Result<usize, AnimWriteError> {
    if body.len() < POOL_START {
        return Err(AnimWriteError::Truncated);
    }
    Ok(u16_at(body, OFF_NUM_FRAMES))
}

fn record_of(body: &[u8], joint: u32) -> Result<usize, AnimWriteError> {
    if body.len() < POOL_START {
        return Err(AnimWriteError::Truncated);
    }
    let n = u16_at(body, OFF_NUM_JOINTS);
    if body.len() < POOL_START + n * RECORD_BYTES {
        return Err(AnimWriteError::Truncated);
    }
    (0..n)
        .map(|i| POOL_START + i * RECORD_BYTES)
        .find(|&r| u32::from_le_bytes(body[r..r + 4].try_into().expect("4 bytes")) == joint)
        .ok_or(AnimWriteError::NoRecord(joint))
}

/// Replace `joint`'s rotation keys with `frames` (x, y, z, w per frame).
pub fn set_rotation_frames(body: &mut Vec<u8>, joint: u32, frames: &[[f32; 4]]) -> Result<(), AnimWriteError> {
    let expected = frame_count(body)?;
    if frames.len() != expected {
        return Err(AnimWriteError::FrameCount { expected, got: frames.len() });
    }
    let record = record_of(body, joint)?;
    let offsets_at = record + 4;
    if (0..ROTATION_CHANNELS).any(|c| i32_at(body, offsets_at + c * FLOAT_BYTES) < 0) {
        return Err(AnimWriteError::Disabled(joint));
    }
    for channel in 0..ROTATION_CHANNELS {
        while (body.len() - POOL_START) % FLOAT_BYTES != 0 {
            body.push(0);
        }
        let offset = i32::try_from((body.len() - POOL_START) / FLOAT_BYTES).map_err(|_| AnimWriteError::PoolOverflow)?;
        for f in frames {
            body.extend_from_slice(&f[channel].to_bits().to_le_bytes());
        }
        let o = offsets_at + channel * FLOAT_BYTES;
        body[o..o + 4].copy_from_slice(&offset.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datid::DatId;

    fn one_joint_clip(frames: u16) -> Vec<u8> {
        let mut b = vec![0u8; POOL_START];
        b[OFF_NUM_JOINTS..OFF_NUM_JOINTS + 2].copy_from_slice(&1u16.to_le_bytes());
        b[OFF_NUM_FRAMES..OFF_NUM_FRAMES + 2].copy_from_slice(&frames.to_le_bytes());
        b[6..10].copy_from_slice(&1.0f32.to_le_bytes());
        let mut rec = vec![0u8; RECORD_BYTES];
        rec[..4].copy_from_slice(&7u32.to_le_bytes());
        rec[4 + 16 + 12..4 + 16 + 16].copy_from_slice(&1.0f32.to_le_bytes());
        b.extend(rec);
        b.resize(b.len().next_multiple_of(16), 0);
        b
    }

    #[test]
    fn edited_rotation_reads_back_and_the_rest_is_untouched() {
        let mut body = one_joint_clip(3);
        let before = body.clone();
        let frames = [[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.5, 0.866], [0.0, 0.0, 0.0, 1.0]];
        set_rotation_frames(&mut body, 7, &frames).unwrap();
        let rec = POOL_START + 4;
        assert_eq!(&body[..rec], &before[..rec]);
        assert_eq!(&body[rec + 16..before.len()], &before[rec + 16..]);
        let anim = crate::skel_anim::parse(DatId::from_str("test"), &body);
        let keys = &anim.key_frame_sets[&7];
        assert_eq!(keys[1].rotation, frames[1]);
        assert_eq!(keys[2].rotation, frames[2]);
    }

    #[test]
    fn refuses_a_missing_joint_and_a_wrong_frame_count() {
        let mut body = one_joint_clip(2);
        assert_eq!(set_rotation_frames(&mut body, 9, &[[0.0; 4]; 2]), Err(AnimWriteError::NoRecord(9)));
        assert_eq!(
            set_rotation_frames(&mut body, 7, &[[0.0; 4]; 3]),
            Err(AnimWriteError::FrameCount { expected: 2, got: 3 })
        );
    }
}
