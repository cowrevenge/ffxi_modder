use std::collections::HashMap;

use crate::datid::DatId;

const POOL_START: usize = 10;

#[derive(Debug, Clone, Copy)]
pub struct KeyFrameTransform {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for KeyFrameTransform {
    fn default() -> Self {
        KeyFrameTransform {
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkeletonAnimation {
    pub id: DatId,
    pub num_joints: usize,
    pub num_frames: usize,
    pub key_frame_duration: f32,
    pub key_frame_sets: HashMap<u32, Vec<KeyFrameTransform>>,
}

impl SkeletonAnimation {
    pub fn get_joint_transform(&self, joint: u32, frame: f32) -> Option<KeyFrameTransform> {
        let set = self.key_frame_sets.get(&joint)?;

        let scaled = frame * self.key_frame_duration;
        if scaled >= (self.num_frames as f32) - 1.0 {
            return Some(set[self.num_frames - 1]);
        }

        let lower = scaled.floor() as usize;
        let delta = scaled - lower as f32;
        Some(interpolate(&set[lower], &set[lower + 1], delta))
    }

    pub fn length_in_frames(&self) -> f32 {
        let n = (self.num_frames as i64 - 1).max(1) as f32;
        n / self.key_frame_duration
    }
}

pub fn nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let r = if dot < 0.0 {
        [-b[0], -b[1], -b[2], -b[3]]
    } else {
        b
    };
    let inv = 1.0 - t;
    let mut q = [
        a[0] * inv + r[0] * t,
        a[1] * inv + r[1] * t,
        a[2] * inv + r[2] * t,
        a[3] * inv + r[3] * t,
    ];
    let mag = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let inv_mag = 1.0 / mag;
    for c in q.iter_mut() {
        *c *= inv_mag;
    }
    q
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let inv = 1.0 - t;
    [
        a[0] * inv + b[0] * t,
        a[1] * inv + b[1] * t,
        a[2] * inv + b[2] * t,
    ]
}

fn interpolate(a: &KeyFrameTransform, b: &KeyFrameTransform, delta: f32) -> KeyFrameTransform {
    KeyFrameTransform {
        rotation: nlerp(a.rotation, b.rotation, delta),
        translation: lerp3(a.translation, b.translation, delta),
        scale: lerp3(a.scale, b.scale, delta),
    }
}

fn read_u16(data: &[u8], off: usize) -> u16 {
    let b = |i: usize| data.get(off + i).copied().unwrap_or(0);
    u16::from_le_bytes([b(0), b(1)])
}

fn read_i32(data: &[u8], off: usize) -> i32 {
    let b = |i: usize| data.get(off + i).copied().unwrap_or(0);
    i32::from_le_bytes([b(0), b(1), b(2), b(3)])
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    let b = |i: usize| data.get(off + i).copied().unwrap_or(0);
    u32::from_le_bytes([b(0), b(1), b(2), b(3)])
}

fn read_f32(data: &[u8], off: usize) -> f32 {
    f32::from_bits(read_u32(data, off))
}

enum Sequence {
    Const(f32),
    Frames(Vec<f32>),
}

impl Sequence {
    fn value(&self, frame: usize) -> f32 {
        match self {
            Sequence::Const(v) => *v,
            Sequence::Frames(v) => v[frame],
        }
    }
}

fn read_sequences(
    data: &[u8],
    pos: &mut usize,
    amount: usize,
    num_frames: usize,
) -> Option<Vec<Sequence>> {
    let mut offsets = Vec::with_capacity(amount);
    for _ in 0..amount {
        offsets.push(read_i32(data, *pos));
        *pos += 4;
    }
    let mut consts = Vec::with_capacity(amount);
    for _ in 0..amount {
        consts.push(read_f32(data, *pos) % 10_000.0);
        *pos += 4;
    }

    if offsets.iter().any(|&o| o < 0) {
        return None;
    }

    let mut seqs = Vec::with_capacity(amount);
    for i in 0..amount {
        if offsets[i] == 0 {
            seqs.push(Sequence::Const(consts[i]));
        } else {
            let base = POOL_START + (offsets[i] as usize) * 4;
            let mut frames = Vec::with_capacity(num_frames);
            for f in 0..num_frames {
                frames.push(read_f32(data, base + f * 4));
            }
            seqs.push(Sequence::Frames(frames));
        }
    }
    Some(seqs)
}

pub fn parse(id: DatId, data: &[u8]) -> SkeletonAnimation {
    if data.len() < POOL_START {
        return SkeletonAnimation {
            id,
            num_joints: 0,
            num_frames: 0,
            key_frame_duration: 0.0,
            key_frame_sets: HashMap::new(),
        };
    }

    let _unk0 = read_u16(data, 0);
    let num_joints = read_u16(data, 2) as usize;
    let num_frames = read_u16(data, 4) as usize;
    let key_frame_duration = read_f32(data, 6);

    let mut pos = POOL_START;
    let mut key_frame_sets: HashMap<u32, Vec<KeyFrameTransform>> = HashMap::new();

    for _ in 0..num_joints {
        let joint_index = read_u32(data, pos);
        pos += 4;

        let rotation = read_sequences(data, &mut pos, 4, num_frames);
        let translation = read_sequences(data, &mut pos, 3, num_frames);
        let scale = read_sequences(data, &mut pos, 3, num_frames).map(|mut channels| {
            // A constant-only scale channel reading exactly zero is an *unwritten* channel, not a bone scaled
            // to nothing. `hum_` joint 89 reads that way in the shipped locomotion clips (`idl1`/`wlk1`/
            // `mvl1`/`mvr1`): its world basis measured exactly 0.0 the frame one of them owned the bone, taking
            // joints 90-93 below it with it — the upper body vanishing mid-strafe. Authored identities read as
            // (1,1,1) and animated channels keep their values verbatim (shipped motion reaches -0.033 to 1.31),
            // so this is only about reading "unused": see `zero_scale_constants_read_as_identity`.
            for channel in &mut channels {
                if let Sequence::Const(value) = channel {
                    if *value == 0.0 {
                        *value = 1.0;
                    }
                }
            }
            channels
        });

        let (Some(rotation), Some(translation), Some(scale)) = (rotation, translation, scale)
        else {
            continue;
        };

        let mut frames = Vec::with_capacity(num_frames);
        for f in 0..num_frames {
            frames.push(KeyFrameTransform {
                rotation: [
                    rotation[0].value(f),
                    rotation[1].value(f),
                    rotation[2].value(f),
                    rotation[3].value(f),
                ],
                translation: [
                    translation[0].value(f),
                    translation[1].value(f),
                    translation[2].value(f),
                ],
                scale: [scale[0].value(f), scale[1].value(f), scale[2].value(f)],
            });
        }
        key_frame_sets.insert(joint_index, frames);
    }

    SkeletonAnimation {
        id,
        num_joints,
        num_frames,
        key_frame_duration,
        key_frame_sets,
    }
}
