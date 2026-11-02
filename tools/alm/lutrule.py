# Check the LUT rule (README (z3)) on a Quartus image of designs/fr: for every
# register whose D is one fitted LUT reading only registers, the RTL's truth
# table (fr.v's `t<i> = <mask> >> {inputs}`) against the image's stored mask
# read as the silicon computes it:
#   LUTOUT_5LUT, half h:  S[32h + {E, D, C, B, A}]
#   LUTOUT_6LUT:         ~S[32 * ~F + {E, D, C, B, A}]
#   python3 -I tools/alm/lutrule.py atoms.txt extract.txt designs/fr/fr.v
import itertools
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
S, attrs = {}, {}
for l in open(sys.argv[2]):
    f = l.rstrip('\n').split('\t')
    m = re.match(r'cw_(?:la_lab|ml_mlab)_r(\d+)_c(\d+)', f[1]) if len(f) > 3 else None
    if not m:
        continue
    if f[0] == 'direct' and f[2].endswith('lut_mask'):
        k = int(re.search(r'xale(\d)', f[2])[1]) + (5 if 'bothalf' in f[2] else 0)
        S[(int(m[2]), int(m[1]), k)] = int(f[3], 2)
    elif f[0] == 'attr':
        attrs[(int(m[2]), int(m[1]), f[2])] = f[3]
rtl = open(sys.argv[3]).read()
tdef = {m[1]: (int(m[2], 16), [s.strip() for s in m[3].split(',')])
        for m in re.finditer(r"wire \[\d+:0\] (t\d+) = \d+'h([0-9a-f]+) >> \{([^}]*)\};", rtl)}
dreg = {m[1]: m[2] for m in re.finditer(r"(r\[\d+\]) <= (t\d+)\[0\];", rtl)}


def xyn(a):
    m = re.search(r'X(\d+)_Y(\d+)_N(\d+)$', atoms[a]['loc'])
    return tuple(map(int, m.groups()))


def base(n):
    return re.sub(r'~DUPLICATE(_\d+)?$', '', n)


res = Counter()
for a, at in atoms.items():
    if at['type'] != 'FF' or '~' in a or a not in dreg:
        continue
    d = [s for p, s in at['ins'] if p == 'D']
    if not d or atoms.get(d[0], {}).get('type') != 'LCELL_COMB':
        continue
    data = [(p[-1], s) for p, s in atoms[d[0]]['ins'] if p.startswith('DATA')]
    # Retimed flops (~ERTM) don't hold the register they are named after.
    if any('ERTM' in s or atoms.get(s, {}).get('type') != 'FF' for _, s in data):
        continue
    ports = {p: base(s) for p, s in data}
    mask, ins = tdef[dreg[a]]
    if set(ins) != set(ports.values()):
        res['skipped: Quartus rewrote the inputs'] += 1
        continue
    fx, fy, fn = xyn(a)
    i, r = divmod(fn, 3)
    k, h = divmod(i, 2)
    path = (f'xtophalf/xale{k}' if k < 5 else f'xbothalf/xale{k - 5}') + f'/xcw_la_le_reg{h}/xreg{r - 1}/reg_input'
    out = attrs.get((fx, fy, path))
    if out not in ('LUTOUT_5LUT', 'LUTOUT_6LUT'):
        res[f'skipped: flop reads {out} (the LUT sits elsewhere)'] += 1
        continue
    s = S.get((fx, fy, k), (1 << 64) - 1)
    ok = True
    for vals in itertools.product([0, 1], repeat=len(ins)):
        sig = dict(zip(ins, vals))
        want = mask >> sum(sig[x] << (len(ins) - 1 - j) for j, x in enumerate(ins)) & 1
        v = {p: 0 for p in 'ABCDEF'}
        v.update({p: sig[x] for p, x in ports.items()})
        idx = v['A'] | v['B'] << 1 | v['C'] << 2 | v['D'] << 3 | v['E'] << 4
        got = s >> (32 * h + idx) & 1 if out == 'LUTOUT_5LUT' else 1 - (s >> (32 * (1 - v['F']) + idx) & 1)
        ok &= got == want
    res[(out, 'agrees' if ok else 'DIFFERS')] += 1
for k, v in sorted(res.items(), key=str):
    print(v, k)
