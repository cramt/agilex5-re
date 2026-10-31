# Pack a block type's map.tsv into the database: the distinct relative
# layouts and per-instance anchors (one per image part).
#   python3 -I tools/bitmap/pack.py <outdir>/<type>/map.tsv db/<die>/bits/<type>
# writes <prefix>.layout.tsv (layout, bit, part, dframe, dbdata, polarity)
# and <prefix>.anchors.tsv (uid, layout, object, part, frame, bdata). See
# docs/db-format.md.
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from layout import layout, load  # noqa: E402
from table import write_table  # noqa: E402


def write(prefix, keys, anchors):
    """keys: [layout key] by id; anchors: [(uid, id, {part: (obj, a0, b0)})]."""
    os.makedirs(os.path.dirname(prefix), exist_ok=True)
    rows = ['layout\tbit\tpart\tdframe\tdbdata\tpolarity\n']
    for lid, key in enumerate(keys):
        for b, part, da, dc, pol in key:
            rows.append(f'{lid}\t{b}\t{part}\t{da}\t{dc}\t{pol}\n')
    write_table(prefix + '.layout.tsv', ''.join(rows))
    rows = ['uid\tlayout\tobject\tpart\tframe\tbdata\n']
    for u, lid, parts in anchors:
        for part, (obj, a0, c0) in sorted(parts.items()):
            rows.append(f'{u}\t{lid}\t{obj}\t{part}\t{a0}\t{c0}\n')
    write_table(prefix + '.anchors.tsv', ''.join(rows))


def main(path, prefix):
    inst = load(path)
    ids, anchors = {}, []
    for u in sorted(inst):
        key, parts = layout(inst[u])
        anchors.append((u, ids.setdefault(key, len(ids)), parts))
    write(prefix, sorted(ids, key=ids.get), anchors)
    print(f'{prefix}: {len(ids)} layouts, {len(anchors)} instances')


if __name__ == '__main__':
    main(*sys.argv[1:])
