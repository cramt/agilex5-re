#!/usr/bin/env bash
# The board test, one file at a time, most telling failure first (README,
# "Board test"):
#   tools/board/run.sh [--dry-run] [--from <step>] [--answers <step>=<y|n>,...]
# Loads each file into the DE25-Nano's SRAM over its USB-Blaster III
# (Quartus Prime Pro's quartus_pgm through chipsmith-vendor; see below),
# prints what the LEDs should do, and asks y/n plus an optional note. Results
# go to specimens/board/runs/<time>.tsv. A gating step that fails stops the
# run with a diagnosis. Every file's SHA-256 is checked against the README
# first; --dry-run does that and walks every step without JTAG, answering y
# (or what --answers says, to exercise the gates).
set -uo pipefail
cd "$(dirname "$0")/../.."
B=specimens/board/p12
dry=0; from=""; answers=""
while (( $# )); do
    case $1 in
        --dry-run) dry=1 ;;
        --from) from=$2; shift ;;
        --answers) answers=",$2," ; shift ;;
        *) echo "usage: $0 [--dry-run] [--from <step>] [--answers <step>=<y|n>,...]" >&2; exit 2 ;;
    esac
    shift
done

# The checksums are the README's, so the files on the bench are the ones its
# text describes.
sums=$(sed -n '/^<!-- board-sha256 begin -->$/,/^<!-- board-sha256 end -->$/p' README.md | grep -E '^[0-9a-f]{64}  ')
[[ -n $sums ]] || { echo "no checksum block in README.md" >&2; exit 1; }
echo "== checksums (README) of $B"
(cd "$B" && sha256sum --quiet -c <<< "$sums") || { echo "files differ from the README's checksums: rebuild with tools/board/build.sh or fix the README" >&2; exit 1; }
echo "   $(wc -l <<< "$sums") files OK"

# The loader is Quartus Prime Pro's programmer (no licence needed, so outside
# tools/quartus.sh's network namespace: its jtagd needs loopback). On the board
# 2026-11-06 openFPGALoader 1.2.0 (tools/board/openfpgaloader.nix, cable
# de25-nano) read the chain but couldn't load even Quartus' blinky: its ISC
# configure poll races (300 polls, passed on a retry) and its transfer loop
# spins on the SDM's busy bit with no timeout. quartus_pgm loads .sof and .rbf
# and prints the SDM's verdict ("Configuration succeeded").
export CHIPSMITH_INSTALL_ROOT=${CHIPSMITH_INSTALL_ROOT:-/pool/dev/altera_pro}
q() { "${CHIPSMITH_VENDOR:-chipsmith-vendor}" run --backend quartus-pro --version 26.1.1 "$1" -- "${@:2}" 2>&1 | grep -v 'connect() failed'; return "${PIPESTATUS[0]}"; }

# The cable and its permission, before the first JTAG step. udev rule:
# nixconf modules/bundles/nixos-general.nix (09fb:6022|6026, plugdev).
# 09fb:6022 is the standalone cable; the DE25-Nano's onboard one enumerates
# as 09fb:6026 (an FT4232H, "TERASIC DE25-Nano"; JTAG on channel A with ADBUS4
# high), which jtagd names "DE25-Nano [<usb port>-iface0]".
pid=6022 port=0
if ! ((dry)); then
    usb=$(grep -lxE '6022|6026' /sys/bus/usb/devices/*/idProduct 2>/dev/null | while read -r f; do
        d=${f%/idProduct}; [[ $(cat "$d/idVendor") == 09fb ]] && printf '/dev/bus/usb/%03d/%03d %s %s\n' "$(cat "$d/busnum")" "$(cat "$d/devnum")" "$(cat "$f")" "${d##*/}"; done | head -1)
    port=${usb##* }; usb=${usb% *}; pid=${usb#* }; usb=${usb% *}
    [[ -n $usb ]] || { echo "no USB-Blaster III (09fb:6022 or 6026) on USB: board powered and its JTAG micro-USB plugged in with a data cable (a charge-only one shows nothing)?" >&2; exit 1; }
    [[ -w $usb ]] || { echo "$usb isn't writable: the 09fb:6022|6026 udev rule is in nixconf; run sudo nixos-rebuild switch and replug" >&2; exit 1; }
    echo "== cable: $usb (09fb:$pid)"
fi
case $pid in 6026) cable="DE25-Nano [$port-iface0]" ;; *) cable="USB-BlasterIII [$port]" ;; esac

mkdir -p specimens/board/runs
log=specimens/board/runs/$(date +%Y%m%d-%H%M%S)$( ((dry)) && echo -dry).tsv
printf 'step\tfile\tsha256\tloader_exit\tloader_error\tanswer\tnote\n' > "$log"
outs=${log%.tsv}; mkdir -p "$outs"
echo "== log: $log (loader output per step in $outs/)"

ids=$(grep -oE '^step [a-z0-9-]+' "$0" | cut -d' ' -f2)
[[ -z $from ]] || grep -qx -- "$from" <<< "$ids" || { echo "no step $from; steps: $(echo $ids)" >&2; exit 2; }
started=$([[ -z $from ]] && echo 1 || echo 0)
summary=()

# A step: <id> <gate> <file or -> <expected behaviour> <diagnosis if not>.
# gate: load = stop if the file doesn't configure; all = stop on that or a
# "n"; info = record and go on; idcode = the JTAG chain read.
step() {
    local id=$1 gate=$2 file=$3 expect=$4 diag=$5
    [[ $started == 1 || $id == "$from" ]] || return 0
    started=1
    echo; echo "== step $id: ${file/#-/IDCODE}"
    local rc=0 out="" sr="" sha="-"
    if [[ $gate == idcode ]]; then
        local cmd=(q jtagconfig --enum)
    else
        sha=$(sha256sum "$B/$file" | cut -c1-16)
        local cmd=(q quartus_pgm -c "$cable" -m jtag -o "p;$B/$file")
    fi
    echo "   \$ ${cmd[*]}"
    if ((dry)); then
        [[ $gate == idcode ]] && out="1) $cable"$'\n  4362C0DD   A5E(A013BB23B|B013BB23BCS)/.. (dry run)'
    else
        out=$("${cmd[@]}" 2>&1); rc=$?
        echo "$out" > "$outs/$id.txt"
        echo "$out" | tail -8 | sed 's/^/   | /'
    fi
    sr=$(grep -m1 -E '^Error' <<< "$out" | tr '\t' ' ')
    if [[ $gate == idcode ]]; then
        local id_read; id_read=$(grep -A1 -F "$cable" <<< "$out" | grep -oE '^ +[0-9A-F]{8}' | head -1 | tr -d ' ')
        id_read=${id_read:+0x${id_read,,}}
        printf '%s\t-\t-\t%s\t-\t%s\t\n' "$id" "$rc" "${id_read:-none}" >> "$log"
        summary+=("$id: IDCODE ${id_read:-none}")
        if [[ ${id_read,,} != 0x4362c0dd ]]; then
            echo; echo "STOP: $diag" | fold -s -w 78; echo "   (read: ${id_read:-nothing})"; finish 1
        fi
        echo "   IDCODE 0x4362c0dd, the A5EB013B"
        return 0
    fi
    echo "   expect: $expect" | fold -s -w 78 | sed '2,$s/^/           /'
    local ans note=""
    if ((dry)); then
        ans=$(grep -oE ",$id=[yn]," <<< "$answers" | cut -d= -f2 | tr -d ,)
        ans=${ans:-y}; echo "   (dry run) answer: $ans"
    else
        while :; do
            read -r -p "   as expected? [y/n] " ans < /dev/tty
            [[ $ans == [yn] ]] && break
        done
        read -r -p "   note (what you saw, Enter for none): " note < /dev/tty
    fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$file" "$sha" "$rc" "${sr:--}" "$ans" "$note" >> "$log"
    summary+=("$id: loader $rc, $ans${note:+ ($note)}")
    if [[ $rc != 0 && ( $gate == load || $gate == all ) ]]; then
        echo; echo "STOP: the file didn't configure (quartus_pgm exit $rc). $diag" | fold -s -w 78; finish 1
    fi
    if [[ $ans == n && $gate == all ]]; then
        echo; echo "STOP: $diag" | fold -s -w 78; finish 1
    fi
    [[ $ans == n ]] && echo "   NOTE: $diag" | fold -s -w 78 | sed '2,$s/^/         /'
    return 0
}
finish() {
    echo; echo "== summary (log $log)"; printf '   %s\n' "${summary[@]}"
    exit "$1"
}

# 1. The chain: the cable, the device, its IDCODE (README (n)).
step idcode idcode - "" \
    "no A5EB013B on the chain. No cable: is the USB-Blaster III (09fb:6026 on the DE25-Nano) in /sys/bus/usb/devices, and may this user open it (nixconf udev rule, plugdev)? jtagconfig --enum lists what jtagd sees. Another IDCODE: record it (the board read 0x4362c0dd on 2026-11-06; the image header holds 0xc362c0dd, README (n))."

# 2. The loader, with a Quartus-made image (phase 5's blinky through pfg).
step control all blinky_pfg.rbf \
    "LED0 blinks at about 1.5 Hz (on 0.67 s, off 0.67 s). KEY0 does nothing." \
    "Quartus' own image doesn't configure through quartus_pgm: the board's setup is the problem (MSEL: JTAG-compatible as shipped; power), not any bitstream here. It configured and blinked on 2026-11-06."

# 3. SDM acceptance of an a5db image with no Quartus in its history.
step scratch load scratch.rbf \
    "LED0 blinks at about 1.5 Hz, like step 2 (a hand-placed 25-bit counter, every bit from the database: idle state, pin tables, clock tree, LUTs)." \
    "Configures but LED0 doesn't blink: suspect the idle state (README (n)) or the clock path; the hash steps below still tell whether the SDM checks hashes. Refused: compare ISC_SR with step 2's; a5db's .rbf of Quartus' blinky equals pfg's byte for byte (README (o)), so the container is right and the image content (idle state) is what the SDM refused."

# 4. What the SDM checks: a hash zeroed, and a negative control.
step zero-io-hash info scratch_zero-io-hash.rbf \
    "Unknown, this is the question: answer y if LED0 blinks (the SDM ignores the IO section's hash), n if it is refused or dark (it checks it)." \
    "the SDM checks the IO hash (or the patched header broke something else): keep a5db's hashes exact."
step zero-design-hash info scratch_zero-design-hash.rbf \
    "Unknown, this is the question: answer y if LED0 blinks (the SDM ignores the design hash), n if it is refused or dark (it checks it)." \
    "the SDM checks the design hash."
step corrupt-core info scratch_corrupt-core.rbf \
    "Refused: quartus_pgm reports a configuration error and LED0 stays dark (one bit flipped in a hashed CORE page, hashes left stale). Answer y if refused." \
    "the SDM configured an image whose CORE page hash is stale: it doesn't check section hashes, so steps zero-io-hash and zero-design-hash prove less."

# 5. Phase 8's A/B pairs: the LUT rule (README (z3)).
step scratch-p5 info scratch_p5.rbf \
    "Configures, LED0 stays dark (phase 5's counter under the old, inverted LUT rule: its first bit can't toggle). Answer y if dark." \
    "The old file blinks: if step scratch blinked too, the counter happens to work under both rules; if step scratch was dark and this blinks, phase 8's LUT rule is wrong (README (z3)) and every LUT-based file below is suspect."
step ctl info ctl.rbf \
    "LED[1:0] count every 0.67 s while SW0 is up and hold while down; LED[3:2] count every 0.67 s, holding KEY1 sets them to 0 at the next tick; LED[5:4] count every 0.67 s on CLOCK2_50, holding KEY0 clears them at once; LED6 toggles every 0.67 s and is off while SW1 is up; LED7 follows LED6 (looks the same) and is off while KEY1 is held." \
    "the current LUT rule or LAB controls fail on silicon. If step scratch blinked: the LAB control lines (README (y)); if it was dark too: the LUT rule (README (z3)). Most designs below use both."
step ctl-p7 info ctl_p7.rbf \
    "Does NOT do what step ctl describes (phase 7's LAB-controls design, LUTs on inverted inputs): answer y if its LEDs misbehave against step ctl's description." \
    "phase 7's ctl works: the inverted LUT inputs didn't matter for its logic, or the LUT rule is wrong; compare with step ctl."

# 6. The newest builds (phase 12, from master).
step blinky all blinky.rbf \
    "LED0 blinks at about 1.5 Hz. KEY0 does nothing." \
    "a5pnr's placement and routing of a carry chain fail where the hand-placed counter (step scratch) worked: suspect the carry chain or a5pnr's clock route (DCM input 1, not Quartus' 2). Everything below is a5pnr's."
step leds info leds.rbf \
    "LED7 toggles every 1.3 s, LED6 twice as fast, lower LEDs too fast to see. Holding KEY0 turns them all off; holding KEY1 inverts them; SW0 up reverses their order." \
    "HVIO inputs (KEY0, KEY1), the IO96 input (SW0) or the eight IO96 outputs; note which part fails."
step acc info acc.rbf \
    "LED7 changes about every 0.3 s and lower LEDs faster, an irregular pattern; holding KEY0 clears them." \
    "a carry chain over three LABs (the chain into the LAB below, README (z6))."
step arith info arith.rbf \
    "LED[3:0] count in binary, LED0 toggling every 1.34 s; with SW3 down LED[7:4] = LED[3:0] + 2*SW2 + SW1 + SW0 (switches down: the same as LED[3:0]; SW0 up: one more); with SW3 up LED[7:4] = LED[3:0] - 3 mod 16." \
    "carry chains: adders with an inverted operand or carry-in."
step ram info ram.rbf \
    "LED[7:0] count up in binary at about 6 Hz. All off or frozen: the M20K isn't written or read." \
    "the M20K (512 x 32 simple dual port)."
step rom info rom.rbf \
    "One LED lit, running LED0 -> LED7 -> LED0 back and forth, 84 ms a step. All dark or all lit: the contents didn't load." \
    "M20K contents in the .rbf (README (y))."
step ramx info ramx.rbf \
    "SW0 up: LED[3:0] count in binary at about 3 Hz; LED[7:4] the same with SW3 up, with SW3 down whatever byte 3 held. SW0 down: the LEDs replay the address counter's previous pass (0 after power-up, every 87 s). Holding KEY1 freezes them." \
    "M20K write/read/byte enables or its output register."
step rf info rf.rbf \
    "LED[3:0] step every 0.17 s through 1 8 F 6 D 4 B 2 9 0 7 E 5 C 3 A; LED[7:4] count down F -> 0 every 2.7 s. SW0 up: LED[3:0] count up one step every 0.34 s and keep doing so after SW0 goes down. Holding KEY1: LED[7:4] show one value changing every 5.4 s." \
    "MLAB RAM. LED[3:0] dark or stuck: contents or read; pattern right but SW0 does nothing: the write side (CLKEN0, write address); LED[7:4] dark: the unrouted address bit of a 16-word RAM doesn't read 1 (README (z6))."
step pll info pll.rbf \
    "LED3 and LED7 toggle together every 0.67 s; LED[3:0] and LED[7:4] count at the same rate. LED[3:0] at half LED[7:4]'s rate: the PLL runs at 50, not 100 MHz; LED[3:0] dark: not running or not locked." \
    "the IOPLL (README (v))."
step io info io.rbf \
    "LED[3:0] follow SW[3:0] at once, except LED2 (takes SW2 once per 1.34 s) and LED3 (dark while KEY0 held). LED4 blinks with a 1.34 s period, LED5 0.67 s and freezes while KEY1 held, LED6 (0.34 s) and LED7 (0.17 s) dark while KEY0 held." \
    "I/O registers (README (z10)). No LED reacts: the IO96 output registers or their clock; LED[3:0] stuck while LED4 blinks: the input registers; LED5 doesn't freeze: an output register's enable (RPI inverter); LED6 not dark under KEY0: an inverted async clear; LED7: the active-low one; LED3: an input register's clear."
step mul info mul.rbf \
    "LED0 dark and LED1 toggling every 0.17 s (LED2 0.34 s, LED3 0.67 s): every product right. LED0 lit: a wrong product; LED[7:4] count wrong products up to 15." \
    "DSP blocks (README (z11)); LED1 not toggling: the DSP clock or the fabric."
step cpu-ff info cpu_ff.rbf \
    "FemtoRV32, register file in flops. SW0 down: LED[7:0] count up in binary, LED0 toggling every 0.25 s; SW0 up: the count runs at 1.1 MHz (LEDs look half lit). Holding KEY0 holds the CPU in reset (LEDs keep their value); releasing restarts the count (the first store shows 1)." \
    "the CPU. All dark: not fetching (M20K contents or read enable); stuck: the delay loop or the branch."
step cpu info cpu.rbf \
    "The same CPU with its register file in MLAB RAM: as step cpu-ff." \
    "cpu-ff worked and this didn't: the MLAB RAM (see step rf)."
step cpu-ys info cpu_ys.rbf \
    "The MLAB CPU synthesized by yosys instead of chipsmith: as step cpu-ff." \
    "one synthesis works and the other not: that synthesis (or, for cpu alone, a5synth's carry-chain and flop mapping)."
step picorv32 info picorv32.rbf \
    "PicoRV32 counting in a loop, LED shows its counter folded to 8 bits: LEDs look dimly lit, changing too fast to read. Holding KEY0 (reset) freezes them; releasing restarts." \
    "the bench's picorv32 (README (z14)); dark or static with KEY0 released: the CPU isn't running."
step sha256 info sha256.rbf \
    "SHA-256 hashing in a loop, LED shows the digest fold: LEDs flicker too fast to read. Holding KEY0 (reset) freezes them." \
    "the bench's sha256; dark or static: not hashing."
step fr info fr.rbf \
    "Random logic between registers, LEDs flicker too fast to read: answer y if it configures with no SDM error." \
    "an SDM error on a dense design."

finish 0
