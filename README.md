# ffxi_modder

Lossless read **and write** for FFXI skinned-mesh DATs, plus two Dhalmel mods built on it:
the neck/head swap and the cow. Existing tooling in this space reads these files; nothing in
[kuluu-ffxi's `ffxi-dat`](https://github.com/cowrevenge/kuluu-ffxi) could encode them, so
the writer here is the point of the repo.

Modded output is meant to be dropped into a [XIPivot](https://github.com/HealsCodes/XIPivot)
overlay at the same `ROM\<dir>\<file>.DAT` path. **The game's own files are never
modified**, and the mod is visible only on the machine running the overlay.

Companion repo with the built DAT and install notes: [`ffxi_mod_files`](https://github.com/cowrevenge/ffxi_mod_files).

## The lossless guarantee

An unedited parse → write cycle reproduces the input **byte-for-byte**. This is what makes
it safe to open a retail DAT, change one mesh section, and write the file back: everything
you did not touch cannot drift.

Verified on this machine against the retail Dhalmel model:

| input | size | result |
|---|---|---|
| retail `ROM\5\15.DAT` | 481,984 B | byte-identical |
| modded `ROM\5\15.DAT` | 501,024 B | byte-identical (writer is lossless on its own output) |

One subtlety it handles: retail leaves **4 garbage padding bytes** after the vertex data.
A writer that zeroed them would produce a file that still loads but is no longer
reproducible, so they are carried through verbatim.

## Usage

```
cargo run --release -- <in.DAT> --check                    prove the writer round trips this file
cargo run --release -- <in.DAT> <out.DAT>                  neck mod: write the modded DAT
cargo run --release --bin dhalmel_cow -- <in.DAT> <out.DAT>  cow mod: write the modded DAT
```

Both mods take the retail or the NextHD Dhalmel; NextHD's has the same mesh, skeleton and
animations, only higher-resolution textures.

`--check` prints `round trip: byte-identical (<n> bytes)` or fails. Run it on any DAT
before trusting a mod built from it.

## Layout

**Writer**

- `skel_mesh_write.rs` — the lossless model for a `0x2A` mesh chunk: single/double
  vertices, textured corners, draw instructions, joint packing, and the 4-byte trailing
  padding.
- `texture_write.rs` — emits a solid DXT3 texture chunk (header copied from a real retail
  texture so fields whose meaning is unknown keep retail values), and re-encodes a DXT3
  texture's colour blocks from new pixels while keeping its alpha blocks byte for byte.
- `skel_write.rs` — in-place skeleton (`0x29`) edits: joint translations, reference offsets,
  bounding-box tops, each re-parsed after writing. `SkelEditor::fit_height(original_top,
  new_top)` is the one every mod that changes the model's height should call: it moves the
  nameplate reference and the full-body boxes so they keep their original gap above the
  model's highest point.
- `skel_anim_write.rs` — replaces a joint's rotation keys in an animation (`0x2B`) chunk.
  New sequences are appended to the float pool and the channel offsets repointed, so shared
  pool data is never overwritten and everything not edited stays byte for byte.
- `chunk.rs` — chunk walker, plus `encode` / `reencode`.

**Reader** — `chunk`, `skel`, `skel_mesh`, `skel_anim`, `texture`, `vos2`, `kind`, `datid`. Copied from
kuluu-ffxi's `ffxi-dat` crate (GPL-3.0-or-later), so **this repo is GPL-3.0-or-later too**
and carries the readers' attribution.

**Tools** — `main.rs` is the neck swap; `bin/dhalmel_cow.rs` is the cow.

## How the Dhalmel swap works

The head is not found by name or by guessing a mesh section. It is derived from the
skeleton:

- **head** = joint **36**, which is the skeleton's look-at reference
  (`skel::standard_position::NECK` — the bone retail bends to aim the face), **plus its 9
  descendants** (ears etc.).
- **neck** = the parent chain from the head down to the first joint that branches.
- **shoulders** = joint **29**, that branching parent, which also carries the front legs.

Then: 273 head/neck triangles removed, 10 neck-base triangles retextured in place to close
the seam, and 265 vertices / 494 triangles added in 4 draws. Every added vertex is weighted
onto the neck/head joints and mirrored on the Z axis the same way the head was, so the
replacement follows head and neck animation exactly — it nods and turns because it *is*
those bones' geometry.

Colour comes from a new 16×16 DXT3 solid-purple (`#9A2BD8`) texture chunk rather than from
`t_factor`; first texel reads back `[156, 40, 222, 136]`.

### Tuning the shape

All at the top of `main.rs`:

| const | meaning |
|---|---|
| `PURPLE` | RGB of the solid texture |
| `SHAFT_RADIUS_SCALE` | shaft radius ÷ removed neck's mean radius |
| `TIP_PER_HEAD_LENGTH` | how far past the head's back edge the tip reaches |
| `SEAM_OVERLAP` | base ring radius ÷ shoulder-seam radius (overlap hides the hole) |
| `FLARE_PER_SEAM_RADIUS`, `FLARE_RINGS` | the flared base |
| `SHAFT_STEP_PER_RADIUS` | ring spacing along the shaft |
| `HALF_RING_SEGMENTS` | ring resolution; the mesh mirrors the other half |
| `GLANS`, `GLANS_PER_RADIUS` | tip profile table (fraction, radius multiple) |
| `BEZIER_HANDLE`, `CURVE_SAMPLES` | the bend from base to tip |

Sizing is measured from the removed head, so it scales to the model rather than being
hardcoded — which is why the same writer drives the planned Galka body swap.

## How the Dhalmel cow works

`bin/dhalmel_cow.rs`, all from the model's own data:

- **Level back.** The shoulder joint drops by the height difference between the rump
  (hips-joint vertices) and the shoulder top, measured from the mesh (1.05 on the Dhalmel).
  The front legs compress to match so the feet stay on the ground: leg bone translations
  scale by the same factor (0.646) and every leg vertex scales along its joint's bone axis,
  so the segments stay joined instead of overlapping.
- **Height.** The nameplate reference (`ABOVE_HEAD`, a fixed offset from the root that
  retail reads straight) and the two full-body bounding boxes keep their original gap above
  the model's new highest point.
- **Short neck.** The bind translations of the neck joints (31-36) are scaled to
  `NECK_SCALE` (0.3). Animation translation is *added* to bind translation, and every one of
  the Dhalmel's 16 clips keys zero translation on the neck, so all of them play on the short
  neck. Neck and head geometry is stored joint-local, so it rides along.
- **Holstein paint.** Both body textures are repainted white with black patches from 3D
  noise over the bind-pose body, so a patch carries across UV seams. The original texel
  brightness stays as shading; the original alpha blocks are kept byte for byte.
- **Cow head.** The Dhalmel head (joint 36 + 9 children, 233 triangles) is replaced by a
  lofted cow head with pink muzzle, horns, ears and eyes, 100% on the head joint. It faces
  the body's forward direction (hips to shoulders) pitched down `HEAD_DROOP` degrees.
- **Udder.** Pink, with teats, on the hips (the branching joint carrying the hind legs and
  tail), just ahead of the hind legs with its top tucked into the belly.
- **Attacks.** `at00` lift then smack, `at10` lift then side nudge, `atm1` double smack. Only
  the neck and head rotation keys are replaced, peaking on each clip's original hit frame
  (where the original head swung furthest); body and legs are untouched.

Colours come from a new 16×16 DXT3 atlas chunk (`cowa`: white, black, pink, horn cream).
Tuning knobs are the `const`s at the top of `bin/dhalmel_cow.rs` (`NECK_SCALE`,
`SPOT_THRESHOLD`, `HEAD_PROFILE`, `HORN_*`, `UDDER_*`, `LIFT_DEG`, `SMACK_DEG`, `NUDGE_DEG`).

## Known caveat: rebuilds are not bit-identical

Re-running the tool reproduces the model but not necessarily the same bytes — 1,708 bytes
differed on this machine, all inside one span on a 4-byte stride: 1,337 float lanes with a
maximum absolute delta of **3.97e-05** on a model ~3.7 units tall. That is
normalised-vertex floating-point noise from the compiler backend, invisible in game, and
both variants round-trip byte-identical. Pin a committed DAT for reproducibility rather
than regenerating it.

## Resolving a model to a DAT path

The Dhalmel is `ROM\5\15.DAT`, arrived at without guessing:

1. **model id 332** — LSB `data/zones/*/mobs.yaml`, all 35 `dhalmel` entries (skill list 80).
2. **DAT file id 1632** — `npc_dat_id()` from kuluu-render's `look_resolver.rs`:
   `model < 1500 → model + 1300`; `model < 3000 → +50295`; `model < 3500 → +96907`;
   otherwise `+98239`.
3. **ROM path** — install-specific: the highest ROM whose VTABLE byte equals its own index
   owns the file, then `dir = entry >> 7`, `file = entry & 0x7F`.

Step 3 depends on **your** VTABLE/FTABLE, so re-resolve it per install — the same file id
can land in a different folder on another client.

## Status

Working: skinned-mesh writer, texture writers, animation rotation writer, Dhalmel neck
swap, Dhalmel cow. The animation writer is the piece the PUP automaton "Overload" motion
edit needs.

## Licence

GPL-3.0-or-later, inherited from the kuluu-ffxi `ffxi-dat` reader modules this is built on.
