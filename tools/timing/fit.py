# Routing and cell delay model from quartus_sta paths (README (u)).
#
#   python3 -I tools/timing/fit.py fit <hops.tsv>... > delays.tsv
#   python3 -I tools/timing/fit.py eval <delays.tsv> <hops.tsv>...
#
# fit: the mean delay of each hop class over the training paths, where a
# routing hop's class is its node type (the delay into that node) and a
# cell hop's is its kind: `lut:<input>` (LUT input a..f to combout, LAB or
# MLAB), `tco` (register clock to output), `reg_d` (combout into the
# register), `tsu`. eval: the data delay each path's hops sum to under the
# model, against quartus_sta's own data delay.
import collections
import re
import statistics
import sys


def paths(files):
    """path id -> (data delay, [(class, incr)])"""
    out = collections.defaultdict(lambda: [None, []])
    for fn in files:
        for line in open(fn):
            f = line.rstrip('\n').split('\t')
            if f[0] == 'P':
                out[f[1]][0] = float(f[2])
                continue
            _, pid, kind, incr, rf, name, etype, fanout = f
            incr = float(incr)
            if kind == 'RE':
                c = 're:' + name.split(':')[0]
                if etype in ('Combinational cell', 'ALM Register'):
                    continue  # a cell's own output node, always 0
            elif kind == 'uTco':
                c = 'tco'
            elif kind == 'uTsu':
                c = 'tsu'
            elif kind == 'IC':
                c = 'ic'
            else:
                m = re.search(r'\|(\w+)$', name)
                port = m.group(1) if m else ''
                cell = 'mlab' if 'MLAB' in etype or name.startswith('MLAB') else 'lab'
                if port == 'combout':
                    c = f'lut:{prev_in}'
                elif port in ('sumout', 'cout'):
                    c = f'arith:{prev_in}>{port}'
                elif port == 'd':
                    c = 'reg_d'
                elif port == 'cin':
                    # A carry chain hop: the carry enters as a cell port, not an IC.
                    c = 'carry'
                    prev_in = 'cin'
                elif port in ('q', ''):
                    c = 'cell:' + etype
                else:
                    c = f'cell:{etype}:{port}'
            if kind == 'IC':
                m = re.search(r'\|(\w+)$', name)
                prev_in = m.group(1) if m else ''
            out[pid][1].append((c, incr))
    return out


def data_hops(hops):
    """The data path's hops: from the first tco on (the clock path comes first)."""
    for i, (c, _) in enumerate(hops):
        if c == 'tco':
            return [h for h in hops[i:] if h[0] != 'tsu']
    return []


def main():
    if sys.argv[1] == 'fit':
        acc = collections.defaultdict(list)
        for d, hops in paths(sys.argv[2:]).values():
            for c, v in data_hops(hops):
                acc[c].append(v)
        print('class\tn\tmean_ns\tmin_ns\tmax_ns')
        for c in sorted(acc):
            v = acc[c]
            print(f'{c}\t{len(v)}\t{statistics.mean(v):.4f}\t{min(v):.3f}\t{max(v):.3f}')
    else:
        model = {}
        for line in open(sys.argv[2]):
            f = line.rstrip('\n').split('\t')
            if f[0] != 'class':
                model[f[0]] = float(f[2])
        errs, missing = [], collections.Counter()
        for pid, (d, hops) in paths(sys.argv[3:]).items():
            dh = data_hops(hops)
            if d is None or not dh:
                continue
            pred = 0.0
            for c, v in dh:
                if c not in model:
                    missing[c] += 1
                pred += model.get(c, v)
            errs.append((pred - d) / d)
        a = sorted(abs(e) for e in errs)
        print(f'{len(errs)} paths: mean error {statistics.mean(errs) * 100:+.1f} %, |error| median {a[len(a) // 2] * 100:.1f} %, '
              f'95th {a[int(len(a) * .95)] * 100:.1f} %, max {a[-1] * 100:.1f} %')
        if missing:
            print('classes not in the model (actual delay used):', dict(missing))


main()
