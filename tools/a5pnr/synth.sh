#!/usr/bin/env bash
# Verilog → a5pnr's JSON netlist with yosys from nixpkgs (README (w)):
#   tools/a5pnr/synth.sh <top> <out.json> <design.v>...
# Cells: $lut (≤ 6 inputs), A5_ARITH carry chains ($alu, yosys/arith_map.v),
# flops with the LAB's controls (README (y): enable, synchronous clear that
# the enable gates, asynchronous clear of either polarity; anything else
# becomes LUT logic; flops power up at 0) and A5_M20K instances
# (yosys/cells.v), and memories that fit as MLAB RAM (yosys/mlab.txt;
# A5_MLAB=0 keeps them in flops). An MLAB reads with no clock, so a
# synchronous read stays the RTL's own register (-nordff): merged into the
# read port, yosys would rebuild it under a name of its own (README (z9)).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
top=$1; out=$2; shift 2
inc=""; for f in "$@"; do inc="$inc -I$(dirname "$(realpath "$f")")"; done
base=${out%.json}
nix shell nixpkgs#yosys -c yosys -q -l "$base.yosys.log" -p "
read_verilog -lib $here/yosys/cells.v
read_verilog$inc $*
synth -top $top -flatten -run :fine $( [[ ${A5_MLAB:-1} == 1 ]] && echo "-nordff" )
$( [[ ${A5_MLAB:-1} == 1 ]] && echo "memory_libmap -lib $here/yosys/mlab.txt" )
memory_map
opt -full
techmap -map $here/yosys/arith_map.v
techmap
opt -fast
dfflegalize -cell \$_DFF_P_ 0 -cell \$_DFFE_PP_ 0 -cell \$_SDFF_PP0_ 0 -cell \$_SDFFCE_PP0P_ 0 -cell \$_DFF_PP0_ 0 -cell \$_DFF_PN0_ 0 -cell \$_DFFE_PP0P_ 0 -cell \$_DFFE_PN0P_ 0
write_rtlil $base.pre.il
"
# LUT mapping two ways, keeping the one with fewer LUTs: muxcover first
# makes each 4:1 mux one 6-LUT, where abc alone built a register file's
# 32:1 read muxes from 2:1 muxes (the CPU: 2,036 → 1,395 LUTs), but it locks
# in mux trees abc would fold into random logic (fr: 505 → 848 ALMs), so
# neither wins everywhere (README (z4)).
map() {
    nix shell nixpkgs#yosys -c yosys -q -l "$base.$1.log" -p "
read_rtlil $base.pre.il
$2
abc -lut 6
opt_merge -share_all t:\$lut
opt_clean -purge
stat
write_json $base.$1.json
"
}
map abc "" & map mux4 "muxcover -mux4 -nodecode; techmap -map $here/yosys/mux4_map.v" & wait
luts() { local n; n=$(grep -E '^ +[0-9]+ +\$lut' "$base.$1.log" | tail -1 | awk '{print $1}'); echo "${n:-0}"; }
pick=abc; (( $(luts mux4) < $(luts abc) )) && pick=mux4
echo "synth: $(luts abc) LUTs with abc alone, $(luts mux4) with muxcover first; using $pick"
mv "$base.$pick.json" "$out"; cp "$base.$pick.log" "$base.yosys.log"; rm -f "$base".{abc,mux4}.json "$base.pre.il"
grep -E "^ +[0-9]+ +(cells|\$|A5_)|Number of cells|\$_|\$lut|A5_" "$base.yosys.log" | tail -12 || true
