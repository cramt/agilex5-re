# Routing and cell delay model from quartus_sta paths (README (u), (z7)).
#
#   python3 -I tools/timing/fit.py fit <hops.tsv>... > delays.tsv
#   python3 -I tools/timing/fit.py eval [--sta old|new] [--worst] <delays.tsv> <hops.tsv>...
#
# fit: the mean delay of each hop class over the training paths, where a
# routing hop's class is its node type (the delay into that node) and a
# cell hop's is its kind: `lut:<input>` (LUT input a..f to combout, LAB or
# MLAB), `tco` (register clock to output), `reg_d` (combout into the
# register), `tsu`. Carry-chain arcs are classed by where they run, as
# quartus_sta reports them (README (z7)): `arith:<in>>cout` within an ALM
# (`:e` at a half-LAB's last ALM, 4 or 9, which is slower),
# `arith:data>cout:la<d>` a data input's look-ahead d ALMs on to that last
# ALM, `arith:cin>cout:skip` a half-LAB's carry-in to its last ALM,
# `arith:<in>>sumout` (`:x` from half 0's data into half 1's sum) and
# `carry:lab` the carry between LABs.
#
# eval: the data delay each path's hops sum to under the model, against
# quartus_sta's own. With --sta, each carry-chain section of a path (its
# data input to the sum it ends in) is priced the way a5pnr's STA strings
# the arcs instead of hop by hop: `old` is phase 8's (data > cout at the
# entry half, cin > cout per half after it), `new` the per-ALM arcs with
# look-ahead (src/sta.rs). --worst keeps each endpoint's worst path only,
# the one an STA has to match.
import collections
import re
import statistics
import sys


def cell_pos(loc):
    """(x, y, alm, half) of a LABCELL/MLABCELL location, else None."""
    m = re.search(r'LABCELL_X(\d+)_Y(\d+)_N(\d+)', loc or '')
    if not m:
        return None
    x, y, n = map(int, m.groups())
    return x, y, n // 6, (n % 6) // 3


def paths(files):
    """path id -> (data delay, [(class, incr, port, pos, name)])"""
    out = collections.defaultdict(lambda: [None, []])
    for fn in files:
        prev_in, prev_pos = '', None
        for line in open(fn):
            f = line.rstrip('\n').split('\t')
            if f[0] == 'P':
                out[f[1]][0] = float(f[2])
                continue
            _, pid, kind, incr, rf, name, etype, fanout = f[:8]
            loc = f[8] if len(f) > 8 else ''
            incr = float(incr)
            pos = cell_pos(loc)
            port = ''
            if kind == 'RE':
                c = 're:' + name.split(':')[0]
                if etype in ('Combinational cell', 'ALM Register'):
                    continue  # a cell's own output node, always 0
                # The LAB's carry out (I88) into the LAB below.
                if re.fullmatch(r'LAB_RE:X\d+Y\d+Z0I88', name):
                    c = 'carry:lab'
            elif kind == 'uTco':
                c = 'tco'
            elif kind == 'uTsu':
                c = 'tsu'
            elif kind == 'IC':
                c = 'ic'
            else:
                m = re.search(r'\|(\w+)$', name)
                port = m.group(1) if m else ''
                if port == 'combout':
                    c = f'lut:{prev_in}'
                elif port in ('sumout', 'cout'):
                    c = arith_class(prev_in, prev_pos, port, pos)
                elif port == 'd':
                    c = 'reg_d'
                elif port == 'cin':
                    # A carry chain hop: the carry enters as a cell port, not an IC.
                    c = 'carry'
                    prev_in, prev_pos = 'cin', pos
                elif port in ('q', ''):
                    c = 'cell:' + etype
                else:
                    c = f'cell:{etype}:{port}'
            if kind == 'IC':
                m = re.search(r'\|(\w+)$', name)
                prev_in = m.group(1) if m else ''
                prev_pos = pos
                port = prev_in
            out[pid][1].append((c, incr, port, pos, name))
    return out


def arith_class(inp, src, port, dst):
    """A carry-chain cell arc's class (README (z7))."""
    if src is None or dst is None or src[:2] != dst[:2]:
        return f'arith:{inp}>{port}'
    d = dst[2] - src[2]
    end = dst[2] % 5 == 4
    if port == 'sumout':
        return f'arith:{inp}>sumout' + (':x' if inp != 'cin' and dst[3] != src[3] else '')
    if inp == 'cin':
        return 'arith:cin>cout:skip' if d else 'arith:cin>cout' + (':e' if end else '')
    if d:
        return f'arith:data>cout:la{d}'
    return f'arith:{inp}>cout' + (':e' if end else '')


def data_hops(hops):
    """The data path's hops: from the first tco on (the clock path comes first)."""
    for i, h in enumerate(hops):
        if h[0] == 'tco':
            return [x for x in hops[i:] if x[0] != 'tsu']
    return []


def load(fn):
    model = {}
    for line in open(fn):
        f = line.rstrip('\n').split('\t')
        if f[0] != 'class':
            model[f[0]] = float(f[2])
    return model


def chain_sections(dh):
    """(first, last) hop indices of each carry-chain section: from the IC
    into a chain cell's data input to the sum it ends in."""
    out, start = [], None
    for i, (c, _, port, pos, _) in enumerate(dh):
        if c == 'ic' and port.startswith('data') and pos and i + 1 < len(dh) and dh[i + 1][0].startswith('arith:'):
            start = i
        if start is not None and c.startswith('arith:') and c.split('>')[1].startswith('sumout'):
            out.append((start, i))
            start = None
        elif start is not None and not (c.startswith('arith:') or c in ('carry', 'carry:lab', 'ic', 're:LAB_RE')):
            start = None
    return out


def lab_index(pos, top):
    """Chain order across LABs: a chain runs down a column."""
    return top - pos[1]


def sta_chain(model, mode, entry, letter, exit_):
    """a5pnr's STA estimate from a data input at entry (x, y, alm, half) to
    the sum at exit_, both in one chain."""
    get = lambda k, alt=None: model.get(k, model.get(alt, 0.0)) if alt else model.get(k, 0.0)
    top = entry[1]
    lin = lambda p: 20 * lab_index(p, top) + 2 * p[2] + p[3]
    a, b = lin(entry), lin(exit_)
    if mode == 'old':
        if a == b:
            return get(f'arith:{letter}>sumout')
        return get(f'arith:{letter}>cout') + (b - a - 1) * get('arith:cin>cout') + get('arith:cin>sumout')
    # new: per-ALM couts with look-ahead (src/sta.rs).
    ea, eb = 10 * lab_index(entry, top) + entry[2], 10 * lab_index(exit_, top) + exit_[2]
    if ea == eb:
        return get(f'arith:{letter}>sumout' + (':x' if exit_[3] != entry[3] else ''), f'arith:{letter}>sumout')
    arr = {}
    k = entry[2]
    arr[ea] = get(f'arith:{letter}>cout' + (':e' if k % 5 == 4 else ''), f'arith:{letter}>cout')
    for e in (4, 9):
        if e > k:
            arr[ea + e - k] = max(arr.get(ea + e - k, 0.0), get(f'arith:data>cout:la{e - k}'))
    for j in range(ea + 1, eb):
        into = arr[j - 1] + (get('carry:lab') if j % 10 == 0 else 0.0)
        arr[j] = max(arr.get(j, 0.0), into + get('arith:cin>cout' + (':e' if j % 5 == 4 else ''), 'arith:cin>cout'))
    return arr[eb - 1] + (get('carry:lab') if eb % 10 == 0 else 0.0) + get('arith:cin>sumout')


def main():
    if sys.argv[1] == 'fit':
        acc = collections.defaultdict(list)
        for d, hops in paths(sys.argv[2:]).values():
            for c, v, *_ in data_hops(hops):
                acc[c].append(v)
            # Setup is on the required side of the path.
            acc['tsu'] += [v for c, v, *_ in hops if c == 'tsu']
        print('class\tn\tmean_ns\tmin_ns\tmax_ns')
        for c in sorted(acc):
            v = acc[c]
            print(f'{c}\t{len(v)}\t{statistics.mean(v):.4f}\t{min(v):.3f}\t{max(v):.3f}')
        return
    args = sys.argv[2:]
    mode, worst = None, False
    while args[0].startswith('--'):
        if args[0] == '--sta':
            mode, args = args[1], args[2:]
        elif args[0] == '--worst':
            worst, args = True, args[1:]
    model = load(args[0])
    errs, missing, chains = [], collections.Counter(), 0
    seen = set()
    for pid, (d, hops) in paths(args[1:]).items():
        dh = data_hops(hops)
        if d is None or not dh:
            continue
        # quartus_sta lists an endpoint's paths worst first.
        endpoint = (pid.split('#')[0], next((h[4] for h in reversed(dh) if h[0] == 'reg_d'), pid))
        if worst:
            if endpoint in seen:
                continue
            seen.add(endpoint)
        skip = set()
        pred = 0.0
        if mode:
            for s, e in chain_sections(dh):
                letter = dh[s][2]
                pred += sta_chain(model, mode, dh[s][3], letter, dh[e][3])
                skip.update(range(s + 1, e + 1))
                chains += 1
        for i, (c, v, *_) in enumerate(dh):
            if i in skip:
                continue
            if c not in model:
                missing[c] += 1
            pred += model.get(c, v)
        errs.append((pred - d) / d)
    a = sorted(abs(e) for e in errs)
    print(f'{len(errs)} paths{f" ({chains} chain sections strung as the {mode} STA)" if mode else ""}: mean error {statistics.mean(errs) * 100:+.1f} %, '
          f'|error| median {a[len(a) // 2] * 100:.1f} %, 95th {a[int(len(a) * .95)] * 100:.1f} %, max {a[-1] * 100:.1f} %')
    if missing:
        print('classes not in the model (actual delay used):', dict(missing))


main()
