# Add aliased bits (several names, one image bit; mapped one at a time by
# single.tcl) to a packed layout, or check that another instance agrees.
#   python3 -I tools/bitmap/aliases.py add   <db prefix> <single map.tsv>
#   python3 -I tools/bitmap/aliases.py check <db prefix> <single map.tsv>
# The offsets come from the mapped instance's own anchors, so the rows hold
# for every instance of the (single) layout.
import csv
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from layout import load  # noqa: E402
from table import open_table, write_table  # noqa: E402


def offsets(prefix, single_map):
    anc = {}
    for r in csv.DictReader(open_table(prefix + '.anchors.tsv'), delimiter='\t'):
        anc.setdefault(r['uid'], {})[r['part']] = r
    rows = []
    for u, bits in load(single_map).items():
        for b, (obj, part, a, c, pol) in bits.items():
            r = anc[u][part]
            assert r['object'] == obj
            rows.append((r['layout'], b, part, a - int(r['frame']), c - int(r['bdata']), pol))
    return rows


def add(prefix, single_map):
    path = prefix + '.layout.tsv'
    with open_table(path) as f:
        text = f.read()
    have = {(r['layout'], r['bit']) for r in csv.DictReader(text.splitlines(), delimiter='\t')}
    new = [r for r in offsets(prefix, single_map) if (r[0], r[1]) not in have]
    write_table(path, text + ''.join('\t'.join(map(str, r)) + '\n' for r in new))
    print(f'{path}: {len(new)} aliased bits added')


def check(prefix, single_map):
    lay = {(r['layout'], r['bit']): (r['part'], int(r['dframe']), int(r['dbdata']), int(r['polarity']))
           for r in csv.DictReader(open_table(prefix + '.layout.tsv'), delimiter='\t')}
    rows = offsets(prefix, single_map)
    ok = sum(lay.get((l, b)) == (p, df, dd, pol) for l, b, p, df, dd, pol in rows)
    print(f'{ok} of {len(rows)} aliased bits at the predicted offsets')


if __name__ == '__main__':
    {'add': add, 'check': check}[sys.argv[1]](*sys.argv[2:])
