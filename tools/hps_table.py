#!/usr/bin/env python3
"""HPS port bits to routing-graph nodes, from a tools/hps_routes.tcl dump.

    tools/hps_table.py hps_routes.txt [TYPE_GLOB ...] > table.tsv

Per port bit of the HPS atom: `in` (the fabric drives the HPS) meets the
graph at the RPII its route enters through (`LEIM → RPII → RPI → SOC_HPS_
WRAPPER_RE`); `out` (the HPS drives the fabric) leaves from the
last SOC_HPS_WRAPPER_RE node on its route (the one driving VIO_TO_CORE). A bit
traced more than once must agree with itself; one with no route is left
out. Lines `TYPE<TAB>BIT<TAB>in|out<TAB>NODE`, sorted, as
db/sm4revb/hps/lwhps2fpga.tsv has them.
"""

import fnmatch
import re
import sys


def node(hop):
    """`RPII_X52_Y51_N0_I44` -> `RPII:X52Y51Z0I44`; the names the dumps use
    for the HPS wrapper's routing elements as the graph's."""
    m = re.fullmatch(r"([A-Z0-9_]+?)_X(\d+)_Y(\d+)_N(\d+)_I(\d+)", hop)
    if not m:
        return None
    kind = {"SOCHPSWRAPPERRE": "SOC_HPS_WRAPPER_RE", "VIOTOCORE": "VIO_TO_CORE"}.get(
        m.group(1), m.group(1))
    return f"{kind}:X{m.group(2)}Y{m.group(3)}Z{m.group(4)}I{m.group(5)}"


def routes(lines):
    """(direction, type, bit, [hop nodes]) per traced route."""
    cur = None
    hops = []
    for l in lines:
        f = l.split()
        if not f:
            continue
        if f[0] == "I":
            cur = ("in", f[1], int(f[2]))
        elif f[0] == "O":
            cur = ("out", f[1], int(f[2]))
        elif f[0] == "T":
            hops = []
        elif f[0] == "R" and cur:
            n = node(f[1])
            if n:
                hops.append(n)
        elif f[0] == "E" and cur:
            yield (*cur, hops)
            hops = []


def meet(direction, hops):
    if direction == "in":
        rpii = [h for h in hops if h.startswith("RPII:")]
        return rpii[-1] if rpii else None
    # The last of the wrapper's nodes on the way out: the route may go on
    # into VIO_TO_CORE, or (Quartus' soft adapter right behind the atom)
    # straight to a register.
    wrapper = [h for h in hops if h.startswith("SOC_HPS_WRAPPER_RE:")]
    return wrapper[-1] if wrapper else None


def main():
    path, globs = sys.argv[1], sys.argv[2:] or ["*"]
    table = {}
    with open(path) as f:
        for direction, ty, bit, hops in routes(f):
            if not any(fnmatch.fnmatchcase(ty, g) for g in globs):
                continue
            n = meet(direction, hops)
            if n is None:
                continue
            key = (ty, bit, direction)
            if table.setdefault(key, n) != n:
                sys.exit(f"{ty}[{bit}] meets both {table[key]} and {n}")
    for (ty, bit, direction), n in sorted(table.items()):
        print(f"{ty}\t{bit}\t{direction}\t{n}")


if __name__ == "__main__":
    main()
