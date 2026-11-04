use crate::datid::DatId;

// research/xim resource/SkeletonSection.kt StandardPosition
pub mod standard_position {
    /// The in-world look-at aims at the target's attach point 3 (FFXiMain.dll retail-2026-09 RVA
    /// 0xD5C86); research/XIClient/src/XIClient/include/World/Actor/EID_INDEX.h EID_NECK names it.
    pub const NECK: usize = 3;
    /// The bend's second record bends the bone named by reference slot 7, which XIM names EID_CHEST;
    /// retail writes the two bend references as the byte pair {3, 7} at FFXiMain.dll retail-2026-09 RVA
    /// 0x2AC7A / 0x2AC7F and resolves each through the reference table by RVA 0x2A9B0.
    pub const CHEST: usize = 7;
    pub const ABOVE_HEAD: usize = 2;
    pub const RIGHT_FOOT: usize = 8;
    pub const LEFT_FOOT: usize = 9;
    pub const LEFT_HAND: usize = 126;
    pub const RIGHT_HAND: usize = 127;
}

const CDCD_SENTINEL: u32 = 0xCDCDCDCD;

/// Bytes between the end of the joint-reference table and the first look-at record, matching the
/// fixed `+0x48` in FFXiMain.dll retail-2026-09 RVA 0x35270; that gap is where the bounding boxes live.
const LOOK_AT_RECORDS_OFFSET: usize = 0x48;

/// Stride of one look-at record, from the same `+12*index` term in FFXiMain.dll retail-2026-09 RVA
/// 0x35270.
const LOOK_AT_RECORD_BYTES: usize = 12;

/// One bounding box is six f32 extents.
const BOUNDING_BOX_BYTES: usize = 6 * 4;

#[derive(Debug, Clone)]
pub struct Joint {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub parent: Option<usize>,
}

/// One entry of a skeleton's reference table (`FFXiMain.dll retail-2026-09` RVA 0x2A9B5 reads an entry as a `word`
/// joint index plus three floats at +2/+6/+0xA and an offset, stride 26).
#[derive(Debug, Clone)]
pub struct JointReference {
    /// The joint this reference stands for: the entry's first `u16` (`FFXiMain.dll retail-2026-09` reads it at
    /// RVA 0x2A9D4) and used directly as a bone index, with no remap behind it (RVA 0x2AFED → 0x2A9B0 in the look-at
    /// bend).
    pub index: usize,
    /// The attach frame's authored Euler rotation in radians, applied X then Y then Z onto a zeroed matrix
    /// (`FFXiMain.dll retail-2026-09` RVA 0x27B80 / 0x27BD0 / 0x27C20 inside the builder at RVA 0x2A780). Measured
    /// across all six PC skeletons it is authored `(0,0,0)` on the slots the look-at bend uses (3, 4 and 7), so no
    /// shipped rig distinguishes the application order today.
    pub rotation: [f32; 3],
    /// The attach frame's translation in the skeleton's own axis space, written as that matrix's translation
    /// (`FFXiMain.dll retail-2026-09` RVA 0x27CF0 to `+0x30/+0x34/+0x38`) — which is why it places relative to the
    /// joint instead of being spun by
    /// [`JointReference::rotation`].
    pub position_offset: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub y_max: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub x_min: f32,
    pub z_max: f32,
    pub z_min: f32,
}

/// One authored angular limit for the look-at bend, as `FFXiMain.dll` retail-2026-09 RVA
/// 0x2B140 reads it: an ellipse in the bone's tangent plane with semi-axes `x_limit` /
/// `y_limit`, projected with `scale`. A record of zeros means that bone does not bend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookAtLimit {
    pub x_limit: f32,
    pub y_limit: f32,
    pub scale: f32,
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub id: DatId,
    pub joints: Vec<Joint>,
    pub references: Vec<JointReference>,
    /// Only the boxes inside [`LOOK_AT_RECORDS_OFFSET`] are authored; past it the section holds
    /// look-at records, not geometry.
    pub bounding_boxes: Vec<BoundingBox>,
    pub look_at_limits: Vec<LookAtLimit>,
}

impl Skeleton {
    pub fn reference_at(&self, standard_index: usize) -> Option<&JointReference> {
        self.references.get(standard_index)
    }

    /// Bend record `record` (0 then 1; the bend skips a zeroed record), as indexed by
    /// `FFXiMain.dll` retail-2026-09 RVA 0x35270 and applied in order by RVA 0x2AC60, which drops
    /// the second record for certain actor statuses.
    pub fn look_at_limit(&self, record: usize) -> Option<&LookAtLimit> {
        self.look_at_limits.get(record)
    }

    /// The vertical extent of the first authored box, its second value less its first (head top to feet in
    /// the skeleton's y-down space): the span retail's chase camera sets its pivot height from
    /// (`FFXiMain.dll retail-2026-09`, RVA 0x1F3A2 reads it through RVA 0x35250).
    pub fn height_span(&self) -> Option<f32> {
        self.bounding_boxes.first().map(|b| b.y_min - b.y_max)
    }
}

fn read_f32(data: &[u8], off: usize) -> f32 {
    f32::from_bits(read_f32_bits(data, off))
}

fn read_f32_bits(data: &[u8], off: usize) -> u32 {
    let b = |i: usize| data.get(off + i).copied().unwrap_or(0);
    u32::from_le_bytes([b(0), b(1), b(2), b(3)])
}

fn read_u16(data: &[u8], off: usize) -> u16 {
    let b = |i: usize| data.get(off + i).copied().unwrap_or(0);
    u16::from_le_bytes([b(0), b(1)])
}

fn read_u8(data: &[u8], off: usize) -> u8 {
    data.get(off).copied().unwrap_or(0)
}

pub fn parse(id: DatId, data: &[u8]) -> Skeleton {
    let num_joints = read_u8(data, 0x02) as usize;

    let mut joints = Vec::with_capacity(num_joints);
    let mut pos = 0x04;
    for i in 0..num_joints {
        let maybe_parent = read_u8(data, pos) as usize;

        let parent = if maybe_parent == i {
            None
        } else {
            Some(maybe_parent)
        };
        pos += 2;

        let rotation = [
            read_f32(data, pos),
            read_f32(data, pos + 4),
            read_f32(data, pos + 8),
            read_f32(data, pos + 12),
        ];
        let translation = [
            read_f32(data, pos + 16),
            read_f32(data, pos + 20),
            read_f32(data, pos + 24),
        ];
        pos += 28;
        joints.push(Joint {
            rotation,
            translation,
            parent,
        });
    }

    let num_references = read_u16(data, pos) as usize;
    pos += 2;
    pos += 2;

    let mut references = Vec::with_capacity(num_references);
    for _ in 0..num_references {
        let index = read_u16(data, pos) as usize;
        let rotation = [
            read_f32(data, pos + 2),
            read_f32(data, pos + 6),
            read_f32(data, pos + 10),
        ];
        let position_offset = [
            read_f32(data, pos + 14),
            read_f32(data, pos + 18),
            read_f32(data, pos + 22),
        ];
        pos += 26;
        references.push(JointReference {
            index,
            rotation,
            position_offset,
        });
    }

    // The section after the reference table is fixed-size, not count-prefixed: the boxes occupy
    // exactly `LOOK_AT_RECORDS_OFFSET` bytes and the look-at records start there whatever the box
    // data looks like. FFXiMain.dll retail-2026-09 RVA 0x35270 indexes a record as
    // `refs_end + 0x48 + 12*i`, so reading past that offset as boxes turns authored limits (and the
    // sentinel padding after them) into fake geometry.
    let look_at_base = pos + LOOK_AT_RECORDS_OFFSET;
    let box_limit = (pos + LOOK_AT_RECORDS_OFFSET).min(data.len());
    let mut bounding_boxes = Vec::new();
    'boxes: while pos + BOUNDING_BOX_BYTES <= box_limit {
        let mut vals = [0f32; 6];
        for v in vals.iter_mut() {
            if read_f32_bits(data, pos) == CDCD_SENTINEL {
                break 'boxes;
            }
            *v = read_f32(data, pos);
            pos += 4;
        }
        bounding_boxes.push(BoundingBox {
            y_max: vals[0],
            y_min: vals[1],
            x_max: vals[2],
            x_min: vals[3],
            z_max: vals[4],
            z_min: vals[5],
        });
    }

    let mut look_at_limits = Vec::new();
    'records: {
        if look_at_base > data.len() {
            break 'records;
        }
        let mut pos = look_at_base;
        loop {
            if pos + LOOK_AT_RECORD_BYTES > data.len() || read_f32_bits(data, pos) == CDCD_SENTINEL
            {
                break;
            }
            look_at_limits.push(LookAtLimit {
                x_limit: read_f32(data, pos),
                y_limit: read_f32(data, pos + 4),
                scale: read_f32(data, pos + 8),
            });
            pos += LOOK_AT_RECORD_BYTES;
        }
    }

    Skeleton {
        id,
        joints,
        references,
        bounding_boxes,
        look_at_limits,
    }
}
