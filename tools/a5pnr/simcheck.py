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
clock = None
for n, d, w in ports:
    if d == 'input' and w == 1 and n in used and ('CLOCK' in n.upper() or n.lower().startswith('clk')):
        clock = n
regs = []
for l in open(ffmap):
    net, flop = l.split()
    base, _, idx = net.rpartition('[')
    idx = idx.rstrip(']')
    if base.startswith('$') or base.startswith('\\$') or base not in widths or base in mod['ports']:
        continue
    ref = f'dut.{base}' if widths[base] == 1 else f'dut.{base}[{idx}]'
    regs.append((ref, f'rb.{flop}'))
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
tb.append('  integer cyc, bad = 0, seed = 1, bias;')
compare = []
for n, d, w in ports:
    if d == 'output':
        for i in range(w):
            if (n if w == 1 else f'{n}[{i}]') in used:
                compare.append(f'      if (od_{n}[{i}] !== or_{n}[{i}]) begin bad = bad + 1; if (bad < 10) $display("cycle %0d: {n}[{i}] rtl %b image %b", cyc, od_{n}[{i}], or_{n}[{i}]); end')
for ref, flop in regs:
    compare.append(f'      if ({ref} !== {flop}) begin bad = bad + 1; if (bad < 10) $display("cycle %0d: {ref} rtl %b image %b", cyc, {ref}, {flop}); end')
inputs = [f'      i_{n} = $random(seed);' for n, d, w in ports if d == 'input' and n != clock]
clk = [f'      #4 i_{clock} = 1; #4 i_{clock} = 0; #1;' if clock else '      #9;']
tb.append('  initial begin')
tb.append('    #1;')
# Phase 1: every cycle from a fresh random state, the same on both sides;
# the ones-bias cycles through 0.5, 0.9, 0.97 and 0.03 so long carries run.
tb.append(f'    for (cyc = 0; cyc < {cycles}; cyc = cyc + 1) begin')
tb.append('      case (cyc % 4) 0: bias = 500; 1: bias = 900; 2: bias = 970; default: bias = 30; endcase')
for ref, flop in regs:
    tb.append(f'      {ref} = (($random(seed) & 1023) < bias); {flop} = {ref};')
tb += inputs + ['      #1;'] + compare + clk + compare
tb.append('    end')
# Phase 2: free running from the last state.
tb.append(f'    for (cyc = {cycles}; cyc < {2 * cycles}; cyc = cyc + 1) begin')
tb += inputs + clk + compare
tb.append('    end')
tb.append(f'    $display("simcheck: {cycles} cycles from random states and {cycles} free-running, {len(regs)} registers compared, %0d mismatches", bad);')
tb.append('    $finish;')
tb.append('  end')
tb.append('endmodule')
with tempfile.TemporaryDirectory() as d:
    open(os.path.join(d, 'tb.v'), 'w').write('\n'.join(tb) + '\n')
    sim = os.path.join(d, 'sim')
    cells = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'yosys', 'cells_sim.v')
    subprocess.run(['nix', 'shell', 'nixpkgs#iverilog', '-c', 'iverilog', '-g2012', '-o', sim, os.path.join(d, 'tb.v'), rb, cells, *rtl], check=True)
    r = subprocess.run(['nix', 'shell', 'nixpkgs#iverilog', '-c', 'vvp', '-n', sim], capture_output=True, text=True)
    print(r.stdout.strip().splitlines()[-12:] and '\n'.join(r.stdout.strip().splitlines()[-12:]))
    sys.exit(0 if 'mismatches' in r.stdout and ', 0 mismatches' in r.stdout else 1)
