#!/usr/bin/env bash
# Run a command in its own memory cgroup, so a runaway can't take the shared
# machine down (an uncapped python loop reached 20 GB and the OOM killer on
# 2026-10-31). No swap, so it fails fast at the cap.
#   tools/cap.sh <limit, e.g. 4G> <command> [args...]
set -euo pipefail
limit=$1; shift
exec systemd-run --user --scope -q -p MemoryMax="$limit" -p MemorySwapMax=0 -- "$@"
