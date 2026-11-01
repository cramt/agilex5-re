# `db/sm4revb`: the configuration database

Everything here is derived from Quartus Prime Pro 26.1.1's own device model
through its Tcl APIs (README (h)), then checked against fitter runs. Nothing
is a Quartus file. `tools/a5db` (Rust) reads it; `tools/bitmap/attrs.py` is
the Python equivalent.

## The configuration image

A `.sof` holds one configuration image (record 0x3a). For sm4revb it is
19,754,724 bytes made of 7 objects, each a header followed by data parts
(`tools/bitmap/imagemap.py`, `tools/a5db/src/db.rs` `PARTS`):

| object | parts | holds |
|---|---|---|
| `perictrl0`, `perictrl2`, `perictrl1` | `hdr`, `data` | HVIO banks (headers), HSSI (`perictrl0` data) |
| `io96_0`, `io96_1` | `hdr`, `t40`, `t41` | the two IO96 banks |
| `sector0`, `sector1` | `hdr`, `cram`, `ram` | the fabric: CRAM is 4,704 frames x 4,320 bits (540 bytes) |

In CRAM a bit address is (frame, bdata), with `bdata = byte * 8 + bit`
inside the frame. Fabric rows are 98-frame groups (frame 200 + 98·(Y − 6)
for Y ≥ 6, frame 4 for Y = 5). Columns run along bdata; sector 0 is the left
half (X ≤ 60) and sector 1 the right half, mirrored.

## Files

- `floorplan.tsv`: every block instance (`uid`) with its BCM type, fitter
  element and X/Y/Z. `<type>_r<y>_c<x>` uids are at fitter X/Y.
- `bits/<type>.layout.tsv`: `layout, bit, part, dframe, dbdata, polarity`.
  The block's named configuration bits, each relative to the anchor of the
  image part it lives in. Polarity 1 means the image bit equals the named
  bit; 0 means the image holds its inverse.
- `bits/<type>.anchors.tsv`: `uid, layout, object, part, frame, bdata`, one
  row per instance and image part (an M20K has a `cram` row for its
  configuration and a `ram` row for its contents). For CRAM the bit is at
  `(frame + dframe, bdata + dbdata)` of that object's sector. For any other
  part, `frame + dframe` is a bit offset into the part (byte = offset / 8,
  bit = offset % 8) and `bdata` / `dbdata` are 0.
- Every block type has exactly one layout.
- A table bigger than 4 MiB is stored xz-compressed as `<name>.tsv.xz`
  (`xz -9`, which Python's `lzma` preset 9 reproduces byte for byte):
  `io96.layout.tsv` (318 MB, 2.7 MB stored) and `sm_hssi_top.layout.tsv`
  (45 MB, 0.9 MB). Readers look for `<name>.tsv` first, then `.tsv.xz`
  (`a5db`'s `db::read_table`, `tools/bitmap/table.py`); `pack.py` and
  `aliases.py` write whichever form the size calls for.
- `bcm_types.json`: the BCM type tree. For each type, `children`
  (instance name → type), `custom` (CUSTOM attribute names) and `direct`
  (DIRECT attribute widths). A block attribute path is the instance names
  down to a leaf type plus the attribute name, e.g.
  `xtophalf/xale0/xcw_la_le_reg0/xreg0/reg_input` on `cw_la_lab`.
- `bcm_settings.tsv`: `intent, attribute, setting, bits, values`. For a
  leaf type's CUSTOM attribute, each setting's bit names (a Tcl list,
  relative to the leaf) and values. A value written `<v>` doesn't matter
  when decoding and is written as `v`.
- `direct.tsv`: `type, attribute, bits_msb_first`. A DIRECT attribute's bit
  names in the order of its value string, MSB first (asm2's
  `get_attribute` format): every DIRECT attribute of the LAB, MLAB, M20K,
  DSP and HVIO blocks (`tools/bcm/direct.tcl`).
- `idle.tsv` (`.tsv.xz`): `type, bit`. The named bits of a block type that
  are 1 in the idle (unused) state; every other mapped bit idles at 0.
  `idle_inst.tsv`: `uid, bit, value`, per-instance exceptions to that.
  `idle_unnamed.tsv`: `object, part, offset`, image bits no named bit covers
  that idle at 1 (offset in bits from the part's start). `a5db
  idle-derive` writes all three from Quartus images; `a5db blank` builds the
  idle image from them (README (n)).
- `io/template.tsv`: the I/O pin template (README (p)), `block, scope, use,
  std, uid, bit, value`. `scope` is `pin` (once per used pin), `lane_any`,
  `lane_in`, `lane_out_only` (once per IO96 lane with any pin, with an
  input, with outputs only) or `bank` (once per IO96 with any pin); `use` is
  in, out, clock_in, in_reg or out_reg (`*` any); `std` the I/O standard
  (`*` any). In `uid` and `bit`, `{uid}`, `{obj}` (io96_0/io96_1), `{pin}`
  (`xx20_u<b>/xx4_u<g>/xx1_u<k>`), `{bank}` (`xx20_u<b>`) and `{n}` (4g + k)
  are substituted and other `{…}` fields are sums of products of integers
  and L (lane), p (pin in lane). A uid `@object/part` is an unnamed image bit
  at that offset. Written by `tools/pins/derive.py`; `a5db pintable`
  expands it (plus the HVIO clock-mux rule in `src/pins.rs`).
- `io/pins.tsv`: `ball, uid, pad, function` for every HVIO and IO96 ball of
  the package.
- `io/de25_pins.tsv`: `signal, ball, use, uid, bit, value`, the template
  expanded for each DE25-Nano pin alone, as input and as output.
- A table of `uid, bit, value` rows (`#` lines are comments, `@object/part`
  uids as above) is what `a5db apply` writes and `a5db pintable` prints.
- `direct_io96.tsv.xz`: `direct.tsv`'s format for the IO96's 4,775 DIRECT
  attributes (4.28 M bits); a5db doesn't load it.

## Design files

`a5db extract`/`build`/`rebuild` read and write a design: tab-separated
lines `pin <spec>`, `edge <src> <dst>`, `attr <uid> <attribute> <setting>`,
`direct <uid> <attribute> <bits MSB first>`, `bit <uid> <bit> <0|1>`, `raw
<object/part> <offset> <0|1>`, applied over the idle image in that order
(README (q), [toolchain.md](toolchain.md)).

## Conventions worth knowing

- `lut_mask` as stored (asm2's DIRECT value) is `rb{j>>5}lut[j & 31]` for
  bit j. A WYSIWYG `tennm_lcell_comb` `lut_mask` W relates to it as
  `W[i] = ¬stored[i XOR 31]` (README (h)).
- Bits that asm2 can't read (`is_bit_defined` false) and spare-row
  instances (`*_rsp*`) are not in the maps.
- A named bit that touches more than one image bit, or an image bit owned
  by several named bits, is left out of the maps (the "clash" counts in
  README (h)). For LABs these are emulation and DFT bits.
