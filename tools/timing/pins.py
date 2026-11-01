# Cell port <-> LAB_RE pin pairs seen in quartus_sta paths (README (w)):
# the LAB_RE node right before a cell's input port, and right after its
# output. Prints: cell kind, X,Y, N (cell index in the LAB), port, LAB_RE pin, count.
#   python3 -I tools/timing/pins.py <paths.txt>...
import collections
import re
import sys

seen = collections.Counter()
for fn in sys.argv[1:]:
    prev = None
    pending = None
    for line in open(fn):
        if not line.startswith(';') or line.count(';') < 9:
            continue
        f = [c.strip() for c in line.strip().strip(';').split(';')]
        if len(f) != 8:
            continue
        total, incr, rf, kind, fanout, loc, etype, elem = f
        m = re.fullmatch(r'(LABCELL|MLABCELL|FF)_X(\d+)_Y(\d+)_N(\d+)', loc)
        lab = re.fullmatch(r'LAB_RE_X(\d+)_Y(\d+)_N0_I(\d+)', loc)
        if pending and lab:
            seen[(pending[0], pending[3], pending[1], 'out:' + pending[2], int(lab.group(3)))] += 1
            pending = None
        elif pending and kind == 'RE' and not m:
            pending = None
        if m and kind == 'IC' and prev:
            port = elem.rsplit('|', 1)[-1]
            seen[(m.group(1), f'{m.group(2)},{m.group(3)}', int(m.group(4)), port, prev)] += 1
        if m and kind in ('CELL', 'uTco'):
            pending = (m.group(1), int(m.group(4)), elem.rsplit('|', 1)[-1], f'{m.group(2)},{m.group(3)}')
        prev = int(lab.group(3)) if lab else None
for k, v in sorted(seen.items()):
    print(*k, v, sep='\t')
