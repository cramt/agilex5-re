#!/usr/bin/env bash
# Map every block type's named bits to image addresses (bitmap.tcl, then
# bitmap.py), one type after another; keeps map.tsv and pairs.txt, deletes
# the intermediate images. Runs in the foreground; wrap it in systemd-run.
#
#   tools/bitmap/run.sh <base.sof> <outdir> [block_type...]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
base=$(realpath "$1"); out=$(realpath -m "$2"); shift 2
py=$(nix build --no-link --print-out-paths --impure --expr \
    'with import <nixpkgs> {}; python3.withPackages (p: [p.numpy])')/bin/python3
types=("$@")
if [[ ${#types[@]} -eq 0 ]]; then
    mkdir -p "$out"
    printf 'load_package asm2\nload_devices\nset h [open_sof %s]\nputs "TYPES [get_block_types [get_device_name $h]]"\n' "$base" > "$out/types.tcl"
    read -ra types < <(cd "$out" && "$here/../quartus.sh" quartus_asm -t types.tcl | sed -n 's/^TYPES //p')
fi
for t in "${types[@]}"; do
    [[ -s $out/$t/map.tsv ]] && continue
    echo "== $t $(date +%T)"
    (cd "$out" && "$here/../quartus.sh" quartus_asm -t "$here/bitmap.tcl" "$base" "$t" "$out/$t" > "$out/$t.asm.log" 2>&1)
    "$py" "$here/bitmap.py" "$base" "$out/$t" | tee "$out/$t.summary"
    rm -f "$out/$t"/*.sof
done
