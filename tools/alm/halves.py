# Which ALM pins each half's LUT reads in a Quartus image (README (z3)):
# for every fitted comb atom (tools/alm/atoms.tcl), its flop fanins as image
# flops (LAB_RE 89 + 4k + 2h + r, tools/alm/ffmap.py's rule), against the
# pins a5pnr readback gives the atom's ALM.
#   python3 -I tools/alm/halves.py atoms.txt readback.v
import re
import sys
from collections import Counter

atoms, cur = {}, None
for l in open(sys.argv[1]):
    f = l.split()
    if f[0] == 'A':
        cur = f[1]
        atoms[cur] = {'type': f[2], 'loc': f[3], 'ins': []}
    elif f[0] == 'I' and cur:
        atoms[cur]['ins'].append((f[1], f[2]))


def loc(a):
    m = re.fullmatch(r'(?:MLABCELL|LABCELL|FF)_X(\d+)_Y(\d+)_N(\d+)', atoms[a]['loc'])
    return tuple(map(int, m.groups())) if m else None


def ffsig(a):
    x, y, n = loc(a)
    i, r = divmod(n, 3)
    k, h = divmod(i, 2)
    return f'l{x}_{y}_o{89 + 4 * k + 2 * h + r - 1}'


# readback: ALM -> pin -> source, from the lo (E0 D0 C0 B A) and hi5
# (E1 D1 C1 B A) lines.
pins = {}
for l in open(sys.argv[2]):
    m = re.match(r'\s*assign l(\d+)_(\d+)_a(\d+)_(lo|hi5) = lut5\(\S+, \{(.*)\}\);', l)
    if m:
        srcs = [s.strip() for s in m[5].split(',')]
        names = ['E0', 'D0', 'C0', 'B', 'A'] if m[4] == 'lo' else ['E1', 'D1', 'C1', 'B', 'A']
        pins.setdefault((int(m[1]), int(m[2]), int(m[3])), {}).update(zip(names, srcs))
stats = Counter()
for a, at in atoms.items():
    if at['type'] != 'LCELL_COMB' or not loc(a):
        continue
    x, y, n = loc(a)
    k, h = n // 6, n // 3 % 2
    p = pins.get((x, y, k))
    if p is None:
        stats['no readback ALM'] += 1
        continue
    ports = {}
    for port, src in at['ins']:
        if port.startswith('DATA') and src in atoms and atoms[src]['type'] == 'FF' and loc(src):
            ports[port[-1]] = ffsig(src)
    nin = sum(1 for port, _ in at['ins'] if port.startswith('DATA'))
    where = {}
    for port, s in ports.items():
        where[port] = ''.join(sorted(name for name, v in p.items() if v == s)) or '-'
    stats[(f'half {h}', nin, tuple(sorted(where.items())))] += 1
for k, v in sorted(stats.items(), key=lambda kv: -kv[1])[:60]:
    print(v, k)
