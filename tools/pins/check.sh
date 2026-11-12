#!/usr/bin/env bash
# Held-out check of the pin template (README (p)): the I/O bits a5db predicts
# for a design's pins against the Quartus image of that design.
#   tools/pins/check.sh <quartus.sof> <out dir> <ball>=<use>[:<std>]...
# Writes <out>/pred.sof (the idle image plus the predicted pin bits) and
# <out>/io_diff.txt, every I/O bit (hviowr, io96 and unnamed io96 header
# bits) where the two differ; empty means the prediction is exact.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$(dirname "$here")")
A=$repo/tools/a5db/target/release/a5db; db=$repo/db/sm4revb
sof=$(realpath "$1"); out=$(realpath -m "$2"); shift 2; mkdir -p "$out"
blank=$repo/specimens/p5/blank.sof
[[ -f $blank ]] || "$repo/tools/cap.sh" 12G "$A" blank "$db" /pool/dev/altera_pro/quartus-pro/26.1.1 "$repo/keys/sof_hmac.key" "$blank"
"$repo/tools/cap.sh" 12G "$A" pins "$db" "$blank" "$out/pred.sof" "$repo/keys/sof_hmac.key" "$@"
"$repo/tools/cap.sh" 12G "$A" imgdiff "$db" "$out/pred.sof" "$sof" | grep -E '^(hvio|pio|unnamed io96)' > "$out/io_diff.txt" || true
echo "$(wc -l < "$out/io_diff.txt") I/O instances differ; $(awk -F'\t' '{s += $2} END {print s + 0}' "$out/io_diff.txt") bits"
