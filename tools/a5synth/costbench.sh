#!/usr/bin/env bash
# chipsmith's LUT mapper under LUT cost tables (A5_LUTCOST, README (z14)):
# per design and table the LUTs by input count, the mapper's depth and the
# ALMs a5pnr packs them into (--pack-only).
#   tools/a5synth/costbench.sh <out dir> <c0,..,c6> [<c0,..,c6> ...]
# Designs: the bench's (tools/bench/designs.tsv) but blinky, leds and acc,
# plus fr. Writes <out>/cost.tsv.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
out=$(realpath -m "$1"); shift
mkdir -p "$out"
db=$repo/db/sm4revb; graph=$repo/specimens/rg/full
designs=(cpu cpu_ff sha256 picorv32 mul fr)
field() { awk -F'\t' -v n="$1" -v c="$2" 'NR==1{for(i=1;i<=NF;i++) h[$i]=i} $1==n{print $h[c]}' <(grep -v '^#' "$repo/tools/bench/designs.tsv"); }
[[ -s $out/cost.tsv ]] || printf 'cost\tdesign\tluts\tl1-4\tl5\tl6\tdepth\talms\n' > "$out/cost.tsv"
for cost in "$@"; do
    for d in "${designs[@]}"; do
        if [[ $d == fr ]]; then
            top=fr; pins=$repo/designs/fr/fr.pins; env=-; srcs=("$repo/designs/fr/fr.v")
        else
            top=$(field "$d" top); pins=$repo/$(field "$d" pins); env=$(field "$d" env)
            IFS=, read -ra srcs <<< "$(field "$d" sources | sed "s|\$C|/pool/dev/chipsmith/examples|g")"
            srcs=("${srcs[@]/#designs/$repo/designs}")
        fi
        [[ $env == - ]] && env=""
        w=$out/${cost//,/}/$d; mkdir -p "$w"
        env ${env:+A5_${env#A5_}} A5_LUTCOST="$cost" "$repo/tools/cap.sh" 12G "$here/target/release/a5synth" "$top" "$w/$top.json" "${srcs[@]}" > "$w/synth.log" 2>&1 \
            || { echo "$d $cost: synthesis failed" >&2; continue; }
        "$repo/tools/cap.sh" 6G "$repo/tools/a5pnr/target/release/a5pnr" "$db" "$graph" "$db/timing/delays.tsv" "$w/$top.json" "$top" "$pins" "$w/x.txt" --pack-only 2> "$w/pack.log" >/dev/null
        python3 -I - "$w/$top.json" "$top" "$w/pack.log" "$w/synth.log" "$cost" "$d" >> "$out/cost.tsv" <<'PY'
import json, re, sys
mod = json.load(open(sys.argv[1]))['modules'][sys.argv[2]]
n = [len(c['connections']['A']) for c in mod['cells'].values() if c['type'] == '$lut']
m = re.search(r'into (\d+) ALMs', open(sys.argv[3]).read())
dep = re.search(r'\(depth (\d+)\)', open(sys.argv[4]).read())
print('\t'.join(map(str, [sys.argv[5], sys.argv[6], len(n), sum(k <= 4 for k in n), n.count(5), n.count(6), dep[1] if dep else '?', m[1] if m else '?'])))
PY
    done
done
column -t "$out/cost.tsv"
