#!/usr/bin/env bash
# Dump a compiled run's fitted atoms (tools/alm/atoms.tcl) for the oracle
# (tools/alm/oracle.sh), in a copy of the run so the run itself stays as
# Quartus left it. The ECO commits nothing, so the copy's .sof is the run's.
#   tools/alm/atoms.sh <run dir> <revision> <out dir>
# Runs Quartus in a systemd unit `atoms-<out dir name>` capped at 20G;
# waits for it and leaves atoms.txt, eco.log and eco.done/eco.failed.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
run=$(realpath "$1"); rev=$2; out=$(realpath -m "$3")
[[ -e $out ]] && { echo "$out exists" >&2; exit 1; }
cp -a "$run" "$out"
cp "$here/atoms.tcl" "$out/eco.tcl"
cat > "$out/eco.run" <<EOF
cd "$out"
if "$repo/tools/quartus.sh" quartus_fit $rev --eco eco.tcl > eco.log 2>&1 && [[ -s atoms.txt ]]; then
    touch eco.done
else
    touch eco.failed
fi
EOF
unit=atoms-$(basename "$out")
systemd-run --user --unit="$unit" -q -p MemoryMax=20G -p MemorySwapMax=0 \
    --setenv=CHIPSMITH_INSTALL_ROOT="$CHIPSMITH_INSTALL_ROOT" \
    --setenv=ORACLE_LICENSE_MAC="$ORACLE_LICENSE_MAC" \
    --setenv=LM_LICENSE_FILE="$LM_LICENSE_FILE" \
    --setenv=PATH="$PATH" \
    bash "$out/eco.run"
echo "started $unit"
