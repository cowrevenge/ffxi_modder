"""Run the all-gear HD pass over every model DAT in the gear table, on the user's PC.

usage:
  python allgear_batch.py --ffxi "C:\\PhoenixXI\\SquareEnix\\FINAL FANTASY XI" \
      --table gear_table.csv --out "C:\\PhoenixXI\\polplugins\\DATs\\AllGearHD" \
      --retexture <path to ffxi_modder retexture.exe> [--workers 8] [--skip skip_ids.txt] [--limit N]

- Resolves each DAT id to ROM<n>\\<dir>\\<file>.DAT through VTABLE/FTABLE.
- Writes <out>\\ROM<n>\\<dir>\\<file>.DAT (a Pivot overlay folder), never touches the game.
- Resumable: a DAT whose output already exists is skipped. Failures go to failed.txt.
- --skip: DAT ids (one per line) left alone, e.g. ones a hand-made mod already covers.
"""
import argparse, csv, os, struct, subprocess, sys, tempfile, zlib, traceback
from concurrent.futures import ProcessPoolExecutor, as_completed
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

def load_tables(root):
    V, F = {}, {}
    for n in range(1, 10):
        s = '' if n == 1 else str(n)
        for d in (os.path.join(root, 'ROM' if n == 1 else f'ROM{n}'), root):
            vp, fp = os.path.join(d, f'VTABLE{s}.DAT'), os.path.join(d, f'FTABLE{s}.DAT')
            if os.path.exists(vp) and os.path.exists(fp):
                V[n] = open(vp, 'rb').read(); F[n] = open(fp, 'rb').read(); break
    return V, F

def rom_path(V, F, fid):
    for n in sorted(V, reverse=True):
        if fid < len(V[n]) and V[n][fid] == n:
            e = struct.unpack_from('<H', F[n], fid * 2)[0]
            return os.path.join('ROM' if n == 1 else f'ROM{n}', str(e >> 7), f'{e & 0x7F}.DAT')
    return None

def work(args):
    src, dst, retexture, race = args
    try:
        os.environ['RACE'] = race        # skin tone the HD pass leaves alone (skin.py)
        import numpy as np
        import tex, allgear
        tmp = tempfile.mkdtemp(prefix='allgear_')
        specs = []
        for name, img in tex.textures(src):
            key = name.split()[-1] or 'tex'
            rgba = allgear.process(img, zlib.crc32(key.encode()) & 0xFFFF)
            raw = os.path.join(tmp, f'{len(specs)}_{key}.rgba'); rgba.tofile(raw)
            specs.append(f'{key}={raw}:{rgba.shape[1]}x{rgba.shape[0]}')
        if not specs:
            return src, 'no textures'
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        r = subprocess.run([retexture, src, dst + '.part'] + specs, capture_output=True, text=True)
        if r.returncode != 0:
            return src, 'retexture: ' + (r.stderr or r.stdout).strip()[-300:]
        os.replace(dst + '.part', dst)
        for f in os.listdir(tmp): os.remove(os.path.join(tmp, f))
        os.rmdir(tmp)
        return src, None
    except Exception:
        return src, traceback.format_exc()[-400:]

if __name__ == '__main__':
    ap = argparse.ArgumentParser()
    ap.add_argument('--ffxi', required=True); ap.add_argument('--table', required=True)
    ap.add_argument('--out', required=True); ap.add_argument('--retexture', required=True)
    ap.add_argument('--workers', type=int, default=max(1, (os.cpu_count() or 2) - 1))
    ap.add_argument('--skip'); ap.add_argument('--limit', type=int)
    a = ap.parse_args()
    V, F = load_tables(a.ffxi)
    skip = set(open(a.skip).read().split()) if a.skip else set()
    race_of = {int(r['dat_id']): r['race'] for r in csv.DictReader(open(a.table)) if r['dat_id'] and r['dat_id'] not in skip}
    ids = sorted(race_of)
    jobs, missing = [], 0
    for fid in ids:
        rel = rom_path(V, F, fid)
        if not rel: missing += 1; continue
        src, dst = os.path.join(a.ffxi, rel), os.path.join(a.out, rel)
        if os.path.exists(dst) or not os.path.exists(src): continue
        jobs.append((src, dst, a.retexture, race_of[fid]))
    if a.limit: jobs = jobs[:a.limit]
    print(f'{len(ids)} DATs in table, {missing} not in this install, {len(jobs)} to do, {a.workers} workers', flush=True)
    done = fail = 0
    with open(os.path.join(a.out if os.path.isdir(a.out) else '.', 'failed.txt'), 'a') as flog, ProcessPoolExecutor(a.workers) as ex:
        for fut in as_completed([ex.submit(work, j) for j in jobs]):
            src, err = fut.result()
            if err: fail += 1; flog.write(f'{src}\t{err!r}\n'); flog.flush()
            else: done += 1
            if (done + fail) % 100 == 0 or done + fail == len(jobs):
                print(f'{done + fail}/{len(jobs)} done, {fail} failed', flush=True)
