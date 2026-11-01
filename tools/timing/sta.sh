#!/usr/bin/env bash
# Dump quartus_sta's detailed setup paths of a compiled run (README (u)):
#   tools/timing/sta.sh <run dir> <out dir> [npaths]
# Copies the run to <out dir>/run, writes <out dir>/paths.txt (report_timing
# -detail full_path -show_routing, slow model). quartus_sta needs libbrotlidec,
# which the install doesn't bundle and chipsmith's compat set lacks (README,
# "quartus_sta fails under chipsmith"); it is passed in on LD_LIBRARY_PATH.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
run=$(realpath "$1"); out=$(realpath -m "$2"); n=${3:-4000}
rev=$(basename "$(ls "$run"/*.qpf)" .qpf)
mkdir -p "$out"; rm -rf "$out/run"; cp -r "$run" "$out/run"
cat > "$out/run/sta.tcl" <<EOS
project_open $rev
create_timing_netlist -model slow
read_sdc
update_timing_netlist
foreach_in_collection c [get_clocks] {
    set name [get_clock_info -name \$c]
    report_timing -setup -to_clock \$name -npaths $n -detail full_path -show_routing -file "$out/paths_\$name.txt"
}
delete_timing_netlist
project_close
EOS
brotli=$(nix build --no-link --print-out-paths nixpkgs#brotli.lib)
cd "$out/run"
CHIPSMITH_INSTALL_ROOT=${CHIPSMITH_INSTALL_ROOT:-/pool/dev/altera_pro} LD_LIBRARY_PATH=$brotli/lib \
    systemd-run --user --scope -q -p MemoryMax=20G -p MemorySwapMax=0 \
    "$repo/tools/quartus.sh" quartus_sta -t sta.tcl > "$out/sta.log" 2>&1
grep -E 'Error|was successful' "$out/sta.log" | tail -2
