//! Reshape the bust of a female chest DAT: add vertices where the shape needs them,
//! then apply a hand-tuned bust shape for the race.
//!
//! usage: bust_shape <skeleton.DAT> <in.DAT> <out.DAT> key=value ...
//!
//! Shape keys (world units of the bind pose; x forward, y down, z side):
//!   depth    how far the bust tip moves forward            (required)
//!   width    half width of the bust, side to side           (default 0.07)
//!   up       reach of the upper slope above the tip         (default 0.10)
//!   down     reach of the under-curve below the tip         (default 0.06)
//!   sag      how far the fullness drops below the tip        (default 0.0)
//!   ax ay az apex override (default: found from the mesh)
//!   levels   subdivision levels inside the bust zone        (default 2)
//!
//! 1. Every vertex is posed on the race skeleton. The bust zone is an ellipse around
//!    each apex (the mirror side follows: these meshes store one side).
//! 2. Inside the zone, edges are split (red-green refinement: a triangle with three
//!    split edges becomes four, with one or two it is fanned so no crack opens).
//!    New vertices sit on the curved surface the normals describe (Phong) and are
//!    re-skinned to their parents' joints, so they animate with the mesh.
//! 3. The shape: each zone vertex moves forward by a smooth bump field of the
//!    side/vertical distance from the tip (a fuller lower half, a longer upper slope),
//!    not by where the old vertex happened to be, so the result is round however the
//!    original was triangulated. The zone rim does not move; seams stay closed.
//! 4. Normals are rebuilt from the new faces inside the zone. Everything else in the
//!    file is copied byte for byte.

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

const PHONG_ALPHA: f32 = 0.75;
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
    w: Vec<(usize, f32)>,
}

#[derive(Clone, Copy, Debug)]
struct Shape {
    depth: f32,
    width: f32,
    up: f32,
    down: f32,
    sag: f32,
    apex: Option<V3>,
    levels: u32,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: {} <skeleton.DAT> <in.DAT> <out.DAT> depth=.. [width=.. up=.. down=.. sag=.. ax= ay= az= levels=..]", args[0]);
        return ExitCode::from(2);
    }
    let mut kv: HashMap<String, f32> = HashMap::new();
    for a in &args[4..] {
        if let Some((k, v)) = a.split_once('=') {
            if let Ok(x) = v.parse() {
                kv.insert(k.to_string(), x);
            }
        }
    }
    let g = |k: &str, d: f32| *kv.get(k).unwrap_or(&d);
    let shape = Shape {
        depth: g("depth", 0.05),
        width: g("width", 0.07),
        up: g("up", 0.10),
        down: g("down", 0.06),
        sag: g("sag", 0.0),
        apex: match (kv.get("ax"), kv.get("ay"), kv.get("az")) {
            (Some(&x), Some(&y), Some(&z)) => Some([x, y, z]),
            _ => None,
        },
        levels: g("levels", 2.0) as u32,
    };
    match run(&args[1], &args[2], &args[3], shape) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(skel_path: &str, input: &str, output: &str, shape: Shape) -> Result<(), String> {
    let skel_bytes = fs::read(skel_path).map_err(|e| format!("{skel_path}: {e}"))?;
    let skel_chunk = chunk::walk(&skel_bytes)
        .flatten()
        .find(|c| c.kind == ChunkKind::Bone as u8)
        .ok_or("no skeleton chunk in the skeleton DAT")?;
    let skeleton = skel::parse(DatId::from_name(&skel_chunk.name), skel_chunk.data);
    let pose = bind_pose(&skeleton)?;

    let bytes = fs::read(input).map_err(|e| format!("{input}: {e}"))?;
    let all: Vec<Chunk> = chunk::walk(&bytes).collect::<Result<_, _>>().map_err(|e| e.to_string())?;

    // apex: the most forward point at bust height across all meshes (stored side)
    let apex = match shape.apex {
        Some(a) => a,
        None => {
            let mut pts: Vec<V3> = Vec::new();
            for c in &all {
                if c.kind == ChunkKind::VertexOs2 as u8 {
                    if let Ok(img) = SkelMeshImage::parse(c.data) {
                        pts.extend(posed(&img, &pose)?.into_iter().map(|v| v.p));
                    }
                }
            }
            let top = pts.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
            let bot = pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            let h = bot - top;
            pts.iter()
                .filter(|p| p[1] > top + 0.12 * h && p[1] < top + 0.45 * h && p[2].abs() > 0.025 && p[2].abs() < 0.16)
                .max_by(|a, b| a[0].total_cmp(&b[0]))
                .map(|p| [p[0], p[1], p[2].abs()])
                .ok_or("no bust apex found")?
        }
    };
    println!("apex {:?}  shape {:?}", apex, shape);

    let end = |c: &Chunk| c.offset + chunk::CHUNK_HEADER_BYTES + c.data.len();
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for c in &all {
        if c.kind == ChunkKind::VertexOs2 as u8 {
            if let Ok(image) = SkelMeshImage::parse(c.data) {
                let body = reshape(image, &pose, apex, &shape).map_err(|e| format!("{}: {e}", c.name_str()))?;
                out.extend(chunk::reencode(&bytes, c, &body));
                continue;
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

fn joint_of_fn(image: &SkelMeshImage) -> impl Fn(u16) -> usize + '_ {
    let use_ja = image.header[OFF_FLAGS3] & FLAG3_USE_JOINT_ARRAY != 0;
    move |r: u16| {
        let slot = (r & SLOT_MASK) as usize;
        if use_ja {
            image.joint_array[slot] as usize
        } else {
            slot
        }
    }
}

fn posed(image: &SkelMeshImage, pose: &[Pose]) -> Result<Vec<Vert>, String> {
    let joint_of = joint_of_fn(image);
    let singles = image.singles.len();
    let jp = |j: usize| pose.get(j).copied().ok_or(format!("joint {j} not in the skeleton"));
    let mut verts = Vec::new();
    for v in 0..singles + image.doubles.len() {
        let [r0, r1] = image.joint_refs[v];
        if v < singles {
            let j = joint_of(r0);
            let p = jp(j)?;
            let s = &image.singles[v];
            verts.push(Vert { p: add(mat_vec(&p.r, s.position), p.t), n: normalize(mat_vec(&p.r, s.normal)), w: vec![(j, 1.0)] });
        } else {
            let d = &image.doubles[v - singles];
            let (j0, j1) = (joint_of(r0), joint_of(r1));
            let (a, b) = (jp(j0)?, jp(j1)?);
            let p = add(add(mat_vec(&a.r, d.position[0]), scale(a.t, d.weight[0])), add(mat_vec(&b.r, d.position[1]), scale(b.t, d.weight[1])));
            let n = normalize(add(scale(mat_vec(&a.r, d.normal[0]), d.weight[0]), scale(mat_vec(&b.r, d.normal[1]), d.weight[1])));
            verts.push(Vert { p, n, w: vec![(j0, d.weight[0]), (j1, d.weight[1])] });
        }
    }
    Ok(verts)
}

/// 0..1 inside the bust zone around either apex (the mirror one included).
fn zone(p: V3, apex: V3, s: &Shape) -> f32 {
    let mut best: f32 = 0.0;
    for side in [1.0f32, -1.0] {
        let a = [apex[0], apex[1], apex[2] * side];
        if p[0] < a[0] - 0.22 {
            continue; // the back of the body is never in the zone
        }
        let dz = (p[2] - a[2]) / (s.width * 1.7);
        let dy = p[1] - a[1];
        let dy = if dy < 0.0 { dy / (s.up * 1.5) } else { dy / (s.down * 1.9) };
        let r2 = dz * dz + dy * dy;
        if r2 < 1.0 {
            best = best.max(1.0 - r2);
        }
    }
    best
}

/// The hand-tuned bust: forward bump at (y, z), 0 at the zone rim.
fn bump(p: V3, apex: V3, s: &Shape) -> V3 {
    let mut best = [0.0f32; 3];
    let mut best_f = 0.0f32;
    for side in [1.0f32, -1.0] {
        let a = [apex[0], apex[1] + s.sag, apex[2] * side];
        if p[0] < a[0] - 0.22 {
            continue;
        }
        let dz = (p[2] - a[2]) / s.width;
        let dy = p[1] - a[1];
        let dy = if dy < 0.0 { dy / s.up } else { dy / s.down };
        let r2 = dz * dz + dy * dy;
        // round dome: smooth cap, flat top, eased to zero at the rim
        let f = if r2 >= 2.4 { 0.0 } else { let t = r2 / 2.4; (1.0 - t).powi(2) * (1.0 + 2.0 * t) };
        if f > best_f {
            best_f = f;
            // forward, plus a touch out to the side and down under the curve
            let out_side = (p[2] - a[2]).signum() * 0.08 * dz.abs().min(1.0);
            let under = if p[1] > a[1] { 0.12 } else { 0.0 };
            best = [s.depth * f, s.depth * f * under, s.depth * f * out_side];
        }
    }
    best
}

fn reshape(mut image: SkelMeshImage, pose: &[Pose], apex: V3, shape: &Shape) -> Result<Vec<u8>, String> {
    let singles = image.singles.len();
    let doubles = image.doubles.len();
    let mut verts = posed(&image, pose)?;
    let use_ja = image.header[OFF_FLAGS3] & FLAG3_USE_JOINT_ARRAY != 0;
    let joint_of = |r: u16| -> usize {
        let slot = (r & SLOT_MASK) as usize;
        if use_ja { image.joint_array[slot] as usize } else { slot }
    };
    let jp = |j: usize| pose.get(j).copied().ok_or(format!("joint {j} not in the skeleton"));

    // stored mirror bits per joint, to give new vertices matching refs
    let mut upper: HashMap<usize, HashMap<u16, usize>> = HashMap::new();
    for (v, refs) in image.joint_refs.iter().enumerate() {
        let used = if v < singles { &refs[..1] } else { &refs[..] };
        for &r in used {
            *upper.entry(joint_of(r)).or_default().entry(r >> FLIPPED_SHIFT).or_default() += 1;
        }
    }
    let slot_of = |j: usize| -> Result<u16, String> {
        if use_ja {
            image.joint_array.iter().position(|&x| x as usize == j).map(|s| s as u16).ok_or(format!("joint {j} not in the joint array"))
        } else {
            Ok(j as u16)
        }
    };
    let ref_for = |j: usize| -> Result<u16, String> {
        let (&bits, _) = upper.get(&j).and_then(|m| m.iter().max_by_key(|(_, n)| **n)).ok_or(format!("joint {j} has no stored ref"))?;
        Ok(pack_joint_ref(slot_of(j)?, bits & SLOT_MASK, bits >> (AXIS_SHIFT - FLIPPED_SHIFT)))
    };

    // triangles per draw op, corners (vertex, uv), winding as drawn
    let mut ops: Vec<Option<Vec<[(usize, [f32; 2]); 3]>>> = Vec::new();
    for ins in &image.instructions {
        ops.push(match ins {
            Instruction::TriMesh(t) => Some(t.iter().map(|c| c.map(|c| (c.index as usize, c.uv))).collect()),
            Instruction::TriStrip { head, tail } => {
                let corners: Vec<(usize, [f32; 2])> = head.iter().chain(tail).map(|c| (c.index as usize, c.uv)).collect();
                Some(corners.windows(3).enumerate().map(|(k, w)| if k % 2 == 0 { [w[0], w[1], w[2]] } else { [w[1], w[0], w[2]] }).collect())
            }
            Instruction::UntexturedTriMesh(_) | Instruction::UntexturedTriStrip { .. } => return Ok(image.encode().map_err(|e| e.to_string())?),
            Instruction::RenderProperties(_) | Instruction::Texture(_) => None,
        });
    }
    if !verts.iter().any(|v| zone(v.p, apex, shape) > 0.0) {
        return image.encode().map_err(|e| e.to_string()); // nothing of the bust in this mesh
    }

    let original = verts.len();
    // local red-green refinement inside the zone
    for _ in 0..shape.levels {
        let in_zone: Vec<bool> = verts.iter().map(|v| zone(v.p, apex, shape) > 0.0).collect();
        let mut mid: HashMap<(usize, usize), usize> = HashMap::new();
        let split = |a: usize, b: usize| in_zone[a] && in_zone[b];
        for op in ops.iter_mut().flatten() {
            let mut next = Vec::with_capacity(op.len() * 3);
            for t in op.iter() {
                let [(a, ua), (b, ub), (c, uc)] = *t;
                let avg = |x: [f32; 2], y: [f32; 2]| [(x[0] + y[0]) / 2.0, (x[1] + y[1]) / 2.0];
                let mut m = |x: usize, y: usize, verts: &mut Vec<Vert>| midpoint(x, y, verts, &mut mid);
                let (sab, sbc, sca) = (split(a, b), split(b, c), split(c, a));
                match (sab, sbc, sca) {
                    (true, true, true) => {
                        let ab = (m(a, b, &mut verts), avg(ua, ub));
                        let bc = (m(b, c, &mut verts), avg(ub, uc));
                        let ca = (m(c, a, &mut verts), avg(uc, ua));
                        next.push([(a, ua), ab, ca]);
                        next.push([ab, (b, ub), bc]);
                        next.push([ca, bc, (c, uc)]);
                        next.push([ab, bc, ca]);
                    }
                    _ => {
                        // rotate so the split edges come first, then fan
                        let corners = [(a, ua), (b, ub), (c, uc)];
                        let flags = [sab, sbc, sca];
                        let count = flags.iter().filter(|x| **x).count();
                        if count == 0 {
                            next.push(*t);
                            continue;
                        }
                        // find rotation r so that edge r (corner r -> r+1) is split and, for 2, edge r+1 too
                        let r = (0..3)
                            .find(|&r| flags[r] && (count == 1 || flags[(r + 1) % 3]))
                            .unwrap();
                        let p0 = corners[r];
                        let p1 = corners[(r + 1) % 3];
                        let p2 = corners[(r + 2) % 3];
                        let m01 = (m(p0.0, p1.0, &mut verts), avg(p0.1, p1.1));
                        if count == 1 {
                            next.push([p0, m01, p2]);
                            next.push([m01, p1, p2]);
                        } else {
                            let m12 = (m(p1.0, p2.0, &mut verts), avg(p1.1, p2.1));
                            next.push([p0, m01, p2]);
                            next.push([m01, p1, m12]);
                            next.push([m01, m12, p2]);
                        }
                    }
                }
            }
            *op = next;
        }
    }

    // the shape: move every zone vertex by the bump field (originals and new ones)
    let moved: Vec<bool> = verts.iter().map(|v| zone(v.p, apex, shape) > 0.0).collect();
    for v in verts.iter_mut() {
        let d = bump(v.p, apex, shape);
        if v.p[2].abs() < 1e-4 {
            v.p = add(v.p, [d[0], d[1], 0.0]); // centreline stays on the mirror plane
        } else {
            v.p = add(v.p, d);
        }
    }
    // rebuild normals of moved vertices from the faces around them
    let mut acc = vec![[0.0f32; 3]; verts.len()];
    for op in ops.iter().flatten() {
        for t in op {
            let (a, b, c) = (t[0].0, t[1].0, t[2].0);
            let n = cross(sub(verts[b].p, verts[a].p), sub(verts[c].p, verts[a].p));
            for k in [a, b, c] {
                acc[k] = add(acc[k], n);
            }
        }
    }
    // the drawn winding may face either way; keep each normal on its old side
    for (k, v) in verts.iter_mut().enumerate() {
        if !moved[k] || dot(acc[k], acc[k]) < 1e-12 {
            continue;
        }
        let mut n = normalize(acc[k]);
        if dot(n, v.n) < 0.0 {
            n = scale(n, -1.0);
        }
        if v.p[2].abs() < 1e-4 {
            n[2] = 0.0;
            n = normalize(n);
        }
        v.n = n;
    }

    // write every vertex back into joint space (moved originals in place, new ones appended)
    for v in 0..original {
        if !moved[v] {
            continue;
        }
        let vt = &verts[v];
        if v < singles {
            let j = joint_of(image.joint_refs[v][0]);
            let p = jp(j)?;
            image.singles[v].position = mat_t_vec(&p.r, sub(vt.p, p.t));
            image.singles[v].normal = mat_t_vec(&p.r, vt.n);
        } else {
            let [r0, r1] = image.joint_refs[v];
            let (j0, j1) = (joint_of(r0), joint_of(r1));
            let (a, b) = (jp(j0)?, jp(j1)?);
            let d = &mut image.doubles[v - singles];
            let (w0, w1) = (d.weight[0], d.weight[1]);
            // keep the original split between the two joints: add the move by weight
            let old = add(add(mat_vec(&a.r, d.position[0]), scale(a.t, w0)), add(mat_vec(&b.r, d.position[1]), scale(b.t, w1)));
            let delta = sub(vt.p, old);
            d.position[0] = add(d.position[0], scale(mat_t_vec(&a.r, delta), w0));
            d.position[1] = add(d.position[1], scale(mat_t_vec(&b.r, delta), w1));
            d.normal = [mat_t_vec(&a.r, vt.n), mat_t_vec(&b.r, vt.n)];
        }
    }
    let mut new_singles: Vec<(usize, SingleVertex, [u16; 2])> = Vec::new();
    let mut new_doubles: Vec<(usize, DoubleVertex, [u16; 2])> = Vec::new();
    for (k, v) in verts.iter().enumerate().skip(original) {
        if v.w.len() == 1 {
            let j = v.w[0].0;
            let p = jp(j)?;
            new_singles.push((k, SingleVertex { position: mat_t_vec(&p.r, sub(v.p, p.t)), normal: mat_t_vec(&p.r, v.n) }, [ref_for(j)?, 0]));
        } else {
            let ((j0, w0), (j1, w1)) = (v.w[0], v.w[1]);
            let (a, b) = (jp(j0)?, jp(j1)?);
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
    println!("  vertices {} -> {total}, triangles -> {tris_out}, moved {}", singles + doubles, moved.iter().filter(|x| **x).count());
    image.encode().map_err(|e| e.to_string())
}

fn midpoint(a: usize, b: usize, verts: &mut Vec<Vert>, mid: &mut HashMap<(usize, usize), usize>) -> usize {
    let key = (a.min(b), a.max(b));
    if let Some(&m) = mid.get(&key) {
        return m;
    }
    let (va, vb) = (verts[a].clone(), verts[b].clone());
    let m = scale(add(va.p, vb.p), 0.5);
    let on_a = sub(m, scale(va.n, dot(sub(m, va.p), va.n)));
    let on_b = sub(m, scale(vb.n, dot(sub(m, vb.p), vb.n)));
    let curved = scale(add(on_a, on_b), 0.5);
    let ang = dot(va.n, vb.n).clamp(-1.0, 1.0).acos().to_degrees();
    let k = PHONG_ALPHA * (1.0 - ((ang - 45.0) / 25.0).clamp(0.0, 1.0));
    let mut p = add(m, scale(sub(curved, m), k));
    if va.p[2].abs() < 1e-4 && vb.p[2].abs() < 1e-4 {
        p[2] = 0.0; // an edge on the mirror plane stays on it
    }
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
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalize(v: V3) -> V3 {
    let l = dot(v, v).sqrt().max(f32::EPSILON);
    [v[0] / l, v[1] / l, v[2] / l]
}
