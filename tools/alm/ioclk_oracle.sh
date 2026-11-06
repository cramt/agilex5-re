#!/usr/bin/env bash
# The I/O registers' clock entries a5pnr routes against the ones Quartus
# routes for the same design and pins (README (z10), board fixes):
#   tools/alm/ioclk_oracle.sh <quartus .sof> <a5pnr design.txt>
# Prints both sets of VIO/HIOL register clock entries (GLOBAL_LOCAL_SEL into
# an I/O block's clock RPI) and exits 1 if they differ. Phase 10 clocked IO96
# lane 4's registers from lane 5's input clock (X106); every a5pnr check
# passed, because they share a5pnr's model, and the LEDs on DN25/DP25/DP30
# held their reset value on the board. Quartus' io (specimens/runs/qio0)
# would have shown X99.
set -euo pipefail
cd "$(dirname "$0")/../.."
sof=$1; design=$2
A=tools/a5db/target/release/a5db; G=specimens/rg/full
# A register clock entry: a GLOBAL_LOCAL_SEL whose next hop is an I/O
# routing tile's clock RPI (VIO I3 on row 5, HIOL I5 on rows 9 and 49).
pick() { grep -oE 'GLOBAL_LOCAL_SEL:X[0-9]+Y(5Z0I3|9Z0I5|49Z0I5)\b' | sort -u; }
q=$(tools/cap.sh 12G "$A" routed db/sm4revb "$G" "$sof" | pick)
o=$(grep -P '^edge\t' "$design" | pick)
echo "quartus: $(echo $q)"
echo "a5pnr:   $(echo $o)"
if [[ $q == "$o" ]]; then echo "ioclk: same entries"; else echo "ioclk: DIFFER"; exit 1; fi
