# The sm4revb database for a toolchain author

This is what a placer and router (nextpnr, or anything else) needs to know
to write an Agilex 5 `A5EB013BB23BE4SCS` bitstream through this repository,
what of it is modelled, how each part was verified, and what isn't covered.
Section letters refer to the README's findings. File formats are in
[db-format.md](db-format.md).

## The pieces

| piece | where | what it gives |
|---|---|---|
| grid and blocks | `db/sm4revb/floorplan.tsv` | every block instance (`<type>_r<y>_c<x>`) at fitter X/Y; 122 x 54 grid, two sectors split at X 60/61 |
| block configuration model | `db/sm4revb/bcm_types.json`, `bcm_settings.tsv`, `direct.tsv` | each block type's attributes: CUSTOM (named settings → named bits) and DIRECT (bit strings, e.g. LUT masks, M20K contents) |
| bit maps | `db/sm4revb/bits/` | every named bit of every instance → image bit (one layout per block type) |
| idle state | `db/sm4revb/idle*.tsv` | the value of every bit when a block is unused |
| I/O | `db/sm4revb/io/template.tsv`, `pins.tsv`, `de25_pins.tsv` | pin use (in, out, clock, IO register) → bits, for every HVIO and IO96 ball |
| routing graph | `tools/rgdump` → `specimens/rg/full` (not committed: it is Quartus' graph, regenerate it from your install in 13 min) | 7.6 M nodes, 26.3 M edges, each edge's mux setting |
| router edge classes | `a5db usable` → `specimens/rg/full/usable.bin` (5 min) | which edges a router may take: fabric and clock muxes with bits, pass-throughs; edges at their mux's idle setting are class 4, not taken (README (w), (y)) |
| pin ↔ graph nodes | `db/sm4revb/io/pin_nodes.tsv` (`a5db pinmap`) | where each ball's input leaves for fabric routing (`in0`, `in1`, `clock`) and where its output is entered (`out`) (README (t)) |
| delays | `db/sm4revb/timing/delays.tsv` (`tools/timing`) | mean delay per routing node type and per cell arc, from quartus_sta paths (README (u)) |
| IOPLL | `db/sm4revb/io/iopll.tsv`, `tools/pll/iopll.py` | an IOPLL's lines for any reference and output frequencies (README (v)) |
| open flow | `tools/a5pnr` (yosys + Rust placer and router) | Verilog → design file → `.sof` → `.rbf` (README (w)) |
| writer | `tools/a5db` (Rust) | design file → `.sof` image → `.rbf` byte-identical to `quartus_pfg`'s |

## The interface: a design file

`a5db build <db> <graph> <quartus root> <hmac.key> design.txt out.sof` and
`a5db rbf <db> out.sof out.rbf <quartus root> <keyed-hash.key>` turn a
design file into a bitstream (README (q), (l)). A design file is what a
placer and router produce, one item per line, tab-separated, applied in this
order over the idle image:

```
pin     V16=clock_in:3.3                         # db/sm4revb/io/template.tsv, src/pins.rs
edge    H6:X107Y5Z0I9   V4:X107Y6Z0I15           # a routing graph edge
attr    cw_la_lab_r6_c107   xtophalf/xale0/xcw_la_le_reg0/xreg0/register_comb   DFF_LATCH
direct  cw_la_lab_r6_c107   xtophalf/xale0/xcw_la_le_lut/lut_mask   0101…   # MSB first
bit     hvio_3_1    xpllwrap_top/…/cr_lockf_early[3]   1   # a named bit no attribute covers
raw     io96_0/hdr  46282   0                    # an image bit no named bit covers
```

`a5db extract` writes the design file of any image and `a5db rebuild`
round-trips it; a Quartus image of the same design is the reference to diff
against. A real toolchain never needs `raw` lines; `bit` lines are only
needed for an IOPLL so far.

The `.sof` container, its HMAC and CRC, the `.rbf` sections, hashes, chain
tables, compression and the signed firmware all come from a5db and the
user's own Quartus install (README (l), (n), (o)); nothing Quartus-made is
committed or needed per design.

## What is modelled, and how it was verified

Verification kinds, strongest first:

- **rebuilt**: a Quartus-compiled design's image was turned into a design
  file and back, and the result equals Quartus' image in every bit;
- **held out**: the design was compiled after the rule was written and not
  used to fit it;
- **readback**: Quartus' own assembler (asm2) reads the a5db image's
  attributes back and agrees with a5db's decode;
- **pfg**: `quartus_pfg` converts the image and its `.rbf` equals a5db's.

| feature | modelled as | verified |
|---|---|---|
| LUTs | `lut_mask` DIRECT. Pins E0, D0, C0, A, B, C1, D1, E1 are `LAB_RE` 8k + 0..7. Half h's 5-LUT (`LUTOUT_5LUT`) is stored[32h + {E_h, D_h, C_h, B, A}]; the 6-LUT (`LUTOUT_6LUT`) is ¬stored[32·¬F + {E, D, C, B, A}] with E = E0, F = E1, C and D on both halves' pins. Direct outputs `LAB_RE` 129 + k / 139 + k − 5 (half 1's 5-LUT) and 5 above (6-LUT) | **Quartus' own fr image simulated against its RTL** (z3); rebuilt (ar, mm, hx); phases 2–7 used the WYSIWYG relation W[i] = ¬stored[i XOR 31], which is not what the silicon computes |
| registers | 4 per ALM (`xcw_la_le_reg{0,1}/xreg{0,1}`); `reg_input` LUTOUT_6LUT/5LUT, ARITH_SUMOUT or LEIM (packed with unrelated logic); `register_comb` DFF_LATCH, REGIN, WIRE_MODE (bypass); `xreg0`'s `ce_sel` and `sclr_sel` gate both registers of the half | rebuilt, held out, readback; Quartus' fr and cql (z3) |
| register controls | two LAB clocks (`regclk`; clock 1 is LAB_RE 84, which Quartus uses for MLAB RAM output registers), two clears (`regclr`), enables (`ce_sel`), synchronous clear (`sclr_sel`), inverted clears through the routing tile's `iss/xclrb_rpi` | rebuilt (ar: async + sync clear, enable, load; hx: up/down, enable, async preset value, sync clear), held out |
| carry chains, shared arithmetic | `adder_mode` ENABLE_ADDER, `adder_or_packed_mode`, `reg_input` ARITH_SUMOUT | rebuilt (32-bit accumulator, ternary and 4-input adders), held out |
| MLAB RAM | RAM is the MLAB's idle mode (a logic MLAB sets `lutram_mode_vcchg_power_saving` and `ram_mode` LUT_MODE); one bit per half-ALM in its 32 mask bits, word a at stored bit 32h + a; read address {E, D1, C1, B, A}, data out through LUTOUT_5LUT; write address LAB_RE 156, 157, 160, 161, 155 (bits 0–4), write enable 80 (CLKEN0), data C0 (half 0) and D0 (half 1); an unrouted address pin reads 1 (a 16-word RAM sits at 32h + 16 + a) | rebuilt (32 x 16, 16 x 8), held out; **Quartus' ml and mi images (mi: initial contents) simulated against their RTL** (z5), (z6); a5pnr writes it (z9) |
| M20K | CUSTOM settings for modes, widths, ports and registers (42 in mm) plus contents: `m20k_eram_add<k>` = `eram_array_r<k/2>[40(k mod 2) + 39 .. ]` in the sector's RAM part | rebuilt (512 x 32 SDP, 1024 x 16 ROM, 2048 x 10 TDP with init), held out |
| DSP | `dsp_mode` (`rmode[4:0]`, a 5-bit code), register and clock selects, signs, `dsp_proginv_{l,h}` | rebuilt (18x18, 27x27 MAC, 9x9 + 18x19), held out |
| general routing | graph edges → mux settings (`MUXINk_` per `muxin[k]`), plus `flop_mode = FAST_WIRE` on every used mux with a hyper-register except LEIMs into LAB control inputs (I80+) | rebuilt (every design), routes of rt0 and fz0 (k), held out |
| clock network | graph edges; edges without an edge API (spine tap 5:1, row clock 32:1, predrive, delay chain, DCM P2C 4:1) through `graph::clock_rule` | rebuilt (ck1, ck2, mm, pl), 22 P2C muxes held out (r) |
| HVIO pins | per-pin bits only; standard in `iio_config_[1:0]` (3.3 V 00, 1.8 V 10) | 159 pins, both directions, held out (ph1, ph2) |
| HVIO clock pins | 2 per bank half: pins k0/k1 through the 2:1 ioclk mux, k2/k3 through the 4:1 ss clock mux | all 32 clock pins, held out (ck2) |
| HVIO IO registers | uses `in_reg`, `out_reg` | 3 pins (ck1), rebuilt |
| IO96 pins | pin, lane and bank rows; lane vref values depend on whether the lane has an input; IOSSM pin map | 22 pins, both directions, held out (ph1, ph2), CLOCK0_50 as a clock |
| IOPLL | a common template plus N, M and C counters, output enables, reference select and charge-pump trims (`tools/pll/iopll.py`) | counters from frequencies: 10 held-out IP configurations; bits: pa (four PLLs) exact, pb held out (README (v)) |
| carry chains | tennm_lcell_comb's p/g masks, stored as written; chain order N0 → N57, then the LAB below | 87 arithmetic halves of ar; a5pnr designs read back and simulated (README (w)) |
| M20K, 512 x 32 simple dual port | 20 settings and the port ↔ `MEDIUM_EAB_RE` pin map from mm's paths, write enable tied | a5pnr's `ram` design (README (w)) |
| M20K enables, output register, contents | WE 37, RE 39 (RE low holds the read data, not just the address: mq, README (z6)), byte enables 55, 56, 22, 21, clock enable 41 when `xsecmux/xclk0/m20k_clkensel` selects it (Quartus' CPU ROM); data bit j in at 57 + j / 40 − j and out at 83 + j, any 32 of the 40 (a5pnr: 8 of each 10-bit byte lane, Quartus without byte enables bits 0–31); output register = both `outreg_byp` idle; WE held low = `xwe0/m20k_invsel` INV_LEIM_SEL; contents `m20k_eram_add<a>` = word a in the lane layout | from mq's quartus_sta paths and contents (512 of 512 words); a5pnr's rom, ramx and CPU designs read back and simulated (README (y)) |
| M20K contents in the `.rbf` | per stored row group and M20K column a 2,568-byte record in each sector's CORE stream | byte-identical to pfg on mq, rom, CPU and held-out mm, hx (README (y)) |
| LAB control lines | CLKEN0/1 = LAB_RE 80/81 (`regclk_ens` per ALM), synchronous clear 82, clock 0 83, CLR1 86 (`regclr` idle, through `iss/xclrb`, inverted by `iss/xclrb_rpi` INV); per-flop `ce_sel`/`sclr_sel` on `xreg0` only; enable gates the clear | ct (README (y)); a5pnr's ctl design read back and simulated |
| LEIM flops | `reg_input` LEIM reads ALM pin A (half 0) or B (half 1) | hx and mm quartus_sta paths (README (y)) |
| carry-chain start | DISABLE_BTO where a chain runs ALM 4 → 5, COMB_PATH where it leaves ALM 9 for the LAB below; the hardware carry-in at a chain start is 0 | 13 LABs of ar, hx, ct, mq; **Quartus' ar and oc (a 64-bit chain over four LABs) simulated against their RTL** (README (z6)) |
| carry timing | `cin → cout` per ALM (0.050 ns, 0.118 at ALM 4 or 9), data → own `cout` 0.21–0.29, data → the half-LAB's or LAB's last `cout` by look-ahead 0.41–0.54, `carry:lab` 0.105 | quartus_sta's reported arcs of phases 6–8 (README (z7)) |
| clock network idle muxes | a clock-network mux nothing configures passes the input its idle setting selects (Quartus takes CLOCK2_50 through `Vertical_seam_tap_mux` I161 → I125 that way) | Quartus' oc simulated against its RTL (README (z6)); in the fabric [unk] |
| IOPLL in the flow | output 0 of V16's PLL enters the DCM at `HVIOWR_RE:X121Y30Z500I16` | pl; a5pnr's pll design (README (y)) |
| idle state | majority over five Quartus images, three per-instance exceptions | every image here differs from it only in its own blocks (n) |
| `.rbf` | a5db's writer | byte-identical to pfg on 78 pairs (o) |

## Rules a placer and router must follow

- **Routing**: route on the graph; an edge's bits come from its mux
  setting (`a5db`'s `graph::Join::edge`). Write `flop_mode = FAST_WIRE` on
  every mux you use that has one (LEIM, LEIM_WB, EDIM, the H/V drivers,
  HIOL `xleim`/`xedim`), except LEIMs into a LAB's control inputs. Quartus
  never leaves a used mux at DIS; whether DIS passes a signal is [unk].
- **Clocks** go from a clock pin's HVIO clock mux (or a PLL, or IO96) through
  a DCM P2C mux, the switchbox, vertical-seam and spine taps into a row
  clock and its predrive into the LAB clock (`LAB_RE` pin 83). At most two
  clock pins per HVIO bank half can be clocks at once. A clock tree only
  spans the sectors it reaches.
- **Pins**: use `a5db pins`/`pintable` for the whole set of pins of a
  design, not one table per pin: IO96 lane and bank rows are shared and
  the lane's vref values depend on the lane's mix of inputs and outputs. An
  input leaves through one of two input register muxes (`in0`/`in1`); start
  its net at that role's node in `io/pin_nodes.tsv` (HVIO `in` is `in1`,
  IO96 `in` is `in0`), a clock pin's at `clock`, and end an output's net at
  `out`. Checked on 432 pin uses of 14 Quartus designs (README (t)).
- **Fabric pins**: ALM k's LUT inputs and register outputs are `LAB_RE`
  pins (README (n)); the LAB clock is pin 83. In arithmetic mode half h's C
  and D inputs are pins 8k + 2, 1 (h = 0) or 5, 6 (h = 1), its flops drive
  89 + 4k + 2h (xreg0) and 90 + 4k + 2h (xreg1), and a carry chain runs
  through the LAB's 20 halves and on into the LAB one row down (README (w)).
- **Arithmetic LUT masks** are stored as written: half h's p (propagate) in
  bits 32h..32h + 15 and g (generate) in 32h + 16..32h + 31, with
  sum = cin ^ p, cout = p ? cin : g. A 5-LUT is stored as written too;
  the 6-LUT output is inverted, with its half select inverted (README (z3)).
- **Never route over** an edge whose mux setting is the mux's idle state
  (`usable` class 4), and don't enter a non-LAB node with several
  pass-through fanins over one of them: neither shows in an image, and
  what such a node passes is unknown (README (y)).
- **LAB inputs**: at most 50 distinct nets on a LAB's data pins (a5pnr's
  budget): Quartus' legalizer rejected LABs with 55, 59 and 61 ("too many
  LAB line inputs"), its own CPU uses at most 52; the rule is [unk]
  (README (z8)).
- **LUT inputs** may be swapped by the router: a 6-LUT's A, B, E, F; a
  5-LUT half's own C, D, E; then rebuild the mask from the pins (README
  (z8)).
- **LABs**: one clock, two enables (one per ALM), one synchronous clear, one
  routed asynchronous clear; a chain starts at ALM 0 (Quartus' fitter
  rejects one from ALM 5 that runs on, README (z4)). A half's two flops
  share their enable and synchronous clear (z3).
- **Group 1 / spare row**: never write the spare row (`_rsp5_` instances)
  or the redundancy tiles; the image has no place for them (s).

## Not covered

- Bidirectional pins and output enables from the fabric, pull-ups, bus
  hold, slew rate, current strength, open drain, Schmitt trigger: asm2 has
  those attributes as `<VIRTUAL>` and no design here used them.
- Differential I/O, the D-PHY standard, IO96 lanes 0–3 and 6–7, the second
  IO96 bank (`pio_1_0`), the left-side DCMs: no DE25-Nano pin reaches them.
- The pin map's `direct` (IO96) and `out2` roles, and output enables.
- IOPLL: PFD frequencies other than 10, 25 and 50 MHz (charge-pump trims),
  bank-half 0 reference pads, the IP's output-frequency ceiling, PLL
  cascading, dynamic phase shift; LVDS/DPA, EMIF/hard memory controllers,
  HSSI, HPS.
- Timing: the delay model is per node type, fitted to Quartus' paths
  (README (u)); Quartus' own RC model (`DYGR_ROUTE_TIMING_INFO`, the
  `timing_routing_delays_*` databases) is not decoded. a5pnr's own STA
  uses the model (README (z)). quartus_sta times an a5pnr placement on
  Quartus' routes (`a5pnr --quartus`, README (z4)); Quartus Pro takes no
  routing constraints, so a5pnr's own routes are timed by the model only.
- I/O registers from a5pnr and in the readback (their clock enters through
  the HIOL's `GLOBAL_LOCAL_SEL` I5 and RPI I29 to `REDMUX` I372; each
  register's active-low clock enable comes from a LUT constant through an
  HIOL RPI set to INV: ct, two pins, README (z6)), logic beside MLAB RAM, the
  second asynchronous clear line, PLL outputs other than V16's output 0
  (README (y), (z5)).
- Hyper-registers used as registers (`flop_mode` FLOP), clock gating in the
  clock network, LAB `lut_mode`/`latch_mode` (no bits in asm2).
- Any check on silicon: the DE25-Nano was off for all of phase 5; see the
  README's "Board test".
