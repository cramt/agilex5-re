# Decode the images bitmap.tcl wrote into a CRAM address for every
# (instance, bit) pair:
#   tools/bitmap/run.sh, or: python3 tools/bitmap/bitmap.py <base.sof> <outdir> (needs numpy)
# Writes <outdir>/map.tsv: uid, bit, value in the base, image object, part,
# then (frame, bdata) for CRAM (bdata = byte * 8 + bit in the 4320-bit
# frame) or (byte, bit) elsewhere, and whether the image bit equals the
# named bit (1) or its inverse (0).
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
    pairs_path = os.path.join(outdir, 'pairs.txt')
    with open(pairs_path) as f:
        width = int(f.readline().split('width=')[1])
    # Dense per-image-bit code arrays: an image's changed positions are
    # distinct, so OR-ing 1 << k into them by fancy indexing is exact.
    nbits = len(base) * 8
    codes = {want: np.zeros(nbits, np.uint32) for want in (1, 0)}
    for want in (1, 0):
        for k in range(width):
            img = np.frombuffer(sof.image(sof.load(os.path.join(outdir, f'{k}.{want}.sof'))), np.uint8)
            p = np.flatnonzero(np.unpackbits(img ^ base_np, bitorder='little'))
            codes[want][p] |= np.uint32(1 << k)
    one, zero = codes[1], codes[0]
    touched = np.flatnonzero(one | zero)
    full = (1 << width) - 1
    a, z = one[touched].astype(np.int64), zero[touched].astype(np.int64)
    good = ((a & z) == 0) & ((a | z) == full)
    clash = int((~good).sum())
    ids, pos = a[good], touched[good]
    order = np.argsort(ids, kind='stable')
    ids, pos = ids[order].tolist(), pos[order].tolist()
    del codes, one, zero, touched, a, z, good
    # pairs.txt is in id order: merge it with the decoded (id, position) list.
    n_pairs = mapped = 0
    i = 0
    with open(pairs_path) as f, open(os.path.join(outdir, 'map.tsv'), 'w') as out:
        f.readline()
        for line in f:
            n_pairs += 1
            pid, u, b, v = line.split()
            pid, v = int(pid), int(v)
            while i < len(ids) and ids[i] < pid:
                i += 1
            hit = False
            while i < len(ids) and ids[i] == pid:
                o, k = divmod(pos[i], 8)
                out.write('\t'.join(map(str, (u, b, v, *addr(o, k), int((base[o] >> k & 1) == v)))) + '\n')
                hit = True
                i += 1
            mapped += hit
    print(f'{n_pairs} pairs, {len(ids)} CRAM bits decoded, {mapped} pairs mapped, '
          f'{clash} CRAM bits with a clash')


if __name__ == '__main__':
    main(*sys.argv[1:])
