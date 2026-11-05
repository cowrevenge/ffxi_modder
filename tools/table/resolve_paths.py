"""Add ROM paths to the gear table from an install's VTABLE/FTABLE files.

usage: python3 resolve_paths.py <ffxi_install_dir> gear_table.csv gear_table_paths.csv

For each file id, the highest ROM n whose VTABLE byte equals n owns the file; its
FTABLE entry e gives ROM<n>\\<e >> 7>\\<e & 0x7F>.DAT.
"""
import sys, os, csv, struct
root, src, dst = sys.argv[1:4]
V, F = {}, {}
for n in range(1, 10):
    d = os.path.join(root, 'ROM' if n == 1 else f'ROM{n}')
    s = '' if n == 1 else str(n)
    vp, fp = os.path.join(d, f'VTABLE{s}.DAT'), os.path.join(d, f'FTABLE{s}.DAT')
    if not (os.path.exists(vp) and os.path.exists(fp)):
        vp, fp = os.path.join(root, f'VTABLE{s}.DAT'), os.path.join(root, f'FTABLE{s}.DAT')
    if os.path.exists(vp) and os.path.exists(fp):
        V[n] = open(vp, 'rb').read(); F[n] = open(fp, 'rb').read()

def path(fid):
    for n in sorted(V, reverse=True):
        if fid < len(V[n]) and V[n][fid] == n:
            e = struct.unpack_from('<H', F[n], fid * 2)[0]
            return f"{'ROM' if n == 1 else f'ROM{n}'}\\{e >> 7}\\{e & 0x7F}.DAT"
    return ''

rows = list(csv.DictReader(open(src)))
for r in rows:
    r['rom_path'] = path(int(r['dat_id'])) if r['dat_id'] else ''
with open(dst, 'w', newline='') as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0])); w.writeheader(); w.writerows(rows)
print(len(V), 'ROM tables;', sum(1 for r in rows if r['rom_path']), 'of', len(rows), 'rows resolved')
