#!/usr/bin/env bash
# Verilog → a5pnr's JSON netlist with yosys from nixpkgs (README (w)):
#   tools/a5pnr/synth.sh <top> <out.json> <design.v>...
# Cells: $lut (≤ 6 inputs), A5_ARITH carry chains ($alu, yosys/arith_map.v),
# $_DFF_P_ flops (enables and synchronous resets become LUT logic; flops
# power up at 0) and A5_M20K instances (yosys/cells.v).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
top=$1; out=$2; shift 2
nix shell nixpkgs#yosys -c yosys -q -l "${out%.json}.yosys.log" -p "
read_verilog -lib $here/yosys/cells.v
read_verilog $*
synth -top $top -flatten -run :fine
memory_map
opt -full
techmap -map $here/yosys/arith_map.v
techmap
opt -fast
dfflegalize -cell \$_DFF_P_ 0
abc -lut 6
opt_clean -purge
stat
write_json $out
"
grep -E "^ +[0-9]+ +(cells|\$|A5_)|Number of cells|\$_|\$lut|A5_" "${out%.json}.yosys.log" | tail -12 || true
