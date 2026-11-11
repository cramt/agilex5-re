#!/usr/bin/env bash
# Clear an SDM wedge on the DE25-Nano: reload phase 1 over JTAG (the HPS
# reboots), then wait for mercury's sshd on the tailnet.
set -euo pipefail
cd "$(dirname "$(realpath "$0")")/../.."
export CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro
chipsmith-vendor run --backend quartus-pro --version 26.1.1 quartus_pgm -- -c "DE25-Nano [3-10-iface0]" -m jtag -o "p;specimens/hps-first/out/spl11/hpsf.hps.rbf" 2>&1 | grep -E 'Info \(18943|Error' || true
for i in $(seq 1 30); do
    timeout 6 ssh -o BatchMode=yes -o ConnectTimeout=4 -i ~/.ssh/id_personal root@mercury.ts.cramt.dk true 2>/dev/null && { echo "mercury up"; exit 0; }
    sleep 5
done
echo "mercury did not come back" >&2; exit 1
