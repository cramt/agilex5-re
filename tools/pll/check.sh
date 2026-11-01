#!/usr/bin/env bash
# IOPLL bits from frequencies against a Quartus image (README (v)):
#   tools/pll/check.sh <quartus.sof> <out dir> <ref ball>:<ref MHz>:<out MHz>[,...] ...
# Writes the PLLs' design-file lines (iopll.py), builds them over the idle
# image with a5db, and compares every named bit under xpllwrap_top of each
# PLL's hviowr instance with the Quartus image.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
A=$repo/tools/a5db/target/release/a5db; db=$repo/db/sm4revb
sof=$(realpath "$1"); out=$(realpath -m "$2"); shift 2; mkdir -p "$out"
Q=${QUARTUS_ROOT:-/pool/dev/altera_pro/quartus-pro/26.1.1}
echo "# IOPLLs predicted by tools/pll/iopll.py" > "$out/design.txt"
uids=()
for s in "$@"; do
    IFS=: read -r ball ref outs <<< "$s"
    python3 -I "$here/iopll.py" "$ball" "$ref" "$outs" >> "$out/design.txt"
    uids+=("$(awk -F'\t' -v b="$ball" '$1 == b {print $2}' "$db/io/pins.tsv")")
done
"$repo/tools/cap.sh" 12G "$A" build "$db" "$repo/specimens/rg/full" "$Q" "$repo/specimens/sof_hmac.key" "$out/design.txt" "$out/pred.sof" 2> "$out/build.log"
total=0; bad=0
for u in "${uids[@]}"; do
    "$repo/tools/cap.sh" 6G "$A" bits "$db" "$out/pred.sof" "$u" | grep '^xpllwrap_top' > "$out/$u.pred"
    "$repo/tools/cap.sh" 6G "$A" bits "$db" "$sof" "$u" | grep '^xpllwrap_top' > "$out/$u.quartus"
    n=$(wc -l < "$out/$u.pred"); d=$(diff "$out/$u.pred" "$out/$u.quartus" | grep -c '^<' || true)
    echo "$u: $n named bits, $d differ"; diff "$out/$u.pred" "$out/$u.quartus" | grep '^[<>]' | head -6 || true
    total=$((total + n)); bad=$((bad + d))
done
echo "total: $total named bits, $bad differ"
