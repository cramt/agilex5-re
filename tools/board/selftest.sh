#!/usr/bin/env bash
# Load one DE25-Nano file and read its verdict off the webcam (README,
# "Self-tests"): the whole LED row as one box, BLINK <Hz> (PASS), LIT (FAIL)
# or DARK.
#   tools/board/selftest.sh <file.rbf> [<label>] [<settle s, default 3>]
#   tools/board/selftest.sh --restore     camera back to auto exposure
# The clip is kept in specimens/p14/cam/<label>.mkv. BOX is the row's box in
# full-resolution pixels; it moves whenever the camera does: find it again
# with cam.py lockin on a clip of row.rbf (1 Hz). LIT/DARK are the box's mean
# on row.rbf's lit and dark half-periods.
set -euo pipefail
cd "$(dirname "$0")/../.."
BOX=${BOX:-830 0 890 86}
LIT=${LIT:-88} DARK=${DARK:-13}
FF=${FFMPEG:-$(cat /tmp/cam/ffpath)}; PY=${CAM_PY:-$(cat /tmp/cam/pypath)}; V4L=${V4L2:-$(cat /tmp/cam/v4l2path)}
if [[ $1 == --restore ]]; then
    "$V4L" -d /dev/video0 -c auto_exposure=3,brightness=0,white_balance_automatic=1
    exit
fi
rbf=$1; label=${2:-$(basename "$rbf" .rbf)}; settle=${3:-3}
mkdir -p specimens/p14/cam
"$V4L" -d /dev/video0 -c auto_exposure=1,exposure_time_absolute=100,white_balance_automatic=0,brightness=0
CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro chipsmith-vendor run --backend quartus-pro --version 26.1.1 \
    quartus_pgm -- -c 'DE25-Nano [3-10-iface0]' -m jtag -o "p;$rbf" > "specimens/p14/cam/$label.pgm.log" 2>&1 || true
if ! grep -q 'Configuration succeeded' "specimens/p14/cam/$label.pgm.log"; then
    echo "$label: LOAD FAILED"; grep -E '^Error' "specimens/p14/cam/$label.pgm.log" | head -3; exit 1
fi
sleep "$settle"
FFMPEG=$FF "$PY" -I tools/board/cam.py record "specimens/p14/cam/$label.mkv" 10
# shellcheck disable=SC2086
FFMPEG=$FF "$PY" -I tools/board/cam.py trace "specimens/p14/cam/$label.mkv" $BOX --lit "$LIT" --dark "$DARK" | sed "s/^/$label: /"
