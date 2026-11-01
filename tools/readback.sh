#!/usr/bin/env bash
# Every attribute of every instance an image changes from the idle state,
# decoded by a5db and read by Quartus's assembler (asm2), side by side.
#   tools/readback.sh <in.sof> <out dir>
# Writes <out>/idiff.txt (a5db idle-diff), <out>/a5db.tsv, <out>/asm2.tsv and
# prints the attributes where the two differ (README (n)).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$here")
A=$repo/tools/a5db/target/release/a5db; db=$repo/db/sm4revb
sof=$(realpath "$1"); out=$(realpath -m "$2"); mkdir -p "$out"
"$repo/tools/cap.sh" 12G "$A" idle-diff "$db" "$sof" > "$out/idiff.txt"
"$repo/tools/cap.sh" 12G "$A" attrs-of "$db" "$sof" $(cut -f1 "$out/idiff.txt" | grep -v -e '^total$' -e '^unnamed') > "$out/a5db.tsv"
cut -f1,2 "$out/a5db.tsv" | tr '\t' ' ' > "$out/list.txt"
(cd "$out" && "$here/quartus.sh" quartus_asm -t "$here/rgdump/getattrs.tcl" "$sof" list.txt) | grep -P '^\S+\t' > "$out/asm2.tsv"
python3 -I - "$out" <<'PY'
import sys
out = sys.argv[1]
a = {tuple(l.split('\t')[:2]): l.rstrip('\n').split('\t')[2] for l in open(f'{out}/a5db.tsv')}
q = {tuple(l.split('\t')[:2]): l.rstrip('\n').split('\t')[2] for l in open(f'{out}/asm2.tsv')}
same = sum(1 for k in a if q.get(k) == a[k])
undecodable = sum(1 for k in a if a[k] == '?')
diff = [(k, a[k], q.get(k)) for k in a if q.get(k) != a[k] and a[k] != '?']
print(f'{len(a)} attributes in {len({k[0] for k in a})} instances: {same} equal, {undecodable} not decodable by a5db, {len(diff)} differ')
for k, x, y in diff[:40]:
    print('  ', *k, 'a5db', x, 'asm2', y)
PY
