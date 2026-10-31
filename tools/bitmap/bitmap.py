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
import sof  # noqa: E402

# sm4revb image layout from sof_meta.pb (README (g)): object, part, start, size.
PARTS = [
    ('perictrl0', 'hdr', 0, 902), ('perictrl0', 'data', 902, 4194304),
    ('io96_0', 'hdr', 4195206, 5855), ('io96_0', 't40', 4201061, 16384), ('io96_0', 't41', 4217445, 529408),
    ('perictrl2', 'hdr', 4746853, 902), ('perictrl2', 'data', 4747755, 4194304),
    ('io96_1', 'hdr', 8942059, 5855), ('io96_1', 't40', 8947914, 16384), ('io96_1', 't41', 8964298, 529408),
    ('perictrl1', 'hdr', 9493706, 2658), ('perictrl1', 'data', 9496364, 4194304),
    ('sector0', 'hdr', 13690668, 412), ('sector0', 'cram', 13691080, 2540160), ('sector0', 'ram', 16231240, 491520),
    ('sector1', 'hdr', 16722760, 284), ('sector1', 'cram', 16723044, 2540160), ('sector1', 'ram', 19263204, 491520),
]
FRAME_BYTES = 540


def addr(off, k):
    """(object, part, a, b): CRAM is (frame, bdata); anything else (byte, bit)."""
    for obj, part, start, size in PARTS:
        r = off - start
        if 0 <= r < size:
            if part == 'cram':
                fr, by = divmod(r, FRAME_BYTES)
                return obj, part, fr, by * 8 + k
            return obj, part, r, k
    return '-', '-', off, k


def main(base_path, outdir):
    base = sof.image(sof.load(base_path))
    base_np = np.frombuffer(base, np.uint8)
    pairs = {}
    with open(os.path.join(outdir, 'pairs.txt')) as f:
        width = int(f.readline().split('width=')[1])
        for line in f:
            i, u, b, v = line.split()
            pairs[int(i)] = (u, b, int(v))
    # For every image, the positions (byte * 8 + bit) that differ from the
    # base; each position collects its code bits by summing 1 << k (the k
    # are distinct, so the sum is the OR).
    codes = {}
    for want in (1, 0):
        pos, val = [], []
        for k in range(width):
            img = np.frombuffer(sof.image(sof.load(os.path.join(outdir, f'{k}.{want}.sof'))), np.uint8)
            p = np.flatnonzero(np.unpackbits(img ^ base_np, bitorder='little'))
            pos.append(p)
            val.append(np.full(len(p), 1 << k, np.int64))
        pos, val = np.concatenate(pos), np.concatenate(val)
        uniq, inv = np.unique(pos, return_inverse=True)
        acc = np.zeros(len(uniq), np.int64)
        np.add.at(acc, inv, val)
        codes[want] = dict(zip(uniq.tolist(), acc.tolist()))
    one, zero = codes[1], codes[0]
    out, clash = [], 0
    full = (1 << width) - 1
    for key in set(one) | set(zero):
        a, z = one.get(key, 0), zero.get(key, 0)
        if a & z or (a | z) != full:
            clash += 1
            continue
        u, b, v = pairs[a]
        o, k = divmod(key, 8)
        cram = base[o] >> k & 1
        out.append((u, b, v, *addr(o, k), int(cram == v)))
    out.sort(key=lambda r: (r[0], r[1]))
    with open(os.path.join(outdir, 'map.tsv'), 'w') as f:
        for r in out:
            f.write('\t'.join(map(str, r)) + '\n')
    mapped = {(r[0], r[1]) for r in out}
    print(f'{len(pairs)} pairs, {len(out)} CRAM bits decoded, {len(mapped)} pairs mapped, '
          f'{clash} CRAM bits with a clash')


if __name__ == '__main__':
    main(*sys.argv[1:])
