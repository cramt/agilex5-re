#!/usr/bin/env bash
# simcheck every built self-test (designs/p14/build.sh) again, and the
# negative control: each self-test's image against its mutant's RTL, which
# must mismatch, or simcheck can't see the one thing the test is about.
#   designs/p14/simcheck_all.sh [<out dir, default specimens/p14>]
# One line per pair to <out>/simcheck_all.tsv.
set -uo pipefail
cd "$(dirname "$0")/../.."
out=${1:-specimens/p14}; P=designs/p14; C=/pool/dev/chipsmith/examples/sha256/src
: > "$out/simcheck_all.tsv"
check() {   # <label> <image dir> <base> <rtl top source>
    local label=$1 o=$2 base=$3 src=$4 extra=() hold=
    case $base in
        cpu_st) extra=(designs/p7/cpu/femtorv32_quark.v) ;;
        sha_st) extra=($C/sha256.v $C/sha256_core.v $C/sha256_k_constants.v $C/sha256_w_mem.v) ;;
        keys) hold=KEY0=1,KEY1=1 ;;
    esac
    local r
    r=$(SIMCHECK_RUN=20000 SIMCHECK_HOLD=$hold tools/cap.sh 12G python3 -I tools/a5pnr/simcheck.py \
        "$o/$base.json" "$base" $P/$base.pins "$o/$base.design.txt.ffmap" "$o/readback.v" 300 \
        "$src" $P/verdict.v "${extra[@]}" 2>&1 | grep -o -E '[0-9]+ mismatches' | tr '\n' ' ')
    echo "$label	$r" | tee -a "$out/simcheck_all.tsv"
}
for name in row row_mut ramx_st ramx_st_mut mlab_st mlab_st_mut cpu_st cpu_st_mut sha_st sha_st_mut keys; do
    base=${name%_mut}; o=$out/$name
    [[ -f $o/readback.v ]] || { echo "$name	not built" | tee -a "$out/simcheck_all.tsv"; continue; }
    src=$P/$base.v; [[ $name == *_mut ]] && src=$o/$base.v
    check "$name" "$o" "$base" "$src"
    # The negative control: the good image against the mutant RTL.
    [[ $name != *_mut && -f $out/${name}_mut/$base.v ]] && check "$name image vs ${name}_mut RTL (must mismatch)" "$o" "$base" "$out/${name}_mut/$base.v"
done
