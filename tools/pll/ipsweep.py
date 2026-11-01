# IOPLL parameter sweep (README (v)): generate Altera's altera_iopll IP for a
# list of (reference, outputs) frequency sets with ip-deploy and
# qsys-generate (no fit), and collect the tennm_iopll atom parameters the IP
# computes (M, N, C counters, VCO and PFD frequencies, bandwidth).
#
#   python3 -I tools/pll/ipsweep.py <configs.txt> <out dir>
#
# configs.txt: one set per line, "<ref MHz> <out0 MHz>[,<out1 MHz>...]".
# Writes <out dir>/params.tsv: config line, then one column per parameter.
# The IP is generated in <out dir>/ip (Altera IP, never committed).
import os
import re
import subprocess
import sys

here = os.path.dirname(os.path.abspath(__file__))
repo = os.path.dirname(os.path.dirname(here))
cfgs = [l.split() for l in open(sys.argv[1]) if l.strip() and not l.startswith('#')]
out = os.path.abspath(sys.argv[2])
os.makedirs(os.path.join(out, 'ip'), exist_ok=True)
tcl = ['set root [file normalize [file join $::quartus(binpath) .. ..]]',
       'set ipd [file join $root qsys bin ip-deploy]', 'set qg [file join $root qsys bin qsys-generate]']
for k, (ref, outs) in enumerate(cfgs):
    fs = outs.split(',')
    params = [f'--component-parameter=gui_reference_clock_frequency={ref}',
              f'--component-parameter=gui_number_of_clocks={len(fs)}']
    params += [f'--component-parameter=gui_output_clock_frequency{i}={f}' for i, f in enumerate(fs)]
    d = os.path.join(out, 'ip', f'c{k}')
    tcl.append(f'file mkdir {{{d}}}; cd {{{d}}}')
    tcl.append(f'if {{[catch {{exec $ipd --component-name=altera_iopll --output-name=pll --family=Agilex\\ 5 --part=A5EB013BB23BE4SCS '
               f'{" ".join(params)} --component-parameter=gui_use_locked=true 2>@1}} r]}} {{ puts "c{k} deploy failed" }}')
    tcl.append('if {[catch {exec $qg pll.ip --synthesis=VERILOG --family=Agilex\\ 5 --part=A5EB013BB23BE4SCS 2>@1} r]} '
               f'{{ puts "c{k} generate failed" }}')
open(os.path.join(out, 'sweep.tcl'), 'w').write('\n'.join(tcl) + '\n')
env = dict(os.environ, CHIPSMITH_INSTALL_ROOT=os.environ.get('CHIPSMITH_INSTALL_ROOT', '/pool/dev/altera_pro'))
subprocess.run([os.path.join(repo, 'tools/quartus.sh'), 'quartus_sh', '-t', os.path.join(out, 'sweep.tcl')],
               cwd=out, env=env, check=True, stdout=open(os.path.join(out, 'sweep.log'), 'w'), stderr=subprocess.STDOUT)
rows, keys = [], []
for k, (ref, outs) in enumerate(cfgs):
    p = {}
    for f in subprocess.run(['find', os.path.join(out, 'ip', f'c{k}'), '-name', 'pll_altera_iopll_*.v'],
                            capture_output=True, text=True).stdout.split():
        for name, val in re.findall(r'^\s*\.(\w+)\s*\(([^()]*)\)', open(f).read(), re.M):
            if val.strip():
                p[name] = val.strip().strip('"')
    for n in p:
        if n not in keys:
            keys.append(n)
    rows.append((f'{ref} {outs}', p))
with open(os.path.join(out, 'params.tsv'), 'w') as f:
    f.write('config\t' + '\t'.join(keys) + '\n')
    for c, p in rows:
        f.write(c + '\t' + '\t'.join(p.get(n, '') for n in keys) + '\n')
print(f'{len(rows)} configs, {sum(1 for _, p in rows if p)} generated, {len(keys)} parameters')
