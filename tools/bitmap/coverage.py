# How much of a .sof image the database names: set bits that are named
# bits, set bits that aren't, per image part.
#   python3 tools/bitmap/coverage.py <in.sof> [db/sm4revb]   (needs numpy)
import collections
import csv
import glob
import os
import sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sof  # noqa: E402
from imagemap import FRAME_BYTES, PARTS  # noqa: E402
from table import open_table  # noqa: E402


def covered_mask(dbdir, nbits):
    start = {(o, p): s for o, p, s, _ in PARTS}
    cov = np.zeros(nbits, bool)
    for anchors in sorted(glob.glob(os.path.join(dbdir, 'bits', '*.anchors.tsv'))):
        prefix = anchors[:-len('.anchors.tsv')]
        lay = collections.defaultdict(lambda: collections.defaultdict(list))
        for r in csv.DictReader(open_table(prefix + '.layout.tsv'), delimiter='\t'):
            lay[r['layout']][r['part']].append((int(r['dframe']), int(r['dbdata'])))
        for a in csv.DictReader(open_table(anchors), delimiter='\t'):
            L = np.array(lay[a['layout']][a['part']])
            base = start[(a['object'], a['part'])] * 8
            if a['part'] == 'cram':
                pos = base + (int(a['frame']) + L[:, 0]) * FRAME_BYTES * 8 + int(a['bdata']) + L[:, 1]
            else:
                pos = base + int(a['frame']) + L[:, 0]
            cov[pos] = True
    return cov


def main(path, dbdir='db/sm4revb'):
    img = np.frombuffer(sof.image(sof.load(path)), np.uint8)
    bits = np.unpackbits(img, bitorder='little').astype(bool)
    cov = covered_mask(dbdir, len(bits))
    print('object part: set bits, named among them, set but unnamed, named positions')
    for o, p, s, size in PARTS:
        a, b = s * 8, (s + size) * 8
        sb, cv = bits[a:b], cov[a:b]
        print(f'{o:10s} {p:5s}: {sb.sum():9d} {(sb & cv).sum():9d} {(sb & ~cv).sum():7d} {cv.sum():10d} / {b - a}')


if __name__ == '__main__':
    main(*sys.argv[1:])
