//! Smooth a skinned mesh DAT (gear, weapons, monsters) by Phong tessellation.
//!
//! usage: smooth_gear <skeleton.DAT> <in.DAT> <out.DAT> [levels]
//!
//! Each level splits every triangle into four. New edge vertices are placed on
//! the curved surface the existing vertex normals describe (Phong
//! tessellation), so flat low-poly panels become curves while every original
//! vertex stays exactly where it was: seams to other gear pieces stay closed.
//! Edges whose end normals diverge sharply are crease edges and are split
//! straight, so hard edges stay hard.
//!
//! New vertices are skinned to the average of their parents' joint weights,
//! keeping the two strongest joints, so they animate with the mesh. Positions
//! are solved back into joint space from the bind pose of `skeleton.DAT` (the
//! race skeleton the gear is worn on). Every other chunk is copied byte for
//! byte.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::process::ExitCode;

use ffxi_modder::chunk::{self, Chunk};
use ffxi_modder::datid::DatId;
use ffxi_modder::kind::ChunkKind;
use ffxi_modder::skel::{self, Skeleton};
use ffxi_modder::skel_mesh_write::{
    pack_joint_ref, DoubleVertex, Instruction, SingleVertex, SkelMeshImage, TexturedCorner, MAX_TRIS_PER_DRAW,
};

const DEFAULT_LEVELS: u32 = 2;
/// Phong tessellation shape factor: 0 is flat, 1 fully on the normal-defined surface.
const PHONG_ALPHA: f32 = 0.75;
/// End normals further apart than this (degrees) mark a crease edge: no bulge.
const CREASE_START_DEG: f32 = 45.0;
const CREASE_FULL_DEG: f32 = 70.0;
const OFF_FLAGS3: usize = 0x02;
const FLAG3_USE_JOINT_ARRAY: u8 = 0x80;
const SLOT_MASK: u16 = 0x7F;
const FLIPPED_SHIFT: u16 = 7;
const AXIS_SHIFT: u16 = 14;

type V3 = [f32; 3];
type M3 = [[f32; 3]; 3];

#[derive(Clone, Copy)]
struct Pose {
    r: M3,
    t: V3,
}

#[derive(Clone)]
struct Vert {
    p: V3,
    n: V3,
    /// (joint, weight), weights summing to 1, strongest first, at most two.
    w: Vec<(usize, f32)>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 || args.len() > 5 {
        eprintln!("usage: {} <skeleton.DAT> <in.DAT> <out.DAT> [levels]", args[0]);
        return ExitCode::from(2);
    }
    let levels = args.get(4).map_or(Ok(DEFAULT_LEVELS), |s| s.parse()).unwrap_or(DEFAULT_LEVELS);
    match run(&args[1], &args[2], &args[3], levels) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(skel_path: &str, input: &str, output: &str, levels: u32) -> Result<(), String> {
    let skel_bytes = fs::read(skel_path).map_err(|e| format!("{skel_path}: {e}"))?;
    let skel_chunk = chunk::walk(&skel_bytes)
        .flatten()
        .find(|c| c.kind == ChunkKind::Bone as u8)
        .ok_or("no skeleton chunk in the skeleton DAT")?;
    let skeleton = skel::parse(DatId::from_name(&skel_chunk.name), skel_chunk.data);
    let pose = bind_pose(&skeleton)?;

    let bytes = fs::read(input).map_err(|e| format!("{input}: {e}"))?;
    let all: Vec<Chunk> = chunk::walk(&bytes).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let end = |c: &Chunk| c.offset + chunk::CHUNK_HEADER_BYTES + c.data.len();
    let mut out = Vec::with_capacity(bytes.len() * 4);
    for c in &all {
        if c.kind == ChunkKind::VertexOs2 as u8 {
            match SkelMeshImage::parse(c.data) {
                Ok(image) => {
                    let body = smooth(image, &pose, levels).map_err(|e| format!("{}: {e}", c.name_str()))?;
                    out.extend(chunk::reencode(&bytes, c, &body));
                    continue;
                }
                Err(e) => println!("{}: left as is ({e})", c.name_str()),
            }
        }
        out.extend_from_slice(&bytes[c.offset..end(c)]);
    }
    out.extend_from_slice(&bytes[all.last().map_or(0, end)..]);
    for c in chunk::walk(&out) {
        let c = c.map_err(|e| format!("output walk: {e}"))?;
        if c.kind == ChunkKind::VertexOs2 as u8 {
            SkelMeshImage::parse(c.data).map_err(|e| format!("output re-read: {e}"))?;
            ffxi_modder::skel_mesh::parse(DatId::from_name(&c.name), c.data);
        }
    }
    fs::write(output, &out).map_err(|e| e.to_string())?;
    println!("wrote {output} ({} -> {} bytes)", bytes.len(), out.len());
    Ok(())
}

fn smooth(mut image: SkelMeshImage, pose: &[Pose], levels: u32) -> Result<Vec<u8>, String> {
    let use_joint_array = image.header[OFF_FLAGS3] & FLAG3_USE_JOINT_ARRAY != 0;
    let joint_of = |r: u16| -> usize {
        let slot = (r & SLOT_MASK) as usize;
        if use_joint_array {
            image.joint_array[slot] as usize
        } else {
            slot
        }
    };
    let singles = image.singles.len();
    let doubles = image.doubles.len();
    let joint_pose = |j: usize| pose.get(j).copied().ok_or(format!("joint {j} not in the skeleton"));

    // Bind-pose world vertices.
    let mut verts: Vec<Vert> = Vec::with_capacity(singles + doubles);
    for v in 0..singles + doubles {
        let [r0, r1] = image.joint_refs[v];
        if v < singles {
            let j = joint_of(r0);
            let jp = joint_pose(j)?;
            let s = &image.singles[v];
            verts.push(Vert { p: add(mat_vec(&jp.r, s.position), jp.t), n: normalize(mat_vec(&jp.r, s.normal)), w: vec![(j, 1.0)] });
        } else {
            let d = &image.doubles[v - singles];
            let (j0, j1) = (joint_of(r0), joint_of(r1));
            let (a, b) = (joint_pose(j0)?, joint_pose(j1)?);
            let p = add(
                add(mat_vec(&a.r, d.position[0]), scale(a.t, d.weight[0])),
                add(mat_vec(&b.r, d.position[1]), scale(b.t, d.weight[1])),
            );
            let n = normalize(add(scale(mat_vec(&a.r, d.normal[0]), d.weight[0]), scale(mat_vec(&b.r, d.normal[1]), d.weight[1])));
            verts.push(Vert { p, n, w: vec![(j0, d.weight[0]), (j1, d.weight[1])] });
        }
    }

    // Each joint's slot and mirror bits as the mesh already stores them.
    let mut upper: HashMap<usize, HashMap<u16, usize>> = HashMap::new();
    for (v, refs) in image.joint_refs.iter().enumerate() {
        let used = if v < singles { &refs[..1] } else { &refs[..] };
        for &r in used {
            *upper.entry(joint_of(r)).or_default().entry(r >> FLIPPED_SHIFT).or_default() += 1;
        }
    }
    let slot_of = |j: usize| -> Result<u16, String> {
        if use_joint_array {
            image.joint_array.iter().position(|&x| x as usize == j).map(|s| s as u16).ok_or(format!("joint {j} not in the joint array"))
        } else {
            Ok(j as u16)
        }
    };
    let ref_for = |j: usize| -> Result<u16, String> {
        let (&bits, _) = upper.get(&j).and_then(|m| m.iter().max_by_key(|(_, n)| **n)).ok_or(format!("joint {j} has no stored ref"))?;
        Ok(pack_joint_ref(slot_of(j)?, bits & SLOT_MASK, bits >> (AXIS_SHIFT - FLIPPED_SHIFT)))
    };

    // Triangles per geometry op, corners as (vertex, uv), winding as drawn.
    let mut ops: Vec<Option<Vec<[(usize, [f32; 2]); 3]>>> = Vec::new();
    for ins in &image.instructions {
        ops.push(match ins {
            Instruction::TriMesh(t) => Some(t.iter().map(|c| c.map(|c| (c.index as usize, c.uv))).collect()),
            Instruction::TriStrip { head, tail } => {
                let corners: Vec<(usize, [f32; 2])> = head.iter().chain(tail).map(|c| (c.index as usize, c.uv)).collect();
                Some(
                    corners
                        .windows(3)
                        .enumerate()
                        .map(|(k, w)| if k % 2 == 0 { [w[0], w[1], w[2]] } else { [w[1], w[0], w[2]] })
                        .collect(),
                )
            }
            Instruction::UntexturedTriMesh(_) | Instruction::UntexturedTriStrip { .. } => {
                return Err("untextured draws are not supported".into())
            }
            Instruction::RenderProperties(_) | Instruction::Texture(_) => None,
        });
    }

    let crease = |a: V3, b: V3| -> f32 {
        let ang = dot(a, b).clamp(-1.0, 1.0).acos().to_degrees();
        1.0 - ((ang - CREASE_START_DEG) / (CREASE_FULL_DEG - CREASE_START_DEG)).clamp(0.0, 1.0)
    };
    let original = verts.len();
    for _ in 0..levels {
        let mut mid: HashMap<(usize, usize), usize> = HashMap::new();
        let mut midpoint = |a: usize, b: usize, verts: &mut Vec<Vert>| -> usize {
            let key = (a.min(b), a.max(b));
            if let Some(&m) = mid.get(&key) {
                return m;
            }
            let (va, vb) = (&verts[a], &verts[b]);
            let m = scale(add(va.p, vb.p), 0.5);
            let on_a = sub(m, scale(va.n, dot(sub(m, va.p), va.n)));
            let on_b = sub(m, scale(vb.n, dot(sub(m, vb.p), vb.n)));
            let curved = scale(add(on_a, on_b), 0.5);
            let k = PHONG_ALPHA * crease(va.n, vb.n);
            let p = add(m, scale(sub(curved, m), k));
            let n = normalize(add(va.n, vb.n));
            let mut w: HashMap<usize, f32> = HashMap::new();
            for &(j, x) in va.w.iter().chain(&vb.w) {
                *w.entry(j).or_default() += 0.5 * x;
            }
            let mut w: Vec<(usize, f32)> = w.into_iter().collect();
            w.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
            w.truncate(2);
            let sum: f32 = w.iter().map(|x| x.1).sum();
            for x in w.iter_mut() {
                x.1 /= sum;
            }
            if w.len() == 2 && w[1].1 < 1e-4 {
                w.truncate(1);
                w[0].1 = 1.0;
            }
            verts.push(Vert { p, n, w });
            let idx = verts.len() - 1;
            mid.insert(key, idx);
            idx
        };
        for op in ops.iter_mut().flatten() {
            let mut next = Vec::with_capacity(op.len() * 4);
            for t in op.iter() {
                let [(a, ua), (b, ub), (c, uc)] = *t;
                let avg = |x: [f32; 2], y: [f32; 2]| [(x[0] + y[0]) / 2.0, (x[1] + y[1]) / 2.0];
                let ab = (midpoint(a, b, &mut verts), avg(ua, ub));
                let bc = (midpoint(b, c, &mut verts), avg(ub, uc));
                let ca = (midpoint(c, a, &mut verts), avg(uc, ua));
                next.push([(a, ua), ab, ca]);
                next.push([ab, (b, ub), bc]);
                next.push([ca, bc, (c, uc)]);
                next.push([ab, bc, ca]);
            }
            *op = next;
        }
    }

    // Write the new vertices back in joint space.
    let mut new_singles: Vec<(usize, SingleVertex, [u16; 2])> = Vec::new();
    let mut new_doubles: Vec<(usize, DoubleVertex, [u16; 2])> = Vec::new();
    for (k, v) in verts.iter().enumerate().skip(original) {
        if v.w.len() == 1 {
            let j = v.w[0].0;
            let jp = joint_pose(j)?;
            new_singles.push((k, SingleVertex { position: mat_t_vec(&jp.r, sub(v.p, jp.t)), normal: mat_t_vec(&jp.r, v.n) }, [ref_for(j)?, 0]));
        } else {
            let ((j0, w0), (j1, w1)) = (v.w[0], v.w[1]);
            let (a, b) = (joint_pose(j0)?, joint_pose(j1)?);
            new_doubles.push((
                k,
                DoubleVertex {
                    position: [scale(mat_t_vec(&a.r, sub(v.p, a.t)), w0), scale(mat_t_vec(&b.r, sub(v.p, b.t)), w1)],
                    weight: [w0, w1],
                    normal: [mat_t_vec(&a.r, v.n), mat_t_vec(&b.r, v.n)],
                },
                [ref_for(j0)?, ref_for(j1)?],
            ));
        }
    }
    let ns = new_singles.len();
    let mut index = vec![0usize; verts.len()];
    for (v, slot) in index.iter_mut().enumerate().take(original) {
        *slot = if v < singles { v } else { v + ns };
    }
    for (i, (k, _, _)) in new_singles.iter().enumerate() {
        index[*k] = singles + i;
    }
    for (i, (k, _, _)) in new_doubles.iter().enumerate() {
        index[*k] = singles + ns + doubles + i;
    }
    let total = singles + ns + doubles + new_doubles.len();
    if total > u16::MAX as usize {
        return Err(format!("{total} vertices overflow u16 indices"));
    }

    let mut instructions = Vec::new();
    let mut tris_out = 0usize;
    for (ins, op) in image.instructions.iter().zip(&ops) {
        match op {
            None => instructions.push(ins.clone()),
            Some(tris) => {
                tris_out += tris.len();
                for batch in tris.chunks(MAX_TRIS_PER_DRAW) {
                    instructions.push(Instruction::TriMesh(
                        batch.iter().map(|t| t.map(|(v, uv)| TexturedCorner { index: index[v] as u16, uv })).collect(),
                    ));
                }
            }
        }
    }
    image.instructions = instructions;
    let tail_refs = image.joint_refs.split_off(singles);
    image.singles.extend(new_singles.iter().map(|x| x.1));
    image.joint_refs.extend(new_singles.iter().map(|x| x.2));
    image.joint_refs.extend(tail_refs);
    image.doubles.extend(new_doubles.iter().map(|x| x.1));
    image.joint_refs.extend(new_doubles.iter().map(|x| x.2));
    println!(
        "  vertices {} -> {}, triangles -> {tris_out} ({} new single, {} new double)",
        singles + doubles,
        total,
        ns,
        new_doubles.len()
    );
    image.encode().map_err(|e| e.to_string())
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
            Some(p) if p < i => Pose { r: mat_mul(&out[p].r, &local.r), t: add(mat_vec(&out[p].r, local.t), out[p].t) },
            Some(p) => return Err(format!("joint {i} has later parent {p}")),
        });
    }
    Ok(out)
}

fn mat_mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for (i, row) in o.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

fn mat_vec(a: &M3, v: V3) -> V3 {
    [0, 1, 2].map(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}

fn mat_t_vec(a: &M3, v: V3) -> V3 {
    [0, 1, 2].map(|i| a[0][i] * v[0] + a[1][i] * v[1] + a[2][i] * v[2])
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(v: V3, s: f32) -> V3 {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: V3) -> V3 {
    let l = dot(v, v).sqrt().max(f32::EPSILON);
    [v[0] / l, v[1] / l, v[2] / l]
}
