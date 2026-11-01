# Phase 8 oracle design fr (README (z3)): what Quartus packs into one ALM.
# Writes fr.v: 192 registers, each the next state of a random function
# (random truth table, 2 to 7 inputs) of an LFSR, the registers and a few
# combinational nodes; neighbouring functions draw on a shared pool of
# inputs so that two of them can share an ALM's inputs. 16 registers are a
# shift register with no logic of their own (LEIM candidates), 16 have an
# asynchronous clear (KEY0), and four XOR trees reduce everything to pins.
#   python3 -I designs/fr/gen.py > designs/fr/fr.v
import random

rnd = random.Random(8)
N, M = 192, 24
lines = ['// Written by designs/fr/gen.py (README (z3)): random functions of 2 to 7',
         '// inputs into registers, for how Quartus fractures ALMs.',
         'module fr (',
         '    input  wire CLOCK1_50,',
         '    input  wire KEY0,',
         '    output wire [3:0] O',
         ');',
         '    wire clk = CLOCK1_50;',
         "    reg [31:0] lfsr = 32'h1;",
         '    always @(posedge clk) lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};',
         f'    reg [{N - 1}:0] r = 0;',
         f'    reg [15:0] sr = 0;',
         f'    wire [{M - 1}:0] c;']
srcs = [f'lfsr[{i}]' for i in range(32)] + [f'r[{i}]' for i in range(N)]


def table(k):
    while True:
        t = rnd.getrandbits(1 << k)
        # Depends on every input (so nothing folds away).
        if all(any((t >> j & 1) != (t >> (j ^ (1 << b)) & 1) for j in range(1 << k)) for b in range(k)):
            return t


def fn(ins):
    k = len(ins)
    return f"{(1 << k)}'h{table(k):0{(1 << k) // 4}x} >> {{{', '.join(ins)}}}"


# Combinational nodes: 3 to 5 inputs, read by several registers.
for j in range(M):
    ins = rnd.sample(srcs[:32 + 4 * j + 8], rnd.choice([3, 4, 5]))
    lines.append(f'    wire [{(1 << len(ins)) - 1}:0] ct{j} = {fn(ins)};')
    lines.append(f'    assign c[{j}] = ct{j}[0];')
pool = []
for i in range(N):
    if i % 6 == 0:
        pool = rnd.sample(srcs + [f'c[{j}]' for j in range(M)], 7)
    k = rnd.choice([2, 3, 3, 4, 4, 5, 5, 5, 6, 6, 7])
    shared = rnd.randint(0, min(k, 5))
    ins = rnd.sample(pool, shared)
    rest = [s for s in srcs + [f'c[{j}]' for j in range(M)] if s not in ins]
    ins += rnd.sample(rest, k - shared)
    rnd.shuffle(ins)
    lines.append(f'    wire [{(1 << k) - 1}:0] t{i} = {fn(ins)};')
    if 32 <= i < 48:
        lines.append(f'    always @(posedge clk or negedge KEY0) if (!KEY0) r[{i}] <= 0; else r[{i}] <= t{i}[0];')
    else:
        lines.append(f'    always @(posedge clk) r[{i}] <= t{i}[0];')
lines.append('    always @(posedge clk) sr <= {sr[14:0], r[5] ^ r[77]};')
lines.append(f'    assign O[0] = ^r[{N // 2 - 1}:0];')
lines.append(f'    assign O[1] = ^r[{N - 1}:{N // 2}];')
lines.append('    assign O[2] = ^sr ^ ^c;')
lines.append('    assign O[3] = ^lfsr;')
lines.append('endmodule')
print('\n'.join(lines))
