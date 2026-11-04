//! Client-side joke mod: replace a monster's neck and head with one
//! procedural purple shaft. It rises from the shoulder seam along the neck's
//! own centerline, blended across the neck bones so it bends like the neck,
//! and ends past the head on the head joint so it nods like the head.
//!
//! usage: ffxi_modder <in.DAT> <out.DAT>     write the modded DAT
//!        ffxi_modder <in.DAT> --check       prove the writer round trips
//!
//! The head joint is the skeleton's look-at reference (skel::standard_position::NECK,
//! the bone retail bends to aim the face); the head is that joint plus its
//! descendants. The neck is the parent chain from the head down to the first
//! joint that branches (the shoulders, which also carry the front legs).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::process::ExitCode;

use ffxi_modder::chunk::{self, Chunk};
use ffxi_modder::datid::DatId;
use ffxi_modder::kind::ChunkKind;
use ffxi_modder::skel::{self, standard_position, Skeleton};
use ffxi_modder::skel_mesh_write::{
    pack_joint_ref, DoubleVertex, Instruction, SingleVertex, SkelMeshImage, TexturedCorner,
    MAX_TRIS_PER_DRAW, TEXTURE_NAME_BYTES,
};
use ffxi_modder::texture_write;

const NEW_TEXTURE_NAME: &[u8; TEXTURE_NAME_BYTES] = b"tim     purple  ";
const NEW_TEXTURE_CHUNK: &[u8; 4] = b"purp";
const NEW_TEXTURE_SIZE: u32 = 16;
const PURPLE: [u8; 3] = [0x9A, 0x2B, 0xD8];
const SOLID_UV: [f32; 2] = [0.5, 0.5];

const FLAG3_USE_JOINT_ARRAY: u8 = 0x80;
const OFF_FLAGS3: usize = 0x02;
const DOMINANT_WEIGHT: f32 = 0.5;
const SLOT_MASK: u16 = 0x7F;
const FLIPPED_SHIFT: u16 = 7;
const AXIS_SHIFT: u16 = 14;
const MIRROR_AXIS_Z: u16 = 3;

/// Half-ring segments; the symmetric mesh mirrors the other half.
const HALF_RING_SEGMENTS: usize = 10;
/// Tip distance past the head's back edge, in head lengths, along the head axis.
const TIP_PER_HEAD_LENGTH: f32 = 2.2;
/// Shaft radius over the removed neck's mean radius.
const SHAFT_RADIUS_SCALE: f32 = 1.15;
/// Base ring radius over the shoulder seam's mean radius, so it overlaps the hole.
const SEAM_OVERLAP: f32 = 1.1;
/// Length of the base flare (seam radius down to shaft radius), in seam radii.
const FLARE_PER_SEAM_RADIUS: f32 = 3.0;
const FLARE_RINGS: usize = 8;
/// Ring spacing along the shaft, in shaft radii.
const SHAFT_STEP_PER_RADIUS: f32 = 0.8;
/// Glans length in shaft radii, and its (fraction from glans start, radius
/// multiple) profile; the apex closes it at fraction 1.
const GLANS_PER_RADIUS: f32 = 1.8;
const GLANS: &[(f32, f32)] = &[
    (0.00, 1.02),
    (0.22, 1.10),
    (0.37, 1.28),
    (0.56, 1.22),
    (0.78, 0.95),
    (0.93, 0.55),
    (0.98, 0.22),
];
const CURVE_SAMPLES: usize = 512;
/// Bezier handle length as a fraction of the base-to-tip chord.
const BEZIER_HANDLE: f32 = 0.35;
const BLEND_EPSILON: f32 = 1e-3;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} <in.DAT> <out.DAT | --check>", args[0]);
        return ExitCode::from(2);
    }
    let input = fs::read(&args[1]).expect("read input DAT");
    let result = if args[2] == "--check" {
        round_trip(&input).map(|n| println!("round trip: byte-identical ({n} bytes)"))
    } else {
        head_swap(&input).map(|out| {
            fs::write(&args[2], out).expect("write output DAT");
            println!("wrote {}", args[2]);
        })
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn chunks(bytes: &[u8]) -> Result<Vec<Chunk<'_>>, String> {
    chunk::walk(bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn chunk_end(c: &Chunk<'_>) -> usize {
    c.offset + chunk::CHUNK_HEADER_BYTES + c.data.len()
}

/// Every chunk copied, every skinned mesh re-encoded through the writer.
fn round_trip(input: &[u8]) -> Result<usize, String> {
    let all = chunks(input)?;
    let mut out = Vec::with_capacity(input.len());
    for c in &all {
        if c.kind == ChunkKind::VertexOs2 as u8 {
            let image =
                SkelMeshImage::parse(c.data).map_err(|e| format!("{}: {e}", c.name_str()))?;
            let body = image.encode().map_err(|e| e.to_string())?;
            out.extend(chunk::reencode(input, c, &body));
        } else {
            out.extend_from_slice(&input[c.offset..chunk_end(c)]);
        }
    }
    out.extend_from_slice(&input[all.last().map_or(0, chunk_end)..]);
    if out != input {
        let at = out
            .iter()
            .zip(input)
            .position(|(a, b)| a != b)
            .unwrap_or(out.len().min(input.len()));
        return Err(format!("round trip differs at byte {at:#x}"));
    }
    Ok(out.len())
}

type V3 = [f32; 3];
type M3 = [[f32; 3]; 3];

#[derive(Clone, Copy)]
struct Pose {
    r: M3,
    t: V3,
}

fn quat_matrix(q: [f32; 4]) -> M3 {
    let [x, y, z, w] = q;
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
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
    [dot(a[0], v), dot(a[1], v), dot(a[2], v)]
}

fn mat_t_vec(a: &M3, v: V3) -> V3 {
    [
        a[0][0] * v[0] + a[1][0] * v[1] + a[2][0] * v[2],
        a[0][1] * v[0] + a[1][1] * v[1] + a[2][1] * v[2],
        a[0][2] * v[0] + a[1][2] * v[1] + a[2][2] * v[2],
    ]
}

/// Bind-pose global transforms, parent first.
fn bind_pose(s: &Skeleton) -> Result<Vec<Pose>, String> {
    let mut out: Vec<Pose> = Vec::with_capacity(s.joints.len());
    for (i, j) in s.joints.iter().enumerate() {
        let local = Pose {
            r: quat_matrix(j.rotation),
            t: j.translation,
        };
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

#[derive(Clone, Copy)]
enum Binding {
    Single(usize),
    Double(usize, usize, f32),
}

struct Control {
    point: V3,
    joint: usize,
}

fn lerp_profile(table: &[(f32, f32)], x: f32) -> f32 {
    let i = table
        .partition_point(|&(k, _)| k <= x)
        .clamp(1, table.len() - 1);
    let ((x0, y0), (x1, y1)) = (table[i - 1], table[i]);
    y0 + (y1 - y0) * ((x - x0) / (x1 - x0)).clamp(0.0, 1.0)
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn point_segment_distance(p: V3, a: V3, b: V3) -> f32 {
    let ab = sub(b, a);
    let t = (dot(sub(p, a), ab) / dot(ab, ab).max(f32::EPSILON)).clamp(0.0, 1.0);
    let d = sub(p, add(a, scale(ab, t)));
    dot(d, d).sqrt()
}

fn mean(v: &[f32]) -> f32 {
    v.iter().sum::<f32>() / v.len().max(1) as f32
}

fn head_swap(input: &[u8]) -> Result<Vec<u8>, String> {
    let all = chunks(input)?;
    let bone = all
        .iter()
        .find(|c| c.kind == ChunkKind::Bone as u8)
        .ok_or("no skeleton chunk")?;
    let skeleton = skel::parse(DatId::from_name(&bone.name), bone.data);
    let pose = bind_pose(&skeleton)?;
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
    let mut neck: Vec<usize> = Vec::new();
    let mut cur = skeleton.joints[head_joint]
        .parent
        .ok_or("head joint is the root")?;
    while children.get(&cur).copied().unwrap_or(0) <= 1 {
        neck.push(cur);
        cur = skeleton.joints[cur]
            .parent
            .ok_or("neck reaches the root without branching")?;
    }
    let shoulders = cur;
    neck.reverse();
    let removed_joints: HashSet<usize> = head.iter().chain(&neck).copied().collect();

    let meshes: Vec<&Chunk> = all
        .iter()
        .filter(|c| c.kind == ChunkKind::VertexOs2 as u8)
        .collect();
    let [mesh_chunk] = meshes[..] else {
        return Err(format!(
            "expected one skinned mesh chunk, found {}",
            meshes.len()
        ));
    };
    let mut image = SkelMeshImage::parse(mesh_chunk.data).map_err(|e| e.to_string())?;
    let use_joint_array = image.header[OFF_FLAGS3] & FLAG3_USE_JOINT_ARRAY != 0;
    let joint_of = |r: u16| -> usize {
        let slot = (r & SLOT_MASK) as usize;
        if use_joint_array {
            image.joint_array[slot] as usize
        } else {
            slot
        }
    };
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

    let singles = image.singles.len();
    let count = image.joint_refs.len();
    let mut world = Vec::with_capacity(count);
    let mut dominant = Vec::with_capacity(count);
    for v in 0..count {
        let [r0, r1] = image.joint_refs[v];
        if v < singles {
            let j = joint_of(r0);
            world.push(add(
                mat_vec(&pose[j].r, image.singles[v].position),
                pose[j].t,
            ));
            dominant.push(j);
        } else {
            let d = &image.doubles[v - singles];
            let (j0, j1) = (joint_of(r0), joint_of(r1));
            let a = add(
                mat_vec(&pose[j0].r, d.position[0]),
                scale(pose[j0].t, d.weight[0]),
            );
            let b = add(
                mat_vec(&pose[j1].r, d.position[1]),
                scale(pose[j1].t, d.weight[1]),
            );
            world.push(add(a, b));
            dominant.push(if d.weight[1] > DOMINANT_WEIGHT {
                j1
            } else {
                j0
            });
        }
    }
    let is_removed: Vec<bool> = dominant
        .iter()
        .map(|j| removed_joints.contains(j))
        .collect();

    // Each joint's mirror ref as the mesh already stores it.
    let mut mirror_refs: HashMap<usize, BTreeMap<u16, usize>> = HashMap::new();
    for (v, refs) in image.joint_refs.iter().enumerate() {
        let used = if v < singles { &refs[..1] } else { &refs[..] };
        for &r in used {
            *mirror_refs
                .entry(joint_of(r))
                .or_default()
                .entry(r >> FLIPPED_SHIFT)
                .or_default() += 1;
        }
    }
    let ref_for = |joint: usize| -> Result<u16, String> {
        let (&upper, _) = mirror_refs
            .get(&joint)
            .and_then(|m| m.iter().max_by_key(|(_, n)| **n))
            .ok_or(format!(
                "joint {joint} has no vertex to copy a mirror ref from"
            ))?;
        let (flipped, axis) = (upper & SLOT_MASK, upper >> (AXIS_SHIFT - FLIPPED_SHIFT));
        if axis != MIRROR_AXIS_Z {
            return Err(format!(
                "joint {joint} mirrors on axis {axis}; the generator builds a z-half"
            ));
        }
        Ok(pack_joint_ref(slot_of(joint)?, flipped, axis))
    };

    // Drop every triangle wholly on the neck or head; the seam is what both sides share.
    let (mut seam_kept, mut seam_removed) = (HashSet::new(), HashSet::new());
    let mut removed = 0usize;
    let mut kept_ops = Vec::with_capacity(image.instructions.len());
    let mut current_texture: Option<[u8; TEXTURE_NAME_BYTES]> = None;
    let mut removed_by_texture: BTreeMap<Option<[u8; TEXTURE_NAME_BYTES]>, usize> = BTreeMap::new();
    for ins in image.instructions.drain(..) {
        if let Instruction::Texture(name) = &ins {
            current_texture = Some(*name);
        }
        let tris = ins.triangles();
        let gone = |t: &[u16; 3]| t.iter().all(|&i| is_removed[i as usize]);
        for t in &tris {
            if gone(t) {
                seam_removed.extend(*t)
            } else {
                seam_kept.extend(*t)
            }
        }
        let gone_count = tris.iter().filter(|t| gone(t)).count();
        if gone_count == 0 {
            kept_ops.push(ins);
            continue;
        }
        removed += gone_count;
        let on_neck = tris
            .iter()
            .filter(|t| t.iter().all(|&i| neck.contains(&dominant[i as usize])))
            .count();
        *removed_by_texture.entry(current_texture).or_default() += on_neck;
        match ins {
            Instruction::TriMesh(t) => {
                let left: Vec<_> = t
                    .into_iter()
                    .filter(|c| !c.iter().all(|c| is_removed[c.index as usize]))
                    .collect();
                if !left.is_empty() {
                    kept_ops.push(Instruction::TriMesh(left));
                }
            }
            Instruction::UntexturedTriMesh(t) => {
                let left: Vec<_> = t
                    .into_iter()
                    .filter(|(i, _)| !i.iter().all(|&v| is_removed[v as usize]))
                    .collect();
                if !left.is_empty() {
                    kept_ops.push(Instruction::UntexturedTriMesh(left));
                }
            }
            strip if gone_count == tris.len() => drop(strip),
            _ => return Err("a strip is only partly on the neck or head".into()),
        }
    }
    image.instructions = kept_ops;
    let seam: Vec<u16> = seam_kept.intersection(&seam_removed).copied().collect();
    if seam.is_empty() {
        return Err("no seam between the kept body and the removed neck".into());
    }

    // The neck's base is weighted to the shoulders, so the bone cut keeps it.
    // Its triangles carry the neck's texture, touch only shoulder or neck
    // joints and connect to the seam: they move to the new texture unchanged,
    // so the shaft meets the body on the model's own neck line.
    let neck_texture = removed_by_texture
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(t, _)| *t)
        .ok_or("nothing was removed")?;
    let band_joints: HashSet<usize> = neck.iter().copied().chain([shoulders]).collect();
    let mut candidates: Vec<(usize, usize, [u16; 3])> = Vec::new();
    let mut texture_now = None;
    for (oi, ins) in image.instructions.iter().enumerate() {
        match ins {
            Instruction::Texture(n) => texture_now = Some(*n),
            Instruction::TriMesh(t) if texture_now == neck_texture => {
                for (ti, c) in t.iter().enumerate() {
                    if c.iter()
                        .all(|c| band_joints.contains(&dominant[c.index as usize]))
                    {
                        candidates.push((oi, ti, [c[0].index, c[1].index, c[2].index]));
                    }
                }
            }
            _ => {}
        }
    }
    let mut region: HashSet<u16> = seam.iter().copied().collect();
    let mut band: HashSet<(usize, usize)> = HashSet::new();
    loop {
        let before = band.len();
        for &(oi, ti, t) in &candidates {
            if !band.contains(&(oi, ti)) && t.iter().any(|v| region.contains(v)) {
                band.insert((oi, ti));
                region.extend(t);
            }
        }
        if band.len() == before {
            break;
        }
    }
    let mut band_tris: Vec<[u16; 3]> = Vec::new();
    for (oi, ins) in image.instructions.iter_mut().enumerate() {
        if let Instruction::TriMesh(t) = ins {
            let mut ti = 0;
            t.retain(|c| {
                let keep = !band.contains(&(oi, ti));
                if !keep {
                    band_tris.push([c[0].index, c[1].index, c[2].index]);
                }
                ti += 1;
                keep
            });
        }
    }
    image
        .instructions
        .retain(|ins| !matches!(ins, Instruction::TriMesh(t) if t.is_empty()));

    // The mirror plane is z = the shoulders' z; the stored half is the side
    // where the local z of the chain joints is negative.
    let mid_z = pose[shoulders].t[2];
    let side = -pose[shoulders].r[2][2].signum();
    for &j in neck.iter().chain([&head_joint]) {
        if pose[j].r[2][2].signum() != pose[shoulders].r[2][2].signum()
            || (pose[j].t[2] - mid_z).abs() > BLEND_EPSILON
        {
            return Err(format!("joint {j} is off the mirror plane"));
        }
    }
    let on_plane = |p: V3| [p[0], p[1], mid_z];
    let centroid = |vs: &mut dyn Iterator<Item = usize>| -> Option<V3> {
        let (mut s, mut n) = ([0.0; 3], 0usize);
        for v in vs {
            s = add(s, world[v]);
            n += 1;
        }
        (n > 0).then(|| on_plane(scale(s, 1.0 / n as f32)))
    };

    // Head axis and length from the head's single-jointed vertices, in head space.
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
    let tip_local = [
        lo[0] + TIP_PER_HEAD_LENGTH * (hi[0] - lo[0]),
        (hi[1] + lo[1]) / 2.0,
        0.0,
    ];
    let tip = on_plane(add(
        mat_vec(&pose[head_joint].r, tip_local),
        pose[head_joint].t,
    ));

    let mut controls = vec![Control {
        point: centroid(&mut seam.iter().map(|&v| v as usize)).expect("seam is non-empty"),
        joint: shoulders,
    }];
    for &j in neck.iter().chain([&head_joint]) {
        if let Some(c) = centroid(&mut (0..count).filter(|&v| is_removed[v] && dominant[v] == j)) {
            controls.push(Control { point: c, joint: j });
        }
    }
    controls.push(Control {
        point: tip,
        joint: head_joint,
    });
    let pts: Vec<V3> = controls.iter().map(|c| c.point).collect();

    let seam_radius = mean(
        &seam
            .iter()
            .map(|&v| {
                let d = sub(world[v as usize], pts[0]);
                dot(d, d).sqrt()
            })
            .collect::<Vec<_>>(),
    );
    let neck_dist: Vec<f32> = (0..count)
        .filter(|&v| is_removed[v] && neck.contains(&dominant[v]))
        .map(|v| {
            pts.windows(2)
                .map(|w| point_segment_distance(world[v], w[0], w[1]))
                .fold(f32::MAX, f32::min)
        })
        .collect();
    if neck_dist.is_empty() {
        return Err("no neck vertices to size the shaft from".into());
    }
    let radius = SHAFT_RADIUS_SCALE * mean(&neck_dist);
    let base_radius = SEAM_OVERLAP * seam_radius;

    // One smooth cubic from the shoulder seam, leaving along the neck and
    // arriving along the head axis; dense samples carry the arc length.
    let head_axis = {
        let r = &pose[head_joint].r;
        normalize([r[0][0], r[1][0], 0.0])
    };
    let (b0, b3) = (pts[0], tip);
    let reach = dot(sub(b3, b0), sub(b3, b0)).sqrt() * BEZIER_HANDLE;
    let b1 = add(b0, scale(normalize(sub(pts[pts.len() / 2], b0)), reach));
    let b2 = sub(b3, scale(head_axis, reach));
    let mut dense: Vec<(V3, f32)> = (0..=CURVE_SAMPLES)
        .map(|i| {
            let f = i as f32 / CURVE_SAMPLES as f32;
            let g = 1.0 - f;
            let p = add(
                add(scale(b0, g * g * g), scale(b1, 3.0 * g * g * f)),
                add(scale(b2, 3.0 * g * f * f), scale(b3, f * f * f)),
            );
            (p, 0.0)
        })
        .collect();
    for i in 1..dense.len() {
        let d = sub(dense[i].0, dense[i - 1].0);
        dense[i].1 = dense[i - 1].1 + dot(d, d).sqrt();
    }
    let length = dense.last().expect("curve has samples").1;
    let flare_len = FLARE_PER_SEAM_RADIUS * seam_radius;
    let glans_len = GLANS_PER_RADIUS * radius;
    if flare_len + glans_len >= length {
        return Err(format!(
            "shaft too short ({length:.3}) for its flare and glans"
        ));
    }
    // The shaft welds onto the seam the neck used to sit on. That seam is
    // slanted, so the first ring starts past its far edge along the curve.
    let start_tangent = normalize(sub(dense[1].0, dense[0].0));
    let flare_start = seam
        .iter()
        .map(|&v| dot(sub(world[v as usize], pts[0]), start_tangent))
        .fold(0.0f32, f32::max);
    let flare_end = flare_start + flare_len;
    if flare_end + glans_len >= length {
        return Err(format!("shaft too short ({length:.3}) past the seam"));
    }
    let mut stations: Vec<f32> = (0..=FLARE_RINGS)
        .map(|i| flare_start + flare_len * i as f32 / FLARE_RINGS as f32)
        .collect();
    let shaft_end = length - glans_len;
    let shaft_steps = ((shaft_end - flare_end) / (SHAFT_STEP_PER_RADIUS * radius))
        .ceil()
        .max(1.0) as usize;
    stations.extend(
        (1..=shaft_steps)
            .map(|i| flare_end + (shaft_end - flare_end) * i as f32 / shaft_steps as f32),
    );
    stations.extend(
        GLANS
            .iter()
            .skip(1)
            .map(|&(u, _)| shaft_end + u * glans_len),
    );
    let radius_at = |s: f32| -> f32 {
        if s < flare_end {
            base_radius + (radius - base_radius) * smoothstep((s - flare_start) / flare_len)
        } else if s < shaft_end {
            radius
        } else {
            radius * lerp_profile(GLANS, (s - shaft_end) / glans_len)
        }
    };
    // A ring blends between the two neck joints whose centroids bracket it.
    let bind = |c: V3| -> Binding {
        let (mut best, mut seg, mut f) = (f32::MAX, 0usize, 0.0f32);
        for (k, w) in pts.windows(2).enumerate() {
            let ab = sub(w[1], w[0]);
            let t = (dot(sub(c, w[0]), ab) / dot(ab, ab).max(f32::EPSILON)).clamp(0.0, 1.0);
            let d = sub(c, add(w[0], scale(ab, t)));
            if dot(d, d) < best {
                (best, seg, f) = (dot(d, d), k, t);
            }
        }
        let (j0, j1) = (controls[seg].joint, controls[seg + 1].joint);
        if j0 == j1 || f > 1.0 - BLEND_EPSILON {
            Binding::Single(j1)
        } else if f < BLEND_EPSILON {
            Binding::Single(j0)
        } else {
            Binding::Double(j0, j1, f)
        }
    };
    let sample_at = |s: f32| -> (V3, V3, Binding) {
        let i = dense.partition_point(|d| d.1 < s).clamp(1, dense.len() - 1);
        let (a, b) = (&dense[i - 1], &dense[i]);
        let k = ((s - a.1) / (b.1 - a.1).max(f32::EPSILON)).clamp(0.0, 1.0);
        let p = add(a.0, scale(sub(b.0, a.0), k));
        (p, normalize(sub(b.0, a.0)), bind(p))
    };

    let z_hat = [0.0, 0.0, side];
    let mut new_verts: Vec<(V3, V3, Binding)> = Vec::new();
    let mut rings: Vec<usize> = Vec::new();
    for &s in &stations {
        let (c, t, binding) = sample_at(s);
        let r = radius_at(s);
        let ds =
            (radius_at(s + BLEND_EPSILON) - radius_at(s - BLEND_EPSILON)) / (2.0 * BLEND_EPSILON);
        let across = normalize(cross([0.0, 0.0, 1.0], t));
        rings.push(new_verts.len());
        for seg in 0..=HALF_RING_SEGMENTS {
            let th = std::f32::consts::PI * seg as f32 / HALF_RING_SEGMENTS as f32;
            let radial = add(scale(across, th.cos()), scale(z_hat, th.sin().max(0.0)));
            new_verts.push((
                add(c, scale(radial, r)),
                normalize(sub(radial, scale(t, ds))),
                binding,
            ));
        }
    }
    let apex = new_verts.len();
    let (_, end_tangent, _) = sample_at(length - BLEND_EPSILON);
    new_verts.push((tip, end_tangent, Binding::Single(head_joint)));

    // Corners index new vertices below `fresh`, existing vertices at
    // `fresh + original index`.
    let fresh = new_verts.len();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    for w in rings.windows(2) {
        for s in 0..HALF_RING_SEGMENTS {
            let (a, b, c, d) = (w[0] + s, w[0] + s + 1, w[1] + s, w[1] + s + 1);
            tris.push([a, c, b]);
            tris.push([b, c, d]);
        }
    }
    let last = *rings.last().expect("stations make rings");
    for s in 0..HALF_RING_SEGMENTS {
        tris.push([apex, last + s, last + s + 1]);
    }

    // Zip the seam (ordered by angle around the curve start) to the first
    // ring, always advancing the side whose next vertex comes first.
    let across0 = normalize(cross([0.0, 0.0, 1.0], start_tangent));
    let angle = |p: V3| -> f32 {
        let d = sub(p, pts[0]);
        let d = sub(d, scale(start_tangent, dot(d, start_tangent)));
        dot(d, z_hat).max(0.0).atan2(dot(d, across0))
    };
    let mut seam_order: Vec<(f32, usize)> = seam
        .iter()
        .map(|&v| (angle(world[v as usize]), fresh + v as usize))
        .collect();
    seam_order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ring0: Vec<(f32, usize)> = (0..=HALF_RING_SEGMENTS)
        .map(|k| {
            (
                std::f32::consts::PI * k as f32 / HALF_RING_SEGMENTS as f32,
                rings[0] + k,
            )
        })
        .collect();
    let (mut i, mut j) = (0usize, 0usize);
    while i + 1 < seam_order.len() || j + 1 < ring0.len() {
        let seam_next = seam_order.get(i + 1).map_or(f32::MAX, |v| v.0);
        let ring_next = ring0.get(j + 1).map_or(f32::MAX, |v| v.0);
        if seam_next <= ring_next {
            tris.push([seam_order[i].1, seam_order[i + 1].1, ring0[j].1]);
            i += 1;
        } else {
            tris.push([seam_order[i].1, ring0[j].1, ring0[j + 1].1]);
            j += 1;
        }
    }

    // Retail stores front faces with cross(b-a, c-a) against the normal; an
    // existing seam vertex faces away from the curve start.
    let corner = |i: usize| -> (V3, V3) {
        if i < fresh {
            (new_verts[i].0, new_verts[i].1)
        } else {
            let p = world[i - fresh];
            let d = sub(p, pts[0]);
            (
                p,
                normalize(sub(d, scale(start_tangent, dot(d, start_tangent)))),
            )
        }
    };
    for t in tris.iter_mut() {
        let p = t.map(|i| corner(i).0);
        let n = t.iter().fold([0.0; 3], |acc, &i| add(acc, corner(i).1));
        if dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), n) > 0.0 {
            t.swap(1, 2);
        }
    }

    // The neck-base band keeps its stored winding.
    tris.extend(band_tris.iter().map(|t| t.map(|i| fresh + i as usize)));

    // Singles go after the existing singles (shifting every double index);
    // doubles go after the existing doubles.
    let new_single_count = new_verts
        .iter()
        .filter(|v| matches!(v.2, Binding::Single(_)))
        .count();
    let old_doubles = image.doubles.len();
    let (mut s_refs, mut d_refs) = (Vec::new(), Vec::new());
    let mut index = vec![0u16; new_verts.len()];
    for (k, &(p, n, binding)) in new_verts.iter().enumerate() {
        match binding {
            Binding::Single(j) => {
                index[k] = (singles + s_refs.len()) as u16;
                image.singles.push(SingleVertex {
                    position: mat_t_vec(&pose[j].r, sub(p, pose[j].t)),
                    normal: mat_t_vec(&pose[j].r, n),
                });
                s_refs.push([ref_for(j)?, 0]);
            }
            Binding::Double(j0, j1, f) => {
                index[k] = (singles + new_single_count + old_doubles + d_refs.len()) as u16;
                let w = [1.0 - f, f];
                let local = |j: usize| mat_t_vec(&pose[j].r, sub(p, pose[j].t));
                image.doubles.push(DoubleVertex {
                    position: [scale(local(j0), w[0]), scale(local(j1), w[1])],
                    weight: w,
                    normal: [mat_t_vec(&pose[j0].r, n), mat_t_vec(&pose[j1].r, n)],
                });
                d_refs.push([ref_for(j0)?, ref_for(j1)?]);
            }
        }
    }
    let shift = |i: u16| {
        if (i as usize) >= singles {
            i + new_single_count as u16
        } else {
            i
        }
    };
    image
        .instructions
        .iter_mut()
        .for_each(|ins| ins.map_indices(shift));
    image
        .instructions
        .push(Instruction::Texture(*NEW_TEXTURE_NAME));
    for batch in tris.chunks(MAX_TRIS_PER_DRAW) {
        image.instructions.push(Instruction::TriMesh(
            batch
                .iter()
                .map(|t| {
                    t.map(|i| TexturedCorner {
                        index: if i < fresh {
                            index[i]
                        } else {
                            shift((i - fresh) as u16)
                        },
                        uv: SOLID_UV,
                    })
                })
                .collect(),
        ));
    }
    let tail_refs = image.joint_refs.split_off(singles);
    image.joint_refs.extend(s_refs);
    image.joint_refs.extend(tail_refs);
    image.joint_refs.extend(d_refs);

    let new_body = image.encode().map_err(|e| e.to_string())?;
    let template = all
        .iter()
        .find(|c| c.kind == ChunkKind::Img as u8)
        .ok_or("no texture chunk to template from")?;
    let texture = texture_write::solid_dxt3(
        template.data,
        *NEW_TEXTURE_NAME,
        NEW_TEXTURE_SIZE,
        NEW_TEXTURE_SIZE,
        PURPLE,
    )
    .map_err(|e| e.to_string())?;
    let last_texture = all
        .iter()
        .rposition(|c| c.kind == ChunkKind::Img as u8)
        .expect("a texture chunk was found above");

    let mut out = Vec::with_capacity(input.len() + new_body.len());
    for (i, c) in all.iter().enumerate() {
        if c.offset == mesh_chunk.offset {
            out.extend(chunk::reencode(input, c, &new_body));
        } else {
            out.extend_from_slice(&input[c.offset..chunk_end(c)]);
        }
        if i == last_texture {
            out.extend(chunk::encode(
                *NEW_TEXTURE_CHUNK,
                ChunkKind::Img as u8,
                &texture,
            ));
        }
    }
    out.extend_from_slice(&input[all.last().map_or(0, chunk_end)..]);
    verify(&out)?;

    println!(
        "shoulders {shoulders}, neck {neck:?}, head {head_joint} (+{} children); removed {removed} triangles",
        head.len() - 1
    );
    println!(
        "neck base: {} triangles retextured in place",
        band_tris.len()
    );
    println!(
        "added {} vertices ({new_single_count} single, {} double) / {} triangles in {} draws",
        new_verts.len(),
        new_verts.len() - new_single_count,
        tris.len(),
        tris.len().div_ceil(MAX_TRIS_PER_DRAW)
    );
    println!(
        "shaft: length {length:.3}, radius {radius:.3}, base radius {base_radius:.3}, {} rings",
        rings.len()
    );
    Ok(out)
}

fn scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The output must walk end to end and read back through kuluu's own readers.
fn verify(out: &[u8]) -> Result<(), String> {
    let all = chunks(out)?;
    if all.last().map(chunk_end) != Some(out.len()) {
        return Err("output does not walk to its end".into());
    }
    for c in &all {
        if c.kind == ChunkKind::VertexOs2 as u8 {
            let mesh = ffxi_modder::skel_mesh::parse(DatId::from_name(&c.name), c.data);
            let purple = mesh
                .meshes
                .iter()
                .filter(|m| m.texture_name.as_bytes() == NEW_TEXTURE_NAME)
                .count();
            println!(
                "verify: {} buffers, {purple} on the new texture",
                mesh.meshes.len()
            );
        } else if c.kind == ChunkKind::Img as u8 && &c.name == NEW_TEXTURE_CHUNK {
            let t = ffxi_modder::texture::decode_texture(c.data).map_err(|e| e.to_string())?;
            println!(
                "verify: texture {}x{} first texel {:?}",
                t.width,
                t.height,
                &t.rgba[..4]
            );
        }
    }
    Ok(())
}
