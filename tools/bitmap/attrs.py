# Decode a block instance's attributes from a .sof image using only the
# database: the packed bit map (db/<die>/bits), the BCM settings dump
# (tools/bcm/settings.tcl) and the BCM type tree (dump_device_for_viewer JSON).
#   python3 -I tools/bitmap/attrs.py <in.sof> <type> <uid> <bits prefix> <settings.tsv> <viewer.json>
# Prints "attribute<TAB>value". CUSTOM attributes resolve to the setting whose
# bit values match ("<v>" values are taken as don't-care); DIRECT ones print
# their bits in the order settings.tsv does not give, so they are skipped.
import collections
import csv
import json
import os
import re
import shlex
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sof  # noqa: E402
from imagemap import FRAME_BYTES, PARTS  # noqa: E402


def read_bits(img, prefix, uid):
    start = {(o, p): s for o, p, s, _ in PARTS}
    anchor = next(r for r in csv.DictReader(open(prefix + '.anchors.tsv'), delimiter='\t') if r['uid'] == uid)
    base = start[(anchor['object'], anchor['part'])]
    f0, d0 = int(anchor['frame']), int(anchor['bdata'])
    bits = {}
    for r in csv.DictReader(open(prefix + '.layout.tsv'), delimiter='\t'):
        if r['layout'] != anchor['layout']:
            continue
        f, d = f0 + int(r['dframe']), d0 + int(r['dbdata'])
        off, k = (base + f * FRAME_BYTES + d // 8, d % 8) if anchor['part'] == 'cram' else (base + f, d)
        v = img[off] >> k & 1
        bits[r['bit']] = v if r['polarity'] == '1' else 1 - v
    return bits


def main(sof_path, btype, uid, prefix, settings, viewer):
    img = sof.image(sof.load(sof_path))
    bits = read_bits(img, prefix, uid)
    types = json.load(open(viewer))['types']
    table = collections.defaultdict(list)
    for line in open(settings):
        i, a, t, s, b, v = line.rstrip('\n').split('\t')
        if t == 'CUSTOM' and b and b != '?':
            table[(i, a)].append((s, shlex.split(b.replace('{', '"').replace('}', '"')), v.split()))

    def walk(t, path):
        for a in types[t]['data'].get('custom', []):
            yield path, t, next(iter(a))
        for child, ct in types[t].get('childMap', {}).items():
            yield from walk(ct, path + child + '/')

    for path, leaf, attr in walk(btype, ''):
        found = []
        for s, names, vals in table.get((leaf, attr), []):
            ok = True
            for n, v in zip(names, vals):
                if v.startswith('<'):
                    continue
                got = bits.get(path + n)
                if got is None or got != int(v):
                    ok = False
                    break
            if ok:
                found.append(s)
        if table.get((leaf, attr)):
            print(f'{path}{attr}\t{found[0] if len(found) == 1 else "|".join(found) or "?"}')


if __name__ == '__main__':
    main(*sys.argv[1:])
