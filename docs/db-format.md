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
  `get_attribute` format). Only LAB/MLAB `lut_mask` so far.

## Conventions worth knowing

- `lut_mask` as stored (asm2's DIRECT value) is `rb{j>>5}lut[j & 31]` for
  bit j. A WYSIWYG `tennm_lcell_comb` `lut_mask` W relates to it as
  `W[i] = ¬stored[i XOR 31]` (README (h)).
- Bits that asm2 can't read (`is_bit_defined` false) and spare-row
  instances (`*_rsp*`) are not in the maps.
- A named bit that touches more than one image bit, or an image bit owned
  by several named bits, is left out of the maps (the "clash" counts in
  README (h)). For LABs these are emulation and DFT bits.
