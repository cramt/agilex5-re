#!/usr/bin/env bash
# Rebuild every board-test file from this tree, run every check on it, and
# collect the files and their SHA-256s for tools/board/run.sh (README, "Board
# test"):
#   tools/board/build.sh [<work dir, default specimens/p12/board>]
# Files go to specimens/board/p12/ (git-ignored) with SHA256SUMS; paste that
# into the README's checksum block. One Quartus-checked design at a time (two
# Quartus jobs at most: check.sh's asm2 and pfg). Hours: run it as a unit,
#   systemd-run --user --unit=a5-board-build -p MemoryMax=24G tools/board/build.sh
set -u
cd "$(dirname "$0")/../.."
export CHIPSMITH_INSTALL_ROOT=${CHIPSMITH_INSTALL_ROOT:-/pool/dev/altera_pro}
export ORACLE_LICENSE_MAC=${ORACLE_LICENSE_MAC:-44:8a:5b:87:88:20} LM_LICENSE_FILE=${LM_LICENSE_FILE:-$HOME/.altera/agilex5.dat}
W=${1:-specimens/p12/board}; B=specimens/board/p12
Q=/pool/dev/altera_pro/quartus-pro/26.1.1; A=tools/a5db/target/release/a5db
C=/pool/dev/chipsmith/examples
mkdir -p "$W" "$B"
log() { echo "$(date +%T) $*" | tee -a "$W/build.log"; }

log "tools: cargo build --release"
for t in a5db a5pnr a5synth; do
    (cd tools/$t && ../cap.sh 12G nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo build --release -q) 2>&1 | tail -3
done

# Quartus' own blinky (phase 5's compile), through pfg: the loader control.
log "control: blinky_pfg.rbf"
cp specimens/board/blinky.sof "$W/blinky.sof"
tools/cap.sh 20G tools/quartus.sh quartus_pfg -c "$W/blinky.sof" "$W/blinky_pfg.rbf" > "$W/blinky_pfg.log" 2>&1
tools/cap.sh 12G $A rbf db/sm4revb "$W/blinky.sof" "$W/blinky_a5db.rbf" $Q keys/rbf_keyed_hash.key > /dev/null
cmp -s "$W/blinky_pfg.rbf" "$W/blinky_a5db.rbf" && log "control: a5db's .rbf equals pfg's" || log "control: a5db's .rbf DIFFERS from pfg's"

# The hand-placed counter from the database alone (designs/blinky_a5db),
# then its hash variants.
log "scratch"
tools/cap.sh 12G python3 -I designs/blinky_a5db/build.py "$W/scratch" > "$W/scratch.log" 2>&1
cp "$W/scratch/blinky_a5db.rbf" "$W/scratch.rbf"
tools/cap.sh 20G tools/quartus.sh quartus_pfg -c "$W/scratch/blinky_a5db.sof" "$W/scratch_pfg.rbf" > "$W/scratch_pfg.log" 2>&1
cmp -s "$W/scratch.rbf" "$W/scratch_pfg.rbf" && log "scratch: a5db's .rbf equals pfg's" || log "scratch: a5db's .rbf DIFFERS from pfg's"
tools/cap.sh 20G tools/readback.sh "$W/scratch/blinky_a5db.sof" "$W/scratch/asm2" 2>&1 | tail -1 | sed 's/^/scratch asm2: /' | tee -a "$W/build.log"
for v in zero-io-hash zero-design-hash corrupt-core; do
    tools/cap.sh 4G python3 -I tools/rbf_patch.py $v "$W/scratch.rbf" "$W/scratch_$v.rbf"
done
for f in blinky_pfg scratch scratch_zero-io-hash scratch_zero-design-hash scratch_corrupt-core; do
    tools/cap.sh 20G tools/quartus.sh quartus_pfg --check_integrity "$W/$f.rbf" 2>&1 | grep -h -E 'Integrity status|Section hash' | head -1 | sed "s/^/$f integrity: /" | tee -a "$W/build.log"
    tools/cap.sh 4G python3 -I tools/bitstream.py check "$W/$f.rbf" 2>&1 | tail -1 | sed "s/^/$f bitstream.py: /" | tee -a "$W/build.log"
done

# Every open-flow design, one at a time.
S=$W/s1
for d in designs/p6/blinky designs/p6/leds designs/p6/acc designs/p6/ram designs/p7/ctl designs/p7/arith \
         designs/p7/rom designs/p7/ramx designs/p7/pll designs/p9/rf designs/p10/io/io designs/p10/mul/mul designs/fr/fr; do
    log "suite: $d"
    SIMCHECK_RUN=3000 SIMCHECK_RUN_RANDOM=1 tools/a5pnr/suite.sh $S/suite $d > /dev/null
done
log "cpu (chipsmith's synthesis, MLAB register file)"
OUT=$S CYC=300 RUN=20000 bash designs/p7/cpu/go.sh cpu
log "cpu_ff (register file in flops)"
A5_MLAB=0 OUT=$S CYC=300 RUN=20000 bash designs/p7/cpu/go.sh cpu_ff
log "cpu_ys (yosys' synthesis)"
A5_SYNTH=yosys OUT=$S CYC=300 RUN=20000 bash designs/p7/cpu/go.sh cpu_ys
log "picorv32"
o=$S/picorv32; s="designs/p10/bench/picorv32_de25.v $C/picorv32/src/picorv32.v"
tools/a5pnr/flow.sh picorv32_de25 designs/p10/bench/picorv32_de25.pins $o $s > $o.flow.log 2>&1
SIMCHECK_CYCLES=0 SIMCHECK_RUN=20000 SIMCHECK_HOLD=KEY0=1 tools/a5pnr/check.sh picorv32_de25 designs/p10/bench/picorv32_de25.pins $o $s > $o.check.log 2>&1
log "sha256"
o=$S/sha256; s="designs/p10/bench/sha256_de25.v $C/sha256/src/sha256.v $C/sha256/src/sha256_core.v $C/sha256/src/sha256_k_constants.v $C/sha256/src/sha256_w_mem.v"
tools/a5pnr/flow.sh sha256_de25 designs/p10/bench/sha256_de25.pins $o $s > $o.flow.log 2>&1
SIMCHECK_CYCLES=300 SIMCHECK_RUN=5000 SIMCHECK_HOLD=KEY0=1 tools/a5pnr/check.sh sha256_de25 designs/p10/bench/sha256_de25.pins $o $s > $o.check.log 2>&1

# The readback's rules against Quartus' own images (README (z6), (z10)).
log "oracles"
bash tools/alm/oracles_p9.sh > "$W/oracles_p9.txt" 2>&1
bash tools/alm/oracles_p10.sh > "$W/oracles_p10.txt" 2>&1
# IO96 lane clocks against Quartus' own compile of io (board fixes, 2026-11-06).
tools/alm/ioclk_oracle.sh specimens/runs/qio0/output_files/io.sof $S/suite/io/io.design.txt 2>&1 | tail -1 | tee -a "$W/build.log"

log "collect"
cp "$W/blinky_pfg.rbf" "$W/scratch.rbf" "$W"/scratch_{zero-io-hash,zero-design-hash,corrupt-core}.rbf "$B/"
# The A/B pairs' old halves are history by design: the same counter and LAB
# controls under the LUT rule phase 8 corrected (README (z3)).
cp specimens/board/scratch_a5db.rbf "$B/scratch_p5.rbf"
cp specimens/board/p7/ctl.rbf "$B/ctl_p7.rbf"
for d in blinky leds acc ram ctl arith rom ramx pll rf io mul fr; do cp "$S/suite/$d/$d.rbf" "$B/$d.rbf"; done
for d in cpu cpu_ff cpu_ys; do cp "$S/$d/cpu.rbf" "$B/$d.rbf"; done
cp "$S/picorv32/picorv32_de25.rbf" "$B/picorv32.rbf"
cp "$S/sha256/sha256_de25.rbf" "$B/sha256.rbf"
(cd "$B" && sha256sum -- *.rbf > SHA256SUMS)
log "done"
echo done > "$W/done"
