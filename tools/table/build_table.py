"""Gear lookup table: every equipment item -> model id -> model DAT id, per race and slot.

Sources: LSB sql/item_equipment.sql (itemId, name, MId, slot mask) and the client's
FFXiMain.dll equipment table (per race, per slot, six (first_file_id, count) bands).
ROM paths are resolved separately from the install's VTABLE/FTABLE (resolve_paths.py).
"""
import re, sys, struct, csv, json
SQL, DLL, OUT = sys.argv[1], sys.argv[2], sys.argv[3]

RACES = {1: 'HumeM', 2: 'HumeF', 3: 'ElvaanM', 4: 'ElvaanF', 5: 'TaruM', 6: 'TaruF', 7: 'Mithra', 8: 'Galka'}
# item slot mask bit -> DLL model slot (0 face, 1 head, 2 body, 3 hands, 4 legs, 5 feet, 6 main, 7 sub, 8 ranged)
SLOTBITS = [(0x10, 1, 'head'), (0x20, 2, 'body'), (0x40, 3, 'hands'), (0x80, 4, 'legs'), (0x100, 5, 'feet'),
            (0x01, 6, 'main'), (0x02, 7, 'sub'), (0x04, 8, 'ranged')]

b = open(DLL, 'rb').read()
# .data section span from the PE section table
pe = struct.unpack_from('<I', b, 0x3C)[0]; nsec = struct.unpack_from('<H', b, pe + 6)[0]
optsz = struct.unpack_from('<H', b, pe + 20)[0]; tab = pe + 24 + optsz
span = None
for i in range(nsec):
    s = tab + 40 * i
    if b[s:s + 8].rstrip(b'\0') == b'.data':
        ptr, size = struct.unpack_from('<I', b, s + 20)[0], struct.unpack_from('<I', b, s + 16)[0]
        span = (ptr, min(ptr + size, len(b)))
base = next(p for p in range(span[0], span[1] - 4, 4) if b[p:p + 4] == bytes.fromhex('A81B0000'))

def model_dat(race, slot, mid):
    row = base + 0x1B0 * (race - 1) + 0x30 * slot
    passed = 0
    for band in range(6):
        first, count = struct.unpack_from('<II', b, row + band * 8)
        if first == 0: continue
        if mid < passed + count: return first + mid - passed
        passed += count
    return None

rows = []
pat = re.compile(r"INSERT INTO `item_equipment` VALUES \((\d+),'((?:[^'\\]|\\.)*)',(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+)\);")
for m in pat.finditer(open(SQL, encoding='utf-8').read()):
    item, name, lvl, ilvl, jobs, mid, shield, script, slot = m.group(1), m.group(2), int(m.group(3)), int(m.group(4)), int(m.group(5)), int(m.group(6)), int(m.group(7)), int(m.group(8)), int(m.group(9))
    for bit, mslot, sname in SLOTBITS:
        if not slot & bit: continue
        if mid == 0: continue
        for r, rn in RACES.items():
            dat = model_dat(r, mslot, mid)
            rows.append(dict(item_id=int(item), name=name, level=lvl, slot=sname, model_id=mid, race=rn, dat_id=dat))
with open(OUT, 'w', newline='') as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0])); w.writeheader(); w.writerows(rows)
items = {r['item_id'] for r in rows}
models = {(r['slot'], r['model_id']) for r in rows}
dats = {r['dat_id'] for r in rows if r['dat_id']}
print(f'equipment table at 0x{base:x}; {len(items)} items with a model, {len(models)} unique (slot, model), '
      f'{len(dats)} unique model DATs over 8 races; {sum(1 for r in rows if r["dat_id"] is None)} rows with no DAT')
for sname in ['head','body','hands','legs','feet','main','sub','ranged']:
    d = {r['dat_id'] for r in rows if r['slot'] == sname and r['dat_id']}
    mm = {r['model_id'] for r in rows if r['slot'] == sname}
    print(f'  {sname:6s} models {len(mm):4d}  DATs {len(d):5d}')
