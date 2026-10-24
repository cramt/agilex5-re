#!/usr/bin/env bash
# Run one Quartus Pro tool with PDB_ASCII_DUMP=on in a copy of a compiled
# project, then move every <db>.ddb.dmp it wrote out of the install tree
# into <out>/devices/..., so the install stays pristine.
#
#   tools/pdbdump.sh <compiled-project-dir> <scratch-dir> <out-dir> <tool> <args...>
#   tools/pdbdump.sh /tmp/agilex5-re/blinky-run /pool/dev/agilex5-scratch/asmdump \
#       specimens/pdbdump quartus_asm blinky
#
# Needs the same environment as tools/quartus.sh (CHIPSMITH, licence vars).
set -euo pipefail
src=$1 work=$2 out=$3; shift 3
here=$(cd "$(dirname "$0")" && pwd)
devices=${QUARTUS_DEVICES:-/pool/dev/altera_pro/quartus-pro/26.1.1/devices}
out=$(mkdir -p "$out" && cd "$out" && pwd)

if find "$devices" -name '*.ddb.dmp' -print -quit | grep -q .; then
    echo "pdbdump.sh: stale .ddb.dmp files already in $devices; move them first" >&2
    exit 1
fi
rm -rf "$work"
mkdir -p "$work"
cp -a "$src/." "$work/"
echo 'PDB_ASCII_DUMP=on' > "$work/quartus.ini"
cd "$work"
status=0
"$here/quartus.sh" "$@" > "$work/tool.log" 2>&1 || status=$?
# Move the traces whether or not the tool succeeded.
(cd "$devices" && find . -name '*.ddb.dmp' -print0) | while IFS= read -r -d '' f; do
    mkdir -p "$out/devices/$(dirname "$f")"
    mv "$devices/$f" "$out/devices/$f"
    echo "moved $f"
done
exit $status
