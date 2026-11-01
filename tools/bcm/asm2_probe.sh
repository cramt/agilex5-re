#!/usr/bin/env bash
# Run asm2_probe.tcl to the end of the list, restarting after an attribute
# that crashes asm2 (it is recorded as "#crash <n>").
#   tools/bcm/asm2_probe.sh <in.sof> <list> <out.tsv>
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
sof=$(realpath "$1"); list=$(realpath "$2"); out=$(realpath -m "$3")
: > "$out"
total=$(grep -c . "$list")
first=0
while (( first < total )); do
    "$here/../cap.sh" 10G "$here/../quartus.sh" quartus_asm -t "$here/asm2_probe.tcl" "$sof" "$list" "$out" "$first" > /dev/null 2>&1 && break
    last=$(grep '^#start ' "$out" | tail -1 | cut -d' ' -f2)
    echo "#crash $last" >> "$out"
    first=$(( last + 1 ))
done
