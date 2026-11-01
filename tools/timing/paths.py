# Parse quartus_sta `report_timing -detail full_path -show_routing` output
# (tools/timing/sta.sh) into hops (README (u)).
#
#   python3 -I tools/timing/paths.py <paths.txt>... > hops.tsv
#
# One row per data-path hop: path id, kind (RE = routing element, CELL,
# uTco, uTsu, IC), incr (ns), rise/fall, the DYGR node name (fitter names
# converted as a5db's route.rs does) or the cell's port, the element type;
# and one `P` row per path: id, data delay, data required minus clock
# (the setup requirement seen from the data path).
import re
import sys

WIRE = {'LOCAL_INTERCONNECT': 'LL', 'BLOCK_INPUT_MUX_PASSTHROUGH': 'LEIM',
        'BLOCK_INPUT_MUX_PASSTHROUGH_WB': 'LEIM_WB', 'HIO_BUFFER': 'HIOB', 'VIO_BUFFER': 'VIOB'}


def dygr(loc):
    m = re.fullmatch(r'(.+)_X(\d+)_Y(\d+)_N(\d+)_I(\d+)', loc)
    if not m:
        return loc
    t, x, y, n, i = m.groups()
    if t in WIRE:
        t = WIRE[t]
    elif re.fullmatch(r'R\d+', t):
        t = 'H' + t[1:]
    elif re.fullmatch(r'C\d+', t):
        t = 'V' + t[1:]
    return f'{t}:X{x}Y{y}Z{n}I{i}'


def rows(path):
    pid = None
    section = None
    for line in open(path):
        if line.startswith('Path #'):
            pid = f'{path}#{line.split()[1].rstrip(":")}'
            section = None
            continue
        if '; Data Arrival Path' in line:
            section = 'arrival'
        elif '; Data Required Path' in line:
            section = 'required'
        elif line.startswith('; Data Delay') or line.startswith('; Data Arrival Time'):
            pass
        if pid is None or not line.startswith(';'):
            continue
        f = [c.strip() for c in line.strip().strip(';').split(';')]
        if f and f[0] == 'Data Delay':
            yield ('P', pid, f[1])
        if len(f) != 8:
            continue
        total, incr, rf, kind, fanout, loc, etype, elem = f
        if section == 'arrival' and kind in ('RE', 'CELL', 'uTco', 'IC'):
            yield ('H', pid, kind, incr, rf, dygr(loc) if kind == 'RE' else elem, etype, fanout)
        if section == 'required' and kind == 'uTsu':
            yield ('H', pid, kind, incr, rf, elem, etype, fanout)


for p in sys.argv[1:]:
    for r in rows(p):
        print('\t'.join(r))
