# The flop map of a Quartus image for tools/a5pnr/simcheck.py, from the
# fitted atoms (tools/alm/atoms.tcl): each FF atom's name, as the RTL's
# register (Quartus' `~DUPLICATE` copies hold the same register), and the
# LAB output its location drives. FF N = 3i + 1 + r is register r of half i
# (ALM i / 2, half i % 2), LAB_RE pin 89 + 4k + 2h + r (README (w), (z3)).
#   python3 -I tools/alm/ffmap.py atoms.txt > image.ffmap
import re
import sys

for line in open(sys.argv[1]):
    if not line.startswith('A '):
        continue
    _, name, typ, loc = line.split()[:4]
    m = re.fullmatch(r'FF_X(\d+)_Y(\d+)_N(\d+)', loc)
    # Hyper-Retiming's flops (`~ERTM`) hold state moved across logic, not
    # the register they are named after.
    if typ != 'FF' or not m or 'ERTM' in name:
        continue
    x, y, n = int(m[1]), int(m[2]), int(m[3])
    i, r = divmod(n, 3)
    if r == 0:
        sys.exit(f'{name} at {loc}: not a register position')
    k, h = divmod(i, 2)
    reg = re.sub(r'~DUPLICATE(_\d+)?$', '', name).replace('|', '.')
    print(f'{reg}\tl{x}_{y}_o{89 + 4 * k + 2 * h + r - 1}')
