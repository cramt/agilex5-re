#!/usr/bin/env bash
# Every phase-9 oracle with its control holds (ORACLE_REUSE: synthesis and extract kept).
cd "$(dirname "$0")/../.."
run() { local d=$1 alm=$2 pins=$3 cyc=$4 hold=$5; shift 5; local dir=specimens/p9/oracle/${alm%0}; printf "%-4s %-55s " "$d" "$hold"; SIMCHECK_HOLD=$hold ORACLE_REUSE=1 tools/alm/oracle.sh specimens/p9/alm/$alm $d $pins $dir $cyc "$@" 2>&1 | grep -oE '[0-9]+ registers compared, [0-9]+ mismatches|rror.*' | tail -n1; }
for h in WE=0,RE=0 WE=1,RE=1 WE=1,RE=0 WE=0,RE=1; do run mq mq0 designs/mq/mq.pins 3000 $h designs/mq/mq.v; done
for h in KEY0=1,KEY1=1,EN=1 KEY0=0,KEY1=1,EN=1 KEY0=1,KEY1=0,EN=1 KEY0=1,KEY1=1,EN=0; do run ar ar0 designs/ar/ar.pins 3000 $h designs/ar/ar.v; done
for h in KEY0=1 KEY0=0; do run pl pl0 designs/pl/pl.pins 3000 $h designs/pl/pl.v designs/pl/pll_sim.v; done
for h in KEY0=0 KEY0=1; do run mi mi0 designs/p9/mi/mi.pins 3000 $h designs/p9/mi/mi.v; done
for h in CEA=1,CEB=1,SCLR=0,ACLR=0,ACLRN=1,CIN=1,DIN=1 CEA=0,CEB=0,SCLR=0,ACLR=0,ACLRN=1 CEA=1,CEB=1,SCLR=1,ACLR=0,ACLRN=1 CEA=1,CEB=1,SCLR=0,ACLR=1,ACLRN=0 CEA=0,CEB=1,SCLR=1,ACLR=0,ACLRN=1,CIN=1; do run oc oc0 designs/p9/oc/oc.pins 3000 $h designs/p9/oc/oc.v; done
# Quartus' CPU without retiming; its RTL copies in the oracle directory carry
# the device's power-up 0s (README (z6)).
O=specimens/p9/oracle/cqr
for h in KEY0=1,SW0=1 KEY0=1,SW0=0; do run cql cqr0 designs/p8/cql/cql.pins 3000 $h $O/rtl/cql.v $O/rtl/femtorv32_logic.v; done
