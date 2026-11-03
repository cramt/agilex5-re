#!/usr/bin/env python3
"""LAB lines per LAB of an exported a5pnr placement (a5pnr --quartus), under
Quartus' rule (README (z14)): a net needs one LAB line per 5 of its data
sinks in the LAB, plus one more per further LEIM the net uses in an ALM (the
same net on C0 and C1, say), and a LAB takes at most 55. Sinks are LUT
inputs, flop D inputs not fed from their own ALM, and flop enables and
clears; carry and clocks are not.

    lablines.py <export .v> <export .qsf> [<unlocked.txt>]

Prints LABs sorted by lines, marking the ones Quartus rejected (unlocked.txt,
tools/bench/unlock_fit.sh), and the separation between rejected and kept.
"""
import collections
import math
import re
import sys

FANOUT = 5
DATA = {'dataa', 'datab', 'datac', 'datad', 'datae', 'dataf'}
CTL = {'clk', 'ena', 'sclr', 'clrn'}


def main():
    v, qsf = sys.argv[1], sys.argv[2]
    rejected = set(open(sys.argv[3]).read().split()) if len(sys.argv) > 3 else set()
    loc, half = {}, {}
    for m in re.finditer(r'set_location_assignment (?:M?LABCELL|FF)_X(\d+)_Y(\d+)_N(\d+) -to (\S+)', open(qsf).read()):
        loc[m[4]] = (int(m[1]), int(m[2]), int(m[3]) // 6)
        half[m[4]] = int(m[3]) % 6 // 3
    cells = []
    for m in re.finditer(r'^\s*(tennm_\w+)\s*(?:#\(.*?\))?\s*(\S+)\s*\((.*?)\);', open(v).read(), re.M | re.S):
        ports = dict(re.findall(r'\.(\w+)\(([^()]*)\)', m[3]))
        cells.append((m[1], m[2], ports))
    driver = {}
    for kind, name, ports in cells:
        for p in ('combout', 'sumout', 'q'):
            if ports.get(p):
                driver[ports[p]] = name
    sinks = collections.defaultdict(lambda: collections.Counter())
    # LEIMs per (LAB, ALM, net): a net on two of an ALM's LEIMs takes a
    # second LAB line (the duplicates' own line assignment, `Heuristic
    # inputs` in Quartus' LBLK debug output).
    leims = collections.defaultdict(set)
    for kind, name, ports in cells:
        if name not in loc:
            continue
        x, y, alm = loc[name]
        lab = f'X{x}_Y{y}'
        for p, net in ports.items():
            if not re.fullmatch(r'n\d+', net or ''):
                continue
            if p in DATA or (p == 'd' and kind == 'tennm_ff'):
                if p == 'd' and loc.get(driver.get(net), (None,) * 3) == (x, y, alm):
                    continue
                sinks[lab][net] += 1
                if p != 'd':
                    h = half[name]
                    leim = {'dataa': 0, 'datab': 1, 'datac': 2 + h, 'datad': 4 + h, 'datae': 6 + h, 'dataf': 7}[p]
                    leims[(lab, alm, net)].add(leim)
            elif p in CTL and p != 'clk':
                # Enables and clears not on a global network enter on LAB
                # lines as well.
                sinks[lab][('ctl', net)] += 1
    dups = collections.defaultdict(collections.Counter)
    for (lab, alm, net), ls in leims.items():
        if len(ls) > 1:
            dups[lab][net] += len(ls) - 1
    rows = []
    for lab, nets in sinks.items():
        lines = sum(math.ceil(c / FANOUT) for c in nets.values())
        extra = sum(math.ceil(c / FANOUT) for c in dups[lab].values())
        rows.append((lines + extra, lines, extra, len(nets), lab in rejected, lab))
    rows.sort()
    kept = [r for r in rows if not r[4]]
    rej = [r for r in rows if r[4]]
    print(f'{len(rows)} LABs, {len(rej)} rejected')
    print('lines unique duplicate nets rejected lab')
    for r in rows[-int(sys.argv[4]) if len(sys.argv) > 4 else -25:]:
        print(*r[:4], 'REJECTED' if r[4] else '-', r[5])
    if kept and rej:
        print(f'kept: lines <= {max(r[0] for r in kept)}, nets <= {max(r[3] for r in kept)}; '
              f'rejected: lines >= {min(r[0] for r in rej)}, nets >= {min(r[3] for r in rej)}')


if __name__ == '__main__':
    main()
