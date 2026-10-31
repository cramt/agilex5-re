#!/usr/bin/env bash
# Map a large block type by extrapolation: brute-force the instances on the
# given rows, probe 4 reference bits on every instance, pack db/<die>/bits.
#
#   tools/bitmap/xrun.sh <base.sof> <workdir> <type> <row>...
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
base=$(realpath "$1"); work=$(realpath -m "$2"); t=$3; shift 3
py=$(nix build --no-link --print-out-paths --impure --expr \
    'with import <nixpkgs> {}; python3.withPackages (p: [p.numpy])')/bin/python3
w=$work/$t; mkdir -p "$w"
rows=" $* "
awk -F'\t' -v t="$t" -v rows="$rows" '$2==t && index(rows, " "$5" ") {print $1}' \
    "$root/db/sm4revb/floorplan.tsv" > "$w/subset.txt"
echo "== $t: $(wc -l < "$w/subset.txt") subset instances, rows$rows $(date +%T)"
if [[ ! -s $w/subset/map.tsv ]]; then
    "$here/../quartus.sh" quartus_asm -t "$here/bitmap.tcl" "$base" "$t" "$w/subset" "$w/subset.txt" > "$w/subset.asm.log" 2>&1
    "$py" "$here/bitmap.py" "$base" "$w/subset"
    rm -f "$w/subset"/*.sof
fi
python3 -I "$here/layout.py" "$w/subset/map.tsv"
python3 -I "$here/extrapolate.py" refbits "$w/subset/map.tsv" "$w/ref.txt" 4
"$here/../quartus.sh" quartus_asm -t "$here/bitmap.tcl" "$base" "$t" "$w/probe" - "$w/ref.txt" > "$w/probe.asm.log" 2>&1
"$py" "$here/bitmap.py" "$base" "$w/probe"
rm -f "$w/probe"/*.sof
python3 -I "$here/extrapolate.py" pack "$w/subset/map.tsv" "$w/probe/map.tsv" "$root/db/sm4revb/bits/$t"
python3 -I "$here/extrapolate.py" check "$root/db/sm4revb/bits/$t" "$w/subset/map.tsv"
