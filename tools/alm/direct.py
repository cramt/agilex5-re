# Which ALM output drives each direct LAB output (LAB_RE 129..148) of a
# Quartus image (README (z3)): match each pin's consumers in the rebuilt
# netlist (a5pnr readback) with the consumers of each fitted comb atom
# (tools/alm/atoms.tcl).
#   python3 -I tools/alm/direct.py atoms.txt readback.v
import re
import sys
from collections import defaultdict

atoms, cur = {}, None
for l in open(sys.argv[1]):
    f = l.split()
    if f[0] == 'A':
        cur = f[1]
        atoms[cur] = {'type': f[2], 'loc': f[3], 'ins': []}
    elif f[0] == 'I' and cur:
        atoms[cur]['ins'].append((f[1], f[2]))


def where(a):
    m = re.fullmatch(r'(?:MLABCELL|LABCELL|FF)_X(\d+)_Y(\d+)_N(\d+)', atoms[a]['loc'])
    if not m:
        return None
    x, y, n = map(int, m.groups())
    return x, y, n


# Consumers (x, y, ALM) of every comb atom, other than its own ALM.
cons = defaultdict(set)
for a, at in atoms.items():
    w = where(a)
    if not w:
        continue
    x, y, n = w
    k = n // 6
    for port, src in at['ins']:
        if src in atoms and atoms[src]['type'] == 'LCELL_COMB' and port != 'CLK':
            sw = where(src)
            if sw and (sw[0], sw[1], sw[2] // 6) != (x, y, k):
                cons[src].add((x, y, k))
# Consumers of every direct pin in the readback.
pcons = defaultdict(set)
for l in open(sys.argv[2]):
    m = re.match(r'\s*assign l(\d+)_(\d+)_a(\d+)_\w+ = (.*);', l)
    if not m:
        continue
    for x, y, p in re.findall(r'l(\d+)_(\d+)_o(\d+)', m[4]):
        if 129 <= int(p) <= 148:
            pcons[(int(x), int(y), int(p))].add((int(m[1]), int(m[2]), int(m[3])))
by_lab = defaultdict(list)
for a in cons:
    x, y, n = where(a)
    by_lab[(x, y)].append(a)
for (x, y, p), s in sorted(pcons.items()):
    best = sorted(by_lab[(x, y)], key=lambda a: -len(cons[a] & s) + 0.01 * len(cons[a] ^ s))
    a = best[0] if best else None
    if a is None:
        print(f'X{x}Y{y} I{p}: no atom')
        continue
    n = where(a)[2]
    nin = len([1 for port, _ in atoms[a]['ins'] if port.startswith('DATA')])
    ports = ''.join(port[-1] for port, _ in atoms[a]['ins'] if port.startswith('DATA'))
    print(f'X{x}Y{y} I{p} p-129={p - 129:2}: N{n:2} ALM {n // 6} half {n // 3 % 2} inputs {nin} {ports:6} match {len(cons[a] & s)}/{len(s)} (atom fans out to {len(cons[a])}) {a}')
