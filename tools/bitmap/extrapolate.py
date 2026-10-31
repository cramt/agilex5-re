# Extend a block type's bit map from a fully mapped subset of instances to
# all of them, using a probe that mapped only a few reference bits for every
# instance (bitmap.tcl with uids "-" and a bits file).
#
#   python3 -I tools/bitmap/extrapolate.py refbits <subset map.tsv> <out bits.txt> [n]
#       pick n (default 4) reference bits present in every subset layout
#   python3 -I tools/bitmap/extrapolate.py pack <subset map.tsv> <probe map.tsv> <db prefix>
#       write <prefix>.layout.tsv / .anchors.tsv for every probed instance
#   python3 -I tools/bitmap/extrapolate.py check <db prefix> <sample map.tsv>
#       compare the packed prediction with an independent full map
#
# An instance's layout is the subset layout whose reference-bit offsets match
# the probe's positions after one shift; zero or several matches are errors.
import csv
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from layout import layout, load  # noqa: E402


def layouts(subset_map):
    out = {}
    for u, bits in load(subset_map).items():
        key, _ = layout(bits)
        out.setdefault(key, u)
    return [dict(((b, (da, dc, pol)) for b, da, dc, pol in key)) for key in out]


def refbits(subset_map, out, n=4):
    lays = layouts(subset_map)
    common = set(lays[0]).intersection(*lays[1:])
    # Spread the picks over the first layout's frame and bdata extent.
    ranked = sorted(common, key=lambda b: (lays[0][b][0], lays[0][b][1]))
    step = max(1, len(ranked) // int(n))
    picks = ranked[::step][:int(n)]
    open(out, 'w').write('\n'.join(picks) + '\n')
    print(f'{len(lays)} layouts, {len(common)} common bits; picked {picks}')


def pack(subset_map, probe_map, prefix):
    lays = layouts(subset_map)
    probe = load(probe_map)
    anchors, bad = [], 0
    for u in sorted(probe):
        pos = probe[u]
        objs = {(o, p) for o, p, *_ in pos.values()}
        fits = []
        for lid, lay in enumerate(lays):
            if not all(b in lay for b in pos):
                continue
            shifts = {(a - lay[b][0], c - lay[b][1]) for b, (_, _, a, c, _) in pos.items()}
            if len(shifts) == 1 and len(objs) == 1:
                fits.append((lid, *shifts.pop()))
        if len(fits) != 1:
            bad += 1
            print(f'{u}: {len(fits)} layouts fit', file=sys.stderr)
            continue
        lid, a0, c0 = fits[0]
        anchors.append((u, lid, *objs.pop(), a0, c0))
    os.makedirs(os.path.dirname(prefix), exist_ok=True)
    with open(prefix + '.layout.tsv', 'w') as f:
        f.write('layout\tbit\tdframe\tdbdata\tpolarity\n')
        for lid, lay in enumerate(lays):
            for b, (da, dc, pol) in sorted(lay.items()):
                f.write(f'{lid}\t{b}\t{da}\t{dc}\t{pol}\n')
    with open(prefix + '.anchors.tsv', 'w') as f:
        f.write('uid\tlayout\tobject\tpart\tframe\tbdata\n')
        for row in anchors:
            f.write('\t'.join(map(str, row)) + '\n')
    print(f'{prefix}: {len(lays)} layouts, {len(anchors)} instances placed, {bad} unplaced')


def check(prefix, sample_map):
    lay = {}
    for r in csv.DictReader(open(prefix + '.layout.tsv'), delimiter='\t'):
        lay.setdefault(r['layout'], {})[r['bit']] = (int(r['dframe']), int(r['dbdata']), r['polarity'])
    anc = {r['uid']: r for r in csv.DictReader(open(prefix + '.anchors.tsv'), delimiter='\t')}
    ok = wrong = missing = 0
    for u, bits in load(sample_map).items():
        a = anc.get(u)
        if a is None:
            missing += len(bits)
            continue
        L = lay[a['layout']]
        for b, (o, p, fr, bd, pol) in bits.items():
            want = L.get(b)
            if want and (o, p) == (a['object'], a['part']) and \
                    (fr, bd, str(pol)) == (int(a['frame']) + want[0], int(a['bdata']) + want[1], want[2]):
                ok += 1
            else:
                wrong += 1
    print(f'sample: {ok} bits predicted exactly, {wrong} wrong, {missing} from unplaced instances')


if __name__ == '__main__':
    {'refbits': refbits, 'pack': pack, 'check': check}[sys.argv[1]](*sys.argv[2:])
