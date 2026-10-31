#!/usr/bin/env python3
"""Check routes from Quartus's `report_routing` against a dumped routing graph.

    rgcheck.py <graph dir> <log with report_routing Info output>...

Each route is a column of fitter node names (R0_X120_Y48_N0_I27, ...). Every
consecutive pair of routing nodes must be an edge in edges.bin.
"""
import re
import struct
import sys
from array import array

# Fitter report name -> DYGR element name, where they differ.
FIT2DYGR = {"LOCAL_INTERCONNECT": "LL", "BLOCK_INPUT_MUX_PASSTHROUGH": "LEIM"}
FIT_RE = re.compile(r"^([A-Z0-9_]+?)_X(\d+)_Y(\d+)_N(\d+)_I(\d+)$")


def dygr_name(fit):
    m = FIT_RE.match(fit)
    if not m:
        return None
    t, x, y, z, i = m.groups()
    t = FIT2DYGR.get(t, t)
    t = re.sub(r"^R(\d+)$", r"H\1", t)
    t = re.sub(r"^C(\d+)$", r"V\1", t)
    return f"{t}:X{x}Y{y}Z{z}I{i}"


def load(gdir):
    idx = {}
    with open(f"{gdir}/nodes.tsv") as f:
        for line in f:
            g, name, _ = line.rstrip("\n").split("\t")
            idx[name] = int(g)
    fanout = {}
    data = open(f"{gdir}/edges.bin", "rb").read()
    a = array("I", data)
    p = 0
    while p < len(a):
        src, n = a[p], a[p + 1]
        fanout[src] = a[p + 2 : p + 2 + n]
        p += 2 + n
    return idx, fanout


def routes(path):
    cur = None
    for line in open(path, errors="replace"):
        m = re.match(r"^Info:\s+(\S+)(?:\s+\d+)?\s*$", line)
        if line.startswith("Info:") and "Location or Port" in line:
            cur = []
            continue
        if cur is not None and m:
            cur.append(m.group(1))
            if "/" in m.group(1) and len(cur) > 1:
                yield cur
                cur = None
        elif cur is not None:
            cur = None


def main():
    gdir, logs = sys.argv[1], sys.argv[2:]
    idx, fanout = load(gdir)
    ok = bad = 0
    for log in logs:
        for r in routes(log):
            hops = [n for n in r[1:-1]]
            ids = []
            for h in hops:
                d = dygr_name(h)
                if d not in idx:
                    print(f"MISSING node {h} -> {d}")
                    bad += 1
                ids.append((h, idx.get(d)))
            for (ha, a), (hb, b) in zip(ids, ids[1:]):
                if a is not None and b is not None and b in fanout.get(a, ()):
                    ok += 1
                else:
                    bad += 1
                    print(f"NO EDGE {ha} -> {hb}")
            print(f"route {r[0]} -> {r[-1]}: {len(hops)} routing nodes")
    print(f"edges found {ok}, missing {bad}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
