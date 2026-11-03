# a5pnr against Quartus Prime Pro

How a5pnr's open flow (yosys, a5pnr, a5db) compares with Quartus Prime Pro
26.1.1 on the same Verilog for the DE25-Nano's A5EB013BB23BE4SCS: time to a
bitstream, resources, and Fmax. It is the Agilex 5 counterpart of chipsmith's
`docs/vs-quartus.md` for the Cyclone II. `tools/bench/vs-quartus-pro.sh`
produces every number here (`specimens/p10/bench/runs.tsv`, not
committed), `tools/bench/table.py` the tables.

## Setup

- **Machine**: Intel Core i7-4770K (4 cores, 8 threads), 31 GB, no swap,
  NixOS, **shared**: two other agents ran builds throughout, so the
  1-minute load average before each build ranged from about 4 to 38 (it is
  in the tables). Each build ran in a systemd unit of its own; wall time and
  CPU time (user + system of every process the build waited for, GNU time)
  are per build.
- **Runs**: three reps, interleaved: per rep and design, Quartus then
  a5pnr. Medians with [min-max].
- **Quartus Pro**: `quartus_syn`, `quartus_fit`, `quartus_asm` through
  `tools/quartus.sh` (chipsmith's sandbox, no network), 2 processors
  (`NUM_PARALLEL_PROCESSORS`, as every Quartus run of this repository),
  every other setting at its default (Hyper-Retiming on). The same Verilog
  as a5pnr's, but the CPU's ROM, which is a5pnr's `A5_M20K` instance and an
  inferred M20K for Quartus (`designs/p8/cql/cql.v`), and its register file,
  forced into MLAB (`ramstyle "MLAB"`) or flops (`"logic"`) for the two CPU
  rows.
- **a5pnr**: `tools/a5pnr/flow.sh`: yosys from nixpkgs (synthesis), a5pnr
  (packing, timing-driven placement, routing), `a5db build` (`.sof`) and
  `a5db rbf` (`.rbf`), phase 10's code.
- **Fmax**: one clock, over-constrained (2 ns), so both fitters work for
  speed; `quartus_sta` (slow 100 °C model) reports Fmax and the Fmax
  restricted by the clock's minimum pulse width. For Quartus' own fit, its
  own routes. For a5pnr, its placement exported as locked WYSIWYG atoms
  (`a5pnr --quartus`, README (z4)) and routed by Quartus, with each LAB
  Quartus' legalizer rejects ("too many LAB line inputs", a rule a5pnr
  doesn't know, README (z8)) left to Quartus, round by round
  (`tools/bench/unlock_fit.sh`). The export doesn't take MLAB RAM, DSP
  blocks or I/O registers yet; for those designs (the MLAB CPU, picorv32,
  mul) only a5pnr's own model on its own routes is given ("model"); phase 9
  found the model prices Quartus' routes about 4 % slower than quartus_sta
  does, and a5pnr's routes 12 % slower than Quartus' on the same placement
  (README (z8)). "model" is the longest path including the pins.

## Verdict

**a5pnr's flow reaches a bitstream 5x to 19x faster than Quartus Pro in
wall time on every design but sha256 (2.1x), with 5x to 18x less CPU
time.** Quartus Pro's floor is high: 19 to 46 minutes for a 13-ALM
blinky on this loaded machine, nearly all of it in `quartus_fit` (whose
periphery placement alone loads for 3 to 4 minutes), where a5pnr takes about
two minutes (35 s of yosys, 35 s of a5pnr of which 18 s is loading the
routing graph, 50 s of `a5db build`). On the CPUs, picorv32 and the DSP
self-test a5pnr takes 3 to 10 minutes; sha256 is the exception, at 21 minutes against
Quartus' 45, because a5pnr's annealer and router scale worse than Quartus'
fitter (sha256's place and route alone take 18 minutes).

**Quartus Pro wins on area** where its synthesis does more: the MLAB CPU
390 ALMs against 453, sha256 1,457 against 2,620, picorv32 378 against 805
(it puts the register file in two M20Ks); a5pnr is level on the small
designs and on the flop CPU (1,394 against 1,454, which Quartus inflates by
duplicating registers for speed). chipsmith's synthesis as a5pnr's front
end closes most of sha256's and picorv32's gap (2,111 and 506 ALMs, README
(z12)), but isn't the bench's flow.

**Quartus Pro wins on Fmax by 2x to 3x** wherever logic depth matters, on its
own placement against a5pnr's: the flop CPU 319 against 161 MHz (a5pnr's
placement, Quartus' routes), sha256 200 against 75. On the small designs
both are at the clock's minimum-pulse-width limit (about 360 MHz
restricted), and a5pnr's placement of blinky and leds routes as fast as
Quartus' own; acc's carry chains reach 465 MHz on a5pnr's placement against
616. Where only a5pnr's own model is available (its own routes), the MLAB
CPU is at 109 MHz against Quartus' 300, picorv32 159 against 344
(restricted) and the DSP self-test 222 against 243. Quartus' advantage is
in its synthesis and placement (Hyper-Retiming on, register duplication,
its own LUT mapping) as much as in its routing: the same Quartus router
reaches about half its own speed on a5pnr's placement of yosys' netlist.

## Full builds

Medians over three interleaved reps with [min-max]; the load is the median
1-minute load average before the build (the machine was shared; Quartus'
times vary by up to 3x with it, a5pnr's by up to 2x). Resources are each
tool's own count after fitting, from rep 1; Fmax from rep 1.

| design | tool | wall s, median [min-max] | CPU s | load | ALMs | LABs | M20K | DSP | Fmax (MHz) |
|---|---|---|---|---|---|---|---|---|---|
| blinky | Quartus Pro | 2359 [1133-2784] | 1887 [1306-1923] | 25 | 13 | 3 | 0 | 0 | 820 (restricted 362) |
| blinky | a5pnr | 124 [78-164] | 106 [85-111] | 38 | 13 | 2 | 0 | 0 | 830 (restricted 366), model 528 |
| leds | Quartus Pro | 2306 [1396-3842] | 1911 [1546-1980] | 34 | 19 | 3 | 0 | 0 | 805 (restricted 366) |
| leds | a5pnr | 129 [114-195] | 116 [116-120] | 27 | 19 | 2 | 0 | 0 | 805 (restricted 366), model 344 |
| acc | Quartus Pro | 1714 [1586-2829] | 1862 [1788-1933] | 22 | 36 | 6 | 0 | 0 | 616 (restricted 362) |
| acc | a5pnr | 128 [126-233] | 117 [117-122] | 23 | 37 | 6 | 0 | 0 | 465 (restricted 353), model 245 |
| cpu | Quartus Pro | 2205 [1779-3760] | 2140 [2043-2314] | 19 | 390 | 54 | 1 | 0 | 300 (restricted 300) |
| cpu | a5pnr | 282 [222-397] | 250 [232-260] | 22 | 453 | 86 | 1 | 0 | model 109 |
| cpu_ff | Quartus Pro | 2703 [1795-2996] | 2541 [2214-2544] | 15 | 1454 | 186 | 1 | 0 | 319 (restricted 319) |
| cpu_ff | a5pnr | 500 [410-621] | 509 [356-555] | 21 | 1394 | 207 | 1 | 0 | 161 (restricted 161), 1 LAB left to Quartus, model 132 |
| sha256 | Quartus Pro | 2688 [2064-2700] | 2465 [2252-2484] | 17 | 1457 | 243 | 0 | 0 | 200 (restricted 200) |
| sha256 | a5pnr | 1258 [887-1280] | 1023 [878-1032] | 25 | 2620 | 301 | 0 | 0 | 75 (restricted 75) (placed with a 44-net LAB budget), 12 LABs left to Quartus, model 72 |
| picorv32 | Quartus Pro | 2239 [1564-2737] | 2103 [1768-2133] | 33 | 378 | 58 | 2 | 0 | 479 (restricted 344) |
| picorv32 | a5pnr | 178 [169-391] | 202 [188-234] | 20 | 805 | 110 | 0 | 0 | model 159 |
| mul | Quartus Pro | 2332 [1649-3196] | 2220 [1921-2236] | 8 | 329 | 45 | 0 | 6 | 243 (restricted 243) |
| mul | a5pnr | 170 [147-346] | 172 [164-186] | 14 | 407 | 89 | 0 | 8 | model 222 |

| design | wall-time speed-up (Quartus / a5pnr) | CPU-time ratio |
|---|---|---|
| blinky | 19.0x | 17.8x |
| leds | 17.9x | 16.4x |
| acc | 13.4x | 15.9x |
| cpu | 7.8x | 8.6x |
| cpu_ff | 5.4x | 5.0x |
| sha256 | 2.1x | 2.4x |
| picorv32 | 12.6x | 10.4x |
| mul | 13.7x | 12.9x |

## Where the time goes

Median seconds per step. a5pnr's `pnr` includes about 18 s of loading the
routing graph; yosys runs from nixpkgs through `nix shell` (a few seconds of
the 35). Quartus' `fit` includes 3–4 minutes of loading periphery placement
data for any design.

| design | yosys | a5pnr | a5db build | a5db rbf | | quartus_syn | quartus_fit | quartus_asm |
|---|---|---|---|---|---|---|---|---|
| blinky | 34 | 34 | 51 | 6 | | 120 | 1,828 | 328 |
| leds | 35 | 42 | 45 | 6 | | 178 | 1,805 | 275 |
| acc | 34 | 45 | 48 | 5 | | 149 | 1,337 | 230 |
| cpu | 49 | 180 | 47 | 6 | | 191 | 1,720 | 297 |
| cpu_ff | 42 | 419 | 48 | 7 | | 193 | 2,205 | 296 |
| sha256 | 75 | 1,105 | 56 | 11 | | 202 | 2,048 | 308 |
| picorv32 | 52 | 83 | 43 | 5 | | 199 | 1,745 | 230 |
| mul | 46 | 81 | 38 | 6 | | 170 | 1,900 | 218 |

## Caveats

- **The machine was shared** with two other agents' builds, at a load of 4
  to 47; the reps interleave the two tools, so both saw the same range, but
  single numbers are noisy (blinky's Quartus run took 1,133 s at load 10
  and 2,784 s at load 30).
- **Quartus' LAB legalizer rejects some of a5pnr's LABs** ("too many LAB
  line inputs") although a5pnr keeps every LAB to 50 distinct nets: one LAB
  of the flop CPU's 207, and of sha256's 301 more than eight, one per
  fitter run. sha256's row is a second a5pnr placement with a 44-net
  budget, of whose 315 LABs Quartus still rejected 12, each left to Quartus
  to place. No count a5pnr has (nets, nets from outside, nets per half-LAB,
  LEIMs used) separates the rejected LABs from the rest; the rule is [unk]
  (README (z13)).
- Quartus' resources come from a fit tuned for speed (a 2 ns clock,
  Hyper-Retiming on), as a5pnr's timing-driven run is; Quartus' smallest fit
  of the same RTL can be smaller (phase 8's flop CPU: 1,082 ALMs).
- a5pnr's Fmax through Quartus' router measures a5pnr's placement, not its
  routing; its own routes are timed only by its own model.

Reproduce: `tools/bench/vs-quartus-pro.sh specimens/p10/bench 3`, then
`tools/bench/unlock_fit.sh` on an export Quartus rejected (`CONTINUE=1` to
go on), sha256's second placement with `A5PNR_LAB_INPUTS=44 … --quartus`,
and `python3 -I tools/bench/table.py specimens/p10/bench`.
