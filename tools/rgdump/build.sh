#!/usr/bin/env bash
# Build librgdump.so against the Quartus install. gcc 13 matches the
# install's own libstdc++ (GLIBCXX_3.4.32), so std::string/vector/shared_ptr
# cross the boundary unchanged.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
Q=${QUARTUS_ROOT:-/pool/dev/altera_pro/quartus-pro/26.1.1}
out=${1:-$here/../../specimens/rg/librgdump.so}
mkdir -p "$(dirname "$out")"
nix shell nixpkgs#gcc13 -c g++ -std=c++17 -O2 -fPIC -shared -o "$out" "$here/rgdump.cpp" \
    -L"$Q/qcore/linux64" -L"$Q/quartus/linux64" \
    -l:libtcl8.6.so -l:libddb_dygr.so -l:libddb_dev.so -l:libddb_dmgr.so -l:libddb_dygr_qdl.so \
    -Wl,-z,defs -Wl,--allow-shlib-undefined
echo "$out"
