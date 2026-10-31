#!/usr/bin/env python3
"""Check that every routing mux selection a .sof sets in one block is the
configuration of some edge in the dumped graph.

    rgcover.py <graph dir> <rtattrs.tcl output> <block uid>

Selections that are not edge choices (clock gates tied to VCC, REGSCAN) are
listed as uncovered.
"""
import sys
from array import array

from rgcheck import bcm_setting


def main():
    g, attrf, uid = sys.argv[1:4]
    uids = open(f"{g}/uids.txt").read().split("\n")
    cfgs = open(f"{g}/configs.txt").read().split("\n")
    u = uids.index(uid)
    a = array("I", open(f"{g}/edges.bin", "rb").read())
    have = set()
    p = 0
    while p < len(a):
        n = a[p + 1]
        r = a[p + 2 : p + 2 + 3 * n]
        for i in range(0, 3 * n, 3):
            if r[i + 1] == u and r[i + 2]:
                have.add(bcm_setting(cfgs[r[i + 2]]))
        p += 2 + 3 * n
    ok = miss = 0
    for line in open(attrf, errors="replace"):
        f = line.rstrip("\n").split("\t")
        if len(f) != 3 or f[0] != uid or not f[1].endswith("/sel"):
            continue
        if f[2] in ("MUX_DIS", "<VIRTUAL: DO NOT USE FOR DEBUG>"):
            continue
        if (f[1], f[2]) in have:
            ok += 1
        else:
            miss += 1
            print("no edge for", f[1], f[2])
    print(f"{uid}: {len(have)} edge settings in the graph; .sof selections covered {ok}, uncovered {miss}")


if __name__ == "__main__":
    main()
