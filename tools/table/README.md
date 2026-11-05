# Gear lookup table

Every equippable item, its model id, and the model DAT for each of the 8 races.

```bash
python3 build_table.py <LSB sql/item_equipment.sql> <FFXiMain.dll> gear_table.csv
python3 resolve_paths.py "<FINAL FANTASY XI folder>" gear_table.csv gear_table_paths.csv
```

- `build_table.py` reads item names, model ids and slots from LSB's `item_equipment.sql`
  and the per-race, per-slot model bands from FFXiMain.dll's `.data` equipment table.
- `resolve_paths.py` adds the `ROM\x\y.DAT` path from the install's VTABLE/FTABLE files.

Many items share one model file (Cotton/Linen Doublet, Eisen/Kampf), so fixing one model
fixes every item on it.
