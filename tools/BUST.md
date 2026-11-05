# Bust shape (`bust_shape`)

```bash
cargo run --release --bin bust_shape -- <race skeleton.DAT> <chest.DAT> <out.DAT> depth=0.055 width=0.046 up=0.085 down=0.05 sag=0.008 levels=2
```

Adds vertices around the bust only (red-green refinement, new vertices on the Phong
surface, re-skinned to their parents' joints), then moves the bust zone by a smooth,
round bump field, rebuilds the normals there, and writes everything else byte for byte.
Seams outside the zone do not move.

Tuned values (Bastok Pride chest, the look picked as "+60%"):

| race | depth | width | up | down | sag |
|---|---|---|---|---|---|
| Hume F | 0.055 | 0.046 | 0.085 | 0.05 | 0.008 |
| Mithra | 0.055 | 0.046 | 0.085 | 0.05 | 0.008 |
| Elvaan F | 0.058 | 0.05 | 0.095 | 0.055 | 0.01 |

For painted sets, re-bake the texture shading from the new shape: run
`tools/paint/racepaint.py body ...` with `BAKE_DAT=<shaped DAT>`, then `retexture` the
shaped DAT.
