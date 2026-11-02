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
# Seconds from systemd's "1h 2min 3.456s" / "141ms".
secs() { python3 -I -c 'import re,sys; s=sys.argv[1]; t=sum(float(v)*{"h":3600,"min":60,"s":1,"ms":.001,"us":1e-6}[u] for v,u in re.findall(r"([\d.]+)(h|min|ms|us|s)", s)); print(f"{t:.1f}")' "$1"; }
unit() {  # unit <log> <command...>: runs it, prints "wall cpu status"
    local log=$1; shift
    systemd-run --user --wait --collect -p MemoryMax=20G -p MemorySwapMax=0 --working-directory="$repo" \
        --setenv=PATH="$PATH" --setenv=HOME="$HOME" --setenv=CHIPSMITH_INSTALL_ROOT --setenv=ORACLE_LICENSE_MAC --setenv=LM_LICENSE_FILE \
        "$@" > "$log" 2>&1
    local st=$?
    echo "$(secs "$(grep -oP 'Service runtime: \K.*' "$log")") $(secs "$(grep -oP 'CPU time consumed: \K.*' "$log")") $st"
}
for rep in $(seq 1 "$reps"); do
    for d in "${designs[@]}"; do
        top=$(field "$d" top); pins=$repo/$(field "$d" pins); env=$(field "$d" env); [[ $env == - ]] && env=""
        IFS=, read -ra srcs <<< "$(field "$d" sources | sed "s|\$C|/pool/dev/chipsmith/examples|g")"
        srcs=("${srcs[@]/#designs/$repo/designs}")
        # Quartus.
        q=$out/q/$d; qr=$out/runs/q_${d}_r$rep
        python3 -I "$here/qproj.py" "$repo" "$d" "$q"
        rm -rf "$qr"; mkdir -p "$out/runs"; cp -r "$q" "$qr"
        load=$(cut -d' ' -f1 /proc/loadavg)
        read -r w c st <<< "$(unit "$qr.unit.log" bash -c "cd $qr && for s in quartus_syn quartus_fit quartus_asm; do $repo/tools/quartus.sh \$s $d > \$s.log 2>&1 || exit 1; done")"
        printf '%s\t%s\tquartus\t%s\t%s\t%s\t%s\t%s\n' "$rep" "$d" "$w" "$c" "$load" "$st" "$qr" >> "$out/runs.tsv"
        # a5pnr.
        ar=$out/runs/a_${d}_r$rep; rm -rf "$ar"
        opts=""
        if [[ $rep == 1 ]]; then opts="--quartus $out/runs/x_$d"; rm -rf "$out/runs/x_$d"; mkdir -p "$out/runs/x_$d"; fi
        load=$(cut -d' ' -f1 /proc/loadavg)
        read -r w c st <<< "$(unit "$ar.unit.log" ${env:+--setenv=A5_${env#A5_}} --setenv=A5PNR_OPTS="$opts" "$repo/tools/a5pnr/flow.sh" "$top" "$pins" "$ar" "${srcs[@]}")"
        if [[ $st != 0 && -n $opts ]]; then
            # The export refuses MLAB RAM, DSPs and I/O registers: time it without.
            rm -rf "$out/runs/x_$d"
            read -r w c st <<< "$(unit "$ar.unit.log" ${env:+--setenv=A5_${env#A5_}} "$repo/tools/a5pnr/flow.sh" "$top" "$pins" "$ar" "${srcs[@]}")"
        fi
        printf '%s\t%s\ta5pnr\t%s\t%s\t%s\t%s\t%s\n' "$rep" "$d" "$w" "$c" "$load" "$st" "$ar" >> "$out/runs.tsv"
        if [[ $rep == 1 ]]; then
            "$repo/tools/timing/sta.sh" "$qr" "$out/sta/q_$d" 1 > /dev/null 2>&1
            if [[ -d $out/runs/x_$d && -s $out/runs/x_$d/$top.qsf ]]; then
                x=$out/runs/x_$d
                ( cd "$x" && for s in quartus_syn quartus_fit; do systemd-run --user --scope -q -p MemoryMax=20G -p MemorySwapMax=0 "$repo/tools/quartus.sh" $s "$top" > $s.log 2>&1 || exit 1; done )
                "$repo/tools/timing/sta.sh" "$x" "$out/sta/x_$d" 1 > /dev/null 2>&1
            fi
        fi
    done
done
