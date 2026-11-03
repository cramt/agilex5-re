#!/usr/bin/env bash
# a5pnr's open flow against Quartus Prime Pro on the same Verilog
# (docs/vs-quartus-pro.md), interleaved, each build in a systemd unit of its
# own so its wall and CPU time come from the unit's accounting:
#   tools/bench/vs-quartus-pro.sh <out> <reps> [design...]   (tools/bench/designs.tsv)
# Per rep and design: Quartus (syn, fit, asm) then a5pnr's flow (synth, pnr,
# build, rbf), the 1-minute load average before each. In rep 1 also
# quartus_sta on Quartus' fit, and a5pnr's placement exported (--quartus),
# routed by Quartus and timed by quartus_sta where the export takes it.
# Rows go to <out>/runs.tsv; tools/bench/table.py writes the tables.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
out=$(realpath -m "$1"); reps=$2; shift 2
export CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro ORACLE_LICENSE_MAC=44:8a:5b:87:88:20 LM_LICENSE_FILE=$HOME/.altera/agilex5.dat
mkdir -p "$out"
[[ -s $out/runs.tsv ]] || printf 'rep\tdesign\ttool\twall_s\tcpu_s\tload1\tstatus\tdir\n' > "$out/runs.tsv"
mapfile -t all < <(awk -F'\t' '!/^#/ && $1 != "name" {print $1}' "$here/designs.tsv")
designs=("${@:-${all[@]}}")
field() { awk -F'\t' -v n="$1" -v c="$2" 'NR==1{for(i=1;i<=NF;i++) h[$i]=i} $1==n{print $h[c]}' <(grep -v '^#' "$here/designs.tsv"); }
# GNU time: wall, and user + system time of every process the build waited
# for. The unit's own CPU accounting misses tools/cap.sh's scopes, which
# leave the unit's cgroup.
gtime=$(nix build --no-link --print-out-paths nixpkgs#time)/bin/time
unit() {  # unit [--setenv=...] <log> <command...>: runs it, prints "wall cpu status"
    local env=()
    while [[ $1 == --setenv=* ]]; do env+=("$1"); shift; done
    local log=$1; shift
    systemd-run --user --wait --collect -q -p MemoryMax=20G -p MemorySwapMax=0 --working-directory="$repo" \
        --setenv=PATH="$PATH" --setenv=HOME="$HOME" --setenv=CHIPSMITH_INSTALL_ROOT --setenv=ORACLE_LICENSE_MAC --setenv=LM_LICENSE_FILE "${env[@]}" \
        "$gtime" -o "$log.time" -f '%e %U %S' "$@" > "$log" 2>&1
    local st=$?
    read -r e u sy < <(tail -n1 "$log.time")
    echo "$e $(python3 -I -c "print(f'{$u + $sy:.1f}')") $st"
}
for rep in $(seq 1 "$reps"); do
    for d in "${designs[@]}"; do
        top=$(field "$d" top); pins=$repo/$(field "$d" pins); env=$(field "$d" env); [[ $env == - ]] && env=""
        IFS=, read -ra srcs <<< "$(field "$d" sources | sed "s|\$C|/pool/dev/chipsmith/examples|g")"
        srcs=("${srcs[@]/#designs/$repo/designs}")
        # Quartus (BENCH_A5PNR_ONLY=1: a5pnr's side only, Quartus' own
        # builds taken from an earlier bench of the same RTL).
        q=$out/q/$d; qr=$out/runs/q_${d}_r$rep
        mkdir -p "$out/runs"
        if [[ -z ${BENCH_A5PNR_ONLY:-} ]]; then
            python3 -I "$here/qproj.py" "$repo" "$d" "$q"
            rm -rf "$qr"; cp -r "$q" "$qr"
            load=$(cut -d' ' -f1 /proc/loadavg)
            read -r w c st <<< "$(unit "$qr.unit.log" bash -c "cd $qr && for s in quartus_syn quartus_fit quartus_asm; do $repo/tools/quartus.sh \$s $d > \$s.log 2>&1 || exit 1; done")"
            printf '%s\t%s\tquartus\t%s\t%s\t%s\t%s\t%s\n' "$rep" "$d" "$w" "$c" "$load" "$st" "$qr" >> "$out/runs.tsv"
        fi
        # a5pnr.
        ar=$out/runs/a_${d}_r$rep; rm -rf "$ar"
        opts=""
        if [[ $rep == 1 ]]; then opts="--quartus $out/runs/x_$d"; rm -rf "$out/runs/x_$d"; mkdir -p "$out/runs/x_$d"; fi
        load=$(cut -d' ' -f1 /proc/loadavg)
        read -r w c st <<< "$(unit ${env:+--setenv=A5_${env#A5_}} --setenv=A5PNR_OPTS="$opts" "$ar.unit.log" "$repo/tools/a5pnr/flow.sh" "$top" "$pins" "$ar" "${srcs[@]}")"
        printf '%s\t%s\ta5pnr\t%s\t%s\t%s\t%s\t%s\n' "$rep" "$d" "$w" "$c" "$load" "$st" "$ar" >> "$out/runs.tsv"
        if [[ $rep == 1 ]]; then
            [[ -n ${BENCH_A5PNR_ONLY:-} ]] || "$repo/tools/timing/sta.sh" "$qr" "$out/sta/q_$d" 1 > /dev/null 2>&1
            # a5pnr's placement, every atom locked, routed by Quartus; a LAB
            # its legalizer refuses is left to it (README (z13), (z14)).
            # BENCH_NO_XFIT=1: the exports only (fit them later, one by one).
            if [[ -z ${BENCH_NO_XFIT:-} && -s $out/runs/x_$d/$top.qsf ]]; then
                systemd-run --user --scope -q -p MemoryMax=20G -p MemorySwapMax=0 "$here/unlock_fit.sh" "$out/runs/x_$d" "$top" "$out/runs/xf_$d" "$out/sta/x_$d" > "$out/runs/xf_$d.log" 2>&1
            fi
        fi
    done
done
