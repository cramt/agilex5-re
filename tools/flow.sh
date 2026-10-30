#!/usr/bin/env bash
# Compile a design through syn, fit and asm with the sandboxed oracle, as a
# transient systemd user unit so it survives the calling shell and stays
# inside the shared machine's memory budget.
#
#   tools/flow.sh designs/fz specimens/runs/fz0          # starts unit a5-fz0
#   tools/flow.sh designs/fz specimens/runs/fz0 --wait   # same, in the foreground
#
# The flow's quartus_sta step is skipped: it can't load under chipsmith
# (README, "quartus_sta fails under chipsmith").
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
src=$(realpath "$1"); run=$(realpath -m "$2"); shift 2
top=$(basename "$(ls "$src"/*.qpf)" .qpf)
mkdir -p "$run"; cp -r "$src"/. "$run"/
unit=a5-$(basename "$run")
cat > "$run/flow.run" <<EOS
set -e
cd "$run"
for step in "quartus_syn $top" "quartus_fit $top" "quartus_asm $top"; do
    t0=\$(date +%s)
    "$here/quartus.sh" \$step > "\${step%% *}.log" 2>&1
    echo "\${step%% *} \$(( \$(date +%s) - t0 )) s" >> times.txt
done
EOS
args=(--user --unit="$unit" -p MemoryMax=20G -p MemorySwapMax=0 --collect
      --setenv=PATH="$PATH" --setenv=HOME="$HOME"
      --setenv=CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro
      --setenv=ORACLE_LICENSE_MAC=44:8a:5b:87:88:20
      --setenv=LM_LICENSE_FILE="$HOME/.altera/agilex5.dat")
[[ ${1:-} == --wait ]] && args+=(--wait --pipe)
systemd-run "${args[@]}" bash "$run/flow.run"
