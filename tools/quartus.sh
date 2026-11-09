#!/usr/bin/env bash
# Run one Quartus Prime Pro 26.1.1 tool with no network, in the current directory.
#
#   tools/quartus.sh quartus_sh --flow compile blinky
#
# Agilex 5 needs Altera's no-cost licence, which is node-locked to a NIC MAC
# address. An isolated network namespace has no NIC, so with ORACLE_LICENSE_MAC
# set we create a `dummy` interface carrying that MAC: a driver with no wire
# and no route, it gives FlexLM a host id and nothing to talk to.
#
#   ORACLE_LICENSE_MAC=aa:bb:cc:dd:ee:ff LM_LICENSE_FILE=~/.altera/agilex5.dat \
#       tools/quartus.sh quartus_sh --flow compile blinky
set -euo pipefail

# Stage 1: a fresh user+net namespace, as root inside it so we may add the
# dummy link. Stage 2: drop back to our own uid before Quartus runs.
case "${ORACLE_NETNS:-}" in
"")
    exec env ORACLE_NETNS=1 ORACLE_UID="$(id -u)" ORACLE_GID="$(id -g)" \
        unshare --user --map-root-user --net "$0" "$@"
    ;;
1)
    # Platform Designer's generator (qsys-generate, run for IP_FILEs) talks
    # to its own workers over 127.0.0.1; a new namespace's lo starts down.
    ip link set lo up
    if [[ -n "${ORACLE_LICENSE_MAC:-}" ]]; then
        ip link add hostid0 address "$ORACLE_LICENSE_MAC" type dummy
        ip link set hostid0 addrgenmode none
        ip link set hostid0 up
    fi
    exec env ORACLE_NETNS=2 \
        unshare --user --map-user="$ORACLE_UID" --map-group="$ORACLE_GID" "$0" "$@"
    ;;
esac

# Anything but loopback and our own unaddressed dummy is a way out. Read the
# namespace's own link list: /sys/class/net still shows the host's NICs.
while read -r name; do
    [[ $name == lo ]] && continue
    if [[ $name == hostid0 ]] && ip -d link show hostid0 | grep -qw dummy \
        && [[ -z $(ip addr show dev hostid0 | grep inet) ]]; then
        continue
    fi
    echo "quartus.sh: refusing to run, network interface $name is visible" >&2
    exit 1
done < <(ip -o link show | awk -F': ' '{ sub(/@.*/, "", $2); print $2 }')

exec "${CHIPSMITH_VENDOR:-chipsmith-vendor}" run --backend quartus-pro --version 26.1.1 "$1" -- "${@:2}"
