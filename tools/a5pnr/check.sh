#!/usr/bin/env bash
# Every check on an open-flow build (README (w)):
#   tools/a5pnr/check.sh <top> <pins.txt> <out dir> <rtl.v>...
#  1. a5db extract of <top>.sof: the image read back as a design file; it must
#     hold exactly a5pnr's edges, attributes and DIRECT values (no bit/raw);
#  2. a5pnr readback rebuilds the netlist from that decode alone, and
#     simcheck.py simulates it against the RTL (random state and inputs);
#  3. tools/readback.sh: asm2 (Quartus' assembler) against a5db's decode;
#  4. quartus_pfg -c on the .sof (its .rbf must equal a5db's) and
#     quartus_pfg --check_integrity on a5db's .rbf; tools/bitstream.py check.
# Results go to <out>/check.txt. CHECK_NO_QUARTUS=1 skips 3 and 4 (while
# iterating, with Quartus busy elsewhere).
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
top=$1; pins=$(realpath "$2"); out=$(realpath "$3"); shift 3
db=$repo/db/sm4revb; graph=$repo/specimens/rg/full; A=$repo/tools/a5db/target/release/a5db
export CHIPSMITH_INSTALL_ROOT=${CHIPSMITH_INSTALL_ROOT:-/pool/dev/altera_pro}
r=$out/check.txt; : > "$r"
# The three parts are independent: asm2 and pfg run while extract and the
# simulation do (README (x)).
[[ -n ${CHECK_NO_QUARTUS:-} ]] && : > "$out/check.asm2" && : > "$out/check.pfg" || (
    "$repo/tools/readback.sh" "$out/$top.sof" "$out/asm2" 2>&1 | tail -3 | sed 's/^/asm2: /' > "$out/check.asm2"
) &
[[ -n ${CHECK_NO_QUARTUS:-} ]] || (
    cd "$out" && "$repo/tools/quartus.sh" quartus_pfg -c "$top.sof" "${top}_pfg.rbf" > pfg.log 2>&1
    "$repo/tools/quartus.sh" quartus_pfg --check_integrity "$top.rbf" > integrity.log 2>&1
    {
        grep -h -E 'Integrity status|rror' "$out/integrity.log" | sed 's/^/integrity: /'
        cmp -s "$out/$top.rbf" "$out/${top}_pfg.rbf" && echo "rbf: a5db's equals quartus_pfg's" || echo "rbf: a5db's differs from quartus_pfg's"
        python3 -I "$repo/tools/bitstream.py" check "$out/$top.rbf" 2>&1 | tail -1 | sed 's/^/bitstream.py: /'
    } > "$out/check.pfg"
) &
specs=$(grep '^pin' "$out/$top.design.txt" | cut -f2)
"$repo/tools/cap.sh" 14G "$A" extract "$db" "$graph" "$out/$top.sof" $specs > "$out/extract.txt" 2> "$out/extract.log"
python3 -I - "$out/$top.design.txt" "$out/extract.txt" "$db/idle.tsv" >> "$r" <<'PY'
import sys
def load(p):
    s = {}
    for l in open(p):
        f = l.rstrip('\n').split('\t')
        if f[0] in ('pin', 'edge', 'attr', 'direct', 'bit', 'raw'):
            s.setdefault(f[0], set()).add('\t'.join(f[1:]))
    return s
a, b = load(sys.argv[1]), load(sys.argv[2])
# A design's `bit` lines at their idle value (iopll.py writes whole fields)
# change nothing an image can show.
import lzma, os
db = os.path.join(os.path.dirname(os.path.abspath(sys.argv[3])), '')
idle_path = sys.argv[3]
txt = lzma.open(idle_path + '.xz', 'rt').read() if not os.path.exists(idle_path) else open(idle_path).read()
ones = {tuple(l.split('\t')[:2]) for l in txt.splitlines()[1:]}
def btype(uid):
    return uid.rsplit('_r', 1)[0] if '_r' in uid and uid.rsplit('_r', 1)[1].split('_c')[0].isdigit() else 'hviowr' if uid.startswith('hvio') else uid
a['bit'] = {x for x in a.get('bit', set()) if (x.split('\t')[2] == '1') != ((btype(x.split('\t')[0]), x.split('\t')[1]) in ones)}
# a5pnr writes only edges and settings that have bits, so the sets must be equal.
for k in ('pin', 'edge', 'attr', 'direct', 'bit', 'raw'):
    x, y = a.get(k, set()), b.get(k, set())
    print(f'extract {k}: design {len(x)}, image {len(y)}, only in design {len(x - y)}, only in image {len(y - x)}')
PY
"$repo/tools/cap.sh" 6G "$here/target/release/a5pnr" readback "$graph" "$db" "$out/extract.txt" "$pins" "$top" "$out/readback.v" 2>> "$r"
python3 -I "$here/simcheck.py" "$out/$top.json" "$top" "$pins" "$out/$top.design.txt.ffmap" "$out/readback.v" ${SIMCHECK_CYCLES:-2000} "$@" >> "$r" 2>&1
wait
cat "$out/check.asm2" "$out/check.pfg" >> "$r"
cat "$r"
