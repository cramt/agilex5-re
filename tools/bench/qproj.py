"""A Quartus Prime Pro project for a bench design (tools/bench/vs-quartus-pro.sh):
   python3 -I tools/bench/qproj.py <repo> <name> <out dir> [period ns]
Pins and I/O standards from the design's pins file, the same Verilog as
a5pnr's (absolute paths), one clock over-constrained so the fitter works for
speed and quartus_sta reports the Fmax it reached (README (z4)). Everything
else is Quartus' default, retiming included. The CPU's two variants use
designs/p8/cql.v (the ROM as inferred M20K) with the register file in MLAB
(ramstyle "MLAB") or in flops (ramstyle "logic", as phase 8's cql)."""
import os, re, sys
repo, name, out = sys.argv[1:4]
period = sys.argv[4] if len(sys.argv) > 4 else '2.000'
C = '/pool/dev/chipsmith/examples'
rows = [l.rstrip('\n').split('\t') for l in open(f'{repo}/tools/bench/designs.tsv') if l.strip() and not l.startswith('#')]
d = dict(zip(rows[0], next(r for r in rows[1:] if r[0] == name)))
os.makedirs(out, exist_ok=True)
srcs = [s.replace('$C', C) for s in d['sources'].split(',')]
srcs = [s if s.startswith('/') else f'{repo}/{s}' for s in srcs]
top, extra = d['top'], []
if d['quartus'] in ('cpu_mlab', 'cpu_logic'):
    style = 'MLAB, no_rw_check' if d['quartus'] == 'cpu_mlab' else 'logic'
    src = open(f'{repo}/designs/p7/cpu/femtorv32_quark.v').read()
    src, n = re.subn(r'reg \[31:0\] registerFile \[31:0\];', f'(* ramstyle = "{style}" *) reg [31:0] registerFile [31:0];', src)
    assert n == 1
    open(f'{out}/femtorv32_bench.v', 'w').write(src)
    srcs = [f'{repo}/designs/p8/cql/cql.v', f'{out}/femtorv32_bench.v']
    top = 'cql'
    extra.append(f'set_global_assignment -name SEARCH_PATH {repo}/designs/p7/cpu')
q = [f'set_global_assignment -name FAMILY "Agilex 5"',
     f'set_global_assignment -name DEVICE A5EB013BB23BE4SCS',
     f'set_global_assignment -name TOP_LEVEL_ENTITY {top}']
q += [f'set_global_assignment -name VERILOG_FILE {s}' for s in srcs] + extra
q += [f'set_global_assignment -name SDC_FILE {name}.sdc',
      'set_global_assignment -name DEVICE_INITIALIZATION_CLOCK OSC_CLK_1_125MHZ',
      'set_global_assignment -name PROJECT_OUTPUT_DIRECTORY output_files',
      'set_global_assignment -name PWRMGT_VOLTAGE_OUTPUT_FORMAT "LINEAR FORMAT"',
      'set_global_assignment -name PWRMGT_LINEAR_FORMAT_N "-12"',
      'set_global_assignment -name NUM_PARALLEL_PROCESSORS 2']
std = {'3.3': '3.3-V LVCMOS', '1.8': '1.8-V LVCMOS', '1.1': '1.1-V'}
for l in open(f'{repo}/{d["pins"]}'):
    f = l.split()
    if len(f) >= 3 and not f[0].startswith('#'):
        q.append(f'set_location_assignment PIN_{f[1]} -to {f[0]}')
        q.append(f'set_instance_assignment -name IO_STANDARD "{std[f[2]]}" -to {f[0]}')
open(f'{out}/{name}.qsf', 'w').write('\n'.join(q) + '\n')
open(f'{out}/{name}.qpf', 'w').write(f'PROJECT_REVISION = "{name}"\n')
open(f'{out}/{name}.sdc', 'w').write(f'create_clock -name CLOCK1_50 -period {period} [get_ports CLOCK1_50]\n')
