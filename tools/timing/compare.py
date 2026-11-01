# a5pnr's timing model on its own routes against quartus_sta on Quartus'
# routes of the same placement (a5pnr --quartus, README (z4)).
#
#   python3 -I tools/timing/compare.py <delays.tsv> <a5pnr_timing.txt> <fmax.txt> <paths.txt>...
#
# Per flop endpoint both reports know: the model's arrival at D on a5pnr's
# routes (setup taken off) against quartus_sta's worst data delay into it on
# Quartus' routes; and the two Fmax figures (register to register).
import re
import statistics
import subprocess
import sys

delays, mine, fmax, *paths = sys.argv[1:]
tsu = next(float(l.split('\t')[2]) for l in open(delays) if l.startswith('tsu\t'))
model = {}
reg2reg = None
for l in open(mine):
    f = l.rstrip('\n').split('\t')
    if f[0] == 'register_to_register':
        reg2reg = float(f[1])
    m = re.search(r'\(ALM (\d+) slot (\d+)\)$', f[2]) if f[0] == 'end' else None
    if m:
        model[f'a{m[1]}_f{m[2]}'] = float(f[1]) - tsu
quartus = {}
# quartus_sta's data delay per path (paths.py's P rows), keyed by endpoint.
hops = subprocess.run([sys.executable, '-I', __file__.replace('compare.py', 'paths.py'), *paths], capture_output=True, text=True, check=True).stdout
dd = {}
for l in hops.splitlines():
    f = l.split('\t')
    if f[0] == 'P':
        dd[f[1]] = float(f[2])
for p in paths:
    to, n = None, None
    for l in open(p):
        if l.startswith('Path #'):
            n = l.split()[1].rstrip(':')
        if l.startswith('; To Node') and n:
            to = l.split(';')[2].strip()
            d = dd.get(f'{p}#{n}')
            if d is not None and (to not in quartus or d > quartus[to]):
                quartus[to] = d
both = sorted(set(model) & set(quartus))
q = [l for l in open(fmax) if re.match(r';\s+[\d.]+ MHz', l)]
print(f'Fmax: quartus_sta {q[0].split(";")[1].strip() if q else "?"} on its routes; the model {1000 / reg2reg:.0f} MHz on a5pnr\'s ({reg2reg:.3f} ns)')
if both:
    diff = [model[k] - quartus[k] for k in both]
    rel = [(model[k] - quartus[k]) / quartus[k] for k in both]
    print(f'{len(both)} endpoints: model on a5pnr routes minus quartus_sta on Quartus routes: mean {statistics.mean(diff):+.3f} ns, median {statistics.median(diff):+.3f} ns ({100 * statistics.median(rel):+.1f} %), worst endpoint model {max(model[k] for k in both):.3f} vs quartus {max(quartus[k] for k in both):.3f} ns')
