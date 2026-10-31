#!/usr/bin/env python3
"""Check routes from Quartus's `report_routing` against a dumped routing graph.

    rgcheck.py <graph dir> <report_routing log> [<rtattrs.tcl output>...]

Each route is a column of fitter node names (R0_X120_Y48_N0_I27, ...). Every
consecutive pair of routing nodes must be an edge in edges.bin. With block
attribute dumps of the same design's .sof (rtattrs.tcl), every configured hop
must also find `<uid> <mux>/sel` set to the edge's input (muxin[k] -> MUXINk_).
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
    uids = open(f"{gdir}/uids.txt").read().split("\n")
    cfgs = open(f"{gdir}/configs.txt").read().split("\n")
    fanout = {}  # src -> {dst: (uid, config)}
    a = array("I", open(f"{gdir}/edges.bin", "rb").read())
    p = 0
    while p < len(a):
        src, n = a[p], a[p + 1]
        recs = a[p + 2 : p + 2 + 3 * n]
        fanout[src] = {recs[i]: (uids[recs[i + 1]], cfgs[recs[i + 2]]) for i in range(0, 3 * n, 3)}
        p += 2 + 3 * n
    return idx, fanout


def bcm_setting(config):
    """'ileim_col/xleim_ale0/xleim_4 muxin[6]' -> ('.../sel', 'MUXIN6_');
    clk_in[2] -> CLK_IN2_, stitch -> STITCH."""
    mux, inp = config.split(";")[0].split(" ")
    m = re.fullmatch(r"([a-z_0-9]+)\[(\d+)\]", inp)
    return f"{mux}/sel", f"{m.group(1).upper()}{m.group(2)}_" if m else inp.upper()


def load_attrs(paths):
    attrs = {}
    for path in paths:
        for line in open(path, errors="replace"):
            f = line.rstrip("\n").split("\t")
            if len(f) == 3:
                attrs[(f[0], f[1])] = f[2]
    return attrs


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
    gdir, logs, attr_files = sys.argv[1], sys.argv[2:3], sys.argv[3:]
    idx, fanout = load(gdir)
    attrs = load_attrs(attr_files)
    ok = bad = set_ok = set_bad = 0
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
                if a is not None and b is not None and b in fanout.get(a, {}):
                    ok += 1
                    uid, cfg = fanout[a][b]
                    if cfg and attrs:
                        attr, want = bcm_setting(cfg)
                        got = attrs.get((uid, attr))
                        good = got == want
                        set_ok += good
                        set_bad += not good
                        print(f"  {ha} -> {hb}: {uid} {attr} = {want}, sof has {got}"
                              f"{'' if good else '  MISMATCH'}")
                    else:
                        print(f"  {ha} -> {hb}: {uid or '-'} {cfg or '(no setting)'}")
                else:
                    bad += 1
                    print(f"NO EDGE {ha} -> {hb}")
            print(f"route {r[0]} -> {r[-1]}: {len(hops)} routing nodes")
    print(f"edges found {ok}, missing {bad}")
    if attrs:
        print(f"mux settings matching the .sof {set_ok}, mismatching {set_bad}")
    sys.exit(1 if bad or set_bad else 0)


if __name__ == "__main__":
    main()
