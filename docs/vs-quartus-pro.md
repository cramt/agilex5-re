# a5pnr against Quartus Prime Pro

How a5pnr's open flow (chipsmith's synthesis, a5pnr, a5db) compares with
Quartus Prime Pro 26.1.1 on the same Verilog for the DE25-Nano's
A5EB013BB23BE4SCS: time to a bitstream, resources, and Fmax. It is the
Agilex 5 counterpart of chipsmith's `docs/vs-quartus.md` for the Cyclone
II. `tools/bench/vs-quartus-pro.sh` produces every number here
(`specimens/p11/bench/runs.tsv`, not committed), `tools/bench/table.py` the
tables. This is phase 11's flow (README (z14)); phase 10's, with yosys as
the front end, is in the phase 10 column below and in git history.

## Setup

- **Machine**: Intel Core i7-4770K (4 cores, 8 threads), 31 GB, no swap,
  NixOS, **shared**: two other agents ran builds throughout, so the
  1-minute load average before each build ranged from about 4 to 38 (it is
  in the tables). Each build ran in a systemd unit of its own; wall time and
  CPU time (user + system of every process the build waited for, GNU time)
  are per build.
- **Runs**: three reps of a5pnr's flow per design. Quartus Pro's builds
  are phase 10's (`specimens/p10/bench`: the same Quartus, RTL, settings
  and machine, three reps interleaved with phase 10's a5pnr, load 8 to
  34); its wall and CPU times saw that load, a5pnr's this one (8 to 21).
  Medians with [min-max].
- **Quartus Pro**: `quartus_syn`, `quartus_fit`, `quartus_asm` through
  `tools/quartus.sh` (chipsmith's sandbox, no network), 2 processors
  (`NUM_PARALLEL_PROCESSORS`, as every Quartus run of this repository),
  every other setting at its default (Hyper-Retiming on). The same Verilog
  as a5pnr's, but the CPU's ROM, which is a5pnr's `A5_M20K` instance and an
  inferred M20K for Quartus (`designs/p8/cql/cql.v`), and its register file,
  forced into MLAB (`ramstyle "MLAB"`) or flops (`"logic"`) for the two CPU
  rows.
- **a5pnr**: `tools/a5pnr/flow.sh`: chipsmith's synthesis
  (`tools/a5synth`), a5pnr (packing, placement under Quartus' LAB-line rule,
  global placement from 1,500 ALMs, timing-driven annealing, routing),
  `a5db build` (`.sof`) and `a5db rbf` (`.rbf`), phase 11's code.
- **Fmax**: one clock, over-constrained (2 ns), so both fitters work for
  speed; `quartus_sta` (slow 100 °C model) reports Fmax and the Fmax
  restricted by the clock's minimum pulse width. For Quartus' own fit, its
  own routes. For a5pnr, its placement exported as locked atoms (`a5pnr
  --quartus`: LUTs, flops, carry chains and MLAB RAM at a5pnr's sites;
  M20Ks and DSP products as RTL Quartus places; I/O registers in their
  pins) and routed by Quartus; a LAB Quartus' legalizer still refuses is
  left to Quartus, round by round (`tools/bench/unlock_fit.sh`). "model" is
  a5pnr's own timing model on its own routes (the longest path including
  the pins); phase 9 found it reads Quartus' routes about 4 % slow.

## Verdict

**a5pnr is now 9x to 32x faster than Quartus Pro in wall time on seven of
eight designs, but sha256 (1.4x).** sha256 got slower (1,891 s against
phase 10's 1,258): its first placement doesn't route (one LAB's local line
two nets want) and a5pnr places it again with the next seed. Phase 12 found
why (two critical connections never yielding one LAB line) and routes it
first time: its place and route takes 436–786 s where it took 2,104
(below).

**On area a5pnr is level or ahead on six designs** (the flop CPU 1,300
against Quartus' 1,454, mul 311 against 329) and behind on three: sha256
1,669 against 1,457 (phase 10: 2,620), picorv32 430 against 378 (805;
Quartus puts the register file in two M20Ks), the MLAB CPU 413 against
390 (453). chipsmith's synthesis with an ALM-aware LUT cost did most of
it.

**On Fmax Quartus still leads on the deep designs, by 1.3x to 1.7x where it
was 2x to 3x**: the flop CPU 190 against 319 MHz (161), sha256 116 against
200 (75), the MLAB CPU 178 against 300 (only the model's 109 before),
picorv32 296 against 344 restricted / 479 (the model's 159). mul is at 236
against 243 and the small designs at the clock's pulse-width limit on
both. Every a5pnr number is now quartus_sta on a5pnr's own placement with
Quartus' routes; Quartus took every LAB of blinky, leds, acc, the flop CPU
and picorv32, and left one LAB of the MLAB CPU and mul and three of
sha256's 220 to itself. Quartus' lead is what it does beyond placing
(Hyper-Retiming, register duplication: the flop CPU's 1,229 registers
become 2,960) and its placement; a5pnr's own routes are a further 12 %
slower than Quartus' on the same placement (the flop CPU, median over 223
endpoints, as phase 9 measured).

## Phase 10 → phase 11 (a5pnr)

| design | ALMs | Fmax, quartus_sta on a5pnr's placement (MHz) | model (MHz) |
|---|---|---|---|
| blinky | 13 → 13 | 830 → 830 | 528 → 528 |
| leds | 19 → 19 | 805 → 805 | 344 → 358 |
| acc | 37 → 37 | 465 → 462 | 245 → 263 |
| cpu (MLAB) | 453 → 413 | – → 178 | 109 → 149 |
| cpu_ff | 1,394 → 1,300 | 161 → 190 | 132 → 158 |
| sha256 | 2,620 → 1,669 | 75 → 116 | 72 → 105 |
| picorv32 | 805 → 430 | – → 296 | 159 → 271 |
| mul | 407 → 311 | – → 236 | 222 → 243 |

## Phase 11 → phase 12 (a5pnr)

Two changes (README (z15)), measured with the same netlists (a5synth on
chipsmith `open` a780413 writes phase 11's netlists byte for byte):
criticality in the router decays from iteration 20, and the model times
I/O registers (input tco, output setup). Router changes leave quartus_sta's
numbers alone where the placement doesn't move (it times Quartus' routes),
so only sha256, now routed on its first placement, and io moved. Single
runs [obs, `specimens/p12/qor`]; Full builds below are still phase 11's.

| design | LABs | Fmax, quartus_sta on a5pnr's placement (MHz) | LABs left to Quartus | model (MHz) | a5pnr place and route (s) |
|---|---|---|---|---|---|
| sha256 | 220 → 233 | 116 → **119** | 3 → **0** | 105 → 105 | 2,104 → 436–786 |
| io | 6 → 4 | 117 → **144** | 0 → 0 | 326 → 169 (register to register 528 → 232) | 36 → 35 |
| cpu_ff, cpu, fr | same | same (same placement) | same | same | routing iterations 44 → 28, 24 → 23, 30 → 30 |

sha256's speed-up over Quartus on the full build, 1.4x in phase 11, isn't
re-measured; its place-and-route step alone is now 2.7x to 4.8x shorter
on the loaded machine.

## Full builds

Medians over three reps with [min-max]; the load is the median 1-minute
load average before the build. Quartus Pro's rows are phase 10's builds
(see Setup). Resources are each tool's own count after fitting, from rep
1; Fmax from rep 1.

| design | tool | wall s, median [min-max] | CPU s | load | ALMs | LABs | M20K | DSP | Fmax (MHz) |
|---|---|---|---|---|---|---|---|---|---|
| blinky | Quartus Pro | 2359 [1133-2784] | 1887 [1306-1923] | 25 | 13 | 3 | 0 | 0 | 820 (restricted 362) |
| blinky | a5pnr | 74 [63-74] | 76 [70-76] | 17 | 13 | 2 | 0 | 0 | 830 (restricted 366), every LAB locked, model 528 |
| leds | Quartus Pro | 2306 [1396-3842] | 1911 [1546-1980] | 34 | 19 | 3 | 0 | 0 | 805 (restricted 366) |
| leds | a5pnr | 75 [73-80] | 76 [73-76] | 18 | 19 | 3 | 0 | 0 | 805 (restricted 366), every LAB locked, model 358 |
| acc | Quartus Pro | 1714 [1586-2829] | 1862 [1788-1933] | 22 | 36 | 6 | 0 | 0 | 616 (restricted 362) |
| acc | a5pnr | 84 [74-93] | 77 [76-78] | 20 | 37 | 6 | 0 | 0 | 462 (restricted 353), every LAB locked, model 263 |
| cpu_ff | Quartus Pro | 2703 [1795-2996] | 2541 [2214-2544] | 15 | 1454 | 186 | 1 | 0 | 319 (restricted 319) |
| cpu_ff | a5pnr | 296 [290-304] | 287 [281-288] | 20 | 1300 | 199 | 1 | 0 | 190 (restricted 190), every LAB locked, model 158 |
| sha256 | Quartus Pro | 2688 [2064-2700] | 2465 [2252-2484] | 17 | 1457 | 243 | 0 | 0 | 200 (restricted 200) |
| sha256 | a5pnr | 1891 [1721-1961] | 1792 [1654-1842] | 14 | 1669 | 220 | 0 | 0 | 116 (restricted 116), 3 LABs left to Quartus, model 105 |
| mul | Quartus Pro | 2332 [1649-3196] | 2220 [1921-2236] | 8 | 329 | 45 | 0 | 6 | 243 (restricted 243) |
| mul | a5pnr | 119 [118-132] | 118 [117-121] | 18 | 311 | 64 | 0 | 8 | 236 (restricted 236), 1 LAB left to Quartus, model 243 |
| cpu | Quartus Pro | 2205 [1779-3760] | 2140 [2043-2314] | 19 | 390 | 54 | 1 | 0 | 300 (restricted 300) |
| cpu | a5pnr | 126 [105-132] | 121 [107-130] | 18 | 413 | 67 | 1 | 0 | 178 (restricted 178), 1 LAB left to Quartus, model 149 |
| picorv32 | Quartus Pro | 2239 [1564-2737] | 2103 [1768-2133] | 33 | 378 | 58 | 2 | 0 | 479 (restricted 344) |
| picorv32 | a5pnr | 131 [113-133] | 128 [114-128] | 14 | 430 | 62 | 0 | 0 | 296 (restricted 296), every LAB locked, model 271 |

| design | wall-time speed-up (Quartus / a5pnr) | CPU-time ratio |
|---|---|---|
| blinky | 32.1x | 25.0x |
| leds | 30.9x | 25.2x |
| acc | 20.4x | 24.3x |
| cpu_ff | 9.1x | 8.9x |
| sha256 | 1.4x | 1.4x |
| mul | 19.6x | 18.7x |
| cpu | 17.5x | 17.7x |
| picorv32 | 17.2x | 16.5x |

## Where the time goes

Median seconds per a5pnr step (`times.txt`; the a5pnr step includes about
18 s of loading the routing graph). Quartus' steps are phase 10's:
`quartus_syn` 120–202 s, `quartus_fit` 1,337–2,205 s (3–4 minutes of it
loading periphery placement data for any design), `quartus_asm` 218–328 s.

| design | synthesis | a5pnr | a5db build | a5db rbf |
|---|---|---|---|---|
| blinky | 0 | 27 | 41 | 5 |
| leds | 0 | 28 | 43 | 6 |
| acc | 1 | 31 | 47 | 5 |
| cpu | 1 | 73 | 41 | 5 |
| cpu_ff | 4 | 243 | 41 | 7 |
| sha256 | 13 | 1,829 | 41 | 7 |
| picorv32 | 2 | 77 | 44 | 6 |
| mul | 3 | 72 | 41 | 5 |

chipsmith's synthesis takes under 15 s where yosys took 34–75.

## Caveats

- **The machine was shared** with other agents' builds. Quartus Pro's times
  are phase 10's (load 8 to 34), a5pnr's from this run (load 8 to 21), so
  the speed-ups mix two load ranges; single numbers are noisy (blinky's
  Quartus run took 1,133 s at load 10 and 2,784 s at load 30).
- **Quartus' LAB legalizer**: its rule is decoded (README (z14): at most 55
  LAB lines, a line per 5 sinks of a net and per 5 further LEIMs it takes
  in an ALM) and a5pnr places by it, but Quartus' own greedy line count
  runs a line or two above a5pnr's model (mul's refused LAB counts 54 by
  the model, the MLAB CPU's one likewise), and in sha256's globally placed
  LABs three were refused by Quartus' FLABS legalization pass at 49–51
  lines by the model, a check other than the line counter [unk]. Each is
  left to Quartus (`tools/bench/unlock_fit.sh`).
- Quartus' resources come from a fit tuned for speed (a 2 ns clock,
  Hyper-Retiming on), as a5pnr's timing-driven run is; Quartus' smallest fit
  of the same RTL can be smaller (phase 8's flop CPU: 1,082 ALMs).
- a5pnr's Fmax through Quartus' router measures a5pnr's placement, not its
  routing; its own routes are timed only by its own model. M20Ks and DSP
  products are placed by Quartus in the export.

Reproduce: `BENCH_A5PNR_ONLY=1 BENCH_NO_XFIT=1
tools/bench/vs-quartus-pro.sh specimens/p11/bench 3`, each export through
`tools/bench/unlock_fit.sh <bench>/runs/x_<design> <top>
<bench>/runs/xf_<design> <bench>/sta/x_<design>`, Quartus Pro's rows and
`sta/q_*` from `specimens/p10/bench`, then `python3 -I
tools/bench/table.py specimens/p11/bench`.
