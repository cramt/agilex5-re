# The CPU design's program (README (z2)), assembled by hand-written encoders
# for the few RV32I instructions it uses, written as prog.vh: a Verilog
# localparam with the 512-word M20K contents (bit 32a + k = word a, bit k).
#   python3 -I designs/p7/cpu/asm.py designs/p7/cpu/prog.vh
import sys

def r(x): return int(x[1:])
def u(op, rd, imm): return (imm & 0xfffff000) | rd << 7 | op
def i(op, f3, rd, rs1, imm): return (imm & 0xfff) << 20 | rs1 << 15 | f3 << 12 | rd << 7 | op
def s(op, f3, rs1, rs2, imm): return (imm >> 5 & 0x7f) << 25 | rs2 << 20 | rs1 << 15 | f3 << 12 | (imm & 0x1f) << 7 | op
def b(f3, rs1, rs2, off):
    return ((off >> 12 & 1) << 31 | (off >> 5 & 0x3f) << 25 | rs2 << 20 | rs1 << 15 | f3 << 12
            | (off >> 1 & 0xf) << 8 | (off >> 11 & 1) << 7 | 0x63)
def j(rd, off):
    return ((off >> 20 & 1) << 31 | (off >> 1 & 0x3ff) << 21 | (off >> 11 & 1) << 20
            | (off >> 12 & 0xff) << 12 | rd << 7 | 0x6f)

# (label, mnemonic, operands); the IO block is at 0x00400000: a store
# there sets LED[7:0] to the low byte, a load from +4 reads {.., SW0}.
SLOW = 0x200000  # delay-loop iterations per count with SW0 down (6 cycles each)
prog = [
    ('start', 'lui', 'x1', 0x00400000),
    (None, 'addi', 'x2', 'x0', 0),
    ('loop', 'lw', 'x5', 4, 'x1'),
    (None, 'andi', 'x5', 'x5', 1),
    (None, 'lui', 'x3', SLOW),
    (None, 'bne', 'x5', 'x0', 'fast'),
    (None, 'jal', 'x0', 'wait'),
    ('fast', 'addi', 'x3', 'x0', 3),
    ('wait', 'addi', 'x3', 'x3', -1),
    (None, 'bne', 'x3', 'x0', 'wait'),
    (None, 'addi', 'x2', 'x2', 1),
    (None, 'sw', 'x2', 0, 'x1'),
    (None, 'jal', 'x0', 'loop'),
]
labels = {l: 4 * k for k, (l, *_) in enumerate(prog) if l}
words = []
for k, (_, m, *a) in enumerate(prog):
    pc = 4 * k
    off = lambda t: labels[t] - pc
    w = {
        'lui': lambda: u(0x37, r(a[0]), a[1]),
        'addi': lambda: i(0x13, 0, r(a[0]), r(a[1]), a[2]),
        'andi': lambda: i(0x13, 7, r(a[0]), r(a[1]), a[2]),
        'lw': lambda: i(0x03, 2, r(a[0]), r(a[2]), a[1]),
        'sw': lambda: s(0x23, 2, r(a[2]), r(a[0]), a[1]),
        'bne': lambda: b(1, r(a[0]), r(a[1]), off(a[2])),
        'jal': lambda: j(r(a[0]), off(a[1])),
    }[m]()
    words.append(w & 0xffffffff)
words += [0x00000013] * (512 - len(words))   # nop
hexs = ''.join(f'{w:08x}' for w in reversed(words))
with open(sys.argv[1], 'w') as f:
    f.write('// Written by asm.py: the program, 512 words (word 0 lowest).\n')
    for k, w in enumerate(words[:len(prog)]):
        f.write(f'// {4 * k:03x}: {w:08x}  {prog[k][1]} {", ".join(map(str, prog[k][2:]))}\n')
    f.write(f"localparam [16383:0] PROG = 16384'h{hexs};\n")
