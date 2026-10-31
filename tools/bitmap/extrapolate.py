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
# the probe's positions after one shift per image part; zero or several
# matches are errors. refbits picks n bits in every part.
import csv
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from layout import layout, load  # noqa: E402
from pack import write  # noqa: E402
from table import open_table  # noqa: E402


def layouts(subset_map):
    """[(layout key, {bit: (part, dframe, dbdata, polarity)})], one per distinct layout."""
    out = {}
    for bits in load(subset_map).values():
        key, _ = layout(bits)
        out.setdefault(key, {b: (part, da, dc, pol) for b, part, da, dc, pol in key})
    return list(out.items())


def refbits(subset_map, out, n=4):
    lays = [lay for _, lay in layouts(subset_map)]
    common = set(lays[0]).intersection(*lays[1:])
    picks = []
    for part in sorted({lays[0][b][0] for b in common}):
        # Spread each part's picks over the first layout's extent.
        ranked = sorted((b for b in common if lays[0][b][0] == part), key=lambda b: lays[0][b][1:3])
        step = max(1, len(ranked) // int(n))
        picks += ranked[::step][:int(n)]
    open(out, 'w').write('\n'.join(picks) + '\n')
    print(f'{len(lays)} layouts, {len(common)} common bits; picked {picks}')


def pack(subset_map, probe_map, prefix):
    lays = layouts(subset_map)
    probe = load(probe_map)
    anchors, bad = [], 0
    for u in sorted(probe):
        pos = probe[u]
        fits = []
        for lid, (_, lay) in enumerate(lays):
            if not all(b in lay for b in pos):
                continue
            parts = {}
            for b, (obj, part, a, c, _) in pos.items():
                lpart, da, dc, _ = lay[b]
                parts.setdefault(part, set()).add((lpart, obj, a - da, c - dc))
            if all(len(v) == 1 for v in parts.values()) and all(next(iter(v))[0] == p for p, v in parts.items()):
                anc = {p: next(iter(v))[1:] for p, v in parts.items()}
                if set(anc) == {pt for pt, *_ in lay.values()}:
                    fits.append((lid, anc))
        if len(fits) != 1:
            bad += 1
            print(f'{u}: {len(fits)} layouts fit', file=sys.stderr)
            continue
        anchors.append((u, *fits[0]))
    write(prefix, [key for key, _ in lays], anchors)
    print(f'{prefix}: {len(lays)} layouts, {len(anchors)} instances placed, {bad} unplaced')


def check(prefix, sample_map):
    lay = {}
    for r in csv.DictReader(open_table(prefix + '.layout.tsv'), delimiter='\t'):
        lay.setdefault(r['layout'], {})[r['bit']] = (r['part'], int(r['dframe']), int(r['dbdata']), r['polarity'])
    anc = {}
    for r in csv.DictReader(open_table(prefix + '.anchors.tsv'), delimiter='\t'):
        anc.setdefault(r['uid'], {})[r['part']] = r
    ok = wrong = missing = 0
    for u, bits in load(sample_map).items():
        a = anc.get(u)
        if a is None:
            missing += len(bits)
            continue
        L = lay[next(iter(a.values()))['layout']]
        for b, (o, p, fr, bd, pol) in bits.items():
            want = L.get(b)
            r = a.get(p)
            if want and r and want[0] == p and o == r['object'] and \
                    (fr, bd, str(pol)) == (int(r['frame']) + want[1], int(r['bdata']) + want[2], want[3]):
                ok += 1
            else:
                wrong += 1
    print(f'sample: {ok} bits predicted exactly, {wrong} wrong, {missing} from unplaced instances')


if __name__ == '__main__':
    {'refbits': refbits, 'pack': pack, 'check': check}[sys.argv[1]](*sys.argv[2:])
