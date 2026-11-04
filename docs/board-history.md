# Board test, phases 4–11 (superseded)

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

