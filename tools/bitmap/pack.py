# Pack a block type's map.tsv into the database: the distinct relative
# layouts and one anchor per instance.
#   python3 -I tools/bitmap/pack.py <outdir>/<type>/map.tsv db/<die>/bits/<type>
# writes <prefix>.layout.tsv (layout, bit, dframe, dbdata, polarity) and
# <prefix>.anchors.tsv (uid, layout, object, part, frame, bdata). For parts
# other than cram, frame/bdata are byte and bit. Polarity 1: the image bit
# equals the named bit.
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from layout import layout, load  # noqa: E402


def main(path, prefix):
    inst = load(path)
    ids, anchors = {}, []
    for u in sorted(inst):
        key, (objs, a0, c0) = layout(inst[u])
        assert len(objs) == 1, f'{u} spans {objs}'
        lid = ids.setdefault(key, len(ids))
        anchors.append((u, lid, *objs[0], a0, c0))
    os.makedirs(os.path.dirname(prefix), exist_ok=True)
    with open(prefix + '.layout.tsv', 'w') as f:
        f.write('layout\tbit\tdframe\tdbdata\tpolarity\n')
        for key, lid in sorted(ids.items(), key=lambda kv: kv[1]):
            for b, da, dc, pol in key:
                f.write(f'{lid}\t{b}\t{da}\t{dc}\t{pol}\n')
    with open(prefix + '.anchors.tsv', 'w') as f:
        f.write('uid\tlayout\tobject\tpart\tframe\tbdata\n')
        for row in anchors:
            f.write('\t'.join(map(str, row)) + '\n')
    print(f'{prefix}: {len(ids)} layouts, {len(anchors)} instances')


if __name__ == '__main__':
    main(*sys.argv[1:])
