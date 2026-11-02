"""The vs-Quartus-Pro tables (docs/vs-quartus-pro.md) from a bench run:
   python3 -I tools/bench/table.py <bench out dir>
Per design and tool: median wall and CPU time over the reps with [min-max],
the median load, and from rep 1 the resources (each tool's own count after
fitting) and Fmax: quartus_sta on Quartus' fit; for a5pnr quartus_sta on
Quartus' routes of a5pnr's placement (--quartus) where the export takes the
design, else a5pnr's own model on its own routes (marked "model")."""
import os, re, statistics, sys
out = sys.argv[1]
rows = [l.rstrip('\n').split('\t') for l in open(f'{out}/runs.tsv')]
hdr, rows = rows[0], [dict(zip(rows[0], r)) for r in rows[1:]]
designs = list(dict.fromkeys(r['design'] for r in rows))

def med(xs):
    xs = sorted(xs)
    return statistics.median(xs), xs[0], xs[-1]

def fmax(path):
    try:
        t = open(path).read()
    except OSError:
        return None
    m = re.search(r';\s*([\d.]+) MHz\s*;\s*([\d.]+) MHz\s*;\s*CLOCK1_50', t)
    return (float(m[1]), float(m[2])) if m else None

def quartus_res(run):
    name = os.path.basename(run).split('_r')[0][2:]
    try:
        t = open(f'{run}/output_files/{name}.fit.summary').read() + open(f'{run}/output_files/{name}.fit.rpt').read()
    except OSError:
        return {}
    g = lambda pat: (re.search(pat, t) or [None, '?'])[1].replace(',', '')
    return {'alms': g(r'Logic utilization \(in ALMs\) : ([\d,]+)'), 'labs': g(r'; Total LABs:\s+partially or completely used\s+; ([\d,]+)'),
            'm20k': g(r'; M20K blocks\s+; ([\d,]+)'), 'dsp': g(r'Total DSP Blocks : ([\d,]+)'), 'luts': g(r'; Combinational ALUT usage for logic\s+; ([\d,]+)')}

def a5pnr_res(run):
    try:
        t = open(f'{run}/pnr.log').read()
    except OSError:
        return {}
    g = lambda pat: (re.search(pat, t) or [None, '?'])[1]
    return {'alms': g(r'a5pnr: (\d+) ALMs'), 'labs': g(r'ALMs in (\d+) LABs'), 'm20k': g(r'(\d+) M20Ks, '), 'dsp': g(r'(\d+) DSPs, '),
            'model': g(r'model Fmax (\d+) MHz'), 'r2r': g(r'register to register [\d.]+ ns, (\d+) MHz')}

print('| design | tool | wall s, median [min-max] | CPU s | load | ALMs | LABs | M20K | DSP | Fmax (MHz) |')
print('|---|---|---|---|---|---|---|---|---|---|')
speed = []
for d in designs:
    q = [r for r in rows if r['design'] == d and r['tool'] == 'quartus' and r['status'] == '0']
    a = [r for r in rows if r['design'] == d and r['tool'] == 'a5pnr' and r['status'] == '0']
    for tool, rs in (('Quartus Pro', q), ('a5pnr', a)):
        if not rs:
            print(f'| {d} | {tool} | (failed) | | | | | | | |')
            continue
        w = med([float(r['wall_s']) for r in rs]); c = med([float(r['cpu_s']) for r in rs]); ld = statistics.median(float(r['load1']) for r in rs)
        r1 = next((r for r in rs if r['rep'] == '1'), rs[0])
        if tool == 'Quartus Pro':
            res = quartus_res(r1['dir']); f = fmax(f'{out}/sta/q_{d}/fmax.txt')
            fs = f'{f[0]:.0f} (restricted {f[1]:.0f})' if f else '?'
        else:
            res = a5pnr_res(r1['dir']); f = fmax(f'{out}/sta/x_{d}/fmax.txt')
            fs = f'{f[0]:.0f} (restricted {f[1]:.0f}), model {res.get("model", "?")}' if f else f'model {res.get("model", "?")}'
        print(f'| {d} | {tool} | {w[0]:.0f} [{w[1]:.0f}-{w[2]:.0f}] | {c[0]:.0f} [{c[1]:.0f}-{c[2]:.0f}] | {ld:.0f} | {res.get("alms", "?")} | {res.get("labs", "?")} | {res.get("m20k", "?")} | {res.get("dsp", "?")} | {fs} |')
    if q and a:
        speed.append((d, med([float(r['wall_s']) for r in q])[0] / med([float(r['wall_s']) for r in a])[0], med([float(r['cpu_s']) for r in q])[0] / max(1e-9, med([float(r['cpu_s']) for r in a])[0])))
print()
print('| design | wall-time speed-up (Quartus / a5pnr) | CPU-time ratio |')
print('|---|---|---|')
for d, w, c in speed:
    print(f'| {d} | {w:.1f}x | {c:.1f}x |')
