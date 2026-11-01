# asm2_probe.tcl output -> bcm_settings.tsv rows (README (q)): for each probed
# attribute whose values change named bits, the attribute's bits are every
# bit any value changed; a value's row holds its own changes and the probed
# image's (idle) value elsewhere. Bits are made relative to the attribute's
# leaf instance, as bcm_settings.tsv stores them.
#   python3 -I tools/bcm/probe2settings.py <probe_out.tsv> <bcm_types.json> <base bits>
# <base bits>: "<uid> TAB <bit> TAB <value>" lines of the probed instances
# (a5db bits output with the uid prepended).
import collections
import json
import sys

types = json.load(open(sys.argv[2]))['types']
base = {}
for l in open(sys.argv[3]):
    u, b, v = l.rstrip('\n').split('\t')
    base[(u, b)] = v
rows = collections.defaultdict(list)
for l in open(sys.argv[1]):
    if l.startswith('#'):
        continue
    f = l.rstrip('\n').split('\t')
    if len(f) < 5 or f[3].startswith('=') or f[4].startswith('<ERR'):
        continue
    rows[(f[0], f[1], f[2])].append((f[3], dict(kv.rsplit('=', 1) for kv in f[4].split(' ') if kv)))
for (btype, uid, attr), vals in rows.items():
    bits = sorted({b for _, ch in vals for b in ch})
    if not bits:
        continue
    path, name = attr.rsplit('/', 1)
    leaf = btype
    for p in path.split('/'):
        leaf = types[leaf]['children'][p]
    rel = [b[len(path) + 1:] for b in bits]
    assert all(b.startswith(path + '/') for b in bits), attr
    for v, ch in vals:
        vs = [ch.get(b, base[(uid, b)]) for b in bits]
        print('\t'.join([leaf, name, v, ' '.join('{' + r + '}' for r in rel), ' '.join(vs)]))
