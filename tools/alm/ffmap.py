# The flop map of a Quartus image for tools/a5pnr/simcheck.py, from the
# fitted atoms (tools/alm/atoms.tcl): each FF atom's name, as the RTL's
# register (Quartus' `~DUPLICATE` copies hold the same register), and the
# LAB output its location drives. FF N = 3i + 1 + r is register r of half i
# (ALM i / 2, half i % 2), LAB_RE pin 89 + 4k + 2h + r (README (w), (z3)).
#   python3 -I tools/alm/ffmap.py atoms.txt [netlist.json top] > image.ffmap
# With the RTL's yosys netlist, a register bit that starts at 1 is mapped
# inverted (`!name`): the device's flops power up at 0, and Quartus keeps
# such a bit inverted (ml's lfsr[0], README (z5)).
import json
import re
import sys

ones = set()
if len(sys.argv) > 3:
    mod = json.load(open(sys.argv[2]))['modules'][sys.argv[3]]
    for n, v in mod['netnames'].items():
        init = v.get('attributes', {}).get('init')
        if init and set(init) <= {'0', '1'}:
            for i, c in enumerate(reversed(init)):
                if c == '1':
                    ones.add(f'{n}[{i}]' if len(v['bits']) > 1 else n)

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
    print(f'{"!" if reg in ones else ""}{reg}\tl{x}_{y}_o{89 + 4 * k + 2 * h + r - 1}')
