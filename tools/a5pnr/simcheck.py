# Simulate a design's RTL against the netlist rebuilt from its image
# (a5pnr readback) with iverilog, from the same random register state, with
# random inputs; compare every output and every mapped register each cycle
# (README (w)).
#
#   python3 -I tools/a5pnr/simcheck.py <netlist.json> <top> <pins.txt> <ffmap> <readback.v> <cycles> <rtl.v>...
#
# The ffmap (a5pnr's) only lines up RTL registers with the image's flops for
# the initial state and the per-cycle comparison; the rebuilt netlist itself
# comes from the image alone.
import json
import os
import random
import subprocess
import sys
import tempfile

js, top, pins, ffmap, rb, cycles, *rtl = sys.argv[1:]
cycles = int(cycles)
mod = json.load(open(js))['modules'][top]
used = {l.split()[0] for l in open(pins) if l.strip() and not l.startswith('#')}
widths = {}
for name, nn in mod['netnames'].items():
    widths[name] = len(nn['bits'])
ports = [(n, p['direction'], len(p['bits'])) for n, p in mod['ports'].items()]
# Clock inputs: the ports that drive a flop's or M20K's clock.
clk_bits = set()
for c in mod['cells'].values():
    for port in ('C', 'CLK', 'REFCLK'):
        clk_bits.update(b for b in c['connections'].get(port, []) if isinstance(b, int))
clocks = [n for n, d, w in ports if d == 'input' and w == 1 and n in used and mod['ports'][n]['bits'][0] in clk_bits]
# The RTL's registers: names declared `reg` in the sources. A flop's map
# entry lists every name of its output net; the register is the one the
# RTL declares as a reg (yosys may name the net after a wire assigned from
# it, as femtorv32's aluIn1 = rs1).
import re
declared = set()
for f in rtl:
    src = re.sub(r'//.*', '', open(f).read())
    for m in re.finditer(r'\breg\b\s*(?:signed\s*)?(?:\[[^\]]*\]\s*)?([^;]*);', src):
        for part in m.group(1).split(','):
            name = re.match(r'\s*([A-Za-z_]\w*)', part)
            if name:
                declared.add(name.group(1))
# A wire that is just another name for a register (`wire [31:0] aluIn1 =
# rs1;`): yosys may keep only the wire's name, and the register then never
# got its random state (README (z9): the CPU's rs1).
alias = {}
for f in rtl:
    src = re.sub(r'//.*', '', open(f).read())
    for m in re.finditer(r'\bwire\b\s*(?:\[[^\]]*\]\s*)?(\w+)\s*=\s*(\w+)\s*;', src):
        if m.group(2) in declared:
            alias[m.group(1)] = m.group(2)
memories = set()
for f in rtl:
    src = re.sub(r'//.*', '', open(f).read())
    for m in re.finditer(r'\breg\b\s*(?:signed\s*)?(?:\[[^\]]*\]\s*)?([A-Za-z_]\w*)\s*\[', src):
        memories.add(m.group(1))
regs = []
for l in open(ffmap):
    names, flop = l.split('\t')
    flop = flop.rstrip('\n')  # an escaped identifier keeps its closing space
    for net in names.split('|'):
        # `!name`: the flop holds the register's inverse.
        inv = net.startswith('!')
        net = net.lstrip('!')
        base, _, idx = net.rpartition('[')
        idx = idx.rstrip(']')
        leaf = base.rsplit('.', 1)[-1].split('[')[0]
        net_base = base
        if leaf not in declared and leaf in alias and base.endswith(leaf):
            base = base[:len(base) - len(leaf)] + alias[leaf]
            leaf = alias[leaf]
        # A word bit of an RTL memory held in MLAB RAM (README (z9)):
        # `mem[word][bit]`, the memory being no net of the netlist.
        if leaf in memories and re.fullmatch(r'.*\[\d+\]', base):
            regs.append((f'dut.{base}[{idx}]', f'~rb.{flop}' if inv else f'rb.{flop}'))
            break
        if base.startswith('$') or net_base not in widths or base in mod['ports'] or leaf not in declared:
            continue
        ref = f'dut.{base}' if widths[net_base] == 1 else f'dut.{base}[{idx}]'
        regs.append((ref, f'~rb.{flop}' if inv else f'rb.{flop}'))
        break

def mem_word(ref):
    """(word, bit) when ref is one bit of an RTL memory word, else None."""
    m = re.fullmatch(r'(.*\[\d+\])\[(\d+)\]', ref)
    if m and m.group(1).rsplit('.', 1)[-1].split('[')[0] in memories:
        return m.group(1), int(m.group(2))
    return None


# Registers the RTL declares one-hot ((* onehot *): yosys may then treat
# other values as impossible, as femtorv32's parallel_case state machine
# does): random states keep them one-hot.
onehot = {n for n, v in mod['netnames'].items() if 'onehot' in v.get('attributes', {})}


def put(flop, val):
    """Set the image's flop to val; an inverted entry gets ~val."""
    return f'{flop[1:]} = ~{val};' if flop.startswith('~') else f'{flop} = {val};'


def assign_all(val):
    """Set every mapped register on both sides to val (a Verilog expression
    evaluated per bit). iverilog shows a hierarchical write to a memory word
    only after a delta, so each word is built in w and written once."""
    out, words = [], {}
    hot = {}
    # A register Quartus duplicated (`~DUPLICATE`) is in the map once per
    # copy; every copy gets the value the RTL register gets.
    copies = {}
    for ref, flop in regs:
        copies.setdefault(ref, []).append(flop)
    done = set()
    for ref, flop in regs:
        if ref in done:
            continue
        done.add(ref)
        base = ref[4:].rsplit('[', 1)[0] if ref.endswith(']') else ref[4:]
        if base in onehot and 'random' in val:
            hot.setdefault(base, []).append((ref, flop))
            continue
        mw = mem_word(ref)
        if mw:
            words.setdefault(mw[0], []).append((mw[1], copies[ref]))
        else:
            out.append(f'      v = {val}; {ref} = v; ' + ' '.join(put(f, "v") for f in copies[ref]))
    for base, bits in hot.items():
        # Every bit of the RTL register, also those yosys derives instead of
        # keeping a flop for (one of femtorv32's four state bits).
        off = mod['netnames'][base].get('offset', 0)
        out.append(f'      bias = $random(seed) % {widths[base]}; if (bias < 0) bias = -bias;')
        flop_of = {ref.rsplit('[', 1)[1].rstrip(']'): flop for ref, flop in bits}
        for i in range(widths[base]):
            k = str(off + i)
            out.append(f'      v = (bias == {i}); dut.{base}[{k}] = v;' + (f' {put(flop_of[k], "v")}' if k in flop_of else ''))
    for word, bits in words.items():
        out.append(f'      w = {word};')
        for b, flops in bits:
            out.append(f'      v = {val}; w[{b}] = v; ' + ' '.join(put(f, "v") for f in flops))
        out.append(f'      {word} = w;')
    return out


rnd = random.Random(1)
tb = ['`timescale 1ns/1ps', 'module tb;']
conn_d, conn_r = [], []
for n, d, w in ports:
    flat = lambda i: (n if w == 1 else f'{n}_{i}_')
    if d == 'input':
        tb.append(f'  reg [{w - 1}:0] i_{n} = 0;')
        conn_d.append(f'.{n}(i_{n})')
        conn_r += [f'.{flat(i)}(i_{n}[{i}])' for i in range(w) if (n if w == 1 else f'{n}[{i}]') in used]
    else:
        tb.append(f'  wire [{w - 1}:0] od_{n}, or_{n};')
        conn_d.append(f'.{n}(od_{n})')
        conn_r += [f'.{flat(i)}(or_{n}[{i}])' for i in range(w) if (n if w == 1 else f'{n}[{i}]') in used]
tb.append(f'  {top} dut({", ".join(conn_d)});')
tb.append(f'  {top}_readback rb({", ".join(conn_r)});')
tb.append('  integer cyc, bad = 0, seed = 1, bias, bad_run = 0, changes = 0;')
tb.append('  reg v; reg [1023:0] w;')
for n, d, w in ports:
    if d == 'output':
        tb.append(f'  reg [{w - 1}:0] last_{n};')
compare = []
for n, d, w in ports:
    if d == 'output':
        for i in range(w):
            if (n if w == 1 else f'{n}[{i}]') in used:
                compare.append(f'      if (od_{n}[{i}] !== or_{n}[{i}]) begin bad = bad + 1; if (bad < 10) $display("cycle %0d: {n}[{i}] rtl %b image %b", cyc, od_{n}[{i}], or_{n}[{i}]); end')
for ref, flop in regs:
    compare.append(f'      if ({ref} !== {flop}) begin bad = bad + 1; if (bad < 10) $display("cycle %0d: {ref} rtl %b image %b", cyc, {ref}, {flop}); end')
inputs = [f'      i_{n} = $random(seed);' for n, d, w in ports if d == 'input' and n not in clocks]
# All clocks tick together: the check is of logic, not of clock-domain timing.
up = ' '.join(f'i_{c} = 1;' for c in clocks)
down = ' '.join(f'i_{c} = 0;' for c in clocks)
clk = [f'      #4 {up} #4 {down} #1;' if clocks else '      #9;']
tb.append('  initial begin')
tb.append('    #1;')
# Phase 1: every cycle from a fresh random state, the same on both sides;
# the ones-bias cycles through 0.5, 0.9, 0.97 and 0.03 so long carries run.
tb.append(f'    for (cyc = 0; cyc < {cycles}; cyc = cyc + 1) begin')
tb.append('      case (cyc % 4) 0: bias = 500; 1: bias = 900; 2: bias = 970; default: bias = 30; endcase')
tb += assign_all('(($random(seed) & 1023) < bias)')
tb += inputs + ['      #1;'] + compare + clk + compare
tb.append('    end')
# Phase 2: free running from the last state.
tb.append(f'    for (cyc = {cycles}; cyc < {2 * cycles}; cyc = cyc + 1) begin')
tb += inputs + clk + compare
tb.append('    end')
# Phase 3 (SIMCHECK_RUN=n, SIMCHECK_HOLD="PORT=v,..."): from the device's
# power-up state (every register 0) with the inputs held, n cycles, e.g. a
# CPU running its program; reports how often the outputs changed.
run = int(os.environ.get('SIMCHECK_RUN', '0'))
if run:
    hold = dict(kv.split('=') for kv in os.environ.get('SIMCHECK_HOLD', '').split(',') if kv)
    outs = [n for n, d, w in ports if d == 'output']
    tb.append('    bad_run = bad;')
    # After random cycles both sides restart from 0. With none, the RTL
    # keeps its declared initial values and the image its power-up 0s, as
    # the device does (a Quartus image implements a register that starts
    # at 1 its own way, README (z3)).
    if cycles:
        tb += assign_all("1'b0")
    for n, d, w in ports:
        if d == 'input' and n not in clocks:
            tb.append(f'    i_{n} = {hold.get(n, 0)};')
    tb.append('    #1;')
    for n in outs:
        tb.append(f'    last_{n} = od_{n};')
    tb.append(f'    for (cyc = 0; cyc < {run}; cyc = cyc + 1) begin')
    tb += clk + compare
    for n in outs:
        tb.append(f'      if (od_{n} !== last_{n}) begin changes = changes + 1; if (changes <= 4 || changes % 16 == 0) $display("run: cycle %0d: {n} %0d", cyc, od_{n}); last_{n} = od_{n}; end')
    tb.append('    end')
    tb.append(f'    $display("simcheck run: {run} cycles from power-up with {hold}, outputs changed %0d times, last %0d, %0d mismatches", changes, ' + (f'od_{outs[0]}' if outs else '0') + ', bad - bad_run);')
tb.append(f'    $display("simcheck: {cycles} cycles from random states and {cycles} free-running, {len(regs)} registers compared, %0d mismatches", bad);')
tb.append('    $finish;')
tb.append('  end')
tb.append('endmodule')
with tempfile.TemporaryDirectory() as d:
    open(os.path.join(d, 'tb.v'), 'w').write('\n'.join(tb) + '\n')
    if os.environ.get('SIMCHECK_KEEP'):
        open(os.environ['SIMCHECK_KEEP'], 'w').write('\n'.join(tb) + '\n')
    sim = os.path.join(d, 'sim')
    cells = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'yosys', 'cells_sim.v')
    # Icarus Verilog 12, pinned: 13 resolves names by lexical position and
    # rejects a use before the declaration (femtorv32_quark.v has many;
    # symbol_search(…, lexical_pos_, …) in
    # https://github.com/steveicarus/iverilog/blob/master/elab_net.cc, no
    # upstream issue). Remove the pin when 13 gets a switch for it or the
    # CPU design no longer needs it.
    iv = 'github:NixOS/nixpkgs/b134951a4c9f3c995fd7be05f3243f8ecd65d798#verilog'
    inc = sorted({os.path.dirname(os.path.abspath(f)) for f in rtl})
    # Both run in their own memory cgroup (tools/cap.sh): an uncapped vvp
    # reached 11 GB and took the agent with it on 2026-11-01. vvp has no
    # cycle bound of its own: a combinational loop in a rebuilt netlist
    # never lets simulated time advance, so it gets a wall-clock bound too.
    cap = [os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'cap.sh'), os.environ.get('SIMCHECK_MEM', '8G')]
    c = subprocess.run([*cap, 'nix', 'shell', iv, '-c', 'iverilog', '-g2012', *[f'-I{i}' for i in inc], '-o', sim, os.path.join(d, 'tb.v'), rb, cells, *rtl], capture_output=True, text=True)
    if c.returncode != 0:
        # The old nixpkgs' evaluation warnings come first; iverilog's errors after.
        print('\n'.join(l for l in c.stderr.splitlines() if 'error' in l.lower())[-4000:])
        sys.exit(1)
    limit = int(os.environ.get('SIMCHECK_TIMEOUT', '3600'))
    r = subprocess.run([*cap, 'timeout', str(limit), 'nix', 'shell', iv, '-c', 'vvp', '-n', sim], capture_output=True, text=True)
    if r.returncode in (124, 137) or (r.returncode != 0 and 'mismatches' not in r.stdout):
        print(f'simcheck: vvp ended with status {r.returncode} (124: over {limit} s; 137: over the memory cap) before finishing')
        print('\n'.join(r.stdout.strip().splitlines()[-12:]))
        sys.exit(1)
    out = r.stdout.strip().splitlines()
    print('\n'.join(out[:12] + (['...'] if len(out) > 24 else []) + out[max(12, len(out) - 12):]))
    sys.exit(0 if 'mismatches' in r.stdout and ', 0 mismatches' in r.stdout else 1)
