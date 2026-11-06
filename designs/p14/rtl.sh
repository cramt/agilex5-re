#!/usr/bin/env bash
# RTL simulation of a self-test or its mutant (tb.v): designs/p14/rtl.sh <name>[_mut] [cycles]
set -euo pipefail
cd "$(dirname "$0")/../.."
name=$1; base=${name%_mut}; cyc=${2:-20000}; P=designs/p14; d=$(mktemp -d)
src=$P/$base.v
[[ $name == *_mut ]] && { sed 's/localparam MUTATE = 0;/localparam MUTATE = 1;/' "$src" > "$d/$base.v"; cp $P/*.vh "$d/" 2>/dev/null || true; src=$d/$base.v; }
extra=()
C=/pool/dev/chipsmith/examples/sha256/src
case $base in
    cpu_st) extra=(designs/p7/cpu/femtorv32_quark.v) ;;
    sha_st) extra=($C/sha256.v $C/sha256_core.v $C/sha256_k_constants.v $C/sha256_w_mem.v) ;;
esac
iv=github:NixOS/nixpkgs/b134951a4c9f3c995fd7be05f3243f8ecd65d798#verilog
tools/cap.sh 8G nix shell $iv -c iverilog -g2012 -I$P -I"$(dirname "$src")" -DTOP=$base -DCYCLES=$cyc -o "$d/sim" \
    $P/tb.v "$src" $P/verdict.v tools/a5pnr/yosys/cells_sim.v "${extra[@]}" 2>&1 | grep -E '\.v:[0-9]+:' || true
tools/cap.sh 8G nix shell $iv -c vvp -n "$d/sim" 2>/dev/null | grep -v -E "^VCD|finish" | sed "s/^/$name: /"
rm -rf "$d"
