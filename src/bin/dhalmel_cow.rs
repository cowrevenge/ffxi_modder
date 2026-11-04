//! Turn the Dhalmel into a Holstein milk cow.
//!
//! usage: dhalmel_cow <in ROM\5\15.DAT> <out.DAT>
//!
//! Stage 1: short neck and black/white paint. Stage 2: cow head, horns,
//! ears, eyes and udder. Stage 3: attack clips lift then nudge or smack.
//! - Neck: the bind translation of every joint hanging off the neck chain is
//!   scaled down. Animation translation is added to the bind translation and
//!   every retail Dhalmel animation keys zero translation on the neck, so all
//!   animations play on the shorter neck. Neck and head geometry is stored
//!   joint-local, so it rides along.
//! - Paint: every texture is repainted white with black patches. Patches come
//!   from 3D noise over the bind-pose body, so they continue across UV seams;
//!   the original texel brightness is kept as shading and the original alpha
//!   blocks are kept byte for byte.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::process::ExitCode;

use ffxi_modder::chunk::{self, Chunk};
use ffxi_modder::datid::DatId;
use ffxi_modder::kind::ChunkKind;
use ffxi_modder::skel::{self, standard_position, Skeleton};
use ffxi_modder::skel_anim;
use ffxi_modder::skel_anim_write;
use ffxi_modder::skel_mesh_write::{
    pack_joint_ref, Instruction, SingleVertex, SkelMeshImage, TexturedCorner, MAX_TRIS_PER_DRAW,
    TEXTURE_NAME_BYTES,
};
use ffxi_modder::texture;
use ffxi_modder::texture_write;

/// Neck bone length kept, as a fraction of the Dhalmel's.
const NECK_SCALE: f32 = 0.3;

const JOINT_RECORDS_AT: usize = 0x04;
const JOINT_RECORD_BYTES: usize = 30;
const JOINT_TRANSLATION_AT: usize = 18;

const OFF_FLAGS3: usize = 0x02;
const FLAG3_USE_JOINT_ARRAY: u8 = 0x80;
const SLOT_MASK: u16 = 0x7F;
const TEXTURE_NAME_AT: usize = 0x01;

const WHITE: [f32; 3] = [242.0, 240.0, 234.0];
const BLACK: [f32; 3] = [24.0, 22.0, 22.0];
/// Patch size: noise cycles per world unit, base and detail octave.
const SPOT_FREQUENCY: f32 = 1.1;
const DETAIL_WEIGHT: f32 = 0.3;
/// Noise level above which a point is black; about a third of the hide.
const SPOT_THRESHOLD: f32 = 0.62;
const SHADE_MIN: f32 = 0.72;
const SHADE_MAX: f32 = 1.12;
const DILATE_PASSES: usize = 3;

const DOMINANT_WEIGHT: f32 = 0.5;
const FLIPPED_SHIFT: u16 = 7;
const AXIS_SHIFT: u16 = 14;
const MIRROR_AXIS_Z: u16 = 3;
const PI: f32 = std::f32::consts::PI;

/// Cow head, in units of the Dhalmel head's half-extent: length, then
/// (fraction of length, vertical radius, side radius, lift) back to nose.
const HEAD_LENGTH: f32 = 2.1;
const HEAD_PROFILE: &[(f32, f32, f32, f32)] = &[
    (0.00, 0.55, 0.55, 0.05),
    (0.08, 0.80, 0.72, 0.10),
    (0.25, 0.88, 0.78, 0.12),
    (0.45, 0.78, 0.66, 0.05),
    (0.65, 0.62, 0.55, -0.10),
    (0.82, 0.62, 0.60, -0.20),
    (0.95, 0.55, 0.55, -0.24),
];
const HEAD_NOSE_T: f32 = 1.0;
/// Face pitched down from level, degrees.
const HEAD_DROOP: f32 = 25.0;
/// Skull starts this far behind the head joint (head half-extents), over the neck end.
const HEAD_BACK: f32 = -0.15;
const MUZZLE_FROM: f32 = 0.74;
const HEAD_SEGMENTS: usize = 10;
const HEAD_SPOT_THRESHOLD: f32 = 0.58;
const HORN_T: f32 = 0.2;
const HORN_THETA: f32 = 0.3 * PI;
const HORN_OUT: f32 = 1.0;
const HORN_UP: f32 = 0.55;
const HORN_FWD: f32 = 0.15;
const HORN_LENGTH: f32 = 0.7;
const HORN_RADIUS: f32 = 0.12;
const HORN_CURL: f32 = 0.25;
const EAR_T: f32 = 0.3;
const EAR_THETA: f32 = 0.5 * PI;
const EAR_OUT: f32 = 1.0;
const EAR_DOWN: f32 = 0.35;
const EAR_BACK: f32 = 0.2;
const EAR_LENGTH: f32 = 0.55;
const EAR_WIDTH: f32 = 0.18;
const EAR_FLATTEN: f32 = 0.3;
const EYE_T: f32 = 0.42;
const EYE_THETA: f32 = 0.38 * PI;
const EYE_RADIUS: f32 = 0.09;
const TUBE_RINGS: usize = 6;
const TUBE_SEGMENTS: usize = 8;
const SPHERE_RINGS: usize = 6;

/// Hips: the branching joint (besides the shoulders and head) with at least
/// this many children: tail plus both hind legs.
const HIP_MIN_CHILDREN: usize = 3;
const LEG_OFF_MIDLINE: f32 = 0.2;
/// Udder sizes as fractions of the hind-leg-to-shoulder length.
const UDDER_AHEAD: f32 = 0.2;
const UDDER_PROBE: f32 = 0.3;
const UDDER_RX: f32 = 0.24;
const UDDER_RY: f32 = 0.16;
const UDDER_RZ: f32 = 0.2;
const UDDER_DROP: f32 = 0.35;
const UDDER_RINGS: usize = 8;
const UDDER_SEGMENTS: usize = 8;
const TEAT_X: [f32; 2] = [-0.45, 0.45];
const TEAT_Z: f32 = 0.45;
const TEAT_BASE: f32 = 0.85;
const TEAT_LENGTH: f32 = 0.12;
const TEAT_RADIUS: f32 = 0.035;

const ATLAS_NAME: &[u8; TEXTURE_NAME_BYTES] = b"tim     cowatlas";
const ATLAS_CHUNK: &[u8; 4] = b"cowa";
const ATLAS_SIZE: u32 = 16;

/// Attack clips and their new neck motion.
const ATTACKS: [([u8; 4], Attack); 3] = [(*b"at00", Attack::Smack), (*b"at10", Attack::Nudge), (*b"atm1", Attack::Double)];
/// Share of the motion on the head joint; the rest is spread over the neck.
const HEAD_SHARE: f32 = 0.35;
const LIFT_DEG: f32 = 40.0;
const SMACK_DEG: f32 = 30.0;
const NUDGE_DEG: f32 = 25.0;
/// The lift peaks at this fraction of the way to the hit frame.
const LIFT_AT: f32 = 0.55;

type V3 = [f32; 3];
type M3 = [[f32; 3]; 3];

#[derive(Clone, Copy)]
struct Pose {
    r: M3,
    t: V3,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} <in.DAT> <out.DAT>", args[0]);
        return ExitCode::from(2);
    }
    let input = fs::read(&args[1]).expect("read input DAT");
    match cowify(&input) {
        Ok(out) => {
            fs::write(&args[2], out).expect("write output DAT");
            println!("wrote {}", args[2]);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn chunk_end(c: &Chunk<'_>) -> usize {
    c.offset + chunk::CHUNK_HEADER_BYTES + c.data.len()
}

fn cowify(input: &[u8]) -> Result<Vec<u8>, String> {
    let all: Vec<Chunk> = chunk::walk(input)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let bone = all
        .iter()
        .find(|c| c.kind == ChunkKind::Bone as u8)
        .ok_or("no skeleton chunk")?;
    let skeleton = skel::parse(DatId::from_name(&bone.name), bone.data);
    let pose = bind_pose(&skeleton)?;

    // Neck chain: parents of the look-at (head) joint down to the first branch.
    let head_joint = skeleton
        .reference_at(standard_position::NECK)
        .ok_or("skeleton has no look-at reference")?
        .index;
    let mut children: HashMap<usize, usize> = HashMap::new();
    for j in &skeleton.joints {
        if let Some(p) = j.parent {
            *children.entry(p).or_default() += 1;
        }
    }
    let mut neck: HashSet<usize> = HashSet::new();
    let mut cur = skeleton.joints[head_joint].parent.ok_or("head joint is the root")?;
    while children.get(&cur).copied().unwrap_or(0) <= 1 {
        neck.insert(cur);
        cur = skeleton.joints[cur].parent.ok_or("neck reaches the root without branching")?;
    }
    let shortened: Vec<usize> = (0..skeleton.joints.len())
        .filter(|&j| skeleton.joints[j].parent.is_some_and(|p| neck.contains(&p)))
        .collect();
    let mut new_bone = bone.data.to_vec();
    for &j in &shortened {
        let at = JOINT_RECORDS_AT + j * JOINT_RECORD_BYTES + JOINT_TRANSLATION_AT;
        for (axis, v) in skeleton.joints[j].translation.iter().enumerate() {
            let o = at + axis * 4;
            new_bone[o..o + 4].copy_from_slice(&(v * NECK_SCALE).to_le_bytes());
        }
    }
    let check = skel::parse(DatId::from_name(&bone.name), &new_bone);
    for (j, (a, b)) in skeleton.joints.iter().zip(&check.joints).enumerate() {
        let want = if shortened.contains(&j) { a.translation.map(|v| v * NECK_SCALE) } else { a.translation };
        if b.translation != want || b.rotation != a.rotation || b.parent != a.parent {
            return Err(format!("skeleton edit read back wrong at joint {j}"));
        }
    }

    // Bind-pose world position of every stored mesh vertex.
    let meshes: Vec<&Chunk> = all.iter().filter(|c| c.kind == ChunkKind::VertexOs2 as u8).collect();
    let [mesh_chunk] = meshes[..] else {
        return Err(format!("expected one skinned mesh chunk, found {}", meshes.len()));
    };
    let mut image = SkelMeshImage::parse(mesh_chunk.data).map_err(|e| e.to_string())?;
    let use_joint_array = image.header[OFF_FLAGS3] & FLAG3_USE_JOINT_ARRAY != 0;
    let joint_of = |r: u16| -> usize {
        let slot = (r & SLOT_MASK) as usize;
        if use_joint_array { image.joint_array[slot] as usize } else { slot }
    };
    let singles = image.singles.len();
    let world: Vec<V3> = (0..image.joint_refs.len())
        .map(|v| {
            let [r0, r1] = image.joint_refs[v];
            if v < singles {
                let j = joint_of(r0);
                add(mat_vec(&pose[j].r, image.singles[v].position), pose[j].t)
            } else {
                let d = &image.doubles[v - singles];
                let (j0, j1) = (joint_of(r0), joint_of(r1));
                add(
                    add(mat_vec(&pose[j0].r, d.position[0]), scale(pose[j0].t, d.weight[0])),
                    add(mat_vec(&pose[j1].r, d.position[1]), scale(pose[j1].t, d.weight[1])),
                )
            }
        })
        .collect();

    // Triangles with UVs, grouped by the texture they draw with.
    let mut by_texture: HashMap<[u8; TEXTURE_NAME_BYTES], Vec<[(usize, [f32; 2]); 3]>> = HashMap::new();
    let mut current = None;
    for ins in &image.instructions {
        match ins {
            Instruction::Texture(n) => current = Some(*n),
            Instruction::TriMesh(tris) => {
                if let Some(n) = current {
                    let e = by_texture.entry(n).or_default();
                    e.extend(tris.iter().map(|t| t.map(|c| (c.index as usize, c.uv))));
                }
            }
            Instruction::TriStrip { head, tail } => {
                if let Some(n) = current {
                    let corners: Vec<_> = head.iter().chain(tail).map(|c| (c.index as usize, c.uv)).collect();
                    by_texture.entry(n).or_default().extend(corners.windows(3).map(|w| [w[0], w[1], w[2]]));
                }
            }
            _ => {}
        }
    }

    let mut new_textures: HashMap<usize, Vec<u8>> = HashMap::new();
    for c in all.iter().filter(|c| c.kind == ChunkKind::Img as u8) {
        let name: [u8; TEXTURE_NAME_BYTES] = c.data[TEXTURE_NAME_AT..TEXTURE_NAME_AT + TEXTURE_NAME_BYTES]
            .try_into()
            .map_err(|_| "texture chunk too short")?;
        let Some(tris) = by_texture.get(&name) else {
            println!("texture {:?}: not drawn by the mesh, left alone", String::from_utf8_lossy(&name));
            continue;
        };
        let decoded = texture::decode_texture(c.data).map_err(|e| e.to_string())?;
        let painted = paint(&decoded.rgba, decoded.width as usize, decoded.height as usize, tris, &world);
        let body = texture_write::recolor_dxt3(c.data, &painted).map_err(|e| e.to_string())?;
        println!(
            "texture {:?} {}x{}: painted from {} triangles",
            String::from_utf8_lossy(&name),
            decoded.width,
            decoded.height,
            tris.len()
        );
        new_textures.insert(c.offset, body);
    }

    // ---- Stage 2: cow head, horns, ears, eyes, udder ----
    let short_pose = bind_pose(&check)?;
    let mut head: HashSet<usize> = HashSet::from([head_joint]);
    let mut grew = true;
    while grew {
        grew = false;
        for (i, j) in skeleton.joints.iter().enumerate() {
            if j.parent.is_some_and(|p| head.contains(&p)) && head.insert(i) {
                grew = true;
            }
        }
    }
    let dominant: Vec<usize> = (0..image.joint_refs.len())
        .map(|v| {
            let [r0, r1] = image.joint_refs[v];
            if v >= singles && image.doubles[v - singles].weight[1] > DOMINANT_WEIGHT {
                joint_of(r1)
            } else {
                joint_of(r0)
            }
        })
        .collect();
    let mut mirror_refs: HashMap<usize, HashMap<u16, usize>> = HashMap::new();
    for (v, refs) in image.joint_refs.iter().enumerate() {
        let used = if v < singles { &refs[..1] } else { &refs[..] };
        for &r in used {
            *mirror_refs.entry(joint_of(r)).or_default().entry(r >> FLIPPED_SHIFT).or_default() += 1;
        }
    }
    let slot_of = |joint: usize| -> Result<u16, String> {
        if use_joint_array {
            image
                .joint_array
                .iter()
                .position(|&j| j as usize == joint)
                .map(|s| s as u16)
                .ok_or(format!("joint {joint} not in the mesh's joint array"))
        } else {
            Ok(joint as u16)
        }
    };
    let ref_for = |joint: usize| -> Result<u16, String> {
        let (&upper, _) = mirror_refs
            .get(&joint)
            .and_then(|m| m.iter().max_by_key(|(_, n)| **n))
            .ok_or(format!("joint {joint} has no vertex to copy a mirror ref from"))?;
        let (flipped, axis) = (upper & SLOT_MASK, upper >> (AXIS_SHIFT - FLIPPED_SHIFT));
        if axis != MIRROR_AXIS_Z {
            return Err(format!("joint {joint} mirrors on axis {axis}; the generator builds a z-half"));
        }
        Ok(pack_joint_ref(slot_of(joint)?, flipped, axis))
    };

    // Head size from its single-jointed vertices, in head space.
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in 0..singles {
        if joint_of(image.joint_refs[v][0]) == head_joint {
            let p = image.singles[v].position;
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
    }
    if lo[0] > hi[0] {
        return Err("no single-jointed head vertices".into());
    }
    let head_half = ((hi[1] - lo[1]) / 2.0).max(hi[2] - lo[2]);

    // Drop every triangle wholly on the old head.
    let mut removed = 0usize;
    let mut kept_ops = Vec::with_capacity(image.instructions.len());
    for ins in image.instructions.drain(..) {
        let tris = ins.triangles();
        let gone = |t: &[u16; 3]| t.iter().all(|&i| head.contains(&dominant[i as usize]));
        let gone_count = tris.iter().filter(|t| gone(t)).count();
        if gone_count == 0 {
            kept_ops.push(ins);
            continue;
        }
        removed += gone_count;
        match ins {
            Instruction::TriMesh(t) => {
                let left: Vec<_> = t
                    .into_iter()
                    .filter(|c| !c.iter().all(|c| head.contains(&dominant[c.index as usize])))
                    .collect();
                if !left.is_empty() {
                    kept_ops.push(Instruction::TriMesh(left));
                }
            }
            strip if gone_count == tris.len() => drop(strip),
            _ => return Err("an op is only partly on the head and cannot be split".into()),
        }
    }
    image.instructions = kept_ops;

    let hips = (0..skeleton.joints.len())
        .filter(|&j| j != shoulders_of(&skeleton, &neck) && !head.contains(&j))
        .find(|&j| children.get(&j).copied().unwrap_or(0) >= HIP_MIN_CHILDREN)
        .ok_or("no hip joint")?;
    let hip_kids: Vec<usize> = (0..skeleton.joints.len())
        .filter(|&j| skeleton.joints[j].parent == Some(hips))
        .collect();
    // World frame of the head: forward is the body's own forward (hips to
    // shoulders, level), pitched down by HEAD_DROOP; up is against world y
    // (y is down); side points toward the stored (non-mirror) half.
    let hp = short_pose[head_joint];
    let side_sign = -hp.r[2][2].signum();
    let body_fwd = sub(short_pose[shoulders_of(&skeleton, &neck)].t, short_pose[hips].t);
    let level = normalize([body_fwd[0], 0.0, 0.0]);
    let droop = HEAD_DROOP.to_radians();
    let fwd = normalize(add(scale(level, droop.cos()), [0.0, droop.sin(), 0.0]));
    let side = [0.0, 0.0, side_sign];
    let up = normalize(cross(side, fwd));
    let up = if up[1] < 0.0 { up } else { scale(up, -1.0) };
    let mid_z = hp.t[2];
    let head_back = HEAD_BACK * head_half;
    let head_at = |x: f32, lift: f32| -> V3 {
        let p = add(add(hp.t, scale(fwd, x)), scale(up, lift * head_half));
        [p[0], p[1], mid_z]
    };

    let mut parts: Vec<Part> = Vec::new();

    // Head: lofted half-ellipses, top (theta 0) to chin (theta pi) through the side.
    let length = HEAD_LENGTH * head_half;
    let sections: Vec<(V3, V3, V3)> = HEAD_PROFILE
        .iter()
        .map(|&(t, ry, rz, lift)| {
            let c = head_at(head_back + t * length, lift);
            let c = [c[0], c[1], mid_z];
            (c, scale(up, ry * head_half), scale(side, rz * head_half))
        })
        .collect();
    let nose_tip = {
        let c = head_at(head_back + HEAD_NOSE_T * length, HEAD_PROFILE[HEAD_PROFILE.len() - 1].3);
        [c[0], c[1], mid_z]
    };
    let muzzle_from = head_back + MUZZLE_FROM * length;
    let mut head_part = loft(&sections, 0.0, PI, HEAD_SEGMENTS, true, Some(nose_tip));
    head_part.joint = head_joint;
    head_part.color = head_part
        .tris
        .iter()
        .map(|t| {
            let c = centroid3(&head_part.pos, t);
            let local_x = dot(sub(c, hp.t), fwd);
            if local_x >= muzzle_from {
                Quad::Pink
            } else if hide(c) > HEAD_SPOT_THRESHOLD {
                Quad::Black
            } else {
                Quad::White
            }
        })
        .collect();
    parts.push(head_part);

    // Horns, ears and eyes sit on the stored side; the mirror makes the pair.
    let on_skull = |t: f32, theta: f32| -> (V3, V3) {
        let i = HEAD_PROFILE.partition_point(|s| s.0 <= t).clamp(1, HEAD_PROFILE.len() - 1);
        let (a, b) = (HEAD_PROFILE[i - 1], HEAD_PROFILE[i]);
        let k = ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
        let lerp = |x: f32, y: f32| x + (y - x) * k;
        let c = head_at(head_back + t * length, lerp(a.3, b.3));
        let c = [c[0], c[1], mid_z];
        let dir = add(scale(up, theta.cos()), scale(side, theta.sin()));
        let p = add(
            c,
            add(
                scale(up, lerp(a.1, b.1) * head_half * theta.cos()),
                scale(side, lerp(a.2, b.2) * head_half * theta.sin()),
            ),
        );
        (p, normalize(dir))
    };
    let (horn_base, _) = on_skull(HORN_T, HORN_THETA);
    let horn_dir = normalize(add(add(scale(side, HORN_OUT), scale(up, HORN_UP)), scale(fwd, HORN_FWD)));
    let mut horn = tube(
        horn_base,
        horn_dir,
        up,
        HORN_LENGTH * head_half,
        |s| HORN_RADIUS * head_half * (1.0 - s).max(0.0).powf(0.8),
        HORN_CURL * head_half,
        up,
    );
    horn.joint = head_joint;
    horn.color = vec![Quad::Cream; horn.tris.len()];
    parts.push(horn);

    let (ear_base, _) = on_skull(EAR_T, EAR_THETA);
    let ear_dir = normalize(add(add(scale(side, EAR_OUT), scale(up, -EAR_DOWN)), scale(fwd, -EAR_BACK)));
    let mut ear = tube(
        ear_base,
        ear_dir,
        up,
        EAR_LENGTH * head_half,
        |s| EAR_WIDTH * head_half * (PI * (0.15 + 0.85 * s)).sin().max(0.15),
        0.0,
        fwd,
    );
    flatten(&mut ear, ear_base, up, EAR_FLATTEN);
    ear.joint = head_joint;
    ear.color = vec![Quad::Black; ear.tris.len()];
    parts.push(ear);

    let (eye_at, eye_out) = on_skull(EYE_T, EYE_THETA);
    let mut eye = sphere(add(eye_at, scale(eye_out, -0.3 * EYE_RADIUS * head_half)), EYE_RADIUS * head_half, up, side);
    eye.joint = head_joint;
    eye.color = vec![Quad::Black; eye.tris.len()];
    parts.push(eye);

    // Udder: hangs from the hips (the branching joint that carries the hind
    // legs), just ahead of the hind legs, top tucked into the belly.
    let legs: Vec<V3> = hip_kids
        .iter()
        .map(|&j| short_pose[j].t)
        .filter(|t| (t[2] - mid_z).abs() > LEG_OFF_MIDLINE)
        .collect();
    if legs.is_empty() {
        return Err("hips have no off-midline children (hind legs)".into());
    }
    let leg_x = legs.iter().map(|t| t[0]).sum::<f32>() / legs.len() as f32;
    let shoulder_x = short_pose[shoulders_of(&skeleton, &neck)].t[0];
    let udder_x = leg_x + UDDER_AHEAD * (shoulder_x - leg_x);
    let belly_y = (0..world.len())
        .filter(|&v| (world[v][2] - mid_z).abs() < UDDER_PROBE && (world[v][0] - udder_x).abs() < UDDER_PROBE)
        .filter(|&v| !skeleton_descends(&skeleton, dominant[v], &hip_kids))
        .map(|v| world[v][1])
        .fold(f32::MIN, f32::max);
    if belly_y == f32::MIN {
        return Err("no belly vertices above the udder".into());
    }
    let body_len = (shoulder_x - leg_x).abs();
    let (urx, ury, urz) = (UDDER_RX * body_len, UDDER_RY * body_len, UDDER_RZ * body_len);
    let down = [0.0, 1.0, 0.0];
    let udder_c = [udder_x, belly_y + ury * UDDER_DROP, mid_z];
    let ax = [1.0, 0.0, 0.0];
    let udder_sections: Vec<(V3, V3, V3)> = (0..=UDDER_RINGS)
        .map(|i| {
            let a = PI * i as f32 / UDDER_RINGS as f32;
            let c = add(udder_c, scale(ax, -urx * a.cos()));
            let k = a.sin().max(0.05);
            (c, scale(down, ury * k), scale(side, urz * k))
        })
        .collect();
    let mut udder = loft(&udder_sections, 0.0, PI, UDDER_SEGMENTS, true, None);
    udder.joint = hips;
    udder.color = vec![Quad::Pink; udder.tris.len()];
    parts.push(udder);
    for &dx in &TEAT_X {
        let base = [udder_c[0] + dx * urx, udder_c[1] + ury * TEAT_BASE, mid_z + side_sign * TEAT_Z * urz];
        let mut teat = tube(base, down, ax, TEAT_LENGTH * body_len, |s| TEAT_RADIUS * body_len * (1.0 - 0.3 * s), 0.0, ax);
        teat.joint = hips;
        teat.color = vec![Quad::Pink; teat.tris.len()];
        parts.push(teat);
    }

    // Vertices: all single-jointed, inserted after the existing singles.
    let mut new_singles: Vec<SingleVertex> = Vec::new();
    let mut new_refs: Vec<[u16; 2]> = Vec::new();
    let mut draws: Vec<[TexturedCorner; 3]> = Vec::new();
    for part in &parts {
        let jp = short_pose[part.joint];
        let base = singles + new_singles.len();
        let mut normals = vec![[0.0f32; 3]; part.pos.len()];
        let mut tris = part.tris.clone();
        for t in tris.iter_mut() {
            let p = t.map(|i| part.pos[i]);
            let inner = scale(add(add(part.inner[t[0]], part.inner[t[1]]), part.inner[t[2]]), 1.0 / 3.0);
            let fc = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            let out = sub(centroid3(&part.pos, t), inner);
            // Retail front faces have cross(b-a, c-a) pointing inward.
            if dot(fc, out) > 0.0 {
                t.swap(1, 2);
            }
            let outward = scale(cross(sub(part.pos[t[1]], part.pos[t[0]]), sub(part.pos[t[2]], part.pos[t[0]])), -1.0);
            for &i in t.iter() {
                normals[i] = add(normals[i], outward);
            }
        }
        let jref = ref_for(part.joint)?;
        for (i, p) in part.pos.iter().enumerate() {
            let n = if dot(normals[i], normals[i]) > 0.0 { normalize(normals[i]) } else { normalize(sub(*p, part.inner[i])) };
            new_singles.push(SingleVertex {
                position: mat_t_vec(&jp.r, sub(*p, jp.t)),
                normal: mat_t_vec(&jp.r, n),
            });
            new_refs.push([jref, 0]);
        }
        for (t, q) in tris.iter().zip(&part.color) {
            let uv = q.uv();
            draws.push(t.map(|i| TexturedCorner { index: (base + i) as u16, uv }));
        }
    }
    let added = new_singles.len();
    let shift = |i: u16| if (i as usize) >= singles { i + added as u16 } else { i };
    image.instructions.iter_mut().for_each(|ins| ins.map_indices(shift));
    image.instructions.push(Instruction::Texture(*ATLAS_NAME));
    for batch in draws.chunks(MAX_TRIS_PER_DRAW) {
        image.instructions.push(Instruction::TriMesh(batch.to_vec()));
    }
    let tail: Vec<SingleVertex> = image.singles.split_off(singles);
    image.singles.extend(new_singles);
    image.singles.extend(tail);
    let tail_refs = image.joint_refs.split_off(singles);
    image.joint_refs.extend(new_refs);
    image.joint_refs.extend(tail_refs);
    let new_mesh = image.encode().map_err(|e| e.to_string())?;

    let template = all
        .iter()
        .find(|c| c.kind == ChunkKind::Img as u8)
        .ok_or("no texture chunk to template from")?;
    let atlas_solid = texture_write::solid_dxt3(template.data, *ATLAS_NAME, ATLAS_SIZE, ATLAS_SIZE, [255; 3])
        .map_err(|e| e.to_string())?;
    let half = (ATLAS_SIZE / 2) as usize;
    let mut atlas_rgba = vec![255u8; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];
    for y in 0..ATLAS_SIZE as usize {
        for x in 0..ATLAS_SIZE as usize {
            let q = Quad::at(x / half, y / half);
            let o = (y * ATLAS_SIZE as usize + x) * 4;
            atlas_rgba[o..o + 3].copy_from_slice(&q.rgb());
        }
    }
    let atlas = texture_write::recolor_dxt3(&atlas_solid, &atlas_rgba).map_err(|e| e.to_string())?;

    // ---- Stage 3: attacks become a lift, then a nudge or smack ----
    let mut new_anims: HashMap<usize, Vec<u8>> = HashMap::new();
    let mut chain: Vec<usize> = neck.iter().copied().chain([head_joint]).collect();
    chain.sort();
    // Which way round the bend axis lifts the head: rotate about the lowest
    // neck joint and see whether the head goes up (y is down).
    let lift_sign = {
        let j = chain[0];
        let parent = skeleton.joints[j].parent.ok_or("neck base has no parent")?;
        let w = mat_vec(&short_pose[parent].r, [0.0, 0.0, 1.0]);
        let d = sub(short_pose[head_joint].t, short_pose[j].t);
        if -cross(w, d)[1] >= 0.0 { 1.0 } else { -1.0 }
    };
    for c in all.iter().filter(|c| c.kind == ChunkKind::AnimMo2 as u8) {
        let Some(style) = ATTACKS.iter().find(|(n, _)| n == &c.name).map(|(_, s)| *s) else {
            continue;
        };
        let anim = skel_anim::parse(DatId::from_name(&c.name), c.data);
        let frames = anim.num_frames;
        // Hit frame: where the original head swings furthest from its start.
        let head_world = |f: usize| -> [f32; 4] {
            let mut q = [0.0, 0.0, 0.0, 1.0];
            let mut path = vec![head_joint];
            while let Some(p) = skeleton.joints[*path.last().expect("non-empty")].parent {
                path.push(p);
            }
            for &j in path.iter().rev() {
                let bind = skeleton.joints[j].rotation;
                let a = anim.key_frame_sets.get(&(j as u32)).map_or([0.0, 0.0, 0.0, 1.0], |k| k[f].rotation);
                q = quat_mul(q, quat_mul(quat_normalize(a), bind));
            }
            q
        };
        let q0 = head_world(0);
        let hit = (0..frames)
            .max_by(|&a, &b| quat_angle(head_world(a), q0).total_cmp(&quat_angle(head_world(b), q0)))
            .unwrap_or(frames / 2)
            .max(2);
        let mut body = c.data.to_vec();
        for &j in &chain {
            let Some(keys) = anim.key_frame_sets.get(&(j as u32)) else { continue };
            let share = if j == head_joint { HEAD_SHARE } else { (1.0 - HEAD_SHARE) / neck_moving(&chain, head_joint) };
            let (start, end) = (quat_normalize(keys[0].rotation), quat_normalize(keys[frames - 1].rotation));
            let new: Vec<[f32; 4]> = (0..frames)
                .map(|f| {
                    let base = quat_nlerp(start, end, f as f32 / (frames - 1).max(1) as f32);
                    let (pitch, yaw) = style.curve(f as f32, hit as f32, frames as f32);
                    let r = quat_mul(
                        quat_axis_angle([0.0, 0.0, lift_sign], (pitch * share).to_radians()),
                        quat_axis_angle([0.0, 1.0, 0.0], (yaw * share).to_radians()),
                    );
                    quat_mul(r, base)
                })
                .collect();
            skel_anim_write::set_rotation_frames(&mut body, j as u32, &new).map_err(|e| format!("{}: {e}", c.name_str()))?;
        }
        println!("anim {}: {frames} frames, hit at {hit}, {:?}", c.name_str(), style);
        new_anims.insert(c.offset, body);
    }

    let last_texture = all
        .iter()
        .rposition(|c| c.kind == ChunkKind::Img as u8)
        .expect("a texture chunk was found above");
    let mut out = Vec::with_capacity(input.len() + new_mesh.len());
    for (i, c) in all.iter().enumerate() {
        if c.offset == bone.offset {
            out.extend(chunk::reencode(input, c, &new_bone));
        } else if c.offset == mesh_chunk.offset {
            out.extend(chunk::reencode(input, c, &new_mesh));
        } else if let Some(body) = new_textures.get(&c.offset).or(new_anims.get(&c.offset)) {
            out.extend(chunk::reencode(input, c, body));
        } else {
            out.extend_from_slice(&input[c.offset..chunk_end(c)]);
        }
        if i == last_texture {
            out.extend(chunk::encode(*ATLAS_CHUNK, ChunkKind::Img as u8, &atlas));
        }
    }
    out.extend_from_slice(&input[all.last().map_or(0, chunk_end)..]);

    let mut short: Vec<usize> = shortened.clone();
    short.sort();
    println!("neck joints {short:?} shortened to {NECK_SCALE} of their length");
    println!(
        "head joint {head_joint}: removed {removed} triangles; added {added} vertices / {} triangles (head, horn, ear, eye, udder on hips {hips}, {} teats per side)",
        draws.len(),
        TEAT_X.len()
    );
    verify(&out)?;
    Ok(out)
}

fn shoulders_of(skeleton: &Skeleton, neck: &HashSet<usize>) -> usize {
    let lowest = *neck.iter().min().expect("neck has joints");
    skeleton.joints[lowest].parent.expect("neck base has a parent")
}

fn neck_moving(chain: &[usize], head_joint: usize) -> f32 {
    chain.iter().filter(|&&j| j != head_joint).count().max(1) as f32
}

fn skeleton_descends(skeleton: &Skeleton, mut j: usize, roots: &[usize]) -> bool {
    loop {
        if roots.contains(&j) {
            return true;
        }
        match skeleton.joints[j].parent {
            Some(p) => j = p,
            None => return false,
        }
    }
}

fn verify(out: &[u8]) -> Result<(), String> {
    let all: Vec<Chunk> = chunk::walk(out).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    if all.last().map(chunk_end) != Some(out.len()) {
        return Err("output does not walk to its end".into());
    }
    for c in &all {
        match c.kind {
            k if k == ChunkKind::VertexOs2 as u8 => {
                SkelMeshImage::parse(c.data).map_err(|e| format!("mesh re-read: {e}"))?;
                let m = ffxi_modder::skel_mesh::parse(DatId::from_name(&c.name), c.data);
                println!("verify: mesh {} buffers", m.meshes.len());
            }
            k if k == ChunkKind::Img as u8 => {
                texture::decode_texture(c.data).map_err(|e| format!("texture re-read: {e}"))?;
            }
            k if k == ChunkKind::AnimMo2 as u8 => {
                let a = skel_anim::parse(DatId::from_name(&c.name), c.data);
                if a.key_frame_sets.values().flatten().any(|k| k.rotation.iter().any(|v| !v.is_finite())) {
                    return Err(format!("anim {} has a non-finite key", c.name_str()));
                }
            }
            _ => {}
        }
    }
    println!("verify: {} chunks walk and re-read", all.len());
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Quad {
    White,
    Black,
    Pink,
    Cream,
}

impl Quad {
    fn at(x: usize, y: usize) -> Quad {
        match (x, y) {
            (0, 0) => Quad::White,
            (1, 0) => Quad::Black,
            (0, _) => Quad::Pink,
            _ => Quad::Cream,
        }
    }
    fn cell(self) -> (f32, f32) {
        match self {
            Quad::White => (0.0, 0.0),
            Quad::Black => (1.0, 0.0),
            Quad::Pink => (0.0, 1.0),
            Quad::Cream => (1.0, 1.0),
        }
    }
    fn uv(self) -> [f32; 2] {
        let (x, y) = self.cell();
        [(x + 0.5) / 2.0, (y + 0.5) / 2.0]
    }
    fn rgb(self) -> [u8; 3] {
        match self {
            Quad::White => [242, 240, 234],
            Quad::Black => [24, 22, 22],
            Quad::Pink => [238, 150, 170],
            Quad::Cream => [226, 214, 180],
        }
    }
}

struct Part {
    pos: Vec<V3>,
    inner: Vec<V3>,
    tris: Vec<[usize; 3]>,
    color: Vec<Quad>,
    joint: usize,
}

/// Rings of `segments + 1` points, `c + a cos(theta) + b sin(theta)` for theta
/// in [t0, t1], joined band to band; optional caps at both ends.
fn loft(sections: &[(V3, V3, V3)], t0: f32, t1: f32, segments: usize, cap_start: bool, tip: Option<V3>) -> Part {
    let mut pos = Vec::new();
    let mut inner = Vec::new();
    let mut tris = Vec::new();
    let mut rings = Vec::new();
    for &(c, a, b) in sections {
        rings.push(pos.len());
        for s in 0..=segments {
            let th = t0 + (t1 - t0) * s as f32 / segments as f32;
            pos.push(add(c, add(scale(a, th.cos()), scale(b, th.sin()))));
            inner.push(c);
        }
    }
    for w in rings.windows(2) {
        for s in 0..segments {
            let (p, q, r, t) = (w[0] + s, w[0] + s + 1, w[1] + s, w[1] + s + 1);
            tris.push([p, r, q]);
            tris.push([q, r, t]);
        }
    }
    if cap_start {
        let c = sections[0].0;
        let back = normalize(sub(sections[0].0, sections[1].0));
        let ci = pos.len();
        pos.push(c);
        inner.push(sub(c, scale(back, 1.0)));
        for s in 0..segments {
            tris.push([ci, rings[0] + s, rings[0] + s + 1]);
        }
    }
    let last = *rings.last().expect("sections");
    let end = sections[sections.len() - 1].0;
    let ti = pos.len();
    pos.push(tip.unwrap_or(end));
    let prev = sections[sections.len().saturating_sub(2)].0;
    inner.push(sub(end, normalize(sub(end, prev))));
    for s in 0..segments {
        tris.push([ti, last + s, last + s + 1]);
    }
    Part { pos, inner, tris, color: Vec::new(), joint: 0 }
}

/// A closed tube from `base` along `dir` (bending `curl` toward `bend` at the
/// tip), radius `r(s)` for s in [0, 1].
fn tube(base: V3, dir: V3, hint: V3, length: f32, r: impl Fn(f32) -> f32, curl: f32, bend: V3) -> Part {
    let a_axis = normalize(cross(dir, hint));
    let b_axis = normalize(cross(dir, a_axis));
    let sections: Vec<(V3, V3, V3)> = (0..=TUBE_RINGS)
        .map(|i| {
            let s = i as f32 / TUBE_RINGS as f32;
            let c = add(add(base, scale(dir, s * length)), scale(bend, curl * s * s));
            let rad = r(s).max(1e-4);
            (c, scale(a_axis, rad), scale(b_axis, rad))
        })
        .collect();
    let tip = add(add(base, scale(dir, length * 1.04)), scale(bend, curl));
    loft(&sections, 0.0, 2.0 * PI, TUBE_SEGMENTS, true, Some(tip))
}

fn sphere(c: V3, r: f32, a: V3, b: V3) -> Part {
    let n = normalize(cross(a, b));
    let sections: Vec<(V3, V3, V3)> = (1..SPHERE_RINGS)
        .map(|i| {
            let phi = PI * i as f32 / SPHERE_RINGS as f32;
            (add(c, scale(n, -r * phi.cos())), scale(a, r * phi.sin()), scale(b, r * phi.sin()))
        })
        .collect();
    let mut p = loft(&sections, 0.0, 2.0 * PI, TUBE_SEGMENTS, true, Some(add(c, scale(n, r))));
    for v in p.inner.iter_mut() {
        *v = c;
    }
    p
}

fn flatten(p: &mut Part, origin: V3, axis: V3, keep: f32) {
    for v in p.pos.iter_mut().chain(p.inner.iter_mut()) {
        let d = dot(sub(*v, origin), axis);
        *v = sub(*v, scale(axis, d * (1.0 - keep)));
    }
}

fn centroid3(pos: &[V3], t: &[usize; 3]) -> V3 {
    scale(add(add(pos[t[0]], pos[t[1]]), pos[t[2]]), 1.0 / 3.0)
}

#[derive(Clone, Copy, Debug)]
enum Attack {
    /// Lift the head, then bring it down hard past rest.
    Smack,
    /// Lift, then swing sideways into the target.
    Nudge,
    /// Two quick short smacks.
    Double,
}

impl Attack {
    /// (pitch, yaw) degrees at frame `f`, peaking on `hit`.
    fn curve(self, f: f32, hit: f32, frames: f32) -> (f32, f32) {
        let ease = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let lift_at = hit * LIFT_AT;
        let settle = || 1.0 - ease((f - hit) / (frames - 1.0 - hit).max(1.0));
        match self {
            Attack::Smack | Attack::Nudge => {
                let swing = if f <= lift_at {
                    LIFT_DEG * ease(f / lift_at.max(1.0))
                } else if f <= hit {
                    let k = ease((f - lift_at) / (hit - lift_at).max(1.0));
                    LIFT_DEG + (-SMACK_DEG - LIFT_DEG) * k
                } else {
                    -SMACK_DEG * settle()
                };
                match self {
                    Attack::Smack => (swing, 0.0),
                    _ => {
                        let yaw = if f <= lift_at { 0.0 } else if f <= hit { NUDGE_DEG * ease((f - lift_at) / (hit - lift_at).max(1.0)) } else { NUDGE_DEG * settle() };
                        (swing.max(0.0) * 0.6, yaw)
                    }
                }
            }
            Attack::Double => {
                let first = hit * 0.5;
                let bump = |c: f32, w: f32| (1.0 - ((f - c) / w).abs()).max(0.0);
                let lift = LIFT_DEG * 0.6 * (bump(first * 0.5, first * 0.5) + bump((first + hit) * 0.5, (hit - first) * 0.5));
                let smack = SMACK_DEG * 0.8 * (bump(first, first * 0.35) + bump(hit, (frames - hit).max(2.0) * 0.6));
                (lift - smack, 0.0)
            }
        }
    }
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n > f32::EPSILON { q.map(|v| v / n) } else { [0.0, 0.0, 0.0, 1.0] }
}

fn quat_axis_angle(axis: V3, angle: f32) -> [f32; 4] {
    let a = normalize(axis);
    let (s, c) = (angle * 0.5).sin_cos();
    [a[0] * s, a[1] * s, a[2] * s, c]
}

fn quat_nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let b = if d < 0.0 { b.map(|v| -v) } else { b };
    quat_normalize([0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t))
}

fn quat_angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]).abs().min(1.0);
    2.0 * d.acos()
}

fn mat_t_vec(a: &M3, v: V3) -> V3 {
    [0, 1, 2].map(|i| a[0][i] * v[0] + a[1][i] * v[1] + a[2][i] * v[2])
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(v: V3) -> V3 {
    let l = dot(v, v).sqrt().max(f32::EPSILON);
    [v[0] / l, v[1] / l, v[2] / l]
}



/// The texture repainted: white or black by 3D noise at each texel's bind
/// position, scaled by the original brightness. Alpha is left as decoded; the
/// encoder keeps the original alpha blocks anyway.
fn paint(rgba: &[u8], w: usize, h: usize, tris: &[[(usize, [f32; 2]); 3]], world: &[V3]) -> Vec<u8> {
    let mut black: Vec<Option<bool>> = vec![None; w * h];
    for t in tris {
        let uv = t.map(|(_, uv)| [uv[0].rem_euclid(1.0) * w as f32, uv[1].rem_euclid(1.0) * h as f32]);
        let p = t.map(|(v, _)| world[v]);
        let x0 = uv.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let x1 = (uv.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil() as usize).min(w);
        let y0 = uv.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let y1 = (uv.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil() as usize).min(h);
        let area = (uv[1][0] - uv[0][0]) * (uv[2][1] - uv[0][1]) - (uv[2][0] - uv[0][0]) * (uv[1][1] - uv[0][1]);
        if area.abs() < f32::EPSILON {
            continue;
        }
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w1 = ((px - uv[0][0]) * (uv[2][1] - uv[0][1]) - (uv[2][0] - uv[0][0]) * (py - uv[0][1])) / area;
                let w2 = ((uv[1][0] - uv[0][0]) * (py - uv[0][1]) - (px - uv[0][0]) * (uv[1][1] - uv[0][1])) / area;
                let w0 = 1.0 - w1 - w2;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let q = add(add(scale(p[0], w0), scale(p[1], w1)), scale(p[2], w2));
                black[y * w + x] = Some(hide(q) > SPOT_THRESHOLD);
            }
        }
    }
    for _ in 0..DILATE_PASSES {
        let prev = black.clone();
        for y in 0..h {
            for x in 0..w {
                if prev[y * w + x].is_some() {
                    continue;
                }
                let n = [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)]
                    .into_iter()
                    .filter(|&(nx, ny)| nx < w && ny < h)
                    .find_map(|(nx, ny)| prev[ny * w + nx]);
                black[y * w + x] = n;
            }
        }
    }

    let lum = |i: usize| 0.299 * rgba[i * 4] as f32 + 0.587 * rgba[i * 4 + 1] as f32 + 0.114 * rgba[i * 4 + 2] as f32;
    let covered: Vec<usize> = (0..w * h).filter(|&i| black[i].is_some()).collect();
    let mean = covered.iter().map(|&i| lum(i)).sum::<f32>() / covered.len().max(1) as f32;
    let mut out = rgba.to_vec();
    for i in 0..w * h {
        let shade = (lum(i) / mean.max(1.0)).clamp(SHADE_MIN, SHADE_MAX);
        let base = if black[i] == Some(true) { BLACK } else { WHITE };
        for c in 0..3 {
            out[i * 4 + c] = (base[c] * shade).clamp(0.0, 255.0) as u8;
        }
    }
    out
}

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ (z as u32).wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn value_noise(p: V3) -> f32 {
    let f = p.map(f32::floor);
    let i = f.map(|v| v as i32);
    let t = [0, 1, 2].map(|a| {
        let x = p[a] - f[a];
        x * x * (3.0 - 2.0 * x)
    });
    let mut acc = 0.0;
    for corner in 0..8 {
        let o = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
        let wgt: f32 = (0..3).map(|a| if o[a] == 1 { t[a] } else { 1.0 - t[a] }).product();
        acc += wgt * hash(i[0] + o[0], i[1] + o[1], i[2] + o[2]);
    }
    acc
}

fn hide(p: V3) -> f32 {
    let base = value_noise(scale(p, SPOT_FREQUENCY));
    let detail = value_noise(scale(p, 2.0 * SPOT_FREQUENCY));
    (1.0 - DETAIL_WEIGHT) * base + DETAIL_WEIGHT * detail
}

fn quat_matrix(q: [f32; 4]) -> M3 {
    let [x, y, z, w] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)],
        [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)],
        [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

fn bind_pose(s: &Skeleton) -> Result<Vec<Pose>, String> {
    let mut out: Vec<Pose> = Vec::with_capacity(s.joints.len());
    for (i, j) in s.joints.iter().enumerate() {
        let local = Pose { r: quat_matrix(j.rotation), t: j.translation };
        out.push(match j.parent {
            None => local,
            Some(p) if p < i => Pose {
                r: mat_mul(&out[p].r, &local.r),
                t: add(mat_vec(&out[p].r, local.t), out[p].t),
            },
            Some(p) => return Err(format!("joint {i} has later parent {p}")),
        });
    }
    Ok(out)
}

fn mat_mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

fn mat_vec(a: &M3, v: V3) -> V3 {
    [0, 1, 2].map(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(v: V3, s: f32) -> V3 {
    [v[0] * s, v[1] * s, v[2] * s]
}
