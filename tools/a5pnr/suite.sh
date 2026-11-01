#!/usr/bin/env bash
# Run the open flow and every check on designs, two at a time, and write one
# summary line per design (README (y)):
#   tools/a5pnr/suite.sh <out root> <designs/pN/name>...
# Each design is <dir>/<name>.v with pins in <dir>/<name>.pins; results go to
# <out root>/<name>/ and <out root>/summary.tsv.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
root=$(realpath -m "$1"); shift
mkdir -p "$root"
one() {
    local d=$1 name; name=$(basename "$d")
    local out=$root/$name
    "$here/flow.sh" "$name" "$repo/$d.pins" "$out" "$repo/$d.v" > "$root/flow_$name.log" 2>&1 \
        && "$here/check.sh" "$name" "$repo/$d.pins" "$out" "$repo/$d.v" > "$root/check_$name.log" 2>&1
    python3 -I - "$name" "$out" "$root" <<'PY'
import re, sys
name, out, root = sys.argv[1:]
f = open(f'{root}/flow_{name}.log').read()
c = open(f'{out}/check.txt').read() if __import__('os').path.exists(f'{out}/check.txt') else ''
g = lambda pat, s, d='?': (re.search(pat, s) or [None, d])[1]
times = dict(re.findall(r'^(\w+) (\d+) s$', open(f'{out}/times.txt').read(), re.M)) if __import__('os').path.exists(f'{out}/times.txt') else {}
ext = 'yes' if c and all(m == ('0', '0') for m in re.findall(r'only in design (\d+), only in image (\d+)', c)) and 'extract' in c else 'NO'
row = [name,
       g(r'a5pnr: (\d+) ALMs', f), g(r'ALMs in (\d+) LABs', f), g(r'(\d+) M20Ks, \d+ nets', f),
       g(r'(\d+) routing iterations', f), g(r'model Fmax (\d+) MHz', f),
       times.get('pnr', '?'), times.get('build', '?'), times.get('rbf', '?'),
       ext, g(r'(\d+) mismatches', c), g(r'(\d+) differ\n', c),
       'yes' if "a5db's equals quartus_pfg's" in c else 'NO', 'OK' if 'Integrity status: OK' in c else 'NO']
open(f'{root}/summary.tsv', 'a').write('\t'.join(row) + '\n')
print('\t'.join(row))
PY
}
[[ -f $root/summary.tsv ]] || printf 'design\tALMs\tLABs\tM20K\titer\tFmax\tpnr_s\tbuild_s\trbf_s\textract=design\tsim_mismatch\tasm2_differ\tpfg_identical\tintegrity\n' > "$root/summary.tsv"
while (( $# )); do
    one "$1" &
    if (( $# > 1 )); then one "$2" & shift; fi
    shift
    wait
done
column -t "$root/summary.tsv"
