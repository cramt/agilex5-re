# Factor a block type's bit map into one relative layout plus a per-instance
# anchor, and report how many instances fit it.
#   python3 -I tools/bitmap/layout.py <outdir>/<type>/map.tsv
# A layout is {bit: (dframe, dbdata)} relative to the instance's minimum
# frame and bdata; instances are grouped by identical layouts, since
# mirrored or edge variants are expected.
import collections
import sys


def load(path):
    inst = collections.defaultdict(dict)
    for line in open(path):
        u, b, v, obj, part, a, c, pol = line.rstrip('\n').split('\t')
        inst[u][b] = (obj, part, int(a), int(c), int(pol))
    return inst


def layout(bits):
    objs = {(o, p) for o, p, *_ in bits.values()}
    a0 = min(a for _, _, a, _, _ in bits.values())
    c0 = min(c for _, _, _, c, _ in bits.values())
    key = tuple(sorted((b, a - a0, c - c0, pol) for b, (_, _, a, c, pol) in bits.items()))
    return key, (sorted(objs), a0, c0)


def main(path):
    inst = load(path)
    groups = collections.defaultdict(list)
    for u, bits in inst.items():
        key, anchor = layout(bits)
        groups[key].append((u, anchor))
    print(f'{len(inst)} instances, {len(groups)} distinct layouts')
    for key, us in sorted(groups.items(), key=lambda kv: -len(kv[1]))[:10]:
        span_a = max(a for _, a, _, _ in key) + 1
        span_c = max(c for _, _, c, _ in key) + 1
        print(f'  {len(us):5d} instances, {len(key)} bits, span {span_a} x {span_c}, e.g. {us[0][0]} at {us[0][1]}')


if __name__ == '__main__':
    main(sys.argv[1])
