#!/usr/bin/env bash
# Build the self-tests (README, "Self-tests") through the open flow and every
# board-file check (extract = design, simcheck from random states and from
# power-up, asm2, pfg identical, integrity):
#   designs/p14/build.sh [<out dir, default specimens/p14>] <name>...
# <name> is row, ramx_st, mlab_st, cpu_st, sha_st or keys; <name>_mut builds
# the same source with `localparam MUTATE = 0` set to 1, the deliberately
# broken variant that must show FAIL on the board. One line per design goes
# to <out>/summary.tsv; the .rbf is <out>/<name>/<top>.rbf.
set -uo pipefail
cd "$(dirname "$0")/../.."
out=specimens/p14
[[ $# -gt 0 && $1 != *_st* && $1 != row* && $1 != keys* ]] && { out=$1; shift; }
P=designs/p14; C=/pool/dev/chipsmith/examples/sha256/src
mkdir -p "$out"
for name in "$@"; do
    base=${name%_mut}; o=$out/$name
    # Extra sources and the simulation's held inputs per design.
    extra=(); hold=; run=20000
    case $base in
        cpu_st) extra=(designs/p7/cpu/femtorv32_quark.v) ;;
        sha_st) extra=($C/sha256.v $C/sha256_core.v $C/sha256_k_constants.v $C/sha256_w_mem.v) ;;
        keys) hold=KEY0=1,KEY1=1 ;;
    esac
    mkdir -p "$o"
    src=$P/$base.v
    if [[ $name == *_mut ]]; then
        grep -q 'localparam MUTATE = 0;' "$src" || { echo "$src has no MUTATE knob"; exit 1; }
        sed 's/localparam MUTATE = 0;/localparam MUTATE = 1;/' "$src" > "$o/$base.v"
        cp $P/*.vh "$o/" 2>/dev/null
        src=$o/$base.v
    fi
    srcs=("$src" $P/verdict.v "${extra[@]}")
    tools/a5pnr/flow.sh "$base" $P/$base.pins "$o" "${srcs[@]}" > "$o/flow.log" 2>&1 || { echo "$name: flow FAILED (tail $o/flow.log)"; continue; }
    SIMCHECK_CYCLES=300 SIMCHECK_RUN=$run SIMCHECK_HOLD=$hold \
        tools/a5pnr/check.sh "$base" $P/$base.pins "$o" "${srcs[@]}" > "$o/check.log" 2>&1
    c=$o/check.txt
    line="$name	$(grep -o -E '[0-9]+ ALMs' "$o/flow.log" | head -1)	sim: $(grep -o -E '[0-9]+ mismatches' "$c" | tr '\n' ' ')	asm2: $(grep -o -E '[0-9]+ differ' "$c" | head -1)	$(grep -q "a5db's equals quartus_pfg's" "$c" && echo pfg-identical || echo PFG-DIFFERS)	$(grep -q 'Integrity status: OK' "$c" && echo integrity-OK || echo INTEGRITY-NOT-OK)	$(sha256sum "$o/$base.rbf" | cut -c1-16)"
    echo "$line" | tee -a "$out/summary.tsv"
done
