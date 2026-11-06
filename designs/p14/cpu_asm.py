# cpu_st's program (README, "Self-tests"): assembles it, runs it on a small
# RV32I model to get the checksum it must store, and writes cpu_prog.vh (the
# M20K contents, PROG, and the checksum, SUM, that the hardware compares):
#   python3 -I designs/p14/cpu_asm.py designs/p14/cpu_prog.vh
# Each turn of the loop exercises the ALU (register and immediate forms,
# shifts, compares), a branch-counted loop (Fibonacci), word, half and byte
# stores and loads into the M20K's data half, and a fresh write each turn (the
# turn count, read back and checked in software). The checksum goes to
# 0x400000 (the hardware compares it with SUM); any software check that fails
# stores to 0x400008 (FAIL).
import sys

def r(x): return int(x[1:])
def u(op, rd, imm): return (imm & 0xfffff000) | rd << 7 | op
def i_(op, f3, rd, rs1, imm, f7=0): return ((imm & 0xfff) | f7 << 5) << 20 | rs1 << 15 | f3 << 12 | rd << 7 | op
def rr(f3, rd, rs1, rs2, f7=0): return f7 << 25 | rs2 << 20 | rs1 << 15 | f3 << 12 | rd << 7 | 0x33
def s(f3, rs1, rs2, imm): return (imm >> 5 & 0x7f) << 25 | rs2 << 20 | rs1 << 15 | f3 << 12 | (imm & 0x1f) << 7 | 0x23
def b(f3, rs1, rs2, off):
    return ((off >> 12 & 1) << 31 | (off >> 5 & 0x3f) << 25 | rs2 << 20 | rs1 << 15 | f3 << 12
            | (off >> 1 & 0xf) << 8 | (off >> 11 & 1) << 7 | 0x63)
def j(rd, off):
    return ((off >> 20 & 1) << 31 | (off >> 1 & 0x3ff) << 21 | (off >> 11 & 1) << 20
            | (off >> 12 & 0xff) << 12 | rd << 7 | 0x6f)

RR = {'add': (0, 0), 'sub': (0, 0x20), 'sll': (1, 0), 'slt': (2, 0), 'sltu': (3, 0), 'xor': (4, 0),
      'srl': (5, 0), 'sra': (5, 0x20), 'or': (6, 0), 'and': (7, 0)}
RI = {'addi': 0, 'slti': 2, 'sltiu': 3, 'xori': 4, 'ori': 6, 'andi': 7}
SH = {'slli': (1, 0), 'srli': (5, 0), 'srai': (5, 0x20)}
LD = {'lb': 0, 'lh': 1, 'lw': 2, 'lbu': 4, 'lhu': 5}
ST = {'sb': 0, 'sh': 1, 'sw': 2}
BR = {'beq': 0, 'bne': 1, 'blt': 4, 'bge': 5, 'bltu': 6, 'bgeu': 7}

def li(rd, v):
    """lui + addi for any 32-bit constant."""
    v &= 0xffffffff
    lo = v & 0xfff
    lo = lo - 0x1000 if lo >= 0x800 else lo
    return [(None, 'lui', rd, (v - lo) & 0xffffffff), (None, 'addi', rd, rd, lo)]

def program(sum_):
    return [
        ('start', 'lui', 'x1', 0x00400000),          # IO
        (None, 'addi', 'x10', 'x0', 0x400),          # data: the M20K's upper half
        (None, 'addi', 'x26', 'x0', 0),              # turn count
        ('loop', 'addi', 'x2', 'x0', 1234),
        *li('x3', 0x12345678),
        (None, 'add', 'x4', 'x2', 'x3'),
        (None, 'sub', 'x5', 'x3', 'x2'),
        (None, 'xor', 'x6', 'x4', 'x5'),
        (None, 'or', 'x7', 'x6', 'x2'),
        (None, 'and', 'x8', 'x7', 'x3'),
        (None, 'slli', 'x9', 'x3', 5),
        (None, 'srli', 'x11', 'x3', 7),
        (None, 'sub', 'x13', 'x0', 'x3'),
        (None, 'srai', 'x12', 'x13', 9),
        (None, 'slt', 'x14', 'x13', 'x2'),
        (None, 'sltu', 'x15', 'x13', 'x2'),
        (None, 'addi', 'x20', 'x0', 11),
        (None, 'sll', 'x21', 'x3', 'x20'),
        (None, 'sra', 'x22', 'x13', 'x20'),
        (None, 'srl', 'x23', 'x13', 'x20'),
        (None, 'xori', 'x24', 'x3', -1),
        (None, 'ori', 'x25', 'x2', 0x700),
        (None, 'andi', 'x27', 'x3', 0x0f0),
        (None, 'sltiu', 'x28', 'x2', 2000),
        # Fibonacci: 25 turns of a counted loop.
        (None, 'addi', 'x16', 'x0', 0),
        (None, 'addi', 'x17', 'x0', 1),
        (None, 'addi', 'x18', 'x0', 25),
        ('fib', 'add', 'x19', 'x16', 'x17'),
        (None, 'addi', 'x16', 'x17', 0),
        (None, 'addi', 'x17', 'x19', 0),
        (None, 'addi', 'x18', 'x18', -1),
        (None, 'bne', 'x18', 'x0', 'fib'),
        # Memory: words, a half and bytes, read back mixed.
        (None, 'sw', 'x4', 0, 'x10'),
        (None, 'sw', 'x13', 4, 'x10'),
        (None, 'sw', 'x9', 8, 'x10'),
        (None, 'sw', 'x0', 12, 'x10'),
        (None, 'sb', 'x2', 13, 'x10'),
        (None, 'sh', 'x3', 14, 'x10'),
        (None, 'lw', 'x29', 0, 'x10'),
        (None, 'lh', 'x30', 6, 'x10'),
        (None, 'lbu', 'x31', 9, 'x10'),
        (None, 'lb', 'x20', 13, 'x10'),
        (None, 'lw', 'x21', 12, 'x10'),
        (None, 'lhu', 'x22', 4, 'x10'),
        # A fresh word each turn, checked in software.
        (None, 'sw', 'x26', 16, 'x10'),
        (None, 'lw', 'x23', 16, 'x10'),
        (None, 'bne', 'x23', 'x26', 'fail'),
        (None, 'blt', 'x2', 'x13', 'fail'),       # 1234 < -0x12345678 is false
        (None, 'bgeu', 'x2', 'x13', 'fail'),      # unsigned: 1234 < 0xedcba988
        # Checksum: rotate-add over every result.
        (None, 'addi', 'x2', 'x0', 0),
        *[ins for reg in [4, 5, 6, 7, 8, 9, 11, 12, 14, 15, 24, 25, 27, 28, 16, 17, 29, 30, 31, 20, 21, 22]
          for ins in [(None, 'slli', 'x3', 'x2', 3), (None, 'srli', 'x2', 'x2', 29), (None, 'or', 'x2', 'x2', 'x3'),
                      (None, 'add', 'x2', 'x2', f'x{reg}')]],
        *li('x3', sum_),
        (None, 'bne', 'x2', 'x3', 'fail'),
        (None, 'sw', 'x2', 0, 'x1'),              # the checksum: hardware compares it with SUM
        (None, 'addi', 'x26', 'x26', 1),
        (None, 'jal', 'x0', 'loop'),
        ('fail', 'sw', 'x2', 8, 'x1'),
        (None, 'jal', 'x0', 'fail'),
    ]

def assemble(prog):
    labels = {l: 4 * k for k, (l, *_) in enumerate(prog) if l}
    words = []
    for k, (_, m, *a) in enumerate(prog):
        off = lambda t: labels[t] - 4 * k
        if m == 'lui': w = u(0x37, r(a[0]), a[1])
        elif m in RR: w = rr(RR[m][0], r(a[0]), r(a[1]), r(a[2]), RR[m][1])
        elif m in RI: w = i_(0x13, RI[m], r(a[0]), r(a[1]), a[2])
        elif m in SH: w = i_(0x13, SH[m][0], r(a[0]), r(a[1]), a[2], SH[m][1])
        elif m in LD: w = i_(0x03, LD[m], r(a[0]), r(a[2]), a[1])
        elif m in ST: w = s(ST[m], r(a[2]), r(a[0]), a[1])
        elif m in BR: w = b(BR[m], r(a[0]), r(a[1]), off(a[2]))
        elif m == 'jal': w = j(r(a[0]), off(a[1]))
        words.append(w & 0xffffffff)
    return words

def sx(v, bits): v &= (1 << bits) - 1; return v - (1 << bits) if v >> (bits - 1) else v

def run(words, stores=2):
    """Decode and execute the words (not the source list) until `stores` IO
    stores; returns them as (address, value)."""
    x = [0] * 32; mem = {}; pc = 0; out = []
    M = 0xffffffff
    while len(out) < stores:
        w = words[pc >> 2]; op = w & 0x7f; rd = w >> 7 & 31; f3 = w >> 12 & 7
        a = x[w >> 15 & 31]; bv = x[w >> 20 & 31]; f7 = w >> 25
        ii = sx(w >> 20, 12); npc = pc + 4; res = None
        if op == 0x37: res = w & 0xfffff000
        elif op in (0x13, 0x33):
            o = bv if op == 0x33 else ii & M; sh = o & 31
            alt = op == 0x33 and f7 == 0x20 or op == 0x13 and f3 == 5 and f7 == 0x20
            res = {0: (a - o) if alt and op == 0x33 else a + o, 1: a << sh, 2: int(sx(a, 32) < sx(o, 32)),
                   3: int(a < (o & M)), 4: a ^ o, 5: (sx(a, 32) >> sh) if alt else a >> sh, 6: a | o, 7: a & o}[f3]
        elif op == 0x03:
            ad = (a + ii) & M; wd = mem.get(ad & ~3, 0) >> 8 * (ad & 3)
            res = {0: sx(wd, 8), 1: sx(wd, 16), 2: wd, 4: wd & 0xff, 5: wd & 0xffff}[f3]
        elif op == 0x23:
            ad = (a + sx((w >> 25) << 5 | (w >> 7 & 31), 12)) & M
            if ad & 0x400000: out.append((ad, bv))
            else:
                n = 1 << f3; sh = 8 * (ad & 3); mask = ((1 << 8 * n) - 1) << sh
                mem[ad & ~3] = (mem.get(ad & ~3, 0) & ~mask | (bv << sh) & mask) & M
        elif op == 0x63:
            bo = sx((w >> 31) << 12 | (w >> 7 & 1) << 11 | (w >> 25 & 0x3f) << 5 | (w >> 8 & 0xf) << 1, 13)
            t = {0: a == bv, 1: a != bv, 4: sx(a, 32) < sx(bv, 32), 5: sx(a, 32) >= sx(bv, 32), 6: a < bv, 7: a >= bv}[f3]
            if t: npc = pc + bo
        elif op == 0x6f:
            jo = sx((w >> 31) << 20 | (w >> 12 & 0xff) << 12 | (w >> 20 & 1) << 11 | (w >> 21 & 0x3ff) << 1, 21)
            res = npc; npc = pc + jo
        else: raise SystemExit(f'model: unknown opcode at {pc:#x}')
        if res is not None and rd: x[rd] = res & M
        pc = npc
    return out

# The checksum the code compares against is a constant in the code itself:
# assemble with 0, read what the model stores at FAIL (the checksum it
# computed), then assemble with that and require two passing turns.
first = run(assemble(program(0)), 1)
assert first[0][0] == 0x400008, first
SUM = first[0][1]
prog = program(SUM)
words = assemble(prog)
got = run(words, 2)
assert got == [(0x400000, SUM)] * 2, got
assert len(words) <= 256, len(words)   # the data half starts at 0x400
words += [0x00000013] * (512 - len(words))   # nop
with open(sys.argv[1], 'w') as f:
    f.write(f'// Written by cpu_asm.py: {len(prog)} instructions, checksum {SUM:#010x}.\n')
    for k, (_, m, *a) in enumerate(prog):
        f.write(f'// {4 * k:03x}: {words[k]:08x}  {m} {", ".join(map(str, a))}\n')
    f.write(f"localparam [31:0] SUM = 32'h{SUM:08x};\n")
    f.write(f"localparam [16383:0] PROG = 16384'h{''.join(f'{w:08x}' for w in reversed(words))};\n")
print(f'{len(prog)} instructions, checksum {SUM:#010x}')
