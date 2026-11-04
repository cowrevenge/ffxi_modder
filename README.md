# ffxi_modder

Lossless read **and write** for FFXI skinned-mesh DATs, plus the Dhalmel neck/head swap
built on it. Existing tooling in this space reads these files; nothing in
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
cargo run --release -- <in.DAT> --check      prove the writer round trips this file
cargo run --release -- <in.DAT> <out.DAT>    run the swap, write the modded DAT
```

`--check` prints `round trip: byte-identical (<n> bytes)` or fails. Run it on any DAT
before trusting a mod built from it.

## Layout

**Writer**

- `skel_mesh_write.rs` — the lossless model for a `0x2A` mesh chunk: single/double
  vertices, textured corners, draw instructions, joint packing, and the 4-byte trailing
  padding.
- `texture_write.rs` — emits a solid DXT3 texture chunk. The header is copied from a real
  retail texture so fields whose meaning is unknown keep plausible retail values.
- `chunk.rs` — chunk walker, plus `encode` / `reencode`.

**Reader** — `chunk`, `skel`, `skel_mesh`, `texture`, `vos2`, `kind`, `datid`. Copied from
kuluu-ffxi's `ffxi-dat` crate (GPL-3.0-or-later), so **this repo is GPL-3.0-or-later too**
and carries the readers' attribution.

**Tool** — `main.rs` is the swap.

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

Working: skinned-mesh writer, solid-texture writer, Dhalmel swap.
Not built yet: a `skel_anim` writer, which is what the PUP automaton "Overload" motion edit
needs. `skel_anim.rs` is read-only today.

## Licence

GPL-3.0-or-later, inherited from the kuluu-ffxi `ffxi-dat` reader modules this is built on.
