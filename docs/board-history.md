# Board test, phases 4–11 (superseded), and the run of 2026-11-06

The board test as each phase from 4 to 11 prepared it, kept because the
phase statuses in the README cite its step numbers ("steps 27–30") and its
SHA-256 lists name the files of that time. **Superseded by
`tools/board/run.sh`** (README, "Board test"), which loads phase 12's
rebuilds of every design, from master, in one gated order. None of these
steps was run: the board stayed off through phase 11. The paths below
(`specimens/board/`, `specimens/board/p7/` … `p10/`) are git-ignored and
hold the files as each phase built them; the runbook uses only
`specimens/board/p12/`, plus two old halves of phase 8's A/B pairs
(`scratch_p5.rbf` is `specimens/board/scratch_a5db.rbf`, `ctl_p7.rbf` is
`specimens/board/p7/ctl.rbf`).
Letters in parentheses, (n) to (z14), are the README's findings.


The open writer's container and `CORE` output is now byte-identical to
pfg's (o), so a5db's `.rbf` of Quartus' blinky *is* `blinky_pfg.rbf`. The
open question is the from-scratch design (n): does an image built only from
the database configure, and does its counter blink LED0 at about 1.5 Hz
(2^25 cycles of CLOCK1_50 per period; KEY0 does nothing)? Then come the
hash questions: whether the SDM checks the IO and design hashes. The files
are in `specimens/board/` (git-ignored); to regenerate them:

```
B=specimens/board; Q=/pool/dev/altera_pro/quartus-pro/26.1.1; A=tools/a5db/target/release/a5db
cp <blinky run>/output_files/blinky.sof $B/blinky.sof
tools/quartus.sh quartus_pfg -c $B/blinky.sof $B/blinky_pfg.rbf          # control, pfg's own
tools/cap.sh 12G python3 -I designs/blinky_a5db/build.py specimens/b5/r5  # 10 min
cp specimens/b5/r5/blinky_a5db.sof $B/scratch.sof
$A rbf db/sm4revb $B/scratch.sof $B/scratch_a5db.rbf $Q specimens/rbf_keyed_hash.key
tools/quartus.sh quartus_pfg -c $B/scratch.sof $B/scratch_pfg.rbf        # equals scratch_a5db.rbf
for v in zero-io-hash zero-design-hash corrupt-core; do
    python3 -I tools/rbf_patch.py $v $B/scratch_a5db.rbf $B/scratch_a5db_$v.rbf; done
for f in $B/*.rbf; do python3 -I tools/bitstream.py check $f; done      # all pass but corrupt-core
```

SHA-256 of the files as built here (phase 5, `specimens/b5/r5`):
`blinky_pfg.rbf` 8b490e78…, `scratch_a5db.rbf` = `scratch_pfg.rbf`
27642610…, `scratch_a5db_zero-io-hash.rbf` 8fb79437…,
`scratch_a5db_zero-design-hash.rbf` 0a8364c3…,
`scratch_a5db_corrupt-core.rbf` e2446c93…. `quartus_pfg --check_integrity`
says OK for all but corrupt-core (`Section hash mismatch`), asm2 reads the
scratch image back with 0 of 25,516 attributes differing, and the `.rbf`
from a5db equals pfg's. **Phase 5 changed the scratch image** in exactly
122 bits: `flop_mode = FAST_WIRE` on the 122 routing muxes it uses, which
every Quartus image sets and phase 4's left at DIS (q). The phase-4 files
are kept in `specimens/board/old_r4/` and the phase-3 ones (the all-literal
`CORE` of blinky) in `specimens/board/old/`; if step 4 fails with the
phase-5 file, the phase-4 one tells whether DIS was the reason.

On the board (DE25-Nano, on-board USB-Blaster III, JTAG; MSEL as shipped):

1. `quartus_pgm -l` and `quartus_pgm -c 1 -a` (through `tools/quartus.sh`
   with USB passed through) to see the cable and read the IDCODE. It
   should be `0xC362C0DD` (n). Add it for `A5EB013B` to openFPGALoader's
   `src/part.hpp` next to `0x4369c0dd` (A5EC008B), as an Agilex 5 entry.
2. Control: `quartus_pgm -c 1 -m jtag -o "p;blinky.sof"` → LED0 blinks.
3. Loader check: `openFPGALoader -c usb-blasterIII blinky_pfg.rbf` → LED0
   blinks. If it doesn't, the loader is the problem, not the bitstream, so
   stop here.
4. **`openFPGALoader -c usb-blasterIII scratch_a5db.rbf`**, and
   `quartus_pgm … -o "p;scratch.sof"` to rule out the loader. Blinks: the
   database's idle state, pin tables, clock path and hand-placed logic
   work on silicon, with no Quartus image anywhere. Doesn't blink but
   configures (`CONF_DONE`, no SDM error): suspect the idle state (n) or
   the design. Fails to configure: compare the SDM status with step 3's.
5. `scratch_a5db_zero-io-hash.rbf` and `scratch_a5db_zero-design-hash.rbf`:
   if it blinks, the SDM ignores that hash.
6. Negative control: `scratch_a5db_corrupt-core.rbf` must fail (one bit in
   a hashed `CORE` page, hashes left stale). If it configures, the SDM
   isn't checking section hashes, and steps 4–5 prove less.

Record for each: LED behaviour, openFPGALoader's exit status, and the SDM
status it prints (`ISC_SR`).

**Phase 6: designs from the open flow** (w), in `specimens/board/p6/`
(git-ignored). Each `<d>.rbf` is a5db's and equals `quartus_pfg`'s
(`<d>_pfg.rbf`); `<d>.sof` is a5db's container. Rebuild them with
`tools/a5pnr/flow.sh <d> designs/p6/<d>.pins specimens/p6/flow/<d>
designs/p6/<d>.v` and check them with `tools/a5pnr/check.sh` (same
arguments). After step 4 above works, load each with
`openFPGALoader -c usb-blasterIII <d>.rbf`:

7. `blinky.rbf`: LED0 blinks at about 1.5 Hz. This is the first image whose
   placement and routing are entirely a5pnr's: if step 4 worked and this
   doesn't, suspect the carry chain (it is the counter) or a5pnr's clock
   route (DCM input 1, not Quartus' 2).
8. `leds.rbf`: LED7 toggles every 1.3 s (counter bit 26), LED6 twice as
   fast, the lower LEDs too fast to see. Holding KEY0 turns them all off
   (the counter is held at 0); holding KEY1 inverts them; SW0 up reverses
   their order. Tests HVIO inputs (KEY0, KEY1), an IO96 input (SW0) and
   eight IO96 outputs.
9. `acc.rbf`: LED7 changes about every 0.3 s and the lower LEDs faster, an
   irregular pattern (the top byte of a 56-bit accumulator adding an LFSR);
   KEY0 held clears them. Tests a carry chain over three LABs.
10. `ram.rbf`: LED[7:0] counts up in binary at about 6 Hz (the M20K's copy
    of the slowly stepping read address); all off or frozen means the M20K
    isn't being written or read.

SHA-256 of the files as built here (each equal to its `_pfg.rbf`): `blinky.rbf` ab5e1d19…, `leds.rbf` d9a3618e…, `acc.rbf` aa25d7f5…, `ram.rbf` 03f8d88f…

Phase 6's acc and ram images lack `secmux_bto = DISABLE_BTO` in a LAB
where their counter's carry runs from ALM 4 into ALM 5 (README (y)): if
step 9 or 10 shows a counter that misbehaves above some bit, that is why;
phase 7's rebuilds below have it.

**Phase 7** (y)–(z2), in `specimens/board/p7/` (git-ignored): each `<d>.rbf`
is a5db's and equals `quartus_pfg`'s `<d>_pfg.rbf`; `<d>.sof` is a5db's
container. Rebuild with `tools/a5pnr/suite.sh specimens/p7/s2
designs/p6/<d> …` (or `designs/p7/<d>`), the CPU with
`designs/p7/cpu/go.sh run3`. Load each with
`openFPGALoader -c usb-blasterIII <d>.rbf`:

11. `blinky.rbf`, `leds.rbf`, `acc.rbf`, `ram.rbf`: as steps 7–10 (phase
    7's placement; leds clears through the LAB's synchronous clear now).
12. `ctl.rbf` (LAB control lines, two clocks): LED[1:0] count up every
    0.67 s while SW0 is up and hold while it is down; LED[3:2] count every
    0.67 s, and holding KEY1 sets them to 0 at the next 0.67 s tick;
    LED[5:4] count every 0.67 s on CLOCK2_50, and holding KEY0 clears them
    at once; LED6 toggles every 0.67 s and is off while SW1 is up; LED7
    follows LED6 one 20 ns cycle later (it looks the same) and is off while
    KEY1 is held.
13. `arith.rbf` (carry chains): LED[3:0] count in binary, LED0 toggling
    every 1.34 s; with SW3 down LED[7:4] = LED[3:0] + 2·SW2 + SW1 + SW0
    (switches down: the same as LED[3:0]; SW0 up: one more); with SW3 up
    LED[7:4] = LED[3:0] − 3 (mod 16). Both LED groups update together.
14. `rom.rbf` (M20K contents): one LED lit, running LED0 → LED7 → LED0 back
    and forth, 84 ms per step (a full sweep 0.67 s). All dark or all lit
    means the contents didn't load.
15. `ramx.rbf` (M20K enables and output register): with SW0 up, LED[3:0]
    count in binary at about 3 Hz (LED0 toggles every 0.34 s); LED[7:4]
    show the same with SW3 up, and with SW3 down whatever byte 3 held.
    With SW0 down the LEDs replay what was written on the address
    counter's previous pass (every 87 s; 0 after power-up). Holding KEY1
    freezes them.
16. `pll.rbf` (IOPLL): LED3 and LED7 toggle together every 0.67 s, and
    LED[3:0] and LED[7:4] count at the same rate (the PLL at 100 MHz
    against CLOCK2_50). LED[3:0] at half LED[7:4]'s rate means the PLL
    runs at 50 MHz; LED[3:0] dark means it doesn't run or isn't locked.
17. **`cpu.rbf` (FemtoRV32 running its program from M20K)**: with SW0 down,
    LED[7:0] count up in binary, LED0 toggling every 0.25 s (LED7 every
    32 s); with SW0 up the count runs at 1.1 MHz (the LEDs look half lit).
    Holding KEY0 holds the CPU in reset (the LEDs keep their value);
    releasing it starts the count again (the first store shows 1).
    All dark: the CPU isn't fetching (suspect the M20K contents or its read
    enable); stuck at one value: the delay loop or the branch.

SHA-256 (phase 7, each equal to its `_pfg.rbf`): `blinky.rbf` 0bb02a06…,
`leds.rbf` 613d1468…, `acc.rbf` b3b45318…, `ram.rbf` 389a48ed…, `ctl.rbf`
bd4d0148…, `arith.rbf` 963e69dc…, `rom.rbf` 16b9d4ba…, `ramx.rbf`
19e17400…, `pll.rbf` f4d1724b…, `cpu.rbf` 285321e8….

**Phase 8** (z3)–(z5), in `specimens/board/p8/` (git-ignored). Phase 8
found that every phase 4–7 image with a LUT in it computes each LUT on
inverted inputs (z3): **steps 4, 8, 9, 12, 14–17 are expected to fail with
the earlier files** (blinky in steps 7 and 11 is pure carry chain and
should work). The steps below test the fix directly; run them first. Each
`<d>.rbf` is a5db's and equals `quartus_pfg`'s `<d>_pfg.rbf`.

18. **`scratch_p8.rbf` against `scratch_a5db.rbf`** (phase 5's, step 4): the
    hand-placed 25-bit counter of `designs/blinky_a5db`, rebuilt with the
    corrected LUT rule. `scratch_p8.rbf`: LED0 blinks at about 1.5 Hz.
    `scratch_a5db.rbf`: LED0 stays dark (its first counter bit can't
    toggle). Both configure. The reverse (old blinks, new dark) means the
    phase 8 rule is wrong; both dark, look at the idle state (step 4).
19. **`ctl_p7.rbf` against `ctl.rbf`**: phase 7's and phase 8's builds of
    the LAB-controls design, behaviour as step 12. Phase 8's should do what
    step 12 says; phase 7's shouldn't (its LUTs, e.g. the XOR reductions
    and the enable logic, are on inverted inputs).
20. `blinky.rbf`, `leds.rbf`, `acc.rbf`, `ram.rbf`, `arith.rbf`, `rom.rbf`,
    `ramx.rbf`, `pll.rbf`: as steps 11–16, now with two LUTs per ALM.
21. **`cpu.rbf`** (FemtoRV32, 1,393 ALMs): as step 17.
22. If step 18 passes but 20 or 21 fail: suspect what only phase 8 added,
    in this order: a 5-LUT's output (`LUTOUT_5LUT`, which no earlier image
    used), two flops sharing a half's enable (the CPU's register file),
    the C1/D1 copies of a 6-LUT's inputs.

Rebuild: `designs/blinky_a5db/build.py specimens/p8/scratch` (checked
against a 25-bit counter in `specimens/p8/scratch/check`: 1,000 cycles from
random states and 3,000 from power-up, 0 mismatches), `tools/a5pnr/suite.sh
specimens/p8/s4 designs/p6/<d> …`, `designs/p7/cpu/go.sh r6` with
`OUT=specimens/p8/cpu`; `ctl_p7.rbf` is `specimens/board/p7/ctl.rbf`.
SHA-256 (phase 8, each a5db `.rbf` equal to its `_pfg.rbf`): `scratch_p8.rbf`
55d574d2…, `scratch_a5db.rbf` 27642610… (phase 5's), `ctl_p7.rbf` bd4d0148…
(phase 7's), `blinky.rbf` 18e90707…, `leds.rbf` bc7ce2aa…, `acc.rbf`
1f7771d7…, `ram.rbf` e40b3676…, `ctl.rbf` 9a5d9a58…, `arith.rbf` 2fc8e316…,
`rom.rbf` 6870707f…, `ramx.rbf` c44717e9…, `pll.rbf` f4e2caf7…, `cpu.rbf`
3e3dd91c….

**Phase 9** (z6)–(z9), in `specimens/board/p9/` (git-ignored). **These are
the current files**; with them, phase 8's A/B pairs (steps 18 and 19) still
come first. The oracle audit (z6) found no rule a5pnr writes wrong, so
**phase 8's files stay valid** (same RTL, behaviour as written); phase 9's
differ in placement and routing (the LAB-input budget, swapped LUT inputs),
except `blinky.rbf`, which is byte-identical to phase 8's. Phase 4–7 files
with LUTs stay wrong (z3). Each `<d>.rbf` is a5db's and equals
`quartus_pfg`'s `<d>_pfg.rbf`; every one passed all four checks
(`specimens/p9/s1/summary.tsv`, `specimens/p9/cpu/*.check.log`).

23. `blinky.rbf`, `leds.rbf`, `acc.rbf`, `ram.rbf`, `ctl.rbf`, `arith.rbf`,
    `rom.rbf`, `ramx.rbf`, `pll.rbf`: as steps 11–16 and 12. ramx with
    KEY1 held: frozen, as before (the M20K's read enable holds its data
    (z6), and the write address has moved on anyway).
24. **`rf.rbf` (MLAB RAM)**: LED[3:0] show the low four bits of word a of a
    32 x 8 RAM, a stepping every 0.17 s: (7a + 1) mod 16, i.e. 1, 8, F,
    6, D, 4, B, 2, 9, 0, 7, E, 5, C, 3, A, then again. LED[7:4] show word
    a mod 16 of a 16 x 4 RAM, 15 − a: counting down F → 0 every 2.7 s.
    SW0 up writes the counter's top byte at the address being read: LED[3:0]
    then count up in binary, one step every 0.34 s, and keep doing so after
    SW0 goes down (the words were overwritten); reconfiguring brings the
    first pattern back. Holding KEY1 writes the counter's top four bits
    into the 16 x 4 RAM: LED[7:4] then show one value that changes every
    5.4 s. Diagnosis: LED[3:0] dark or stuck, contents not loaded or the
    read broken; the pattern but SW0 does nothing, the write side (CLKEN0,
    the write address lines); LED[7:4] dark, the unrouted address bit of a
    16-word RAM doesn't read 1 (z6).
25. **`cpu.rbf`** (FemtoRV32 with its register file in MLAB RAM, 453 ALMs):
    as step 17. **`cpu_ff.rbf`**: the same CPU with its register file in
    flops (phase 9's code, `A5_MLAB=0`), as step 17. If `cpu_ff.rbf` works
    and `cpu.rbf` doesn't, suspect the MLAB RAM (step 24 first); if
    neither does but phase 8's `cpu.rbf` (step 21) works, suspect phase 9's
    routing (swapped LUT inputs).
26. `fr.rbf`: random logic between registers (designs/fr), LEDs flicker
    too fast to read; only "configures, no SDM error" is checkable.

Rebuild: `tools/a5pnr/suite.sh specimens/p9/s1 designs/p6/<d> designs/p7/<d>
designs/fr/fr designs/p9/rf`, `OUT=specimens/p9/cpu designs/p7/cpu/go.sh r1`
and, with `A5_MLAB=0`, `… go.sh ff1`. SHA-256 (phase 9, each a5db `.rbf`
equal to its `_pfg.rbf`): `blinky.rbf` 18e90707… (= phase 8's), `leds.rbf`
06ae3d5b…, `acc.rbf` ddd12ace…, `ram.rbf` fe6b2b2c…, `ctl.rbf` aa26db1b…,
`arith.rbf` 40ba5b24…, `rom.rbf` cd577dcf…, `ramx.rbf` c8d6cd36…, `pll.rbf`
c67946ab…, `rf.rbf` 9125aab9…, `fr.rbf` 8fcf2750…, `cpu.rbf` 910c213b…,
`cpu_ff.rbf` 031446c8….

**Phase 10** (z10)–(z12), in `specimens/board/p10/` (git-ignored). **These
are the current files**; phase 8's A/B pairs (steps 18 and 19) still come
first, then the new pair (step 27). Each `<d>.rbf` is a5db's and equals
`quartus_pfg`'s `<d>_pfg.rbf`; every one passed all four checks
(`specimens/p10/s1/summary.tsv`, `specimens/p10/cpu/*.check.log`,
`specimens/p10/synth/v2/cpu`). blinky, acc, ram, rom, ramx, pll and rf are
byte-identical to phase 9's; leds, ctl, arith and fr differ in placement
and routing only (a5pnr now numbers nets no name covers, so its constant
LUTs' nets move), not in what they compute.

27. **`cpu_cs.rbf` against `cpu.rbf`**: the same FemtoRV32 (register file
    in MLAB RAM), synthesized by chipsmith (`A5_SYNTH=chipsmith`, 450 ALMs)
    and by yosys (453 ALMs): both as step 17. One working and the other not
    points at that synthesis (or, for `cpu_cs.rbf` alone, at a5synth's
    `A5_ARITH` and flop mapping).
28. **`io.rbf` (I/O registers)**: KEY0, KEY1 and SW[3:0] are sampled in
    their pins' input registers, LED[7:0] come from output registers.
    LED[3:0] follow SW[3:0] at once, except LED2, which takes SW2 once per
    1.34 s, and LED3, which stays dark while KEY0 is held. LED4 blinks with
    a 1.34 s period, LED5 with 0.67 s and freezes while KEY1 is held, LED6
    (0.34 s) and LED7 (0.17 s) go dark while KEY0 is held. Diagnosis: no
    LED reacts at all, the IO96 output registers or their clock (VIO X106,
    X107 `GLOBAL_LOCAL_SEL` I3); LED[3:0] stuck while LED4 blinks, the
    input registers (HVIO clock row 49, IO96 lane 5 flyby); LED5 doesn't
    freeze, an output register's clock enable (through an RPI inverter);
    LED6 not dark under KEY0, an inverted asynchronous clear (RPI), LED7 not
    dark, the straight active-low one; LED3 not dark, an input register's
    clear. The GPIO pins (H16, Y1 in; C2, P1 out, P1 with an OE register)
    need a probe; C2 toggles at 25 MHz XOR the inputs.
29. **`mul.rbf` (DSP blocks)**: eight products, 9x9 to 27x27, signed and
    unsigned, each checked against a shift-and-add reference. LED0 dark
    and LED1 toggling every 0.17 s (LED2 0.34 s, LED3 0.67 s): every
    product right. LED0 lit: a wrong product; LED[7:4] count them in
    binary up to 15, so all four lit means wrong products keep coming (one
    mode wrong every pass fills it within a millisecond), a lower count a
    rare error. LED1 not toggling: the DSP clock or the fabric.
30. `blinky.rbf`, `leds.rbf`, `acc.rbf`, `ram.rbf`, `ctl.rbf`, `arith.rbf`,
    `rom.rbf`, `ramx.rbf`, `pll.rbf`, `rf.rbf`, `fr.rbf`: as steps 23, 24
    and 26. **`cpu.rbf`**, **`cpu_ff.rbf`**: as step 25.

Rebuild: `tools/a5pnr/suite.sh specimens/p10/s1 designs/p6/<d> designs/p7/<d>
designs/fr/fr designs/p9/rf designs/p10/io/io designs/p10/mul/mul`,
`OUT=specimens/p10/cpu designs/p7/cpu/go.sh r1`, with `A5_MLAB=0` `… go.sh
ff1`, and with `A5_SYNTH=chipsmith` `OUT=specimens/p10/synth/v2 … go.sh cpu`.
SHA-256 (phase 10, each a5db `.rbf` equal to its `_pfg.rbf`): `io.rbf`
a4869e91…, `mul.rbf` 2ec946ab…, `cpu.rbf` 0ef29cb3…, `cpu_ff.rbf`
955d6bf8…, `cpu_cs.rbf` 877930f8…, `leds.rbf` 40eb7666…, `ctl.rbf`
cfce635e…, `arith.rbf` dd830f2a…, `fr.rbf` 99f66baa…; `blinky.rbf`
18e90707…, `acc.rbf` ddd12ace…, `ram.rbf` fe6b2b2c…, `rom.rbf` cd577dcf…,
`ramx.rbf` c8d6cd36…, `pll.rbf` c67946ab…, `rf.rbf` 9125aab9… (= phase
9's).

## The run of 2026-11-06 (phase 12's files, `tools/board/run.sh`'s steps)

The DE25-Nano on the onboard USB-Blaster III (09fb:6026), IDCODE
`0x4362c0dd`, every file loaded with `quartus_pgm` through chipsmith-vendor.
Nobody was at the bench: SW0–SW3 down, no key pressed, LEDs read by a
webcam. Quartus' blinky (control) blinks; a5db's scratch counter blinks
like it; the zero-io-hash and zero-design-hash variants configure and blink
(the SDM checks neither hash); corrupt-core is refused by quartus_pgm on
the host ("Fail to check bitstream integrity", Error 19192) and never
reaches the SDM; blinky blinks. The LED steps:

All 19 files were re-loaded with quartus_pgm (every one "Configuration succeeded") and re-recorded at minimum exposure
(exposure_time_absolute=50, brightness=-64). Clips are in /tmp/cam/v/<id>.mkv and analyses in <id>.a2.
At this exposure the 8 LEDs separate cleanly (centres x=1041..1104, 9 px pitch, y=800; /tmp/cam/ledpos2.json). The
analyzers are /tmp/probe/leds2.py (per-LED state, rates, run-length pattern sequence) and /tmp/probe/chan.py
(light integrated in the unsaturated R/B halo, which separates "steady lit" from "fast-toggling, half lit").

Conventions: the LEDs are active-low, so "lit" on the board = design value 0. Hex values below are design values
(LED7..LED0). Rates are full cycles per second. A steady LED gives 100% integrated light, fast ~50% duty gives about 75-80%
(gamma), and its saturated centre still reads FULL. KEY/SW parts are not checked. Every switch-dependent design
agrees that SW0 to SW3 are all down (SW1 down too).

| step | verdict | one-line evidence |
|---|---|---|
| scratch-p5 | PASS (y) | LED0 steady for 10 s, no blink; dim (~32% of a lit LED). LED1-7 dark |
| ctl | PASS (visible parts) | b and c count every 0.67 s, LED6=LED7 at 1.49 Hz, LED[1:0] held at 0 (SW0 down) |
| ctl-p7 | PASS (y: misbehaves) | only LED6 toggles (1.50 Hz); LED7 doesn't follow, LED[5:2] stuck 0, LED[1:0] dim |
| leds | PASS (visible parts) | LED7 0.37 Hz, LED6 0.75, LED5 1.50, LED4 2.97, LED3 5.93, LED2 11.7 Hz: binary count, normal order (SW0 down) |
| acc | PASS | LED7 1.49 Hz (0.34 s per change), LED6 2.95, LED5 5.9, LED4 11.6 Hz; LED[3:0] faster, half lit |
| arith | PASS (SW3..0 down) | LED[3:0] count 2,3,...,F,0,1 every 40 frames (1.34 s); LED[7:4] == LED[3:0] every frame |
| ram | PASS | binary count 0x13 -> 0x2D, 5 frames per value (6 Hz), LED0 2.95 Hz |
| rom | PASS | one dark LED bounces LED0 -> LED7 -> LED0, 2.5 frames (83 ms) per step |
| ramx | INCONCLUSIVE | all 0x00 for 30 s, as predicted with SW0 down (nothing is ever written), but this exercises nothing; needs SW0 up |
| rf | PASS (read side) | LED[3:0] 6 D 4 B 2 9 0 7 E 5 C 3 A 1 8 F..., LED[7:4] = 15-a, 5 frames (0.17 s) per step |
| pll | **FAIL** | LED[7:4] (CLOCK2_50) count correctly (LED7 0.75 Hz); LED[3:0] (100 MHz PLL domain) steady 0 |
| io | **FAIL** | LED6 toggles at 2.97 Hz as expected; LED4, LED5, LED7 steady lit (0), should blink 0.75/1.49/5.96 Hz |
| mul | PASS | LED0 lit (=0: no wrong product), LED1 2.97 Hz (0.17 s), LED2 1.50, LED3 0.75 Hz, LED[7:4]=0 |
| cpu-ff | PASS (SW0 down) | binary count 0x0C -> 0x1C..., ~7.5 frames per value, LED0 1.99 Hz (0.25 s) |
| cpu | **FAIL** | all 8 LEDs uniformly half lit (fast toggling), no 0.25 s count, though cpu-ff (same program, same SW0) counts |
| cpu-ys | **FAIL** | same as cpu: uniformly half lit, no slow count |
| picorv32 | PASS (camera-limited) | all 8 LEDs uniformly half lit (~79% light, not steady, not dark), as "dimly lit, too fast to read" predicts |
| sha256 | PASS (camera-limited) | same: all 8 half lit, neither dark nor static-full |
| fr | PASS | configured, no SDM error; O[3:0] on LED[3:0] half lit (fast random XORs), LED[7:4] unused and dark |

### Evidence and notes per step

**scratch-p5**: 302 frames, LED0 never changes (frame-to-frame cv 5%). Its integrated light is 3.2 against about 10
for a steady lit LED0 (blinky's lit frames are 9.4 to 10.6), with a centre of 177 against 235. So LED0 is either
weakly driven or toggling fast at a low duty (roughly 8% linear duty under gamma 2.2); the camera can't tell which.
Either way it isn't the 1.49 Hz blink, so the answer is y.

**ctl** (source designs/p7/ctl.v). The pattern repeats every 8 x 10 frames: 14 D4 28 E8 3C FC 00 C0. Here
b = LED[3:2] and c = LED[5:4] step together every 0.67 s (tick = &pre), and d = LED6 = pre[24] toggles at 1.49 Hz.
e = LED7 follows d. a = LED[1:0] stays 0, so SW0 is down. LED6 isn't held off, so SW1 is down. The runbook's
"LED6 toggles every 0.67 s" is a full period; the source and the board give a 0.335 s half-period. The same
period/half-period mislabel is in step control's and scratch's "on 0.67 s, off 0.67 s". KEY0/KEY1 not checked.

**ctl-p7**: LED6 toggles at 1.50 Hz (so d = pre[24] works). LED7 is steady lit (e stuck at 0, not following d).
LED[5:2] are steady lit (b and c never count, so tick/t2 = &pre don't fire, or the counters don't). LED1 and LED0 are
steady but dim (54% and 30% of full light). It doesn't behave like ctl, which was expected.

**leds** (source designs/p6/leds.v): c[26:19] count, so LED7 = c[26] gives 0.37 Hz (1.34 s half-period). LED1 and
LED0 alias above 15 Hz. The MSB is on LED7, so SW0 is down, and KEY1 isn't held (no inversion). KEY0, KEY1 and SW0
not checked.

**acc** (source designs/p6/acc.v): a[55] predicts 1.49 Hz and the board gives 1.49 Hz. LED3 aliases (23.8 -> 6.6 Hz).
LED2..LED0 are faster still, read FULL and are half lit in the halo. KEY0 not checked.

**arith**: 23 runs over 30 s: 22 33 44 ... FF 00 11 ... 88 (LED[7:4] and LED[3:0] equal throughout, both counting up
once per 1.34 s). With SW3 down, LED[7:4] = LED[3:0] + 2*SW2 + SW1 + SW0, so SW0, SW1 and SW2 are down.
Counting up rather than down also confirms the active-low reading. SW parts not checked.

**ram**: 15 s, 0x13 to 0x2D, 5 frames per value, so 6 Hz. LED7 stays 0 (the count stays below 0x80 inside the clip).

**rom**: 6 s, a clean bounce: 04 02 01 02 04 08 10 20 40 80 40 20 ... at 2 to 3 frames per step (83 ms).

**ramx** (source designs/p7/ramx.v): WE = SW0. With SW0 down nothing is ever written, so the M20K reads its
power-up 0 forever, and recording longer won't change that. The LEDs did show 0x00 for 30 s, but a dead
M20K or output register would show the same. Re-test with SW0 up.

**rf** (source designs/p9/rf.v): low nibble = (7a+1) mod 16 and high nibble = 15 - a[3:0] in every frame (e.g. C6, BD,
A4, 9B, 82, 79...). A full sweep takes 16 x 0.17 = 2.7 s, which matches the runbook's "every 2.7 s". So the
initial MLAB contents load and both RAMs read, including the 16x4 RAM's unrouted address bit. The single-frame
0x02 between 0x13 and 0x0A is a transition frame. SW0 and KEY1 (the write side) not checked.

**pll** (source designs/p7/pll.v; FAIL). b[25:22] on CLOCK2_50 counts correctly: LED7 0.75 Hz, LED6 1.50, LED5
2.97, LED4 5.93 Hz. a[26:23] on the IOPLL's 100 MHz should give LED3 0.75 Hz (twice LED7's rate at the same bit
weight: a[26] at 100 MHz = b[25] at 50 MHz). Instead LED[3:0] are steady lit for 20 s (halo 12.5-12.8, the steady
level; toggling LEDs in the same clip read ~6), so a = 0 and holds its initial value. The PLL's output clock never
reaches the counter: the IOPLL doesn't lock or run, or its output isn't routed to the clock network. a can't be
toggling fast; that would need a GHz-range clock. This matches the runbook's "LED[3:0] dark (text) = not running or
not locked".

**io** (source designs/p10/io/io.v; FAIL). The source predicts LED4 = cnt[25] (0.75 Hz), LED5 = cnt[24] with KEY1
enable (1.49 Hz), LED6 = cnt[23] with async clear on posedge rst (2.98 Hz) and LED7 = cnt[22] with async clear on
negedge k0 (5.96 Hz). The board shows LED6 at 2.97 Hz, which is correct, so cnt, the clock, KEY0's input register
and an output register with an async clear all work. LED4, LED5 and LED7 are steady lit (design 0). Their halo
light is 12.2-13.1 with 0.8-2.9% frame-to-frame cv, against 7.3 and 84% for LED6, so they really are static, not
fast. LED4 is the plain case (`led[4] <= cnt[25]`, no enable or clear), so suspect the output registers' D input or
clock for DN25 (LED4), DP25 (LED5) and DP30 (LED7), not the control lines. LED[3:0] = 0 fits SW[3:0] all down
(LED3 = 0 would also hide a KEY0 fault). The KEY and SW parts, and the GPIO pins GQ/GZ, not checked.

**mul**: 20 s: 06 08 0A 0C 0E 00 02... is LED[3:1] counting with LED0 = 0 and LED[7:4] = 0. That means no wrong
products, and LED1 runs at 2.97 Hz (0.17 s half-period).

**cpu-ff**: 0x0C, 0x0D, ... 0x1C, about 7.5 frames (0.25 s) per value, LED0 1.99 Hz. That is the SW0-down count.
KEY0 not checked.

**cpu, cpu-ys** (source designs/p7/cpu/cpu.v, asm.py; FAIL). All 8 LEDs read FULL at the centre, and their halo
light is 9.7-10.7 against 12.5-13.2 for steady all-lit ramx, with a cv of only 1-2%. That is the "half lit" look of a
fast count, which the source gives with SW0 up (3-turn delay, 1.1 MHz). cpu-ff, the same program loaded minutes
earlier with the same SW0, counts at 0.25 s, so SW0 is down. The program reads SW0 into x5 (`lw x5,4(x1)`) and
loads the delay into x3 (`lui x3,0x200000`). A fast count means the MLAB register file returns a wrong x5 (non-zero,
so it takes the fast path) or a wrong x3 (short delay). The CPU fetches and stores, so the M20K and the store path
work. Both chipsmith's and yosys' synthesis fail the same way, which points at the MLAB RAM's read or write on
silicon (see rf: its MLAB reads are right, but its write side was never exercised; SW0 down). It can't be fully
excluded that the LEDs are stuck at a partial level rather than counting fast, but the 75-80% halo matches fast ~50%
duty (acc, fr).

**picorv32, sha256**: they look the same as cpu: every LED half lit, with halo light identical to cpu's within
noise. Neither dark nor steady full, so they are consistent with "flicker too fast to read". The camera can't
prove they are changing rather than stuck at a mid level. KEY0 freeze not checked; that check would settle it.

**fr**: configured with no SDM error (the runbook's whole criterion). LED[3:0] are half lit (6.4/6.2/5.3/5.4 halo,
centres saturated); LED[7:4] are unused pins and dark.

### After the fixes (branch `board-fixes`, the same night)

Each failure was found against Quartus' own images and board probes
(`designs/p13`), fixed in a5pnr, and its rebuilt file loaded and recorded
the same way:

| step | rebuilt file on the board |
|---|---|
| pll | LED[7:4] 0.75/1.49/2.98/5.96 Hz (LED7..4), LED[3:0] the same (LED3..0): the 100 MHz counter runs at twice the CLOCK2_50 one's rate |
| io | LED4 0.75 Hz, LED5 1.49, LED6 2.98, LED7 5.96; LED[3:0] 0 (switches down) |
| cpu | binary count, LED0 1.98 Hz, LED1 0.99, LED2 0.50, LED3 0.25, LED4 0.12 (0.25 s per value) |
| cpu-ys | the same, LED0 1.94 Hz, LED1 0.99, LED2 0.50 |

Causes and fixes are in the README ("Board test", (v), (z9), (z10)).

## Self-tests, the afternoon of 2026-11-06 (`tools/board/selftest.sh`)

The board had been power-cycled after an earlier hang (the SDM refused
loads). After that, Quartus' blinky loaded again, and so did every
self-test file. The HPS now boots NixOS from an SD card (host mercury), with
no effect seen on JTAG loads or the LEDs. Camera box `830 0 890 86`, lit 88,
dark 13 (the camera had moved since the morning's box `900 120 980 210`).
Clips in `specimens/p14/cam/`.

| file | verdict | rate, duty | swing (p95 − p5) | mean |
|---|---|---|---|---|
| row (calibration) | BLINK | 1.00 Hz, 0.50 | 75.4 | 50.3 |
| row_mut | LIT | – | 0.6 | 87.7 |
| ramx_st | BLINK | 1.00 Hz, 0.50 | 75.5 | 50.4 |
| ramx_st_mut | LIT | – | 0.6 | 88.1 |
| mlab_st | BLINK | 1.00 Hz, 0.50 | 75.6 | 50.4 |
| mlab_st_mut | LIT | – | 0.8 | 87.4 |
| cpu_st | BLINK | 1.00 Hz, 0.50 | 75.7 | 50.0 |
| cpu_st_mut | LIT | – | 0.7 | 87.2 |
| sha_st | BLINK | 1.00 Hz, 0.50 | 74.4 | 47.9 |
| sha_st_mut | LIT | – | 0.6 | 84.8 |
| keys (idle) | DARK | – | 0.2 | 11.6 |

keys was left loaded for a person to test (README, "Self-tests").
