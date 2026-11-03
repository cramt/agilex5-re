#!/usr/bin/env bash
# yosys (tools/a5pnr/synth.sh) against chipsmith's synthesis (tools/a5synth)
# on a design list (README (z12)): LUTs, flops, carry halves, MLAB bits and
# the ALMs a5pnr packs them into (--pack-only).
#   tools/a5synth/compare.sh <out dir> <name>:<top>:<pins>:<src.v,...>[:MLAB=0] ...
# Writes <out>/compare.tsv.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
out=$(realpath -m "$1"); shift
mkdir -p "$out"
db=$repo/db/sm4revb; graph=$repo/specimens/rg/full
printf 'design\tsynth\tluts\tflops\tarith\tmlab_bits\talms\tlabs_min\tseconds\n' > "$out/compare.tsv"
for spec in "$@"; do
    IFS=: read -r name top pins srcs opt <<< "$spec"
    IFS=, read -ra files <<< "$srcs"
    for s in yosys chipsmith; do
        d=$out/$name.$s; mkdir -p "$d"
        t0=$SECONDS
        env ${opt:+A5_$opt} A5_SYNTH=$s "$repo/tools/a5pnr/synth.sh" "$top" "$d/$top.json" "${files[@]}" > "$d/synth.log" 2>&1 || { echo "$name $s: synthesis failed" >&2; continue; }
        secs=$((SECONDS - t0))
        "$repo/tools/cap.sh" 6G "$repo/tools/a5pnr/target/release/a5pnr" "$db" "$graph" "$db/timing/delays.tsv" "$d/$top.json" "$top" "$pins" "$d/x.txt" --pack-only 2> "$d/pack.log" >/dev/null
        python3 -I - "$d/$top.json" "$top" "$d/pack.log" "$name" "$s" "$secs" >> "$out/compare.tsv" <<'PY'
import json, re, sys
mod = json.load(open(sys.argv[1]))['modules'][sys.argv[2]]
t = [c['type'] for c in mod['cells'].values()]
luts = sum(x == '$lut' for x in t); ffs = sum(x.startswith('$_DFF') or x.startswith('$_SDFF') for x in t)
ar = sum(x == 'A5_ARITH' for x in t); ml = sum(x == '$__A5_MLAB_' for x in t)
m = re.search(r'into (\d+) ALMs', open(sys.argv[3]).read())
print('\t'.join(map(str, [sys.argv[4], sys.argv[5], luts, ffs, ar, ml, m[1] if m else '?', (int(m[1]) + 9) // 10 if m else '?', sys.argv[6]])))
PY
    done
done
column -t "$out/compare.tsv"
