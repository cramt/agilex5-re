# Quartus Prime Pro 26.1 `.ddb` format (PDB version 25)

How Quartus Prime Pro 26.1.1 stores its device databases
(`devices/10nm/...`), and how `tools/ddb` reads them. This is the successor of
the Quartus II 13 format in
[cyclone2-re/docs/ddb-format.md](../../cyclone2-re/docs/ddb-format.md).

Tags as in the README: **[obs]** observed (checked against Quartus's own
`PDB_ASCII_DUMP` trace unless stated), **[inf]** inferred, **[unk]** unknown.

## The oracle trace

Put `PDB_ASCII_DUMP=on` in `quartus.ini` in the working directory. Every
PDB database a Quartus tool opens is then traced to `<database>.ddb.dmp`
next to the database, inside the install tree. `tools/pdbdump.sh` runs a tool
that way in a scratch copy of a project and moves the traces out to
`specimens/pdbdump/devices/...` afterwards, so the install stays pristine.
[obs]

The trace format differs from Quartus 13:

- One line per primitive read: the value as stored (hex, width = 2 digits
  per byte), then the C++ member path (`ptr.m_pin_list.val[3]`). **Values
  equal the raw bytes**: v25 has no delta encoding. [obs]
- Pointers print as `@<in-memory address> <raw value> <member>`. [obs]
- A deferred object body starts with a line `@<address>: <CLASS>`. [obs]
- String lengths print twice (`n`, then `n+1`); only `n` is stored. [obs]
- Doubles print as `%.15E`. [obs]
- **Plain `int` array elements are not traced at all.** That covers every
  `m_int_data[N]`, `m_double_data[N]`, the inner elements of
  `vector<vector<int>>` and `vector<int>` slots of the generic info bodies.
  Vector *counts* are traced; `m_pad_id_list`-style member vectors are traced
  element by element. [obs] The untraced arrays are located by byte
  alignment and their lengths are the bit counts of the matching
  `*_enum_is_valid` vector. [obs]

`tools/ddb/verify/trace_tokens.py` turns a trace into a token stream;
`ddb trace <file.ddb> <schema>` prints the same stream from the decoder;
`verify/tokcmp.py` requires the streams to be identical and the decoder's
`table#id` to map one-to-one onto the trace's object addresses.
`verify/run.sh` runs all of it.

## Outer container [obs]

| off | type | trace name | value |
|-----|------|-----------|-------|
| 0x00 | i32 | `version_number` | 0x19 (25) |
| 0x04 | u16 | `header_id` | 0x0BAD |
| 0x06 | i32 + i16 | (UNUSED) | 0 |
| 0x0C | u16 | `m_size_of_header` | 0xAA |
| 0x0E | u16 | `m_end_of_header_pos` | 0xAA |
| 0x10 | c_str | `version_string` | `Version 26.1.1 Build 130 08/06/2026 SC Pro Edition` |
| | u64 | `m_size_of_body` | uncompressed |
| | u64 | `m_end_of_body_pos` | file offset |
| | u64 | `m_size_of_footer` | uncompressed |
| | i32 ×4 | `m_compression_info.{is_enabled,type,chunk_size,level}` | 1, 2 or 0, 0, 7 or 1 |
| | c_str | `pdb_core_signature` | SHA-1("") in every file |
| | u64 | `m_end_of_footer_pos` | = file size |
| | u64 | `m_num_user_sections` | 0 (1 in two files, see below) |

`c_str` is an i32 length that counts a NUL which is not stored. Body is
`[0xAA, end_of_body)`, footer `[end_of_body, end_of_footer)`, each its own
stream. Compression type 2 is raw LZMA1 (5-byte properties header, no size,
no end marker; the size comes from the header), type 0 is zlib ending in a
sync flush. Chunk size is 0 everywhere.

`ddb check` on the 771 files relevant to sm4revb (`devices/10nm/sm4revb/*`,
`ddb_sundancemesa_*`, `ddb_z1698c_*`, `devices/ddb_cumulative_*`): **765
parse** (header, both streams to the declared size, footer to its last
byte); 612 are LZMA1, 153 zlib. The other 6 are not PDB: `die_comb` and
`info_counts` are `ac 0f 01 00 00 00 00 00` + zlib text ("Family/device/part
info - ASCII dump by DEV"), and the four `.mdb` files are a different
mapped-database format (below). `ddb_sm4revb_bcm_powerdown.ddb` and
`ddb_dspf_re_reduced_graph.ddb` declare one user section but have no bytes
outside the three sections; where it lives is [unk].

## Footer [obs]

```
i16   number_of_classes
c_str m_root_type_name
repeat:
  i16 class_id; i32 class_size; u32 hash_value;
  i32 number_of_instances; i32 ptr_table_size; c_str class_name
```

No `processed` byte any more, and `number_of_instances` is filled in
(`ptr_table_size` = instances + 1).

## Body [obs]

- Integers are raw little-endian, no deltas.
- `std::string`: i32 length + bytes, no NUL.
- `std::vector<bool>`: i32 bit count + ceil(n/32) u32 words.
- Containers: i32 count + elements; maps are key, value pairs.
- **Pointer**: one i32 `v`. `v == 0` null. `v & 3 == 0`: a new object of
  footer class `v >> 2`, next id in the pointer table of the declared type.
  `v & 3 == 2`: back-reference, id = last + (v >> 2) (arithmetic), where
  `last` is the table's previous *back-reference* (new objects don't move
  it; it starts at 0). `+2` is "same as last", `+6` "next", `-2` "previous".
  `v & 3` odd is never seen.
- The root object (`ptr` `+4`, class 1) is inline. Every other object body
  is deferred and written table by table, the most recently created table
  with pending objects first, all of it in id order (as in Quartus 13).

## Schemas

`tools/ddb/schema/*.schema` describe classes as data (field, type), and one
interpreter (`src/reader.rs`) decodes them. Verified token for token:

| schema | trace checked | tokens | objects | also decodes |
|---|---|---|---|---|
| `pkg` | `ddb_sm4revb_eb_b_b795b_csvpbga795b_pkg` | 52,054 | 841 | all 96 sm4revb `*_pkg.ddb` |
| `pad` | `ddb_z1698c_pad`, `ddb_sundancemesa_pad` | 191,187 + 2,200 | 1,226 + 80 | the pad file of all 9 Agilex 5 dies |
| `hdr` | `ddb_sm4revb_eb_b_b795b_cs_hdr` | 327,184 | 6,617 | all 96 sm4revb `*_hdr.ddb` |

"Also decodes" means the body is consumed to its last byte with every
pointer in range; there is no trace for those files.

## `pkg` and `pad`: balls -> pads -> I/O locations

Same generic info-body design as Quartus 13 (fixed slot arrays plus
`*_enum_is_valid` bit vectors), with bigger arrays and new classes
(`DEV_PKG_NET` power nets, `DEV_IO_RULE`). `ddb pins <pkg> <pad>` joins
`DEV_PIN::m_pad_id_list` to `DEV_PAD_BODY::m_id` and prints every slot.

| slot | meaning | tag |
|---|---|---|
| `DEV_PIN::m_string_data[0]` | ball name (`DF35`) | [obs] |
| `DEV_PIN::m_string_data[2]` | pad name the ball bonds to (`P61X0Y0`, `HVIO_B1_2_1_X2Y3`) | [inf] |
| `DEV_PIN::m_string_data[11]` | pin-out function (`D2_D1_2A_DQ17_N`, `HVIO_B1_2_1_6C`) | [inf] |
| `DEV_PIN::m_pad_id_list` | `DEV_PAD_BODY::m_id` of the pad | [obs] structure |
| `DEV_PAD_BODY::m_string_data[1]` | location `X<x>Y<y>SUB_LOC<n>` | [inf] |
| `DEV_PAD_BODY::m_string_data[3]` | `Column I/O`, `JTAG`, ... | [inf] |
| `DEV_PAD_BODY::m_string_data[4]` | functions (`IOB, CLK_T_2A_1n, DIFF_IO_2A_T18n, DQ2`) | [inf] |
| `DEV_PAD_BODY::m_string_data[12]` | block type (`io96`, `hviowr`) | [inf] |
| `DEV_PAD_BODY::m_string_data[25]` | I/O block instance (`pio_0_0`, `hvio_3_1`) | [inf] |
| `DEV_PAD_BODY::m_int_data[0]` | global id, key of `m_ids_by_global_id` | [inf] |
| `DEV_PAD_BODY::m_io_standards` | supported standards, via `DEV_IO_STANDARD_DESC_BODY::m_string_data[0]` | [inf] |

## `hdr`: device and element tables

`DEV_BASIC_INFO_BODY` holds the part strings (`A5EB013BB23B_CS`, `Agilex 5`,
`sm4revb`, `falconmesa`, ...) and 3,302 `DEV_ELEMENT_DESC_BODY` entries:
`m_dev_element_data[0]` = element enum, `m_string_data[0..1]` = name and
description (`LE_COMB`, `FF`, `IO_PAD`, routing wires `H0 H1 H2 H4 H6 H12
V1 V4 V8 LAB_LINE`, clock muxes ...). 6 `DEV_LOCATION_DESC` are the
coordinate kinds (`INDEX X Y Z SUB_LOC B`). [obs] structure, [inf] meaning.

## Not PDB: `.udm` and `.mdb`

`devdata/10nm/sm4revb/sm4revb.udm` (482 MB, mapped by `devdata/udm.paths`)
and the `ddb_*_dspf_routing_models_*.mdb` files share a header starting
`02 00 00 00 00 00 00 00 08 ...` followed by 64-bit absolute offsets (the
last near the file size) and a `SCHEMA_ID` record (at 0x2fe0 in the udm).
[obs] The udm holds the full-chip hierarchy (`MODULE_SPARSE`,
`INPUTPORT_SPARSE`, RTL block and register names). [obs] No per-die
routing `.ddb` exists in Quartus Pro, so the routing graph is most likely
built from the udm at load time. [inf] Its format is [unk].
