"""Rebuild blinky_a5db's netlist from the image alone and simulate it.

Inputs are read from the finished image, not from build.py's intent:

  routed.tsv   a5db routed <db> <graph> blinky_a5db.sof (the mux edges the
               image selects)
  fan.tsv      a5db fanout <graph> <prefixes> (hardwired edges: those with no
               block uid, and programmable inverters)
  attrs.tsv    uid, attribute, value: a5db attrs of the three LABs
               (tools/readback.sh's a5db.tsv, which asm2 agreed with)

Every LUT input pin is traced back over the edges to the LAB_RE output pin
that drives it, the LUT masks are turned into functions (WYSIWYG
W[i] = not stored[i ^ 31], README (h)), and the result is clocked: it must
count, LED0's IO96 pin must hang off the counter's top bit, and every used
LAB clock must reach back to CLOCK1_50.

  python3 -I designs/blinky_a5db/netcheck.py routed.tsv fan.tsv attrs.tsv
"""
import random
import re
import sys

X, ROWS = 107, (6, 7, 8)
# LAB_RE pins (README (n)): LUT input port p of ALM k is 8k + IN[p]; the
# ALM's reg0/xreg0 output is 89 + 4k, its reg0/xreg1 output 90 + 4k.
IN = {'A': 3, 'B': 4, 'C': 2, 'D': 1, 'E': 0, 'F': 7}
CLOCK_SRC = 'HVIOWR_RE:X121Y30Z500I2'
LED_PIN = 'IO96_RE:X61Y0Z0I3959'
# The clock-mux edges graph::clock_rule decodes have no edge API either, but
# select an input (README (k)): a 4:1 or 5:1 mux's input to its output, a
# row clock's 32:1 mux (node 44 + 7j) and its predrive (50 + 7j). They count
# only when a5db routed lists them.
RULE = re.compile(r'^CE_CC_CLKMUX\w+_INPUT:.*\tCE_CC_CLKMUX\w+_OUTPUT:|^SCLKMUX_RE:.*\tSCLKMUX_RE:\S+I(\d+)$')


def rule_edge(a, b):
    m = RULE.match(f'{a}\t{b}')
    return bool(m) and (m.group(1) is None or (int(m.group(1)) - 44) % 7 in (0, 6) and 44 <= int(m.group(1)) < 128)
# A programmable inverter (RPII -> RPI) has a uid but no `sel` bits: it
# passes its one input (README (k)).
PASS = re.compile(r'^RPII:.*\tRPI:')


def alm(k):
    return f'xtophalf/xale{k}' if k < 5 else f'xbothalf/xale{k - 5}'


def main(routed, fan, attrs):
    adj = {}
    for l in open(routed):
        a, b = l.split('\t')[:2]
        adj.setdefault(a, set()).add(b)
    for l in open(fan):
        a, b, uid = l.rstrip('\n').split('\t')[:3]
        if (not uid and not rule_edge(a, b)) or PASS.match(f'{a}\t{b}'):
            adj.setdefault(a, set()).add(b)
    att = {}
    for l in open(attrs):
        u, a, v = l.rstrip('\n').split('\t')
        att[u, a] = v

    def reach(src):
        seen, todo = {src}, [src]
        while todo:
            for b in adj.get(todo.pop(), ()):
                if b not in seen:
                    seen.add(b)
                    todo.append(b)
        return seen

    # Cells: every ALM whose LUT mask isn't the idle one, by its output pin.
    cells = {}
    for r in ROWS:
        uid = f'cw_la_lab_r{r}_c{X}'
        for k in range(10):
            p = f'{alm(k)}/xcw_la_le_reg0'
            if att.get((uid, f'{p}/xreg0/register_comb')) == 'DFF_LATCH':
                out, reg = f'LAB_RE:X{X}Y{r}Z0I{89 + 4 * k}', True
            elif att.get((uid, f'{p}/xreg1/register_comb')) == 'WIRE_MODE':
                out, reg = f'LAB_RE:X{X}Y{r}Z0I{90 + 4 * k}', False
            else:
                continue
            s = att[uid, f'{alm(k)}/xcw_la_le_lut/lut_mask']
            stored = [int(s[63 - j]) for j in range(64)]
            cells[out] = dict(row=r, k=k, reg=reg, w=[1 - stored[i ^ 31] for i in range(64)], ins={})
    # Who drives each LUT input.
    drives = {}
    for src in cells:
        for n in reach(src):
            drives.setdefault(n, []).append(src)
    for c in cells.values():
        for port, off in IN.items():
            d = drives.get(f'LAB_RE:X{X}Y{c["row"]}Z0I{8 * c["k"] + off}', [])
            assert len(d) <= 1, f'pin driven by {d}'
            if d:
                c['ins'][port] = d[0]
        # An unconnected input must not matter.
        used = {'ABCDEF'.index(p) for p in c['ins']}
        for i in range(64):
            for b in set(range(6)) - used:
                assert c['w'][i] == c['w'][i ^ 1 << b], f'{c["row"]}/{c["k"]} depends on unconnected input {b}'
    regs = sorted(o for o, c in cells.items() if c['reg'])
    print(f'{len(cells)} cells ({len(regs)} registered), {sum(len(c["ins"]) for c in cells.values())} connections')

    def step(q):
        val = dict(q)
        comb = [o for o, c in cells.items() if not c['reg']]
        for _ in comb:              # settle the combinational chain
            for o in comb:
                c = cells[o]
                val[o] = c['w'][sum(val.get(s, 0) << 'ABCDEF'.index(p) for p, s in c['ins'].items())]
        nq = {}
        for o in regs:
            c = cells[o]
            nq[o] = c['w'][sum(val.get(s, 0) << 'ABCDEF'.index(p) for p, s in c['ins'].items())]
        return nq

    # Bit order: from all-zero, the FF that toggles when exactly the known
    # lower bits are 1 is the next bit.
    order = []
    for _ in regs:
        q = {o: int(o in order) for o in regs}
        n = step(q)
        new = [o for o in regs if o not in order and n[o] != q[o]]
        assert len(new) == 1, f'bit {len(order)}: {len(new)} candidates'
        order.append(new[0])
    as_int = lambda q: sum(q[o] << i for i, o in enumerate(order))
    rnd = random.Random(1)
    for _ in range(5000):
        v = rnd.getrandbits(len(order))
        q = {o: v >> i & 1 for i, o in enumerate(order)}
        assert as_int(step(q)) == (v + 1) % (1 << len(order)), f'{v:#x} does not count'
    print(f'counts: {len(order)}-bit binary counter (5000 random states step to state + 1)')
    led = [s for s in cells if LED_PIN in reach(s)]
    assert led == [order[-1]], f'LED0 driven by {led}'
    print(f'LED0 ({LED_PIN}) is driven by the top bit, {order[-1]}: {50e6 / 2 ** len(order):.2f} Hz at 50 MHz')
    clk = reach(CLOCK_SRC)
    for r in ROWS:
        assert f'LAB_RE:X{X}Y{r}Z0I83' in clk, f'row {r} LAB clock not reached'
    print(f'LAB clocks of rows {ROWS} reached from {CLOCK_SRC} (CLOCK1_50)')


if __name__ == '__main__':
    main(*sys.argv[1:4])
