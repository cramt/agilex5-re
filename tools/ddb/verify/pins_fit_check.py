# Check `ddb pins` against a Quartus Pro fit report: every ball of the
# "All Package Pins" table must have the same pad number, and every placed
# user I/O ("Input Pins"/"Output Pins") the same X/Y as the pad location.
# usage: python3 -I pins_fit_check.py pins.csv <fit report>...
import csv
import re
import sys

rows = list(csv.DictReader(open(sys.argv[1])))
by_ball = {}
for r in rows:
    by_ball.setdefault(r['pin_s0'], []).append(r)

def tables(path):
    cur = None
    for line in open(path, errors='replace'):
        if line.startswith('; ') and line.rstrip().endswith(';') and cur is None:
            cells = [c.strip() for c in line.strip().strip(';').split(';')]
            if len(cells) > 3:
                cur = cells
                continue
        if cur is not None and line.startswith('; '):
            cells = [c.strip() for c in line.strip().strip(';').split(';')]
            if len(cells) == len(cur):
                yield dict(zip(cur, cells))
        elif cur is not None and not line.startswith('+'):
            cur = None

pad_ok = pad_bad = xy_ok = xy_bad = 0
for rpt in sys.argv[2:]:
    for row in tables(rpt):
        if 'Pad Number' in row and 'Location' in row:
            ball = row['Location']
            want = row['Pad Number']
            got = [r['pad_id'] for r in by_ball.get(ball, [])]
            if want == '':
                ok = got in ([''], [])
            else:
                ok = want in got
            if ok:
                pad_ok += 1
            else:
                pad_bad += 1
                print(f'{rpt}: ball {ball} pad {want!r}, decoded {got}')
        elif 'X coordinate' in row and 'Pin #' in row:
            ball = row['Pin #']
            locs = [r['pad_s1'] for r in by_ball.get(ball, [])]
            m = [re.match(r'X(\d+)Y(\d+)SUB_LOC(\d+)', l) for l in locs]
            xy = [(x.group(1), x.group(2)) for x in m if x]
            if (row['X coordinate'], row['Y coordinate']) in xy:
                xy_ok += 1
            else:
                xy_bad += 1
                print(f'{rpt}: {row["Name"]} at {ball} X{row["X coordinate"]}Y{row["Y coordinate"]}, decoded {locs}')
print(f'pad numbers: {pad_ok} match, {pad_bad} differ; placed I/O X/Y: {xy_ok} match, {xy_bad} differ')
sys.exit(1 if pad_bad or xy_bad else 0)
