#!/usr/bin/env bash
# The CPU through the open flow and every check (README (z2)):
#   designs/p7/cpu/go.sh <run>   (in specimens/p7/cpu, or OUT=<dir>)
set -u
cd /pool/dev/agilex5-re-wt/phase3
out=${OUT:-specimens/p7/cpu}/$1
tools/a5pnr/flow.sh cpu designs/p7/cpu/cpu.pins $out designs/p7/cpu/cpu.v designs/p7/cpu/femtorv32_quark.v > $out.flow.log 2>&1
SIMCHECK_CYCLES=${CYC:-300} SIMCHECK_RUN=${RUN:-20000} SIMCHECK_HOLD=KEY0=1,SW0=1 \
    tools/a5pnr/check.sh cpu designs/p7/cpu/cpu.pins $out designs/p7/cpu/cpu.v designs/p7/cpu/femtorv32_quark.v > $out.check.log 2>&1
