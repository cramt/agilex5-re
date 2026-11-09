#!/usr/bin/env bash
# The HPS shell of an HPS-first Quartus build, as a5db design-file lines
# (README, "HPS-first"): from `a5db extract` of its .sof, keep the IO96 and
# pio lines (io96_1 is the HPS EMIF, phase 1's; io96_0 must stay what the IO
# section and its hash say) and the RPI inverters of the HPS boundary tiles
# (row 51, `cw_rt_vio_*`), which tie the HPS inputs no logic drives (an
# undriven input reads 1, INV makes it 0 [obs, GIC]). Quartus' logic, routing, DFT flops
# and clock delays are left out; a5pnr's design file goes on top.
#   tools/hps_shell.sh <extract.txt> > shell.txt
set -euo pipefail
awk -F'\t' '
    $1 == "raw" && $2 ~ /^io96_/ { print; next }
    ($1 == "attr" || $1 == "bit" || $1 == "direct") && $2 ~ /^pio_/ { print; next }
    $1 == "attr" && $2 ~ /^cw_rt_vio_(lab|mlab)_base_r51_/ && $3 ~ /rpi_sel$/ { print; next }
' "$1"
