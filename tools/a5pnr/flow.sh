#!/usr/bin/env bash
# The open flow (README (w)): Verilog → yosys → a5pnr → a5db build → a5db rbf.
#   tools/a5pnr/flow.sh <top> <pins.txt> <out dir> <design.v>...
# Writes <out>/<top>.json (netlist), .design.txt (a5db design file, plus
# .ffmap), .sof and .rbf, and <out>/times.txt. Needs specimens/rg/full with
# usable.bin (tools/rgdump/rgdump.sh, then a5db usable) and the two keys from
# tools/sof.py key and tools/bitstream.py key in specimens/. A5PNR_OPTS goes to
# a5pnr (e.g. "--pack single").
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
top=$1; pins=$(realpath "$2"); out=$(realpath -m "$3"); shift 3
srcs=(); for f in "$@"; do srcs+=("$(realpath "$f")"); done
Q=${QUARTUS_ROOT:-/pool/dev/altera_pro/quartus-pro/26.1.1}
db=$repo/db/sm4revb; graph=$repo/specimens/rg/full
mkdir -p "$out"; : > "$out/times.txt"
step() { local name=$1; shift; local t0=$SECONDS; "$@"; echo "$name $((SECONDS - t0)) s" >> "$out/times.txt"; }
step synth "$here/synth.sh" "$top" "$out/$top.json" "${srcs[@]}"
step pnr "$repo/tools/cap.sh" 6G "${A5PNR_BIN:-$here/target/release/a5pnr}" "$db" "$graph" "$db/timing/delays.tsv" "$out/$top.json" "$top" "$pins" "$out/$top.design.txt" ${A5PNR_OPTS:-} 2> >(tee "$out/pnr.log" >&2)
step build "$repo/tools/cap.sh" 12G "$repo/tools/a5db/target/release/a5db" build "$db" "$graph" "$Q" "$repo/specimens/sof_hmac.key" "$out/$top.design.txt" "$out/$top.sof"
step rbf "$repo/tools/cap.sh" 8G "$repo/tools/a5db/target/release/a5db" rbf "$db" "$out/$top.sof" "$out/$top.rbf" "$Q" "$repo/specimens/rbf_keyed_hash.key"
cat "$out/times.txt"
