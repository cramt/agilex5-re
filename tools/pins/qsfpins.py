# Pin specs (<ball>=<use>:<std>) of a design, for a5db pins/pintable and
# tools/pins/check.sh: locations and standards from its .qsf, directions
# from the top-level ports of its .v (input/output lines). Ports listed as
# clock ports (-c name...) are clock_in.
#   python3 -I tools/pins/qsfpins.py <design dir> [-c port...]
import glob
import re
import sys

d = sys.argv[1]
clocks = set(sys.argv[3:]) if len(sys.argv) > 2 and sys.argv[2] == '-c' else set()
qsf = open(glob.glob(f'{d}/*.qsf')[0]).read()
v = open(glob.glob(f'{d}/*.v')[0]).read()
loc = dict((p, b) for b, p in re.findall(r'set_location_assignment PIN_(\S+) -to (\S+)', qsf))
std = {}
for s, p in re.findall(r'IO_STANDARD "([^"]+)" -to (\S+)', qsf):
    std[p] = s
short = {'3.3-V LVCMOS': '3.3', '1.8-V LVCMOS': '1.8', '1.1-V': '1.1'}
dirs = {}
head = re.search(r'module\s+\w+\s*\((.*?)\);', re.sub(r'//[^\n]*', '', v), re.S).group(1)
kind, rng = None, None
for tok in head.split(','):
    m = re.match(r'\s*(input|output)\s+(?:wire\s+)?(\[[^\]]*\])?\s*(\w+)\s*$', tok)
    if m:
        kind, rng, n = m.groups()
    else:
        n = tok.strip()
    if rng:
        hi, lo = map(int, rng.strip('[]').split(':'))
        for i in range(min(hi, lo), max(hi, lo) + 1):
            dirs[f'{n}[{i}]'] = kind
    else:
        dirs[n] = kind
out = []
for p, b in loc.items():
    s = std.get(p) or std.get(re.sub(r'\[\d+\]$', '[*]', p)) or std.get('*')
    base = re.sub(r'\[\d+\]$', '', p)
    use = 'clock_in' if (p in clocks or base in clocks) else {'input': 'in', 'output': 'out'}[dirs[p]]
    out.append(f'{b}={use}:{short[s]}')
print(' '.join(out))
