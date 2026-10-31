# Decode single.tcl's images: every image bit each named bit flips.
#   python3 tools/bitmap/single.py <base.sof> <outdir>   (needs numpy)
# Writes <outdir>/map.tsv in bitmap.py's format; aliased bits (several
# names, one image bit) come out as several rows with the same address.
import os
import sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sof  # noqa: E402
from imagemap import addr  # noqa: E402


def main(base_path, outdir):
    base = sof.image(sof.load(base_path))
    base_np = np.frombuffer(base, np.uint8)
    rows = multi = none = 0
    with open(os.path.join(outdir, 'pairs.txt')) as f, open(os.path.join(outdir, 'map.tsv'), 'w') as out:
        for line in f:
            n, u, b, v = line.split()
            img = np.frombuffer(sof.image(sof.load(os.path.join(outdir, f'{n}.sof'))), np.uint8)
            pos = np.flatnonzero(np.unpackbits(img ^ base_np, bitorder='little')).tolist()
            none += not pos
            multi += len(pos) > 1
            for p in pos:
                o, k = divmod(p, 8)
                out.write('\t'.join(map(str, (u, b, v, *addr(o, k), int((base[o] >> k & 1) == int(v))))) + '\n')
                rows += 1
    print(f'{rows} rows; {none} bits change nothing, {multi} change several image bits')


if __name__ == '__main__':
    main(*sys.argv[1:])
