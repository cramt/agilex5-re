#!/usr/bin/env bash
# Apply a quartus_fit ECO script to a copy of a compiled run, then assemble,
# as a transient systemd user unit (a5-eco-<name>). Leaves <out>/eco.done.
#
#   tools/eco.sh specimens/runs/fz0 specimens/runs/eco/z zero.tcl
#
# The ECO script is plain ECO commands (no project_open); quartus_fit --eco
# loads the "final" snapshot, runs it and commits the result.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
base=$(realpath "$1"); out=$(realpath -m "$2"); script=$(realpath "$3")
top=$(basename "$(ls "$base"/*.qpf)" .qpf)
rm -rf "$out"; mkdir -p "$(dirname "$out")"; cp -r "$base" "$out"; cp "$script" "$out/eco.tcl"
rm -f "$out/eco.done" "$out/eco.failed"
cat > "$out/eco.run" <<EOS
cd "$out"
if "$here/quartus.sh" quartus_fit $top --eco eco.tcl > eco.log 2>&1 \
   && grep -q "Successfully committed final database" eco.log \
   && "$here/quartus.sh" quartus_asm $top > asm.log 2>&1; then
    touch eco.done
else
    touch eco.failed
fi
EOS
systemd-run --user --unit="a5-eco-$(basename "$out")-$$" -p MemoryMax=12G -p MemorySwapMax=0 --collect -q \
    --setenv=PATH="$PATH" --setenv=HOME="$HOME" \
    --setenv=CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro \
    --setenv=ORACLE_LICENSE_MAC=44:8a:5b:87:88:20 \
    --setenv=LM_LICENSE_FILE="$HOME/.altera/agilex5.dat" \
    bash "$out/eco.run"
