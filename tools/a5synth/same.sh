#!/usr/bin/env bash
# Two a5synth binaries (e.g. before and after moving the chipsmith pin) on
# every board and bench design: the netlists they write must be identical.
#   tools/a5synth/same.sh <a5synth A> <a5synth B> <out dir>
# Prints one line per design (same/DIFFERENT) and exits 1 on any difference.
set -uo pipefail
cd "$(dirname "$0")/../.."
a=$(realpath "$1") b=$(realpath "$2") out=$(realpath -m "$3")
C=/pool/dev/chipsmith/examples
mkdir -p "$out"
specs=()
for d in designs/p6/blinky designs/p6/leds designs/p6/acc designs/p6/ram designs/p7/ctl designs/p7/arith \
         designs/p7/rom designs/p7/ramx designs/p7/pll designs/p9/rf designs/p10/io/io designs/p10/mul/mul designs/fr/fr; do
    specs+=("$(basename $d):$(basename $d)::$d.v")
done
specs+=("cpu:cpu::designs/p7/cpu/cpu.v,designs/p7/cpu/femtorv32_quark.v"
        "cpu_ff:cpu:A5_MLAB=0:designs/p7/cpu/cpu.v,designs/p7/cpu/femtorv32_quark.v"
        "picorv32:picorv32_de25::designs/p10/bench/picorv32_de25.v,$C/picorv32/src/picorv32.v"
        "sha256:sha256_de25::designs/p10/bench/sha256_de25.v,$C/sha256/src/sha256.v,$C/sha256/src/sha256_core.v,$C/sha256/src/sha256_k_constants.v,$C/sha256/src/sha256_w_mem.v")
bad=0
for spec in "${specs[@]}"; do
    IFS=: read -r name top env srcs <<< "$spec"
    IFS=, read -ra files <<< "$srcs"
    for s in a b; do
        bin=$a; [[ $s == b ]] && bin=$b
        env $env tools/cap.sh 12G "$bin" "$top" "$out/$name.$s.json" "${files[@]}" > "$out/$name.$s.log" 2>&1 \
            || { echo "$name: $s failed (see $out/$name.$s.log)"; bad=1; }
    done
    if cmp -s "$out/$name.a.json" "$out/$name.b.json"; then
        echo "$name same $(sha256sum < "$out/$name.a.json" | cut -c1-16)"
    else
        echo "$name DIFFERENT"; bad=1
    fi
done
exit $bad
