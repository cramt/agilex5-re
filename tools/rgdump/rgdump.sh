#!/usr/bin/env bash
# Dump the routing graph of A5EB013B (die sm4revb) with Quartus's own DYGR API.
#
#   tools/rgdump/rgdump.sh [outdir]      # default specimens/rg/full, ~2 min
#
# Writes nodes.tsv (gid - 0x80000000, DYGR name, fanout count), edges.bin
# (u32 src, u32 n, u32 dst[n] per node) and types.tsv (per element type:
# nodes, edges). Needs no project and no licence.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
out=$(realpath -m "${1:-$root/specimens/rg/full}")
lib=$("$here/build.sh" "$root/specimens/rg/librgdump.so" | tail -1)
mkdir -p "$out"
tcl=$(mktemp --suffix=.tcl)
cat > "$tcl" <<EOS
load_package advanced_device
load_device -part A5EB013BB23BE4SCS
load_die_info
load $lib Rgdump
rg_dump $out
EOS
cd "$out"
CHIPSMITH_INSTALL_ROOT=${CHIPSMITH_INSTALL_ROOT:-/pool/dev/altera_pro} \
    systemd-run --user --scope -q -p MemoryMax=16G -p MemorySwapMax=0 \
    "$root/tools/quartus.sh" quartus_fit -t "$tcl" 2>&1 | grep -E 'rg_|rror'
rm -f "$tcl"
