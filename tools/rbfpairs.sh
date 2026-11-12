#!/usr/bin/env bash
# Regression for a5db rbf: for every .sof in a list, write the .rbf and
# compare it with pfg's .rbf beside it (README (o)).
#   tools/rbfpairs.sh <list of .sof paths, relative to the list's directory>
# The 78 phase-4 pairs: tools/rbfpairs.sh specimens/core/pairs.txt
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd); repo=$(dirname "$here")
list=$(realpath "$1"); base=$(dirname "$list")
Q=${QUARTUS_ROOT:-/pool/dev/altera_pro/quartus-pro/26.1.1}
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
same=0; differ=0
while read -r s; do
    o=$out/x.rbf
    "$here/cap.sh" 4G "$repo/tools/a5db/target/release/a5db" rbf "$repo/db/sm4revb" "$base/$s" "$o" "$Q" "$repo/keys/rbf_keyed_hash.key"
    if cmp -s "$o" "$base/${s%.sof}.rbf"; then same=$((same + 1)); else differ=$((differ + 1)); echo "DIFF $s"; fi
done < "$list"
echo "identical $same, differ $differ"
[ "$differ" = 0 ]
