#!/bin/sh
# Check every schema against Quartus's own PDB_ASCII_DUMP trace.
#   tools/ddb/verify/run.sh [specimens/pdbdump/devices] [quartus devices dir]
# For each (database, schema) pair: decode with `ddb trace`, turn the
# .ddb.dmp into the same token stream, and require identical streams, the
# body consumed to its last byte, and a consistent pointer <-> address map.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
dumps=${1:-$here/../../../specimens/pdbdump/devices}
devices=${2:-/pool/dev/altera_pro/quartus-pro/26.1.1/devices}
ddb=$here/../target/release/ddb
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0
while read -r db schema; do
    [ -z "$db" ] && continue
    n=$(basename "$db")
    if [ ! -f "$dumps/$db.ddb.dmp" ]; then echo "SKIP $db (no trace)"; continue; fi
    python3 -I "$here/trace_tokens.py" "$dumps/$db.ddb.dmp" > "$tmp/$n.trace"
    if "$ddb" trace "$devices/$db.ddb" "$here/../schema/$schema.schema" > "$tmp/$n.rust" 2> "$tmp/$n.err" \
        && python3 -I "$here/tokcmp.py" "$tmp/$n.rust" "$tmp/$n.trace" > "$tmp/$n.cmp"; then
        echo "OK   $db ($schema): $(tail -1 "$tmp/$n.cmp")"
    else
        echo "FAIL $db ($schema)"; cat "$tmp/$n.err" "$tmp/$n.cmp" 2>/dev/null | tail -5; fail=1
    fi
done <<'LIST'
10nm/sm4revb/ddb_sm4revb_eb_b_b795b_csvpbga795b_pkg pkg
10nm/sm4revb/ddb_z1698c_pad pad
10nm/ddb_sundancemesa_pad pad
10nm/sm4revb/ddb_sm4revb_eb_b_b795b_cs_hdr hdr
10nm/sm4revb/ddb_sm4revb_place place
LIST
exit $fail
