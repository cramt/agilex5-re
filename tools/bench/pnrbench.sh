#!/usr/bin/env bash
# a5pnr variants on synthesized netlists, several seeds, three at a time:
# the model's register-to-register Fmax and the run time per build (README
# (z14)).
#   tools/bench/pnrbench.sh <out dir> <seeds> <name>=<a5pnr binary>... -- <design>:<netlist.json>:<top>:<pins>...
# Writes <out>/pnr.tsv: variant, design, seed, MHz (register to register),
# MHz (longest path), place s, route s.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
out=$(realpath -m "$1"); seeds=$2; shift 2
variants=(); while [[ $1 != -- ]]; do variants+=("$1"); shift; done; shift
mkdir -p "$out"
db=$repo/db/sm4revb
jobs=()
for v in "${variants[@]}"; do for d in "$@"; do for s in $(seq 1 "$seeds"); do jobs+=("$v|$d|$s"); done; done; done
one() {
    IFS='|' read -r v d s <<< "$1"
    IFS='=' read -r vn bin <<< "$v"; IFS=: read -r dn json top pins <<< "$d"
    log=$out/$vn.$dn.$s.log
    [[ -s $log ]] || "$repo/tools/cap.sh" 4G "$bin" "$db" "$repo/specimens/rg/full" "$db/timing/delays.tsv" "$json" "$top" "$pins" "$out/$vn.$dn.$s.txt" --seed "$s" ${A5PNR_OPTS:-} > /dev/null 2> "$log"
    rm -f "$out/$vn.$dn.$s.txt"
}
export -f one; export out repo db
printf '%s\n' "${jobs[@]}" | xargs -P "${PAR:-3}" -I{} bash -c 'one "$@"' _ {}
printf 'variant\tdesign\tseed\tr2r_mhz\tlongest_mhz\tplace_s\troute_s\n' > "$out/pnr.tsv"
for j in "${jobs[@]}"; do
    IFS='|' read -r v d s <<< "$j"; vn=${v%%=*}; dn=${d%%:*}
    python3 -I - "$out/$vn.$dn.$s.log" "$vn" "$dn" "$s" >> "$out/pnr.tsv" <<'PY'
import re, sys
t = open(sys.argv[1]).read()
g = lambda p: (re.search(p, t) or [None, '?'])[1]
print('\t'.join([sys.argv[2], sys.argv[3], sys.argv[4], g(r'register to register [\d.]+ ns, (\d+) MHz'), g(r'model Fmax (\d+) MHz'), g(r'place ([\d.]+) s'), g(r'route ([\d.]+) s')]))
PY
done
column -t "$out/pnr.tsv"
