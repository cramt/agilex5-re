#!/usr/bin/env bash
# Phase 10's oracles (README (z10), (z11)) with their control holds, the other
# inputs random every cycle (SIMCHECK_RUN_RANDOM). ORACLE_REUSE: synthesis
# and extract kept.
cd "$(dirname "$0")/../.."
run() { local d=$1 alm=$2 pins=$3 cyc=$4 hold=$5; shift 5; local dir=specimens/p10/oracle/${d}; printf "%-4s %-40s " "$d" "${hold:-(all random)}"; SIMCHECK_RUN_RANDOM=1 SIMCHECK_HOLD=$hold ORACLE_REUSE=1 tools/alm/oracle.sh $alm $d $pins $dir $cyc "$@" 2>&1 | grep -oE 'simcheck run: .*|rror.*' | grep -oE '[0-9]+ mismatches|rror.*' | tail -n1; }
# I/O registers: Quartus' iq, every control held each way, then all random;
# its RTL copy carries the device's power-up values (README (z10)).
O=specimens/p10/oracle/iq/rtl
for h in EN=1,ACLR=0,ACLRN=1,SCLR=0 EN=0,ACLR=0,ACLRN=1,SCLR=0 EN=1,ACLR=1,ACLRN=1,SCLR=0 EN=1,ACLR=0,ACLRN=0,SCLR=0 EN=1,ACLR=0,ACLRN=1,SCLR=1 ""; do
    run iq specimens/p10/alm/iq0 designs/p10/iq/iq.pins 3000 "$h" $O/iq.v; done
O=specimens/p10/oracle/ct/rtl
for h in CEA=1,CEB=1,SCLR=0,ACLR=0,ACLRN=1 ""; do run ct specimens/p9/alm/ct0 designs/ct/ct.pins 3000 "$h" $O/ct.v; done
