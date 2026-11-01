#!/usr/bin/env bash
# The ALM model against Quartus' own image (README (z3)): decode a compiled
# run's .sof, rebuild its netlist with a5pnr readback, and simulate it
# against the RTL from power-up (the RTL at its initial values, the image at
# its 0s; Quartus' retimed flops left out of the map).
#   tools/alm/oracle.sh <run dir with atoms.txt (tools/alm/atoms.tcl)> <top> <pins> <out dir> <cycles> <rtl.v>...
# SIMCHECK_HOLD as for simcheck.py (e.g. KEY0=1).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
run=$(realpath "$1"); top=$2; pins=$(realpath "$3"); out=$(realpath -m "$4"); cycles=$5; shift 5
db=$repo/db/sm4revb; graph=$repo/specimens/rg/full
mkdir -p "$out"
"$repo/tools/a5pnr/synth.sh" "$top" "$out/$top.json" "$@" > "$out/synth.log" 2>&1
# a5db's pin uses (README (p)) from the netlist's port directions; a clock
# pin is one that clocks a flop.
specs=$(python3 -I - "$out/$top.json" "$top" "$pins" <<'PY'
import json, sys
mod = json.load(open(sys.argv[1]))['modules'][sys.argv[2]]
clk = {b for c in mod['cells'].values() for p in ('C', 'CLK') for b in c['connections'].get(p, [])}
for l in open(sys.argv[3]):
    f = l.split()
    if not f or f[0].startswith('#'):
        continue
    name, ball, volt = f[:3]
    port, _, bit = name.partition('[')
    p = mod['ports'][port]
    b = p['bits'][int(bit.rstrip(']')) if bit else 0]
    kind = 'out' if p['direction'] == 'output' else 'clock_in' if b in clk else 'in'
    print(f'{ball}={kind}:{volt}')
PY
)
"$repo/tools/cap.sh" 12G "$repo/tools/a5db/target/release/a5db" extract "$db" "$graph" "$run/output_files/$top.sof" $specs > "$out/extract.txt" 2> "$out/extract.log"
"$repo/tools/cap.sh" 6G "$repo/tools/a5pnr/target/release/a5pnr" readback "$graph" "$db" "$out/extract.txt" "$pins" "$top" "$out/readback.v" 2> "$out/readback.log"
python3 -I "$here/ffmap.py" "$run/atoms.txt" > "$out/$top.ffmap"
SIMCHECK_RUN=$cycles python3 -I "$repo/tools/a5pnr/simcheck.py" "$out/$top.json" "$top" "$pins" "$out/$top.ffmap" "$out/readback.v" 0 "$@" | tail -3
