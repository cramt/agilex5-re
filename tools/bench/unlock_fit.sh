#!/usr/bin/env bash
# Route an exported a5pnr placement (a5pnr --quartus) with Quartus, leaving
# each LAB its legalizer rejects ("too many LAB line inputs", README (z8),
# (z13)) to Quartus, one per round, up to ROUNDS (10) rounds; then
# quartus_sta. CONTINUE=1 goes on from an earlier run's work dir.
#   tools/bench/unlock_fit.sh <export dir> <top> <work dir> <sta out>
# Writes <work dir>/unlocked.txt (the LABs left to Quartus).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
x=$(realpath "$1"); top=$2; w=$(realpath -m "$3"); sta=$(realpath -m "$4")
if [[ -z ${CONTINUE:-} ]]; then
    rm -rf "$w"; mkdir -p "$w"; cp "$x/$top.v" "$x/$top.sdc" "$x/$top.qpf" "$x/$top.qsf" "$w/"
    [[ -f $x/a5pnr_timing.txt ]] && cp "$x/a5pnr_timing.txt" "$w/"
    : > "$w/unlocked.txt"
fi
cd "$w"
[[ -n ${CONTINUE:-} ]] || "$repo/tools/quartus.sh" quartus_syn "$top" > quartus_syn.log 2>&1 || exit 1
for round in $(seq 1 "${ROUNDS:-10}"); do
    if "$repo/tools/quartus.sh" quartus_fit "$top" > quartus_fit.log 2>&1; then
        "$repo/tools/timing/sta.sh" "$w" "$sta" 1
        exit 0
    fi
    lab=$(grep -oP 'Error \(20931\).*LAB\): LAB_\KX[0-9]+_Y[0-9]+' quartus_fit.log | head -1 | tr -d ' ')
    [[ -z $lab ]] && { echo "fit failed for another reason" >&2; exit 1; }
    echo "$lab" >> unlocked.txt
    grep -v -E "set_location_assignment (LABCELL|FF|MLABCELL)_${lab}_" "$top.qsf" > q.tmp && mv q.tmp "$top.qsf"
done
exit 1
