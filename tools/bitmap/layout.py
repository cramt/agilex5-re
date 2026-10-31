# Factor a block type's bit map into relative layouts plus per-instance
# anchors, and report how many instances share each layout.
#   python3 -I tools/bitmap/layout.py <outdir>/<type>/map.tsv
# Each image part an instance touches (CRAM, M20K contents, a periphery
# header, …) gets its own anchor: the minimum (frame, bdata) for CRAM, the
# minimum bit offset elsewhere. A layout is {bit: (part, dframe, dbdata,
# polarity)}; instances are grouped by identical layouts, since mirrored or
# edge variants are expected.
import collections
import sys


def load(path):
    """{uid: {bit: (object, part, a, b, polarity)}}. Outside CRAM, a is the
    bit offset in the part (map.tsv's byte * 8 + bit) and b is 0."""
    inst = collections.defaultdict(dict)
    for line in open(path):
        u, b, v, obj, part, a, c, pol = line.rstrip('\n').split('\t')
        a, c = int(a), int(c)
        if part != 'cram':
            a, c = a * 8 + c, 0
        inst[u][b] = (obj, part, a, c, int(pol))
    return inst


def layout(bits):
    """(layout key, {part: (object, a0, b0)})."""
    anchors = {}
    for obj, part, a, c, _ in bits.values():
        o, a0, c0 = anchors.get(part, (obj, a, c))
        assert o == obj, f'part {part} spans {o} and {obj}'
        anchors[part] = (obj, min(a0, a), min(c0, c))
    key = tuple(sorted((b, part, a - anchors[part][1], c - anchors[part][2], pol)
                       for b, (_, part, a, c, pol) in bits.items()))
    return key, anchors


def main(path):
    inst = load(path)
    groups = collections.defaultdict(list)
    for u, bits in inst.items():
        key, anchors = layout(bits)
        groups[key].append((u, anchors))
    print(f'{len(inst)} instances, {len(groups)} distinct layouts')
    for key, us in sorted(groups.items(), key=lambda kv: -len(kv[1]))[:10]:
        parts = sorted({p for _, p, *_ in key})
        print(f'  {len(us):5d} instances, {len(key)} bits in {parts}, e.g. {us[0][0]} at {us[0][1]}')


if __name__ == '__main__':
    main(sys.argv[1])
