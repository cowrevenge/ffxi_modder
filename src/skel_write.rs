//! In-place edits to a skeleton (0x29) chunk body, the inverse of the layout
//! `skel::parse` reads: u8 joint count at 0x02, one 30-byte joint record per
//! joint from 0x04 (u8 parent, u8, quaternion f32 x4, translation f32 x3), a
//! u16 reference count plus 2 bytes, 26-byte reference records (u16 joint,
//! f32 x3 rotation, f32 x3 offset), then the bounding boxes (six f32 each,
//! y-top first). Every edit writes one float field and is checked by
//! re-parsing, so nothing outside that field can move.
//!
//! [`fit_height`] is the one to call after any mesh edit that changes the
//! model's height: it moves the nameplate reference and the full-body boxes so
//! they keep their original gap above the model's highest point.

use crate::datid::DatId;
use crate::skel::{self, standard_position, Skeleton};

const JOINT_RECORDS_AT: usize = 0x04;
const JOINT_RECORD_BYTES: usize = 30;
const JOINT_TRANSLATION_AT: usize = 18;
const REFERENCE_HEADER_BYTES: usize = 4;
const REFERENCE_RECORD_BYTES: usize = 26;
const REFERENCE_OFFSET_AT: usize = 14;
const BOUNDING_BOX_BYTES: usize = 24;
const FLOAT_BYTES: usize = 4;
const Y: usize = 1;

/// The first two boxes span the whole body; the third is the ground footprint.
pub const FULL_BODY_BOXES: usize = 2;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SkelWriteError {
    #[error("joint {0} is out of range")]
    Joint(usize),
    #[error("reference {0} is out of range")]
    Reference(usize),
    #[error("bounding box {0} is out of range")]
    Box(usize),
    #[error("field at {0:#x} is past the end of the body")]
    Truncated(usize),
    #[error("edit read back wrong: {0}")]
    ReadBack(&'static str),
    #[error("the nameplate reference hangs off joint {0}, not the root")]
    NameplateNotOnRoot(usize),
}

pub struct SkelEditor {
    body: Vec<u8>,
    base: Skeleton,
}

impl SkelEditor {
    pub fn new(body: &[u8]) -> Self {
        SkelEditor {
            body: body.to_vec(),
            base: skel::parse(DatId::from_str("edit"), body),
        }
    }

    /// The skeleton as first parsed, before any edit.
    pub fn original(&self) -> &Skeleton {
        &self.base
    }

    pub fn current(&self) -> Skeleton {
        skel::parse(DatId::from_str("edit"), &self.body)
    }

    fn put(&mut self, at: usize, v: f32) -> Result<(), SkelWriteError> {
        let field = self
            .body
            .get_mut(at..at + FLOAT_BYTES)
            .ok_or(SkelWriteError::Truncated(at))?;
        field.copy_from_slice(&v.to_le_bytes());
        Ok(())
    }

    fn references_at(&self) -> usize {
        JOINT_RECORDS_AT + self.base.joints.len() * JOINT_RECORD_BYTES + REFERENCE_HEADER_BYTES
    }

    fn boxes_at(&self) -> usize {
        self.references_at() + self.base.references.len() * REFERENCE_RECORD_BYTES
    }

    pub fn set_joint_translation(&mut self, joint: usize, t: [f32; 3]) -> Result<(), SkelWriteError> {
        if joint >= self.base.joints.len() {
            return Err(SkelWriteError::Joint(joint));
        }
        let at = JOINT_RECORDS_AT + joint * JOINT_RECORD_BYTES + JOINT_TRANSLATION_AT;
        for (axis, v) in t.iter().enumerate() {
            self.put(at + axis * FLOAT_BYTES, *v)?;
        }
        if self.current().joints[joint].translation != t {
            return Err(SkelWriteError::ReadBack("joint translation"));
        }
        Ok(())
    }

    pub fn set_reference_offset(&mut self, reference: usize, offset: [f32; 3]) -> Result<(), SkelWriteError> {
        if reference >= self.base.references.len() {
            return Err(SkelWriteError::Reference(reference));
        }
        let at = self.references_at() + reference * REFERENCE_RECORD_BYTES + REFERENCE_OFFSET_AT;
        for (axis, v) in offset.iter().enumerate() {
            self.put(at + axis * FLOAT_BYTES, *v)?;
        }
        if self.current().references[reference].position_offset != offset {
            return Err(SkelWriteError::ReadBack("reference offset"));
        }
        Ok(())
    }

    /// The box's top (`y_max`, the first of its six floats; y is down).
    pub fn set_box_top(&mut self, index: usize, top: f32) -> Result<(), SkelWriteError> {
        if index >= self.base.bounding_boxes.len() {
            return Err(SkelWriteError::Box(index));
        }
        self.put(self.boxes_at() + index * BOUNDING_BOX_BYTES, top)?;
        if self.current().bounding_boxes.get(index).map(|b| b.y_max) != Some(top) {
            return Err(SkelWriteError::ReadBack("box top"));
        }
        Ok(())
    }

    /// Keep the nameplate and the full-body boxes the same distance above the
    /// model's highest point as they were in the original: `original_top` and
    /// `new_top` are the highest bind-pose vertex y before and after the mesh
    /// edit (y is down, so higher is smaller). The nameplate reference is read
    /// as a fixed offset from the root, so it must hang off joint 0.
    pub fn fit_height(&mut self, original_top: f32, new_top: f32) -> Result<HeightFit, SkelWriteError> {
        let reference = standard_position::ABOVE_HEAD;
        let nameplate = self
            .base
            .reference_at(reference)
            .cloned()
            .ok_or(SkelWriteError::Reference(reference))?;
        if nameplate.index != 0 {
            return Err(SkelWriteError::NameplateNotOnRoot(nameplate.index));
        }
        let shift = new_top - original_top;
        let mut offset = nameplate.position_offset;
        offset[Y] += shift;
        self.set_reference_offset(reference, offset)?;
        let boxes: Vec<f32> = self.base.bounding_boxes.iter().take(FULL_BODY_BOXES).map(|b| b.y_max).collect();
        for (i, top) in boxes.iter().enumerate() {
            self.set_box_top(i, top + shift)?;
        }
        Ok(HeightFit {
            nameplate: (nameplate.position_offset[Y], offset[Y]),
            boxes: boxes.iter().map(|t| (*t, t + shift)).collect(),
        })
    }

    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

/// (before, after) of each field [`SkelEditor::fit_height`] moved.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightFit {
    pub nameplate: (f32, f32),
    pub boxes: Vec<(f32, f32)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENTINEL: u32 = 0xCDCD_CDCD;

    /// Two joints, three references, two boxes, then sentinel padding to the
    /// look-at records.
    fn sample() -> Vec<u8> {
        let mut b = vec![0u8, 0, 2, 0];
        for (parent, t) in [(0u8, [0.0f32, -1.0, 0.0]), (0u8, [1.0, 0.0, 0.0])] {
            b.push(parent);
            b.push(0);
            for q in [0.0f32, 0.0, 0.0, 1.0] {
                b.extend_from_slice(&q.to_le_bytes());
            }
            for v in t {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&[0, 0]);
        for (joint, y) in [(0u16, 0.0f32), (1, 0.0), (0, -6.0)] {
            b.extend_from_slice(&joint.to_le_bytes());
            for v in [0.0f32, 0.0, 0.0, 0.0, y, 0.0] {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        for top in [-6.4f32, -6.2] {
            for v in [top, 0.0, 1.0, -1.0, 1.0, -1.0] {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        while b.len() % 4 != 0 {
            b.push(0);
        }
        for _ in 0..8 {
            b.extend_from_slice(&SENTINEL.to_le_bytes());
        }
        b
    }

    #[test]
    fn fit_height_moves_only_the_nameplate_and_box_tops() {
        let body = sample();
        let mut ed = SkelEditor::new(&body);
        let fit = ed.fit_height(-5.0, -3.0).unwrap();
        assert_eq!(fit.nameplate, (-6.0, -4.0));
        assert_eq!(fit.boxes, vec![(-6.4, -4.4), (-6.2, -4.2)]);
        let out = ed.into_body();
        let changed: Vec<usize> = (0..body.len()).filter(|&i| body[i] != out[i]).collect();
        assert!(changed.len() <= 3 * FLOAT_BYTES, "changed bytes {changed:?}");
        let s = skel::parse(DatId::from_str("t"), &out);
        assert_eq!(s.references[standard_position::ABOVE_HEAD].position_offset[Y], -4.0);
        assert_eq!(s.joints[1].translation, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn joint_translation_round_trips_and_out_of_range_is_refused() {
        let mut ed = SkelEditor::new(&sample());
        ed.set_joint_translation(1, [0.5, 0.25, 0.0]).unwrap();
        assert_eq!(ed.current().joints[1].translation, [0.5, 0.25, 0.0]);
        assert_eq!(ed.set_joint_translation(9, [0.0; 3]), Err(SkelWriteError::Joint(9)));
        assert_eq!(ed.set_box_top(5, 0.0), Err(SkelWriteError::Box(5)));
    }
}
