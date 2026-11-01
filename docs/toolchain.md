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
| LUTs | `lut_mask` DIRECT; WYSIWYG mask W relates as `W[i] = ¬stored[i XOR 31]`; LUT input A–F of ALM k is `LAB_RE` pin 8k + 3, 4, 2, 1, 0, 7 | rebuilt (ar, mm, hx), held out (hx), readback; fitter ECO runs (h) |
| registers | 4 per ALM (`xcw_la_le_reg{0,1}/xreg{0,1}`); `reg_input` LUTOUT_6LUT/5LUT, ARITH_SUMOUT or LEIM (packed with unrelated logic); `register_comb` DFF_LATCH, REGIN, WIRE_MODE (bypass) | rebuilt, held out, readback |
| register controls | two LAB clocks (`regclk`), two clears (`regclr`), enables (`ce_sel`), synchronous clear (`sclr_sel`), inverted clears through the routing tile's `iss/xclrb_rpi` | rebuilt (ar: async + sync clear, enable, load; hx: up/down, enable, async preset value, sync clear), held out |
| carry chains, shared arithmetic | `adder_mode` ENABLE_ADDER, `adder_or_packed_mode`, `reg_input` ARITH_SUMOUT | rebuilt (32-bit accumulator, ternary and 4-input adders), held out |
| MLAB RAM | `ram_mode` LUTRAM_32DEEP on the MLAB's `din`, `csel` and `lram_sigctrl` | rebuilt (32 x 16, 16 x 8), held out |
| M20K | CUSTOM settings for modes, widths, ports and registers (42 in mm) plus contents: `m20k_eram_add<k>` = `eram_array_r<k/2>[40(k mod 2) + 39 .. ]` in the sector's RAM part | rebuilt (512 x 32 SDP, 1024 x 16 ROM, 2048 x 10 TDP with init), held out |
| DSP | `dsp_mode` (`rmode[4:0]`, a 5-bit code), register and clock selects, signs, `dsp_proginv_{l,h}` | rebuilt (18x18, 27x27 MAC, 9x9 + 18x19), held out |
| general routing | graph edges → mux settings (`MUXINk_` per `muxin[k]`), plus `flop_mode = FAST_WIRE` on every used mux with a hyper-register except LEIMs into LAB control inputs (I80+) | rebuilt (every design), routes of rt0 and fz0 (k), held out |
| clock network | graph edges; edges without an edge API (spine tap 5:1, row clock 32:1, predrive, delay chain, DCM P2C 4:1) through `graph::clock_rule` | rebuilt (ck1, ck2, mm, pl), 22 P2C muxes held out (r) |
| HVIO pins | per-pin bits only; standard in `iio_config_[1:0]` (3.3 V 00, 1.8 V 10) | 159 pins, both directions, held out (ph1, ph2) |
| HVIO clock pins | 2 per bank half: pins k0/k1 through the 2:1 ioclk mux, k2/k3 through the 4:1 ss clock mux | all 32 clock pins, held out (ck2) |
| HVIO IO registers | uses `in_reg`, `out_reg` | 3 pins (ck1), rebuilt |
| IO96 pins | pin, lane and bank rows; lane vref values depend on whether the lane has an input; IOSSM pin map | 22 pins, both directions, held out (ph1, ph2), CLOCK0_50 as a clock |
| IOPLL | 40 attributes, 10 counters, 19 bits in the HVIO's `xpllwrap_top` | one configuration (50 → 100 MHz), rebuilt |
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
  input leaves through one of two input register muxes (`in0`/`in1`), and
  that choice goes with the route out of the pin; which graph node each mux
  feeds is [unk], so take the default (`in`).
- **Fabric pins**: ALM k's LUT inputs and register outputs are `LAB_RE`
  pins (README (n)); the LAB clock is pin 83.
- **Group 1 / spare row**: never write the spare row (`_rsp5_` instances)
  or the redundancy tiles; the image has no place for them (s).

## Not covered

- Bidirectional pins and output enables from the fabric, pull-ups, bus
  hold, slew rate, current strength, open drain, Schmitt trigger: asm2 has
  those attributes as `<VIRTUAL>` and no design here used them.
- Differential I/O, the D-PHY standard, IO96 lanes 0–3 and 6–7, the second
  IO96 bank (`pio_1_0`), the left-side DCMs: no DE25-Nano pin reaches them.
- The pin ↔ routing-graph node map for every pin (which fabric node carries
  a pin's input or output); routes so far start and end at the nodes
  Quartus used.
- IOPLL settings for any frequency but the one compiled; PLL cascading,
  dynamic phase shift, LVDS/DPA, EMIF/hard memory controllers, HSSI, HPS.
- Timing: the graph has no delays (`DYGR_ROUTE_TIMING_INFO` isn't dumped),
  so a router can't do timing-driven routing from this database.
- Hyper-registers used as registers (`flop_mode` FLOP), clock gating in the
  clock network, LAB `lut_mode`/`latch_mode` (no bits in asm2).
- Any check on silicon: the DE25-Nano was off for all of phase 5; see the
  README's "Board test".
