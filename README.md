# agilex5-re

Groundwork for an open bitstream database and toolchain for the Altera
Agilex 5, starting with the `A5EB013BB23BE4SCS` on the Terasic DE25-Nano.
This is the third target after the Cyclone II (`cyclone2-re`) and the
Zynq-7000. The open toolchain is chipsmith (branch `open`).

Quartus Prime Pro is used only as an **oracle** and as a source of device
data to reverse engineer, the same way `cyclone2-re` uses Quartus II 13. It
runs sandboxed with no network (`tools/quartus.sh`). Quartus-derived files
go in `specimens/`, which is ignored and never committed.

Every finding has a tag:

- **[obs]**: observed. Seen on this machine, or stated outright in an Altera
  document that is linked.
- **[inf]**: inferred from names, structure or other sources. Not verified.
- **[unk]**: unknown, and listed so it gets answered.

## Target

| | |
|---|---|
| Board | Terasic DE25-Nano |
| Part | `A5EB013BB23BE4SCS` [obs]: [Terasic product page](https://www.terasic.com.tw/cgi-bin/page/archive.pl?CategoryNo=123&Language=English&No=1384&PartNo=2). Quartus reports `{Agilex 5} VPBGA 795`, speed grade 4. The Sep 2025 [user manual](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7704/P0804.pdf) says `…SR1`, the engineering-sample suffix, so early boards may carry ES silicon [inf]. |
| Die | `sm4revb` [obs]: Quartus loads `devices/10nm/sm4revb/ddb_sm4revb_eb_b_b795b_cs_*` for this part. |
| Quartus | Prime Pro 26.1.1 build 130, the newest release. 25.1.1 is the first with programming-file ("Preliminary") support for this part; 25.3 onwards lists it as Final ([release notes](https://docs.altera.com/r/docs/683706/26.1/quartus-prime-pro-edition-version-26.1-software-and-device-support-release-notes/devices-with-final-device-support?contentId=tXQr1czj4DhMcOgtM54heg)) [obs]. |
| Install | `/pool/dev/altera_pro/quartus-pro/26.1.1` (32 GB), via chipsmith's `quartus-pro` backend (branch `quartus-pro`). |

DE25-Nano pins used by `designs/blinky` [obs, manual section 3.6 and
[spersvold/de25_nano](https://github.com/spersvold/de25_nano)]:

| Signal | Pin | I/O standard |
|---|---|---|
| CLOCK1_50 (50 MHz) | PIN_V16 | 3.3-V LVCMOS |
| KEY[0] | PIN_C8 | 3.3-V LVCMOS |
| LED[0] | PIN_DF35 | 1.1-V |

`CLOCK0_50` is PIN_DJ35 (1.1 V) and `CLOCK2_50` is PIN_BF23 (3.3 V). The
rest of `LED[1..7]` are DJ32, DN22, DP23, DN25, DP25, DJ27, DP30.

## Licensing

- **Agilex 5 E-series is free, but the Fitter needs a licence file** [obs].
  Synthesis (`quartus_syn`) runs with no licence at all. `quartus_fit` stops
  with `Warning (24850): Failed to acquire license for Quartus Agilex 5.
  quartus_agilex5e not found in product list` and `Error (119013): Current
  license file does not support the A5EB013BB23BE4SCS device`.
- The licence is Altera's no-cost node-locked one (feature
  `quartus_agilex5e`), issued against a NIC MAC address
  ([licensing guide](https://docs.altera.com/r/docs/683472/26.1/altera-fpga-software-installation-and-licensing/acquiring-free-no-cost-licenses)).
  Free since 24.1. It reportedly expires after a year [inf, distributor
  article].
- So the oracle can synthesise today, but it can't place, route or assemble
  until the user fetches a licence. `tools/quartus.sh` already handles
  that licence: with `ORACLE_LICENSE_MAC` set it gives the network namespace
  a `dummy` interface carrying the licensed MAC, with no address and no
  route. FlexLM gets a host id but nothing to connect to.

## Findings

### (a) Security and configuration model (SDM)

- **Agilex 5 is configured by the Secure Device Manager (SDM)**, a hardened
  processor that runs Altera configuration firmware (CMF). Every `.sof` and
  `.rbf` starts with that firmware, and Quartus inserts it automatically
  ([Agilex 5 config UG, SDM](https://docs.altera.com/r/docs/813773/26.1.1/device-configuration-user-guide-agilex-5-fpgas-and-socs/secure-device-manager))
  [obs].
- **The firmware is always authenticated.** The CMF is "developed,
  validated, and signed by Altera". The boot ROM authenticates and decrypts
  it on every configuration, whatever the owner has fused
  ([security whitepaper](https://docs.altera.com/api/khub/documents/i9xH61Lzj1uF8ZvxXUmBMQ/content),
  [HPS TRM](https://docs.altera.com/r/docs/814346/25.3.1/hard-processor-system-technical-reference-manual-agilextm-5-socs/booting-and-configuration-fpga-configuration-first-mode-overview))
  [obs].
- **Owner authentication of the fabric bitstream is optional.** It only
  takes effect once an owner root key hash is programmed into physical or
  virtual eFuses. "The unprovisioned device ignores non-Intel signature
  chains" ([Agilex security UG](https://device.report/m/ebb5ba0155d0f5a84f9ebf0fcdb79083b6e7d26cf450aa56bc92d3dadde1a2bb.pdf))
  [obs]. An integrity check (hash per section) applies either way [obs].
  `quartus_pfg -c -o sign_later=ON design.sof unsigned.rbf` makes an
  unsigned `.rbf` on purpose [obs, same guide].
- **The container format is documented.** Altera's
  [Security Technical Reference Manual](https://docs.altera.com/r/docs/d710186/current/security-technical-reference-manual)
  (2026-08) describes the sections: 4 KiB aligned, magic numbers, a
  descriptor and signature block for each one. Firmware section header magic
  is `0x97566593` at offset 0x400, and CMF type is `0x00464D43`. It also
  covers signature chain entries and permission bits such as `SIGN_SDM_FW`
  and `SIGN_FPGA` [obs, not yet checked against a real `.rbf`]. OPAE's
  `pacsign` and `altera-fpga/librsu` parse parts of it [obs].
- **JTAG configuration:** the bitstream goes to the SDM over JTAG using an
  SDM command channel. openFPGALoader can load an `.rbf` into SRAM on Agilex 3
  and 5 (`Altera::agilex_program`, IR codes `ISC_PROGRAM 0x002`,
  `COMMAND 0x201`, `ISC_SR 0x202`). It has been tested on a USB-Blaster III
  ([issue #605](https://github.com/trabucayre/openFPGALoader/issues/605))
  [obs]. Flash writes are not supported, because the SDM packet commands for
  them are undocumented
  ([PR #721](https://github.com/trabucayre/openFPGALoader/pull/721)) [obs].
  `part.hpp` only knows IDCODE `0x4369c0dd` (A5EC008B). The A5EB013B
  IDCODE has to be added [obs, local checkout], and reading it needs the
  board, which is out of scope here.
- **Verdict for an open bitstream writer** [inf]: feasible on an unfused
  board. The writer emits the fabric sections (CRAM and so on) and their
  hashes itself, and copies the **Altera-signed SDM firmware section
  verbatim** from a Quartus-built bitstream for the same part and Quartus
  version. That blob is redistributable only as far as Altera's licence
  allows, so the open flow should extract it from the user's own Quartus
  install, not ship it. It can't be generated or changed. A DE25-Nano
  whose owner fuses are blown would reject the open bitstream unless it is
  signed with that owner key; `quartus_sign` or an open signer could do it.
  Unknown: whether the firmware section depends on design options
  (configuration clock, I/O, HPS) and how often it changes between Quartus
  releases [unk].

### (b) Device database: same PDB/.ddb serialization, newer version

- **Location** [obs]: `<install>/devices/` (Pro's equivalent of
  `quartus/common/devinfo`). Agilex 5 data is under `devices/10nm/`, one
  directory per die (`sm1`, `sm4`, `sm4revb`, `sm5`, `sm7`, `sm7revb`,
  `fmm2`, `fmm3`), plus family files `ddb_sundancemesa_{hdr,info,pad}.ddb`
  ("Sundance Mesa" is the internal family name). The `agilex5` qdz holds
  4,151 `.ddb` files.
- **Same container family as Quartus 13** [obs]: magic `0x0bad` at offset 4,
  the version string, body and footer sections, and a footer class table.
  The version is **25** (Quartus 13 used 0x11), and the header is 0xAA bytes
  (was 0x74). cyclone2-re's `ddb info` rejects every file (`unsupported
  header version 25`, or a misparse), so it needs a v25 header path. Quartus's
  own trace names every new header field:

  | field | example |
  |---|---|
  | `version_number` | 0x19 |
  | `header_id` | 0x0BAD |
  | `m_size_of_header`, `m_end_of_header_pos` | 0xAA |
  | `version_string` | `Version 26.1.1 Build 130 08/06/2026 SC Pro Edition` |
  | `m_size_of_body`, `m_end_of_body_pos`, `m_size_of_footer` | u64 |
  | `m_compression_info.{is_enabled,type,chunk_size,level}` | 1, 2, 0, 7 |
  | `pdb_core_signature` | `da39a3ee…0709` (SHA-1 of the empty string) |
  | `m_end_of_footer_pos` | u64, the file size |
  | `m_num_user_sections` | 0 |

  Compression type 2 is **raw LZMA1** (a 5-byte properties header `5d`
  plus the dictionary size, then the stream), and type 0 is zlib.
  `tools/pdb25_probe.py` decompresses bodies and footers to exactly the
  sizes the header declares. The footer class table gains
  `number_of_instances` and `ptr_table_size`. Class names follow the
  same style (`DEV_PIN`, `DEV_PKG_INFO_BODY`, `DYGR_PLACE_INFO`,
  `ASMMDB_ROOT`).
- The reader is now `qcore/linux64/libccl_pdb.so` (was `libdb_pdb.so`)
  [obs].
- **`bcm2bits_db/*.bcm.xml` is encrypted** [obs/inf]. About 27 per die
  (`cw_la_lab`, `cw_ml_mlab`, `cw_mm_m20k`, `cw_rt_lab_base`, `io96`, …).
  Despite the name they are not XML: an 8-byte header `ac ef 01 00 00 00 00
  00` followed by data with 8.0 bits/byte entropy, a multiple of 16 bytes
  long. That looks like AES [inf]. `programming_sequence.pb.enc` uses the
  same wrapper with `ac 0f`, followed by plain zlib (empty for sm1).
  Quartus must hold the key [inf]. These "BCM to bits" tables are probably
  the per-block configuration bit maps, so decrypting them could replace a
  lot of fuzzing [inf]. Cyclone II had no such files.
- Other per-die files [obs]: `*_meta.pb` (protobuf; `sof_meta.pb` names
  sectors, frames, `_dr` and `_CRAM` regions, which describes the bitstream
  sector layout [inf]), `.mdb` routing models, `.udm`, `ddb_*_asmm.ddb`
  (zlib, 353 MB uncompressed for sm1, root class `ASMMDB_ROOT`; probably
  the successor of `asm`/`asmdb` [inf]), and timing per corner
  (`*_timing_*`, `*_fdi.ddb`).
- Where the routing graph lives: not in any `.ddb`. See (e).

### (c) `PDB_ASCII_DUMP` still works

- [obs] `PDB_SEGMENT_IMPL::PDB_SEGMENT_IMPL` in `libccl_pdb.so` does
  `this[0x58] = cfg_is_on("PDB_ASCII_DUMP")` (the string is built inline
  from two immediates at file offset 0x28ea9, which is why `strings` only
  shows `PDB_ASCI`).
- [obs] With `PDB_ASCII_DUMP=on` in `quartus.ini` in the working directory
  (Quartus prints `Info: Using INI file …`), every database a tool reads is
  traced to **`<database>.ddb.dmp` next to the database itself**, not to
  stdout as in Quartus 13. The format is close to the old one, but v25
  stores raw values (no deltas), so a line is value and C++ member name;
  plain `int` arrays are not traced at all
  ([docs/pdb-v25.md](docs/pdb-v25.md#the-oracle-trace)). For example
  `ptr.m_ids_by_string … = "DF35"` in the sm4revb package dump. A
  `quartus_syn` run plus one `get_part_info` call wrote 9 dumps (about
  290 MB, mostly `ddb_cumulative_info`).
- Consequence: the dumps land in the install tree. `tools/pdbdump.sh` runs
  a tool in a scratch copy of a project and moves every `.ddb.dmp` to
  `specimens/pdbdump/` afterwards; the install had no `.dmp` left after the
  runs here [obs].

### (d) Prior art

- **No public bitstream or device-database reverse engineering exists for
  Stratix 10, Agilex 7, Agilex 5 or Agilex 3** [obs: GitHub and web searches,
  and the
  [FLF list](https://www.fabienm.eu/flf/materiel/liberation-des-fpga/),
  which has only Mistral (Cyclone V) and project-chibi (MAX 10) for Altera].
- Security material is limited to vendor documents and advisories:
  CVE-2020-8737
  ([INTEL-SA-00388](https://www.altera.com/product-security/advisories/archive/intel-sa-00388),
  Stratix 10 SDM firmware), CVE-2023-41092, and Intel's SDM papers
  ([DTIC AD1052301](https://apps.dtic.mil/sti/pdfs/AD1052301.pdf)). Those
  papers describe per-sector Local Sector Managers and chained hash
  authentication.
- Useful public code: openFPGALoader's Agilex SRAM loader, OPAE `pacsign`
  (bitstream container and signature blocks), `altera-fpga/librsu`.
- The `PDB_ASCII_DUMP` trick is only documented in `cyclone2-re`.

### (e) Phase 1: v25 reader and device model

`tools/ddb` is a Rust crate forked from cyclone2-re's reader. Format
details are in [docs/pdb-v25.md](docs/pdb-v25.md).

- **Container** [obs]: v25 header (0xAA bytes), raw LZMA1 or zlib body and
  footer, v25 footer class table. `ddb check` parses **765 of the 771**
  files relevant to sm4revb to the declared sizes (612 LZMA1, 153 zlib).
  The other 6 are not PDB: 2 zlib text dumps behind an `ac 0f` wrapper and
  4 `.mdb` mapped databases. `ddb info` prints header and class table.
- **Body encoding** [obs]: no deltas any more; pointers are `class << 2`
  for a new object and `(rel << 2) | 2` for a back-reference. Class
  layouts live in `tools/ddb/schema/*.schema` files that one interpreter
  reads. `ddb trace` re-emits Quartus's trace token stream from the
  decoder, and `tools/ddb/verify/run.sh` compares it with the real trace.

  | schema | checked token for token against the trace | decodes cleanly |
  |---|---|---|
  | `pkg` (balls) | VPBGA 795 `_cs` package: 52,054 tokens, 841 objects | all 96 sm4revb packages |
  | `pad` (pads, I/O standards) | `z1698c` (sm4revb) and family pad files: 193,387 tokens | the pad file of all 9 Agilex 5 dies |
  | `hdr` (part info, 3,302 element types) | `eb_b_b795b_cs`: 327,184 tokens, 6,617 objects | all 96 sm4revb `hdr` files |
  | `place` (grid, block types, clock regions) | `ddb_sm4revb_place`: 6,319 tokens | |

- **(a) Pin map, DE25-Nano** [obs]: `ddb pins <pkg> <pad>` joins balls to
  pads. All 795 balls carry the same pad number as the blinky fit
  report's "All Package Pins" table (795/795,
  `verify/pins_fit_check.py`), and the 3 placed I/Os sit at the decoded
  pad X/Y:

  | signal | ball | pad | location | block | bank (fit report) |
  |---|---|---|---|---|---|
  | CLOCK1_50 | V16 | 383 | X108Y53 SUB_LOC764 | `hviowr` `hvio_3_1` | 6C, 3.3 V |
  | KEY0 | C8 | 403 | X107Y53 SUB_LOC764 | `hviowr` `hvio_3_1` | 6D, 3.3 V |
  | LED0 | DF35 | 124 | X62Y0 SUB_LOC121 | `io96` `pio_0_0` | 2A_T, 1.1 V |
  | CLOCK0_50 | DJ35 | 125 | X62Y0 SUB_LOC118 | `io96` | 2A |
  | CLOCK2_50 | BF23 | 427 | X121Y8 SUB_LOC1186 | `hviowr` `hvio_1_1` | 6E |

  The fit report's Z (778 for V16, 123 for DF35) is not the pad's
  SUB_LOC; it is presumably the I/O buffer cell [inf]. Which slot holds the
  bank is [unk] (no string slot matches the report's bank names).
- **(b) Routing graph** [obs for the counts, inf for the storage]: the
  fabric grid is **122 x 54** (`DYGR_BLK_INTERFACES_INFO::m_nx/m_ny`;
  pads run to X121, Y53). Quartus's route report lists **3,293,202
  routing resources** in 34 classes, among them 500,456 block input
  muxes, 526,400 block interconnects, 526,400 direct links, R0/R1/R2/R4/R6/
  R12 = 401,380 / 229,360 / 115,620 / 117,500 / 118,440 / 34,404 and
  C1/C4/C8 = 234,240 / 229,360 / 22,936. The element enum table in `hdr`
  names the wire types (`H0 H1 H2 H4 H6 H12 V1 V4 V8 LAB_LINE`, clock
  muxes). **No routing/icname `.ddb` exists** in Quartus Pro. The graph is
  most likely built at load time from `devdata/10nm/sm4revb/sm4revb.udm`
  (482 MB, the `.mdb` mapped-database format, which holds the full-chip
  module and port hierarchy) [inf]. Decoding the udm is [unk] and the next
  step for routing.
- **(c) Configuration bits** [obs for structure, unk for meaning]:
  `ddb_sm4revb_asmm.ddb` (`ASMMDB_ROOT`, 392,070,987-byte body) is one
  opaque member, `m_pdb`, which Quartus serializes with raw
  `get_int`/`get_char` calls. The trace therefore only gives widths, at
  about 0.9 MB/s; a full trace would take about 5 hours, so only a
  131 MB prefix was kept. Its first part is a table of **4,911,334**
  `(i32 link, i32 len, chars)` records ending at body offset 0xB97E7E6:
  configuration-bit and feature names such as `cram_mux2_hg[3]`,
  `powerdown_mode`, `iossm_mem_init_N_bit[k]` (40 x 32,768 of those), and
  11,210 `cram*` names. The other 198 MB, after the table, is a different
  record format and is [unk]. Decoding it needs the reader code (REA on
  the asmm library) or a full trace. The `bcm2bits_db` files stay
  encrypted [inf].

### (f) Phase 1: bitstream container (blinky)

`tools/bitstream.py` prints the structure of a `.sof` or `.rbf`. Numbers
are from blinky (`/tmp/agilex5-re/pfg`, not committed).

- **`.sof`** [obs]: gzip (536,285 bytes) of a 19,783,011-byte image: `SOF\0`,
  u32 0, u32 record count (13), then `(u16 tag, u32 length, data)`
  records. Tag 0x01 is the Quartus version, 0x02 the part
  (`a5eb013bb23be4scs`), 0x2f the die and snapshot (`sm4revb`,
  `snap-REL1.0SM4revC0__26ww115a--Grp14`), and 0x36 an 18,499-byte
  protobuf (the same kind as `sof_meta.pb`). The first **0x3a record
  holds the uncompressed configuration image: 19,754,724 bytes**, exactly
  the size `devices/10nm/sm4revb/sof_meta.pb` declares. Of its 4,822 4 KiB
  pages, 1,457 are non-zero. Three more 0x3a records (3,458 / 725 / 5,166
  bytes, protobuf-like) follow, then 0x24 (`design_hash.bin`), 0x41
  (40 bytes, hash-like) and 0x08 (a 2-byte CRC). The `.sof` has no SDM
  firmware; `quartus_pfg` adds it.
- **`.rbf`** (`quartus_pfg -c blinky.sof blinky.rbf`, 970,752 bytes;
  `--check_integrity`: OK) [obs]: magic 0x62294895 at 0, then three
  4 KiB-aligned sections, each with header magic 0x97566593:

  | section | offset | size | content |
  |---|---|---|---|
  | `CMF` | 0x000000 | 434,176 | SDM firmware 26.1.1 (0x1A010100), build id `Sign-26.1.1sm-7`; header at +0x400 as in TRM table 35 |
  | `IO` | 0x06A000 | 413,696 | I/O configuration; "IO hash" from the pfg log sits at +0x348 |
  | `CORE` | 0x0CF000 | 122,880 | fabric; "Design hash" at +0x1E0 |

  Each section's first two 4 KiB blocks end in a CRC-32/BZIP2 (MSB-first
  poly 0x04C11DB7, init and xorout 0xFFFFFFFF) over `[block,
  block+0xFFC)`, stored as a little-endian word [obs, phase 2]. The pfg log's two "Keyed hash" values are at
  +0xBB0 of `IO` and `CORE`. Which bytes the IO/design hashes cover is
  [unk]: plain SHA-256 of the section, of the `.sof` image and of 4 KiB
  sub-ranges don't match. The 19.75 MB image shrinks to 120 KB of `CORE`,
  so design sections are compressed or sparse [inf].
- **Firmware is copied, not generated** [obs]: the `CMF` section is
  byte-identical to `devices/programmer/firmware/agilex5e.zip:
  sm4revb/SHA_384/main.cmf` from the install, **except 5 bytes**: the
  word at 0x1F00 goes from 0 to 2 (probably the count or pointer of the
  sections that follow [inf]), and the CRC at 0x1FFC is recomputed. So
  an open writer can take the firmware from the user's own install, patch
  one word and one CRC, and leave Altera's signature untouched. That
  resolves the "can't be generated" concern in (a); whether 0x1F00 depends
  on more than the section count is [unk].
- `-o sign_later=ON` produces a byte-identical `.rbf`: the default
  output already has no owner signature on the design sections [obs].

### (g) Phase 2: `.sof` integrity and the sector image

`tools/sof.py` reads, edits and re-signs a `.sof`; `tools/pbraw.py` dumps
any protobuf (`sof_meta.pb`, the `.sof` 0x36 record) without a schema.

- **The `.sof` has two integrity values, and `quartus_pfg` checks both**
  [obs]. Changing one byte anywhere gives `Error (19192): ... The SOF file
  content is corrupted`.
  - Record 0x08 is a **CRC-16/X-25** (reflected poly 0x8408, init and
    xorout 0xFFFF), little endian, over every byte before its own data
    (`PGM_COMMON::calculate_sof_crc16` in `libpgm_common.so`).
  - Record 0x41 is 8 zero bytes and an **HMAC-SHA256** over every byte
    before the HMAC itself (`OSTREAM_HMAC_CRC_TRANSPORT` in
    `qcore/linux64/libpgm_pof.so`). The key is `HMAC_KEY_1` XOR a 32-byte
    mask that `HMAC_TRANSPORT`'s constructor stores. It is Altera's, so it
    isn't committed: `sof.py key` derives it from the local install into
    `specimens/`.
  - With both recomputed, `quartus_pfg` turns a hand-edited `.sof` into an
    `.rbf` in about 20 s. A flipped image bit becomes a changed `.rbf`
    with no fitter run, so the `.sof` → `.rbf` mapping (Plan step 2) can
    be measured bit by bit. A re-signed but unedited `.sof` gives a
    byte-identical `.rbf`, so pfg output is deterministic [obs].
- **Image layout** [obs, `sof_meta.pb` field 3 against the blinky image]:
  the 19,754,724-byte image is 7 programming objects in a row, each a
  header (type 9) followed by data, with sizes from field 9
  (`pgm_<name>~<n>`, entries `(type, ?, count, width in bits)`):

  | image offset | object | parts |
  |---|---|---|
  | 0 | `pgm_perictrl~0` | 902-byte header, type 29: 1,048,576 x 32 bits |
  | 4,195,206 | `pgm_io96~0` | 5,855-byte header, type 40: 4,096 x 32, type 41: 132,352 x 32 |
  | 4,746,853 | `pgm_perictrl~2` | as `~0` |
  | 8,942,059 | `pgm_io96~1` | as `~0` |
  | 9,493,706 | `pgm_perictrl~1` | 2,658-byte header, type 29 |
  | 13,690,668 | `pgm_sectorbnxplv~0` | 412-byte header, **type 0 (CRAM): 4,704 frames x 4,320 bits**, type 1: 24,576 x 160 bits |
  | 16,722,760 | `pgm_sectorbnxplv~1` | 284-byte header, type 0, type 1 |

  So the A5EB013B fabric is **two sectors of type `sectorbnxplv`**, which
  matches pfg's "Regular SEU info => 2 sector(s)". Each sector's CRAM is
  4,704 frames of 4,320 bits (540 bytes); `sof_meta.pb` field 4 builds it
  as `sectorbnxplv_CRAM` = 48 x `_frames` = 48 x 98 x `_dr` (4,320 bits).
  Which of the two `perictrl`/`io96` copies is which instance is [inf]
  from the header sizes. Type 1 (160-bit words) is all zero in blinky
  (memory contents, presumably) [inf].
- **The CRAM is highly regular** [obs]: sector 0 of blinky has 1,549,318
  bits set but only 55 distinct frames, 945 of them identical. Set bits
  repeat every 16 bytes (`ff` at byte 0, 14, 16, 30, …), so most of them
  are default values, not design bits [inf].

#### CORE section: 2D compression

`python3 -I tools/core_decode.py decode x.rbf out.bin` rebuilds both
sectors' CRAM (2 x 4,704 x 540 bytes) from the `CORE` section; `verify
x.rbf x.sof` compares it with the `.sof` image. The encoder is
`FBGEN_KM_DC_THREAD::compress` and `FBGEN_ND_2D_ENGINE` in
`quartus/linux64/libpgm_fbgen.so` (decompiled with rz-ghidra);
`final_2d_compression` in `libpgm_bitasm.so` only decides whether to use it.

- **Pages and streams** [obs]: the per-entry word of the SHA-384 chain
  tables (g2) is the stream id: 0 = sector 0, 1 = sector 1, 2 and
  0x80000002 shared pages, 4 = the next chain table, inserted after the
  78th entry. Pages of the two streams alternate once both are present.
  Per stream, in page order: lib pages (count in the `0x15 (n, stream, 4)`
  record of the action list at +0x6000), a 256-entry byte table, a page
  holding a deflated 482,784-byte block that is all zero in every specimen,
  a second byte table, the BBM page, then one raw-deflate frame stream
  (`0x17 (n, 0x810, …)` gives the page count of both). The byte tables
  and the zero block don't take part in CRAM decoding [obs]; what they are
  for is [unk].
- **BBM page** [obs]: 15 words `(id, width/16 bits, 98, 0)`, a u16, then
  48 rows of `u16 0` and 60 nibbles, low nibble first. Row 0 is the IP id
  of each of the 60 columns; rows 1..47 hold one code per column for one
  98-frame group, in the order the groups are stored. Widths: id 1 and 2 =
  10 bytes, 3 = 24, 4 and 5 = 16, 6 and 15 = 8. The 60 columns add up to
  a **656-byte physical frame**.
- **Frame stream** [obs]: one packet per frame, `04 00 a4 00`, 4 zero
  bytes, then the literal columns (codes 0 and ≥ 8) in column order, each
  as stored in the physical frame. Groups go 0, 2, 3, …, 47: **group 1
  (frames 98–195) is never stored**. Randomising it gives a byte-identical
  frame stream, so pfg drops it.
- **Codes** [obs from the decompile; each one checked against specimens]:
  0 literal; 1 all zero; 2..7 copy of lib `code-2` for that id; 8 and 9
  literal XOR frame `f-18` of the same group, for frames 26..97, on the
  bytes whose 2-bit class is 1 or 2 (`get_row_history_iprow_data`); 10+k
  literal XOR lib k. The class bytes, measured on the random specimen:
  id 1: 8–9 / 6–7, id 2: 0–1 / 2–3, id 3: 10–11 / 12–13, id 4: 4–5 / 6–7.
  Ids 5, 6 and 15 never got codes 2..15 here, and whether they can is [unk].
- **Libs** [obs]: only ids 1..4 have them. The `0x1f` action record
  (`stream, 0x01260000, 0x0372024c, 0x436, …` in blinky) holds each id's
  lib offset in 8-byte units (0, 0x126, 0x24c, 0x372, end 0x436). Each lib
  is 98 frames of the id's physical bytes at a stride rounded up to 4
  (under 12 bytes) or 8: blinky has 2, 2, 1 and 1 libs. The libs are
  extracted from both sectors together (editing one sector changes both
  streams) and stored once per stream.
- **Physical frame ↔ dr** [obs, fitted bit by bit on a random `.sof`,
  then checked]: read the dr from bit 4,319 down to 0 into physical bits
  0, 1, … (LSB first). Each of the 253 `s4` groups that `sof_meta.pb`
  lists at bit g is stored as 12 bits: `g+5, g+1, g+4, g`, then the
  nibbles of the pairs `(g+3, g+7)` and `(g+2, g+6)`. A nibble is `00ab`,
  or has its bit 0 set (`1001`, `1101`, `1011`, …) when exactly one of a,
  b is 1. Seven dr ranges (256–275, 1664–1675, 2080–2099, 2496–2515,
  3392–3403, 3808–3827, 4192–4215; 128 bits) have no physical bit, and
  flipping them leaves the stream unchanged. The 4-bit groups at 4264, 3488
  and 1760 become `pad pad b+2 b+3 pad pad b b+1`. 32 zero bits sit at
  physical bit 2528 (inside the id 6 column).
- **The S4 flag is the one gap** [unk]: which bit of the pair a flagged
  nibble stands for depends on a fixed per-(frame, nibble) polarity table.
  It is identical in two different random specimens. When the flag fires
  is [unk] too: it isn't explained by any single dr bit within ±300 frames
  of either sector (`fill_core_frame_data` takes it from `S4_SII_BIT`
  lists, which `BITASM_CRAM_REPAIRER` supplies). The decoder reads a
  flagged nibble as (1, 0) unless it is `1100`/`1110` (bits 0, 1
  [, 2] set), and that is right for 97% of the flagged nibbles in the
  random specimen.
- **Verification** [obs] (group 1 and the 128 dropped bits excluded):
  blinky decodes exactly, including 6 flagged nibbles. So do a frame of
  `0x5a` in sector 0 and one in sector 1. 300 random bit flips per sector
  leave 1 frame with 2 wrong bits, and 40 random frames plus 100 flips per
  sector leave 3 frames wrong. Both residues are flagged nibbles of the
  other polarity. An all-random `.sof` leaves 165 frames per sector wrong,
  all in flagged nibbles; every other physical bit matches.
  `core_decode.py reencode` rebuilds pfg's inflated packet streams
  byte for byte from the decoded physical frames, BBM and libs.
- **Not yet an encoder** [unk]: the deflate itself is solved [obs]:
  `zlib.compressobj(9, DEFLATED, -13, 8, Z_FIXED)` recompresses blinky's two
  CORE frame streams byte for byte (found by the IO work; without `Z_FIXED`
  no zlib setting matches). The encoder's lib and code choice
  (`run_row_target`, `update_with_libs`, `run_combined_target`) isn't
  reimplemented. Whether the SDM accepts an all-literal stream (every code
  0, plain `00ab` nibbles) is the obvious first board test.

#### IO section

`python3 -I tools/io_decode.py decode x.rbf out.pkl` rebuilds the periphery
parts of the image (perictrl headers and data, io96 headers, t40, t41) from
the `IO` section, with a mask of the bits it carries. `verify x.rbf x.sof`
compares them with the `.sof`. `encode x.sof ref.rbf [x.rbf]` goes the other
way: it builds the six design streams from the `.sof` image and checks them,
inflated and deflated, against `x.rbf`. `layout x.rbf` prints the page map.
Specimens: blinky, fz0, a pattern `.sof` (every data word = part tag and word
index), 25 `.sof` files with random headers, and one with every IO part
random (`/tmp/a5p2/io-re/`, not committed).

- **Pages** [obs]: the chain tables (g2) hash 73 pages in blinky. The
  section is, in page order: header, signature block, chain table; the
  `enable_pm.fw` module header; `cssm.fw`; an action page; the three
  perictrl streams; `cssm_reset_release.fw`; `cssm_ux_phy.fw`;
  `io_config.fw`; an action page; the three io96 streams. Each `.fw` comes
  from `devices/programmer/firmware/agilex5e.zip:sm4revb/SHA_384/`, copied
  page for page without its last page (an `MFST` manifest). The module
  header page is hashed (flag 0x80000002) and the module body is not. The
  module bodies hold their own deflate streams, which are the same for every
  design. They are the +0x5000, +0x13000 and +0x25000 streams found
  earlier. Only the six design streams and the two action pages change with
  the design. A design that needs more pages gets more chain tables (flag
  4) inside the t41 stream, as in `CORE`.
- **Deflate** [obs]: each design stream is a run of flag-0 pages holding one
  raw-deflate stream. `zlib.compressobj(9, DEFLATED, -13, 8, Z_FIXED)`
  (level 9, 8 KiB window, fixed Huffman codes) reproduces every one byte for
  byte in all 29 specimens. The same setting also reproduces both `CORE`
  frame streams of blinky, so the "not zlib" note in the CORE subsection
  only applies to the default strategies.
- **Action pages** [obs]: one `0x17` record per stream. Its word 0 is the
  stream's page count, and it lists the send targets as 6-word entries
  with CNOC addresses (perictrl 0x300000 and 0x10300000; io96 0x200000 and
  0xc200000). Nothing else changes between blinky and the all-random
  specimen, except that the t41 record grows by 7 zero words when that
  stream has chain tables inside it [unk].
- **Commands** [obs]: an inflated stream is a list of 8-byte aligned
  commands. A word with low byte 0x0c or 0x0d is a control pair
  (`0x0400000d 0`, `0x3c00000c 0`, `0x00c8000c 0`); what they do is [unk],
  delays or barriers [inf]. Every other command is a send: `u32 target |
  flag << 8 | n << 16`, `u32 0`, n words, zero-padded to 8 bytes (one pad
  word when n is odd). The target is an index into the action record's
  target list. A send is at most 2,048 bytes.
- **Packets** [obs]: `[u32 total][u32 0x20][u32 0][u32 count][4 x u32 0]`,
  then `count` records `[u32 length][u32 index][length - 8 bytes]`.
- **Streams**, in chain order [obs]:

  | # | content | layout |
  |---|---|---|
  | 0 | perictrl0 data | packets `[u32 1][u32 4n]` + `(byte address, byte length, data)` records, each packet followed by `0x0400000d 0 0x3c00000c 0`. A record is cut to fill a packet; a packet closes when fewer than 12 bytes are left |
  | 1 | perictrl2 header (target 0), perictrl1 header (target 1) | one packet per header, with one record; cut into sends that each start with the byte offset they carry; flag 1 on the last send |
  | 2 | perictrl0 header | as stream 1 |
  | 3 | io96_0 and io96_1 headers | one packet per field kind (D, C, B, A) and io96, 10 records each; `0x00c8000c 0` after each |
  | 4 | t41, both io96 | `[u32 address][data]` sends, io96_0 then io96_1 per block; flag 1 on the last block |
  | 5 | t40, both io96 | as stream 4 |

- **perictrl headers** [obs]: the record payload is 8 zero bytes, header
  bytes `[0, len - 24)` in reverse order with each byte bit-reversed, then 2
  zero bytes. The last 24 bytes of each header (perictrl0/2: 878..901,
  perictrl1: 2634..2657) are not sent. This mirrored layout is why the
  first flip experiment looked like "byte x → x + 72": byte x lands at
  937 − x.
- **io96 headers** [obs, bit-signature map over 24 random specimens, every
  bit accounted for]: a header is 10 blocks. Each block starts on a byte
  and holds bit fields A (800 bits), B (1,472), C (1,472 in blocks 0–7, then
  744 and 202) and D (1,401; 1,467 in blocks 1 and 5; none in blocks 8 and
  9). Field k of a kind becomes record k. It is split into 32-bit
  little-endian words, which go out last word first behind zero padding,
  in 192-byte payloads (108 for A). The 58 padding bits between blocks are
  not sent. The 10-block structure suggests 8 byte lanes plus 2 other
  units [inf].
- **t40/t41** [obs]: t40 goes to 0x47b000 + offset. t41 `[0, 1024)` goes to
  address 0 and `[1024, 0x7b400)` to 0x400000 + offset − 1024. The last
  4 KiB, `[0x80400, 0x81400)`, go to 0x47f000. **t41 `[0x7b400, 0x80400)` is
  never sent**: t40 covers 0x47b000..0x47efff in its place. Blocks are 2 KiB
  and are sent even when they're zero.
- **Not carried at all** [obs, pattern specimen]: perictrl1 and perictrl2
  data (2 x 4 MiB). Of perictrl0 data, only a fixed list of 521 byte ranges
  (29,544 bytes, all below 0x210000) is sent. The list is the same for
  every specimen, and blinky has 11 set bytes outside it. Where the list
  comes from is [unk]: it isn't in `sof_meta.pb` or stored plainly in the
  install. For now `encode` reads it from any pfg `.rbf` (`ref.rbf`). Since
  pfg ignores these bits, they don't matter to the device unless another
  section carries them [inf].
- **Verification** [obs]: `verify` finds 0 differing bits on all 29
  specimens. `encode` rebuilds all six streams, inflated and deflated,
  byte for byte on all 29. So the IO design streams can be written from a
  `.sof` image. To assemble a full section, the writer also needs the
  copied firmware pages, the two action pages (copied, with the page counts
  patched), and the chain tables, header and hashes from (g2). That
  assembly isn't written yet.
- **[unk]**: the source of the perictrl0 register list; the control pairs;
  why only the perictrl header and t40/t41 sends set flag 1; what the 24
  dropped perictrl header bytes, the io96 header padding and t41
  `[0x7b400, 0x80400)` hold, if anything; the t41 action record's extra
  words.

### (g2) Phase 2: every `.rbf` integrity value is reproducible (Plan step 1)

`python3 -I tools/bitstream.py check x.rbf` recomputes them all; it passes
on blinky and on pfg's output for edited `.sof` files [obs]. Per design
section (`IO`, `CORE`), in 4 KiB blocks:

| where | what |
|---|---|
| block 0 (+0x000) | section header (TRM table 35); at +0xC0 the SHA-384 of the descriptor block |
| block 0 +0xBB0 | **keyed hash**: HMAC-SHA256 over `[section, +0xBB0)`. The key is a 32-byte constant in `libpgm_bitasm.so` (`BITASM_COMMON::calculate_keyed_hash`); `bitstream.py key` derives it from the install into `specimens/`, and it isn't committed |
| block 1 (+0x1000) | SHA-384 of block 0 at +0; the place a signature goes [inf] |
| block 2 (+0x2000) | descriptor: `u32 1, u32 0x488, u32 count (22 in CORE)`, then per data block `u32 flags/route` + SHA-384 of the 4 KiB block |
| end of blocks 0 and 1 | CRC-32/BZIP2, see (f) |

- The "IO hash" (`IO` +0x348, `CORE` +0x234) and "Design hash" (`CORE`
  +0x1E0) are **not computed by pfg**: they come from the assembler's
  `.sof`, the IO hash as a hex string in the OPTIONS record (field 1.20)
  and the design hash in record 0x36 field 2.10 [obs]. The IO hash in
  `CORE` equalling the one in `IO` presumably ties a core image to its
  periphery image [inf]. Whether the SDM checks either against the data
  is [unk], and needs the board.
- So an open writer can produce every hash and CRC itself. Only the
  descriptor's per-entry flag word (0, 1, 0x80000002, …) is [unk] as to
  meaning; it can be copied by position for now.

### (h) Phase 2: Quartus's own configuration model is scriptable

These remove most of the need for fit-based fuzzing. Commands and argument
forms were recovered from the binaries, since `-help` prints nothing.

- **`asm2`, the assembler's Tcl package** [obs]. Run `quartus_asm -t
  script.tcl` (not `quartus_sh`: under `quartus_sh`, `load_devices`
  fails), then `load_package asm2; load_devices`. No licence is needed.
  - `open_sof f.sof` returns a handle; `write_sof $h out.sof` writes it
    back in about 1.2 s. An unmodified round trip has a byte-identical
    image.
  - `get_block_types sm4revb` lists 26 block types.
  - `get_uids sm4revb <type>` lists instances, 11,549 in all: 2,392
    `cw_la_lab`, 2,392 `cw_ml_mlab`, 366 `cw_mm_m20k`, 192 `cw_dssm`,
    4,992 `cw_rt_lab_base`, 768 `cw_rt_m20k_dsp_base`, 104 + 104
    `cw_rt_vio_{lab,mlab}_base`, 96 `cw_rt_hiol_base`, 5 `hviowr`,
    2 `io96`, and clock blocks (`cw_cc_*`). Instance names are
    `<type>_r<row>_c<col>`.
  - Each block type has a fixed list of named configuration bits
    (`get_bit_names sm4revb <type>`; 1,165 for `cw_la_lab`) and attributes
    (`get_attribute_names`; 1,085), with `get_attribute_values` and
    `get_bit_names_for_attribute`. A LAB is 10 ALMs
    (`x{top,bot}half/xale0..4`); `lut_mask` is `rb1lut[]` there.
  - `get_bit $h <uid> <bit>` and `set_bit $h <uid> <bit> <0|1>` read and
    write single named bits, and `get_attribute`/`set_attribute` do the
    same for attributes. So **set a named bit, `write_sof`, diff the
    image** maps it to a CRAM address with no fitter run. Example:
    `cw_la_lab_r50_c62 xtophalf/xale0/xcw_la_le_lut/rb1lut[0]` is sector 1,
    frame 4605, byte 14, bit 7.
  - `get_max_cram_badd 0` / `get_max_cram_bdata 0` = 4704 / 4320, which
    matches the `.sof` sector geometry.
  - `get_cram_bitname_by_address sm4revb 0 <0|1> <a> <b>` names a CRAM bit,
    e.g. `0 0 7 2208` is `xtophalf/xale4/xcw_ml_le_lut/rb1lut[16]`. But
    `<a>`/`<b>` are not plain (frame, bit): frame 4605 fails `data <
    data_width`. Its axes are [unk].
  - `set_bit_routing`/`get_bit_routing $h <re_uid> <signal>` set routing
    by routing-element id. `dump_re_network` needs an open project
    (`hdb_mgr`). Both are untested [unk].
- **`Bcm_device`, the block configuration model** [obs]. `load
  quartus/linux64/libddb_dbcm_tcl.so Bcm_device` in `quartus_sh`, then
  `read_bcm_device -device sm4revb; create_bcm_chip`. This is the
  decrypted content of `bcm2bits_db`, one level up: block intents
  (`cw_la_lab` → `cw_la_halflab` → `cw_la_ale` → `cw_la_le_lut`, …),
  attributes with type and legal settings (`lut_mask` DIRECT, size 64;
  `hipi_clken` CUSTOM {TIE_OFF REG_PATH COMB_PATH}), and setting → named
  bit values (`get_setting_bit_values -bcm ce_la_lcb_adder -attribute
  hipi_clken -setting REG_PATH` = `0 1` for bits `rclken rntot`). Routing
  tiles (`cw_rt_lab_base`) carry mux attributes such as
  `ifdim_col/xfdim_ale0/xfdim_0/sel` ∈ {MUXIN0_ … MUXIN7_, STITCH,
  MUX_DIS}. Which wire drives each MUXIN is the routing graph, still [unk].
- **Floorplan** [obs]: `dump_uid_floorplan -device sm4revb -output_file f`
  (Bcm_device) gives every instance its fitter coordinates, saved as
  [`db/sm4revb/floorplan.tsv`](db/sm4revb/floorplan.tsv) (11,408
  instances). `<type>_r<y>_c<x>` is X/Y in fitter terms: `cw_la_lab_r48_c120`
  is `LAB_X120_Y48`, where `designs/fz` put its LUTs. There are 52 LAB
  columns, each with an MLAB column at X+1, and 2,340 non-spare LABs and
  MLABs.
- **Database files** (committed, derived from the dumps below):
  `db/sm4revb/bcm_types.json` (671 BCM types: children by instance name,
  CUSTOM attribute names, DIRECT attribute widths) and
  `db/sm4revb/bcm_settings.tsv` (21,346 `intent, attribute, setting, bits,
  values` rows: every CUSTOM setting that drives bits).
- **Every CUSTOM attribute's setting → bit values** [obs]:
  `tools/bcm/settings.tcl` walks all 670 BCM intents and writes 196,315
  `(intent, attribute, setting, bits, values)` rows. Routing and clock
  muxes are one-hot plus an enable bit; for example `ce_rt_edim_clkmux sel
  = CLK_IN3_` is `cram_off_hg=1, cram_hg[3]=1`, other `cram_hg` 0, and
  `MUX_DIS` is all zero. DIRECT attributes (`lut_mask`, memory-controller
  fields) are plain bit vectors.
- **Named bit → CRAM address maps, from Quartus's own assembler** [obs].
  `tools/bitmap/run.sh <base.sof> <outdir> <type…>` runs
  `bitmap.tcl` under `quartus_asm`. It gives every (instance, bit) pair an
  id, writes 2 × ⌈log2 ids⌉ `.sof` files, each flipping the pairs whose id
  has bit k set (or clear), and `bitmap.py` reads every changed image bit's
  id back. A CRAM bit hit by more than one pair is reported as a clash.
  `pack.py` stores one relative layout plus a per-instance anchor for each
  image part the instance touches in
  [`db/sm4revb/bits/`](db/sm4revb/bits/) (format:
  [docs/db-format.md](docs/db-format.md)). **Every block type has
  exactly one layout.

  | type | instances | bits/instance | mapped | layouts | where |
  |---|---|---|---|---|---|
  | `cw_la_lab` | 2,340 (spare rows skipped) | 1,165 | 1,140 (2,667,600 pairs; 7,020 clash bits) | **1** | CRAM, 94 frames x 16 bdata |
  | `cw_ml_mlab` | 2,340 | 1,206 | 1,148 (2,686,320 pairs; 14,040 clash bits) | **1** | CRAM, 94 x 16, same row frames as LABs |
  | `hviowr` | 5 | 5,927 | 4,748 | 1 | `perictrl{0,1,2}` object headers |
  | `cw_rt_lab_base` (routing tile) | 4,888 | 4,398 | 4,394, by extrapolation from rows 5, 6, 7, 27, 28, 50, 51 (728 instances, 3,201,744 pairs) | **1** | CRAM, 94 x 48 |
  | clock network (`cw_cc_*`, 13 types), `cw_vr_leaf_ctrl_c_bf` | 2–84 each | 25–480 | all | 1–4 per type | CRAM |
  | `cw_rt_vio_{lab,mlab}_base` | 104 each | 142 | 142 | 1 | CRAM |
  | `cw_rt_m20k_dsp_base` (M20K/DSP routing) | 752 | 5,831 | all, extrapolated from rows 5, 6, 27, 51 | 1 | CRAM, 94 x 64 |
  | `cw_dssm` (DSP) | 188 | 521 | all | 1 | CRAM |
  | `sm_hssi_top` | 1 | 186,662 | all | 1 | `perictrl0` data region |
  | `cw_mm_m20k` (M20K) | 358 | 20,588 | all, extrapolated from rows 5, 6, 27, 43, 51 | 1 | configuration in CRAM, 20,480 content bits in the sector's `ram` part |
  | `io96` | 2 | 4,312,033 | 4,303,895 (`pio_0_0` brute-forced; `pio_1_0` placed by probe and checked on 2,000 random bits, 0 wrong) | 1 | `io96_*` `hdr`/`t40`/`t41` (PHY, hard memory controller, IOSSM memory init) |

  The LAB anchors decompose exactly [obs]: frame = 200 + 98·(Y − 6),
  so one LAB row per 98-frame group (`sof_meta`'s `_frames` unit), and
  bdata depends only on X, at 64 bdata per column. Sector 0 holds X ≤ 59 with
  bdata growing toward X = 1 (4176); sector 1 holds X ≥ 62 mirrored (X =
  62 → 112, X = 120 → 4176). The two sectors are mirror images about the
  centre seam, but the LAB's internal layout is the same in both.
- **Extrapolation, validated** [obs]: brute force costs about 1 s per
  million `set_bit` calls per image, so a 21.5 M-pair type (`cw_rt_lab_base`)
  would take 5 h. `extrapolate.py` takes the layouts from a fully mapped
  subset of rows. A probe maps 4 reference bits for every instance
  (`bitmap.tcl … - bits.txt`, under 3 min). Each instance gets the layout
  whose reference offsets match after a single shift. Checked on LABs: 3
  rows (156 instances) plus the probe predict **all 2,667,600 LAB bit
  positions of the full map exactly, 0 wrong**.
- **Routing tiles** [obs]: the extrapolated `cw_rt_lab_base` map predicts
  **all 659,100 bit positions of 150 random tiles outside the subset rows**
  (brute-forced independently), 0 wrong. Tile row Y = 5 sits in 98-frame
  group 0 (frame 4) and Y = 6..51 in groups 2..47 (frame 200 + 98·(Y − 6),
  as for LABs). Group 1 holds no routing tiles, consistent with pfg never
  storing it. With LAB, MLAB and routing tiles, **99.87 % of the set CRAM
  bits in the fz0 image are named bits** (1,547,304 of 1,549,318 in
  sector 0). The remaining 38–44 set bits per row group are [unk] until the
  M20K/DSP types are mapped.
- **Aliased bits** [obs]: some named bits share one image bit with others
  (MLAB `r_nram`, `r_n32mode`, `rmdl_init`, all `remulation` bits), so the
  binary code drops them as clashes. `single.tcl`/`single.py` flip them one
  at a time on one instance, and `aliases.py` adds them to the layout,
  checked on a second instance in the other sector. MLAB 58 → 6 physical
  bits, LAB 25, routing tiles 4, `cw_cc_ts_base*` 18 each: all at the
  predicted offsets on the second instance.
- **Coverage** [obs]: `tools/bitmap/coverage.py` marks every named bit
  position in the image. On fz0, **every set bit in both sectors' CRAM and
  headers is a named bit** (1,549,318 + 1,549,288). On blinky 4 set bits
  were left, all MLAB RAM-mode aliases that are now mapped.
- **Cross-checked against the fitter** [obs]: the 256 CRAM bits that the
  ECO runs changed (fz's four LUT masks, all-0 against all-1) are, by the
  asm2 map, exactly `cw_la_lab_r48_c120` `xtophalf/xale{0..3}/xcw_la_le_lut`
  `rb{0,1}lut[*]`, polarity "equal". So `rb*lut` holds the inverted LUT mask,
  and the fitter's `LABCELL_X120_Y48_N0/6/12/18` are ALMs 0–3 of the top
  half.
- **LUT mask bit order, through the fitter** [obs, ECO `-mask` convention]: six more ECO runs gave
  all four LUTs mask `Σ{2^i : bit k of i}` (k = 0..5). Each changed CRAM
  bit's on/off pattern over the runs is its mask index. In all four ALMs,
  **`lut_mask[i]` is `rb{i >> 5}lut[31 − (i & 31)]`, stored inverted**. So
  index 0 is `rb0lut[31]`, 31 is `rb0lut[0]`, 32 is `rb1lut[31]` and 63 is
  `rb1lut[0]`. asm2's `get_bit_names_for_attribute … lut_mask` lists
  `rb1lut[31..0], rb0lut[31..0]`, which is mask bits 32..63 then 0..31 in
  that order. The fitter could permute LUT inputs, but these four
  identically connected LUTs all came out with the same identity-like map
  [inf: no permutation here].
- **Attribute decode from the database alone agrees with Quartus's reader**
  [obs]. `tools/bitmap/attrs.py` reads an instance's named bits from a
  `.sof` through `db/sm4revb/bits` and picks the matching setting from the
  BCM settings dump. For `cw_la_lab_r48_c120` (used by fz) and
  `cw_la_lab_r10_c1` (unused) in fz0, **all 315 decodable CUSTOM attributes
  equal asm2's `get_attribute`**. Another 83 are `<VIRTUAL>` in Quartus, and
  25 DFT/emulation attributes can't be decoded because their bits are among
  the 25 unmapped ones. Asm2's DIRECT `lut_mask` is the raw bits,
  `lut_mask[j] = rb{j>>5}lut[j & 31]` (10/10 ALMs). So the WYSIWYG/ECO mask
  is `W[i] = ¬BCM[i XOR 31]`: inputs A–E are inverted in silicon, and the
  stored value too [inf]. fz0's original LUTs don't decode to their
  WYSIWYG masks (fitter input permutation [inf]); the ECO `-mask` values
  land as given.
  On blinky, a different fit, the database finds exactly the instances the
  design uses: `cw_la_lab_r48_c109` plus MLABs `r47_c108` and `r48_c108`,
  the only ones differing from the unused state. Decoded attributes equal
  Quartus's on all 315 decodable for the LAB and all 318 for MLAB
  `r47_c108` [obs]. The third instance's query hit an asm2 internal error
  (`asm2_state.cpp:347`), so it is unchecked.
- **`tools/a5db` (Rust): read and write attributes without Quartus**
  [obs]. `a5db attrs|bits <db> <sof> <uid>` decodes; `a5db set <db> in.sof
  out.sof <hmac.key> <uid> attr=value…` writes CUSTOM settings and DIRECT
  bit strings (`db/sm4revb/direct.tsv`; LAB/MLAB `lut_mask` so far) and
  re-signs the `.sof`.
  - Decode: on fz0's `cw_la_lab_r48_c120`, 325 attributes equal asm2's
    `get_attribute` (315 CUSTOM, 10 `lut_mask`) and none differ, in 0.8 s.
  - Encode: setting a 64-bit `lut_mask` plus `register_comb=REGIN`,
    `reg_input=LUTOUT_6LUT`, `ce_sel=CE` and `hipi_clken=REG_PATH` on
    `cw_la_lab_r10_c1` gives **an image byte-identical to the one asm2's own
    `set_attribute` + `write_sof` produces** (13 bytes changed in both).
    asm2 reads all five values back, and pfg turns it into an `.rbf` that
    `bitstream.py check` passes.
- **Quartus Pro ECO works for fuzzing** [obs]. `quartus_fit <rev> --eco
  eco.tcl` loads the post-fit snapshot (about 4 min) and runs ECO commands
  (`modify_lutmask`, `place_node`, `make_connection`,
  `get_node_location`). It commits with "Successfully committed final
  database", then `quartus_asm` runs (`tools/eco.sh`). On `designs/fz`
  (HVIO pins only: fit 20 min, compared with 74 min for blinky with its
  IO96 pin), the four WYSIWYG LUTs sit at
  `LABCELL_X120_Y48_N0/N6/N12/N18`. Their all-zero and all-one masks
  differ in exactly 256 CRAM bits (sector 1, frames 4374–4409, bdata
  4176–4183), and **LUT mask bits are stored inverted** (a mask of all
  ones gives all zeros) [obs].

### (i) Phase 2: routing graph

`tools/rgdump` gets sm4revb's routing graph from Quartus's own API, so there
is no udm decoder to write. `librgdump.so` (C++, built against the install
by `build.sh`) is loaded into `quartus_fit -t` with Tcl `load`. It walks the
graph that DYGR builds from `sm4revb.udm` and asks the assembler's edge API
how each edge is configured.

- **Entry point** [obs]: `load_package advanced_device; load_device -part
  A5EB013BB23BE4SCS; load_die_info` loads the route info. No project or
  licence is needed, and it runs under `quartus_fit` only (`quartus_sh` lacks
  the package). The shim then calls exported C++ functions:
  - `DMGR_UTILITY::get_current_device()->get_die_info()` gives the
    `DYGR_DIE_INFO`.
  - `DYGR_ROUTE_INFO::gids()` iterates every routing node. Router gids are
    `0x80000000 + n` (`DYGR_DIE_INFO_BODY::is_router_id`), and gids below
    that are placement sites. The iterator is a pimpl whose vtable slots
    are `[2]` deref, `[3]` `operator!=`, `[4]` `++` and `[6]` `set_to_end`.
  - `get_fanout`/`get_fanin` return edges, and
    `get_element_string_name` returns the node name (`H0:X120Y48Z0I27`).
  - `DYGR_QDL::EMULATOR::obtain(dev)->get_edge_api(src, dst)` gives the
    block uid (`get_uid`) and `get_mux_config` gives the mux instance and
    input, e.g. `cw_rt_lab_base_r48_c120`, `ileim_col/xleim_ale0/xleim_4`,
    `muxin[6]`. `CONFIG_DATA` starts with `{u64, std::string mux,
    std::string input}`; the rest is [unk]. The uid is `NULL` until
    `Bcm_device`'s `read_bcm_device -device sm4revb; create_bcm_chip` has
    run in the same process.
- **Size** [obs]: **7,616,798 nodes and 26,327,684 edges**, in 1,066
  element types. The dump takes 13 min (2 min without the edge
  configurations) and two dumps are byte-identical. 22,718,018 edges carry
  a mux setting: 12,849 distinct (mux, input) pairs over 6,186 blocks.
  - Every edge into a fabric wire carries a setting: H0–H12, V1–V8, LL,
    LEIM and LEIM_WB, 21.2 M edges.
  - 3,609,666 edges have no edge API: all 568,516 LEIM → LAB_RE pin edges
    (hardwired, single fanin) and the internals of DFT, HSSI, HVIO and IO
    blocks.
  - Most nodes are not fabric routing: DFT scan chains, IJTAG, HSSI and
    PCIe internals, IO96 (types `SM_HSSI_*`, `IJTAG_*`, `HVIOWR_*`, …).
- **Against the fitter's resource count** [obs]: 33 of the 34 classes in
  fz0's "Routing Usage Summary" have a DYGR type of the same count:
  - R0/R1/R2/R4/R6/R12 = H0/H1/H2/H4/H6/H12 (401,380 / 229,360 / 115,620 /
    117,500 / 118,440 / 34,404).
  - C1/C4/C8 = V1/V4/V8 (234,240 / 229,360 / 22,936).
  - Block input muxes = LEIM (500,456), block input mux wrapbacks = LEIM_WB
    (43,616), block interconnects = LL (526,400).
  - HIO/VIO buffers = HIOB/VIOB, programmable inverts = RPI/RPII (57,078),
    redundancy muxes = REDMUX, row clock tap-offs = ROWCLK_TAPOFF, and the
    small `DCM_mux`, `DELAY_CHAIN`, `Switchbox_clock_mux`,
    `Vertical_seam_tap_mux`, `IO75XDDRFMXCKTREE_*` and `IOFBRADAPT_*_BUF_*`
    classes.

  These 33 add up to 2,766,802. The remaining 526,400 is exactly "Direct
  links", which has no DYGR type of its own (LL already accounts for
  block interconnects).
- **Names** [obs]: the fitter's `R0_X120_Y48_N0_I27` is DYGR's
  `H0:X120Y48Z0I27`. Rn = Hn, Cn = Vn, `LOCAL_INTERCONNECT` = LL,
  `BLOCK_INPUT_MUX_PASSTHROUGH` = LEIM, and `LAB_RE` is the same in both.
- **Against Quartus's own route** [obs]: `report_routing -to l[k].lut
  -to_port DATAA..DATAF` (ports are upper case) in `quartus_fit fz --eco`
  on a copy of fz0 prints the six LUT input routes of `l[0]`. All 22 hops
  are edges of the graph (`rgcheck.py`). Each of the 16 configured hops
  predicts `<uid> <mux>/sel`, and fz0's `.sof` has exactly that value in
  all 16 (asm2 `get_attribute`, `rtattrs.tcl`), for example
  `ibdim_col/xbdim_ale3/xbdim_8/sel = MUXIN3_` for `LAB_RE I103 → R0 I27`.
  Of the 68 `sel` attributes the `.sof` sets in `cw_rt_lab_base_r48_c120`
  (not `MUX_DIS`), 60 are the setting of some graph edge (`rgcover.py`).
  The other 8 are clock gates tied to `VCC` and two `REGSCAN` reset
  selections, which are not edge choices [inf].
- **Setting rule** [obs on 60 settings in one tile, inf in general]:
  input `name[k]` sets `sel = NAMEk_` (`muxin[6]` → `MUXIN6_`, `clk_in[2]`
  → `CLK_IN2_`), and `stitch` sets `STITCH`. **The fanin order is not the
  MUXIN index**: `LEIM I4`'s second fanin `LL I1` is `muxin[6]`. So the
  edge API is needed and the fanin position can't stand in for it. 768
  configurations name sub-muxes too (`…;s2 clkin[0];s1 clk[0]`, clock
  muxes); how they map to attributes is not checked.

Regenerate (outputs go to `specimens/rg/`, which is git-ignored):

```
tools/rgdump/rgdump.sh                    # -> specimens/rg/full: nodes.tsv, edges.bin, uids.txt, configs.txt, types.tsv
# the fz0 routes: copy specimens/runs/fz0, then quartus_fit fz --eco with
#   report_routing -to {l[0].lut} -to_port DATAA   (route printed as Info lines)
quartus_asm -t tools/rgdump/rtattrs.tcl fz.sof cw_rt_lab_base_r48_c120 > attrs.txt
python3 tools/rgdump/rgcheck.py specimens/rg/full eco.log attrs.txt
python3 tools/rgdump/rgcover.py specimens/rg/full attrs.txt cw_rt_lab_base_r48_c120
```

`edges.bin` holds, per node, `u32 src, u32 n` and then `n` times `u32 dst,
u32 uid id, u32 config id`, with gids minus `0x80000000`. Line k of
`uids.txt`/`configs.txt` is id k, and id 0 means none.

[unk]:
- Which resource "Direct links" (526,400) is.
- Edge delays and timing (`DYGR_ROUTE_TIMING_INFO`, not dumped).
- The meaning of `CONFIG_DATA`'s first word and tail.
- The attribute mapping of sub-mux configurations.
- Only one tile and one design were checked against a bitstream; checking
  V/H wires that cross tiles and other tile types (`cw_rt_m20k_dsp_base`,
  `cw_rt_hiol_base`, `cw_rt_vio_*`) needs a design that routes through them.

### (j) Phase 3: big tables stored compressed

Tables bigger than 4 MiB in `db/` are `xz -9` files (`<name>.tsv.xz`):
`io96.layout.tsv` (318 MB) is 2.7 MB and `sm_hssi_top.layout.tsv` (45 MB)
is 0.9 MB [obs]. `a5db` (`db::read_table`, pure-Rust `lzma-rust2`) and the
Python tools (`tools/bitmap/table.py`) read `.tsv` or `.tsv.xz`; `pack.py`
and `aliases.py` write whichever the size calls for, and Python's `lzma`
preset 9 writes the same bytes as `xz -9`. Every `a5db attrs`/`bits` output
on fz0 (io96 `pio_0_0`, `hssi_mono_0_0`, `hvio_3_1`, a LAB) and
`coverage.py`'s output are byte-identical before and after [obs]. The old
plain files stay in history.

### (k) Phase 3: routing graph joined to the bit maps (Plan step 3 → 4)

`a5db` joins every graph edge's mux setting to the named bits it writes
(`tools/a5db/src/graph.rs`, `route.rs`), so a routed net, as a list of
graph edges, becomes CRAM bits:

- `a5db join <db> <graph>` resolves all 22.7 M configured edges and prints
  why the rest have no bits;
- `a5db route <db> <graph> in.sof out.sof <hmac.key> <routes>` writes a
  set of routes into a `.sof`;
- `a5db routed <db> <graph> in.sof` reads routing back: every edge whose
  setting the image holds;
- `a5db routecheck <db> <graph> in.sof <routes> [out.sof key]` puts every
  routing mux of `in.sof` in its idle state, writes only the routes' bits
  and compares the result with `in.sof`.

`<graph>` is a `tools/rgdump` directory (git-ignored, about 13 min to
regenerate). Routes come from `tools/rgdump/routes.tcl`, run in
`quartus_fit <rev> --eco` on a copy of a compiled run: `report_routing` for
every connected input of every atom (`report_connections` crashes the
fitter, so the ports come from `read_atom_netlist`).

- **The join rule**: an edge's `mux input[k]` selects the setting of
  `<mux>/sel` that holds `INPUTk` between `_` boundaries: `muxin[6]` →
  `MUXIN6_`, `clk_in[0]` → `CLK_IN0_`, `stitch` → `STITCH` [obs, every
  configured edge of the two designs below]. A configuration with stages
  (`mux in;stage in…`) sets `<mux>/<stage>/sel` per stage, and the top
  `sel` drives no bits. The vertical-seam form `…clk_32_l_0… clkin1;xclkmux21
  clk1` is used by rt0's clock and checks out [obs]. The 768 DCM forms
  `mux clk[0];s2 clkin[0];s1 clk[0]` would set `<mux>/s2/sel = CLKIN0_` and
  `<mux>/s1/sel = CLK24_CLK16_CLK8_CLK0_` (the stage-1 setting whose name
  contains `CLK0`), but neither design uses one [inf]. A programmable
  inverter's `rpi_in` is `NON_INV` [inf].
- **Every configured edge is classified** [obs, `a5db join`]: 19,746,758 of
  the 22,718,018 resolve to bits (23,318,498 distinct routing bits). The
  rest:
  - pass-throughs with nothing to set: 18,664 through plain buffers with
    no BCM model (`ilaboutb_buf`, `i_mux`), 48,356 through inverters and
    delay chains whose leaf has no `sel` (`*_rpi`, `*_delay_chain`), and
    1,565,476 into `ce_rt_xleim_inv`, whose `sel` drives no bits;
  - 1,316,752 hyper-register clock inputs (`clk`, `clk_delayed`, `nclk`),
    which `flop_mode` sets, not `sel`;
  - 22,012 into blocks without a bit map: redundancy tiles
    (`cw_rd_redtl_bf`, `cw_cc_vseam_*_redt_bf`, 17,132) and spare-row
    tiles (`*_rsp*`, 4,880) [unk].
- **Clock muxes behind edges with no edge API** [obs]: some clock-network
  edges carry no configuration in the graph but do select a mux input, so
  the join adds three rules from the node names (`graph::clock_rule`):
  - `CE_CC_CLKMUX51_INPUT … In → OUTPUT` (the spine tap's 5:1 mux) sets
    `cw_cc_tap_cs_bf` `cell_k/mux51_dolsgn_cs_dolsgn_0/sel = CLKINn`, where
    k is the `SPINE_TAP_RE` index the cell drives;
  - `SCLKMUX_RE` input k (k < 32) → row clock j's mux node `44 + 7j` sets
    `xrowclkmux/xrmx{l,r}{j mod 6}/rmux32to1_dolsgn_rc_dolsgn_0` `s1` to the
    setting holding `CLKINk` and `s2 = CLKIN(k/8)_` (row clocks 0–5 are
    `l`, 6–11 `r`, by node index, not by side);
  - row clock j's mux node → its predrive node `50 + 7j` sets
    `mux_predrive_{l,r}{j mod 6}` to `CLKIN1_` (input 0 is the delay
    chain).

  Check: every clock-block attribute Quartus set in fz0 and rt0 was put back
  to its idle value, the designs' routes re-applied with `a5db route`, and
  all of them came back equal to Quartus's: the switchboxes, seam taps,
  spine taps and row muxes of both designs. One DCM periphery mux,
  `cw_cc_dcm_hssi_p2c_bf` `mp2c_mux_bot/mux11to8/clock_int0 = CLKIN2`
  (`CE_CC_CLKMUX41_LOW_INPUT Z111 I2`), has no rule yet: one instance can't
  fix the order of its 32 muxes [unk] (phase 4: a rule from the graph, (n)). Designs clocked from an HVIO pin
  pass through it.
- **The idle state of a routing mux is `VCC` where it has that setting,
  else `MUX_DIS`** [obs]: Quartus ties every unused HIOL clock mux
  (`iclkmux*`, 16 per tile, 94 tiles) to `VCC`; all others idle at 0.
- **Checked against Quartus-routed designs** [obs]. fz0 (four LUTs, a shift
  register) and `designs/rt` (a 32-bit LFSR and a CRC-32 in two place
  regions at X8–14 Y6–10 and X104–114 Y42–48, so nets cross both sectors
  on H0–H12 and V1–V8 wires; clock from `CLOCK1_50` through the DCM,
  switchbox, seam, spine and row-clock muxes):

  | | fz0 | rt0 |
  |---|---|---|
  | routed connections (`report_routing`) | 48 | 723 |
  | hops, all of them graph edges | 1,710 | 17,528 |
  | edges written by the join (with the clock rules below) | 67 | 3,157 |
  | routing bits Quartus set | 1,617 | 6,334 |
  | of them, set by the join from the routes | 1,609 | 6,229 |
  | set by the join but not by Quartus | 0 | 0 |
  | left: LL → LEIM edges into FF data inputs | 8 | 105 |

  The bits left over are exactly the decoded settings of LL → LEIM edges
  into `xleim_1`/`xleim_6`, the FF data-input muxes. Those connections (FF
  `D` fed by a LUT) are the ones `report_routing` crashes the fitter on, so
  the dump skips them. `a5db routed` on the Quartus image finds 75 and
  3,262 active edges: exactly the edges the routes write plus those.
- **"Direct links" resolved** [obs]: a direct link is an LL node driven
  straight from a LAB_RE (a block output fed back into a block's local
  interconnect), one per LL node, hence 526,400. rt0's fit report counts 548
  block interconnects and 67 direct links; its routes use 548 distinct LL
  nodes, 67 of them reached straight from a LAB_RE.
- Fitter names map to DYGR names as before, and the fitter drops DYGR's
  underscores in periphery types (`HVIOWRX1RGMIIWRCONNRE` is
  `HVIOWR_X1_RGMII_WR_CONN_RE`); `HIO_BUFFER` is `HIOB` and
  `BLOCK_INPUT_MUX_PASSTHROUGH_WB` is `LEIM_WB`.

Reproduce (graph and routes are Quartus output, so git-ignored):

```
tools/rgdump/rgdump.sh                                 # specimens/rg/full
tools/flow.sh designs/rt specimens/runs/rt0 --wait     # ~25 min
cp -r specimens/runs/rt0 /tmp/rt0r && cd /tmp/rt0r && tools/quartus.sh quartus_fit rt --eco <repo>/tools/rgdump/routes.tcl
a5db routecheck db/sm4revb specimens/rg/full specimens/runs/rt0/output_files/rt.sof /tmp/rt0r/routes.txt
```

### (l) Phase 3: the `.rbf` writer (Plan step 6)

`a5db rbf <db> <in.sof> <out.rbf> <quartus root> <keyed-hash.key>` writes the
whole `.rbf` from a `.sof` image with no Quartus tool in the loop
(`tools/a5db/src/rbf/`). It reads three things from the user's own install
and commits none of them: the signed firmware (`main.cmf` and the `.fw`
modules from `devices/programmer/firmware/agilex5e.zip`), two codebooks from
`devices/programmer/sm4revb_npcf.ddb`, and the CRAM `s4` group list from
`sof_meta.pb`. The key is `tools/bitstream.py key`'s.

- **Verified** [obs] on all 60 `.sof`/`.rbf` pairs from phases 2 and 3
  (blinky, fz0, the random-header, random-IO, pattern, random-CRAM and
  bit-flip specimens):
  - the `CMF` section is byte-identical to pfg's;
  - the `IO` section is byte-identical to pfg's, including the specimens
    whose t41 stream needs four chain tables and the one with random
    perictrl0 data;
  - `bitstream.py check` passes, and the `CORE` header's per-design fields
    (+0x78 sum, design hash, IO hash) equal pfg's;
  - `quartus_pfg --check_integrity` says `Integrity status: OK` for the
    fz0 `.rbf`; with one `CORE` bit flipped it fails with `Section hash
    mismatch, expect 0x992B32EA`, which is the +0x3C word below.
  - The all-literal `CORE` decodes (`core_decode.py verify`) to exactly the
    `.sof` CRAM: 0 frames differ on fz0, blinky, the random-CRAM specimen and
    v_dense. Every design-independent `CORE` page (both codebooks, the
    482,784-byte block, the BBM header, the second action page, the
    sector-header stream) is byte-identical to pfg's.
- **`CORE` is byte-identical since phase 4** [obs]: phase 3 stored every
  column literally with one zero lib per IP id (fz0's `CORE` was 248 KiB
  against pfg's 112 KiB). a5db now reproduces pfg's S4 flags, lib and BBM
  code choice and page interleave, so the whole `.rbf` equals pfg's (o).

**The S4 flag rule** (phase 2 [unk], solved in phase 4, see (o)):

- The lists live in `devices/programmer/fm_sector_bnxplv_rif_t{0,1,2}.ddb`
  (the sector type is shared with Agilex 7). Each holds 1,568 frame slots
  of `S4_SII` → `S4_SII_BIT` (u32 key) → one `S4_SI` (u32 frame', u32
  bit', u8 flag, u8 value), 217,520, 262,400 and 233,920 entries. File t
  covers image frames 1,568·t … 1,568·t + 1,567, frame' is always the slot
  and bit' always the key, so an entry's condition always holds [obs].
- The slot is the image frame itself, and the keys are bits of a swizzled
  frame, not dr bits. That is why phase 3's frame and row-offset searches
  failed: they matched keys against the dr bit `a`. The full rule and its
  check are in (o) [obs].
- `BITASM_CRAM_REPAIRER::get_4bit_code` is not on pfg's path for this part
  (it never runs during `quartus_pfg -c`, by breakpoint) [obs].

Section format, now complete [obs, every item checked on the 60 pairs]:

- **Header block** (+0): constants (the same in every specimen; the writer
  carries them as offset/hex runs) plus:
  - +0x08 the section size;
  - **+0x3C: the first 4 bytes of SHA-256 over [+0x2000, end)**, which pfg's
    integrity check calls the "section hash". Flipping one `CORE` bit
    changes only `CORE`'s, one IO bit only `IO`'s;
  - **+0x78: the byte sum (u32) of every object header plus both sectors'
    CRAM** of the image. One flipped CRAM bit moved it by exactly 1 in both
    sections; t40/t41 and perictrl data are not included. +0x7C and +0x80
    hold a constant `0xc362c0dd`, the JTAG IDCODE (n);
  - +0xC0 SHA-384 of the first chain table, the design and IO hashes
    (g2), the keyed hash at +0xBB0, and **+0xBD0: SHA-384 over
    [+0x2000, end)**; then the CRC at +0xFFC.
- **Block 1**: SHA-384 of block 0, and **at +0xF00 the number of sections
  that follow**: 2 in `CMF` (the word (f) saw patched from 0 to 2), 1 in
  `IO`, 0 in `CORE`. It is 2 in blinky, fz0 and all other specimens, so the
  CMF patch only depends on the section count [obs].
- **Chain tables**: `u32 1, u32 16 + 52n, u32 n, u32 0`, then n entries of
  SHA-384 and flag. A table holds at most 78 entries. When more pages
  remain, entry 57 is the flag-4 link to the next table, whose page sits at
  that point in the file, and 20 more entries follow it; the last table
  takes the rest.
- **Pages**: a firmware module is its pages without the trailing `MFST`
  page, the first hashed with flag 0x80000002. A stream is its deflate
  output cut into zero-padded pages. `IO`: `enable_pm.fw`, `cssm.fw`, action
  page, streams 0–2, `cssm_reset_release.fw`, `cssm_ux_phy.fw`,
  `io_config.fw`, action page, streams 3–5. `CORE`: `lsm_init.fw`, action
  page, both sectors' lib pages (sector 0 then 1), then the two sectors'
  remaining pages alternating (codebook, block, codebook, BBM, frames; the
  longer one runs on alone), `unfreeze_routing_fw.fw`, action page, the
  sector-header stream, `enter_user_mode_fw.fw`.
- **Action pages**: TLV records that are the same in every specimen except
  that a **0x17 record (one per stream) has the stream's page count (link
  pages excluded) in word 0 and ends in a page bitmap**, one bit per page,
  set for sector-1 pages in `CORE` and zero in `IO`. That is the "7 extra
  words" phase 2 saw in t41's record: ⌈pages / 32⌉ − 1 words beyond the
  first. `CORE`'s `0x1f` records hold the lib offsets and `0x15 (n, s, 4)`
  the lib page count.
- **The perictrl0 register list** (phase 2 [unk]): every 32-bit word that
  holds an `sm_hssi_top` named bit, except 8 control words: the `ux_quad`
  clock-mux configuration at 0xA1000 and seven `ss_ctrl` CSR words at
  0x20F000, 0x20F004, 0x20F034, 0x20F114, 0x20F120, 0x20F124 and 0x20F128.
  That reproduces pfg's 521 ranges exactly. Why those 8 are skipped is
  [inf]: the firmware owns them.
- **The codebooks** are NPCF parameters (`FBGEN_GEN::copy_altera_codebook`
  reads `GZIP_ALTERA_LITERALS` through `FBGEN_BLOCK_CONFIG::get_param_values`).
  `sm4revb_npcf.ddb` stores them sparsely as 12-byte `(value, offset, 0)`
  records; it holds three 256-entry tables, and pfg's first codebook page
  is the second table, its second page the first, one u64 per entry.
- **The 482,784-byte block** after the first codebook is generated: 47
  groups × 4 records of `8, w, 128, 2` and 2,564 zero bytes, with w = 12,
  16, 20, 25.
- **The sector-header stream** (one page after the second `CORE` action
  page): both sectors' headers, twice. Each is a send of the header's bits
  reversed and shifted by 24 bits, behind a command word (0x0CA8004C,
  0x08A8004C, then 0x0CA80040, 0x08A80040). Each pass overwrites the low
  bits of the send: the first pass sets bits 0–223 to zero except a fixed
  list of ones (plus 4 ones further up), the second bits 0–31 (o). After
  each pass come the control pairs `0x0064000d` and `0x0004000c`. What the
  forced bits mean is [unk].

### (m) Phase 3: a design written by a5db, read back by Quartus (Plan step 4 → 7)

A small design added to fz0's image entirely by a5db, then checked with
Quartus's own assembler: `s[0]` (fz's shift-register bit, LAB X120 Y48) →
a NOT LUT in the unused LAB X40 Y20 (sector 0, the other half of the
chip) → a flip-flop clocked from fz's clock network.

```
L=cw_la_lab_r20_c40; P=xtophalf/xale0
a5db set db/sm4revb fz0.sof t0.sof specimens/sof_hmac.key $L \
    $P/xcw_la_le_lut/lut_mask=0101…01 $P/xcw_la_le_reg0/xlabclk_mux/regclk=DFF_LABCLK0 \
    $P/xcw_la_le_reg0/xreg0/reg_input=LUTOUT_6LUT $P/xcw_la_le_reg0/xreg0/register_comb=DFF_LATCH \
    $P/xcw_la_le_reg0/xreg0/leout=LEOUT $P/xcw_la_le_reg0/xreg_aclr/regclr=CLR2 \
    $P/xcw_la_le_regctrl/xcw_la_le_regctrl_reg/ce_sel=CE $P/xcw_la_le_regctrl/xcw_la_le_regctrl_reg/clr_sel=CLR \
    xsecmux/xcw_la_lcb_clkgen/xlabclk0/smxclk_tie_off=DISABLE_TIEOFF
a5db net db/sm4revb specimens/rg/full t0.sof t1.sof specimens/sof_hmac.key \
    'LAB_RE:X120Y48Z0I107>LAB_RE:X40Y20Z0I3' 'clock:<fz0 clock-route nodes>>LAB_RE:X40Y20Z0I83'
quartus_asm -t tools/rgdump/getattrs.tcl t1.sof <uid attribute list>
```

- `a5db net` is a breadth-first router over the graph: it takes hardwired,
  pass-through and resolvable edges through nodes the image doesn't already
  drive (`routed` gives those), and a `clock:` net stays off H/V/LL/LEIM
  wires. The data net is 18 hops: H6, H0, five H12, V8, H12, two V8, H0,
  V4, H2, H0, LL, LEIM into ALM 0's DATAA (LAB_RE I3). The clock net
  branches off fz's switchbox at X121 Y51, then goes through switchbox
  X60 Y51, seam tap X60 Y27, spine tap X28 (cell 6), row mux X28 Y20 (row
  clock 6) and row-clock tap X40 Y20 into the LAB clock (LAB_RE I83).
- **Quartus reads it back as intended** [obs]: asm2's `get_attribute` on
  the result returns every setting the design meant. That is all 17 data
  mux settings (`MUXINk_` for each hop's `muxin[k]`), the 8 clock-path
  settings (including the rule-driven tap, row-mux and predrive ones), the
  LUT mask (NOT DATAA in asm2's DIRECT form) and the 8 register and LAB
  clock settings.
- `quartus_pfg -c` accepts the a5db-signed `.sof`. `a5db rbf` on it gives
  `CMF` and `IO` byte-identical to pfg's, and both `CORE`s decode to its
  CRAM (pfg's with 4 S4-flagged nibbles, a5db's with none).
- **Limits**: the base image is still fz0's, so the device defaults, the
  I/O and the clock source come from Quartus; the new FF's output goes
  nowhere. Writing a whole image from defaults needs every block's idle
  values (I/O, HSSI, IO96 included) [unk]. Whether the logic works needs
  the board.

### (n) Phase 4: a whole image from the database (Plan step 4, no Quartus base)

- **The IO hash is SHA-256 over the two IO96 objects** [obs]: image bytes
  `[4,195,206, 4,746,853)` then `[8,942,059, 9,493,706)`, that is `io96_0`
  `hdr`, `t40`, `t41`, then the same for `io96_1`. It equals OPTIONS field
  1.20 of blinky, fz0 and rt0 (two different values). The assembler's
  `ASM2_INSTANCE::do_io_hash` (`libcomp_asm2.so`) hashes the sections of
  every periphery sector it is given with `EVP_sha256`; on this part only
  the IO96 objects take part. So the IO hash that (g2) found copied into the
  `IO` and `CORE` headers is a function of the image, and a writer with no
  Quartus `.sof` can compute it.
- **The idle state of every block** [obs on four designs]: `a5db
  idle-derive db/sm4revb <sof>…` reads Quartus images of designs that use
  different blocks (blinky, fz0, rt0, and `designs/idle`, one toggle FF in
  bank 6E so that hvio_3_1, both IO96 banks and the V16 clock path stay
  unused). It writes three tables:
  - `idle.tsv(.xz)`: per block type, the named bits that idle at 1. That
    is the majority vector over all instances in all images. For example
    661 per LAB/MLAB, 994 per `hviowr`, 929,257 per `io96`, 45,923 for
    `sm_hssi_top`, and none for any routing tile.
  - `idle_inst.tsv`: per-instance exceptions. There are only two:
    `hvio_1_1` and `hvio_2_1` hold `cr_hvioc = 0` on their four DCM
    clock/data muxes.
  - `idle_unnamed.tsv`: 420 set image bits that no named bit covers, all
    in the IO96 headers (210 per bank).

  `a5db blank` builds the image from these tables alone. `a5db idle-diff`
  on each Quartus image then leaves only that design's own blocks: idle0
  differs in its LAB X120 Y17, two HIOL tiles and `hvio_1_1` (84 bits),
  and fz0 in its LAB, routing, clock path and pins (513 bits). Block types
  no design touched (M20K, DSP, HSSI) agree in all four images.
  **Held out** [obs]: `designs/ho` (a 23-bit counter from V16 on LED1,
  DJ32), compiled afterwards and not used in the derivation, differs from
  the idle image only in its own blocks (954 bits). Those are the V16
  clock path, LAB X107 Y7, MLABs X108 Y6–7, their routing, `hvio_3_1`'s
  V16 bits and LED1's 107 `pio_0_0` bits plus one IO96 header bit. Every
  set CRAM bit in all five images is a named bit, so the spare-row and
  redundancy tiles, which have no maps, are all zero there, and the idle
  image leaves them zero.
- **The `.sof` container from the install alone** (`sofgen.rs`, `a5db
  blank`/`wrap`) [obs]. Records 0x01 (version, taken from a database
  header), 0x02, 0x2f (die and snapshot from `sof_meta.pb`), 0x03, 0x13 and
  0x36 (`sof_meta.pb` with the record sizes and design hash filled in) are
  the same as the assembler's. The others:
  - **PROGRAMMING_SEQUENCE** (0x3a index 4) is the install's
    `programming_sequence.pb.enc` inflated, byte for byte.
  - **HANDOFF** (index 2) is the HPS handoff of a design without HPS.
  - **OPTIONS** (index 3) is a constant with the IO hash spliced in.
  - **SLD archive** (0x24) holds `design_hash.bin`, the project's `.sld`
    and `jtag_id.txt` = `0xC362C0DD`.

  Wrapping blinky's image gives HANDOFF, OPTIONS and PROGRAMMING_SEQUENCE
  byte-identical to Quartus' records. The design hash comes from the
  netlist database in Quartus (`flow_get_design_hash` [inf, by name]). a5db
  has no netlist, so it uses SHA-256 over the two sector objects; pfg
  copies it without checking (g2).
- **`0xc362c0dd`**, the constant at +0x7C/+0x80 of the `IO` and `CORE`
  headers (l), **is the device's JTAG IDCODE** [obs: the assembler writes
  the same value to `jtag_id.txt`]. It has the Altera manufacturer id
  (0x6E in bits 1–11, as in openFPGALoader's `0x4369c0dd` for the
  A5EC008B). So it is the IDCODE to add for the board test [inf until the
  board reads it].
- **Pin features as bit tables** (`db/sm4revb/io/`, `a5db apply`) [obs]:
  each pin is the named bits that differ between idle0 (pin unused) and
  blinky, kept to that pin's buffer:
  - `V16_clock_in.tsv`: 9 `hvio_3_1` bits. The input buffer enable and
    config, the P2C I/O clock mux and the clock-or-data demux toward the
    clock network.
  - `DF35_out.tsv`: 107 `pio_0_0` bits plus one unnamed `io96_0` header bit
    (46282, idle 1, cleared). That is lane `xio12phytop_5` pin 1's output
    driver, bypasses and the read-training bits the IO96 lane controllers
    set when any lane is used.

  - `DJ32_out.tsv` (LED1, from `designs/ho`): lane 5 pin 5. It is
    DF35's table with pin 1 → 5 in the four per-pin blocks (`txdigitop`,
    `txtop`, `drvstatctrl`, `x_gpio`), the pin's IOSSM init bit 29037 →
    29041 (29036 + pin) and the header bit 46282 → 46286. That prediction
    equals the observed bits, 107 of 107; the other 94 bits are the same
    for both pins. So an IO96 GPIO output is a template over (lane, pin)
    [obs for two pins of one lane; other lanes inf].

  Rows starting with `@object/part` write bits no named bit covers.
- **The ALM's fabric pins** [obs, rt0's atom locations against its routes]:
  LUT input A–F of ALM k (k = 0–4 top half, 5–9 bottom) is `LAB_RE` pin
  8k + 3, 4, 2, 1, 0, 7. FF slot m (two per ALM, `reg0`/`reg1`) drives
  pins 89 + 2m (`xreg0`) and 90 + 2m (`xreg1`). The LAB clock is pin 83.
- **The DCM P2C clock muxes, rule** (k) [obs on two of the 32 muxes,
  inf for the rest]: these muxes have no edge API. Their 32
  `CE_CC_CLKMUX41_LOW` nodes (Z111–Z142) are told apart by the
  `HSSI_P2C_RE` index n that their output reaches through the hardwired
  `MUX_BOT`/`MUX_TOP` and `P2C_MUX_X4` nodes. The attribute is
  `mp2c_mux_{bot if n < 16, else top}/mux(4m+3)to(4m)/clock_int(n mod 4)/sel`
  with m = n / 4, and input i sets `CLKINi`. Idle is `VSS`. Two checks:
  - V16 (blinky, fz0, rt0, ho): Z111, n = 8, input 2 → `mux11to8/
    clock_int0 = CLKIN2`.
  - `designs/gc` (CLOCK2_50 from `hvio_1_1`, forced global, compiled for
    this): Z121, n = 2, input 3 → `mux3to0/clock_int2 = CLKIN3`.

  Quartus set exactly that attribute in both. `a5db routed` on gc's image
  decodes the edge, and the clock traces through routed and hardwired
  edges from BF23's `HVIOWR_RE` through DCM r23, the P2C mux, switchbox,
  seam and spine taps and the row clock into its LAB (X120 Y16). gc also
  differs from the idle image only in its own blocks (355 bits).
- **`a5db net` builds one tree per net** (`from>to;to;…`): each sink's
  branch may start anywhere on the tree so far, so a clock's trunk is
  shared, not duplicated.

**A blinky with no Quartus image under it** [obs]. `designs/blinky_a5db/
build.py` runs these steps (10 min, `specimens/b5/r4`):

1. `a5db blank`;
2. `a5db apply` with the V16 and DF35 tables;
3. `a5db set` on three LABs (X107 Y6–8). They hold 25 LUT+FF ALMs, a
   synchronous 25-bit counter in groups of five, and 4 carry LUTs in wire
   mode;
4. `a5db net` for the clock and the 30 data nets: 125 branches and 535
   hops. The clock takes Quartus' V16 path to the X121 Y51 switchbox, then
   a5db's own tree through seam X121 Y27, spine tap X93 (cell 6) and three
   row clocks;
5. `a5db wrap` and `a5db rbf`.

Verified:

- **Quartus' assembler reads back the intended settings**
  (`tools/readback.sh`). The image differs from idle in 27 instances (1,786
  bits). asm2's `get_attribute` and a5db's decode agree on every one of
  their 24,871 decodable attributes, and none differ. The other 5,174 have
  bits outside the maps: IO96 DFT and loop-back selects, and the same 85
  emulation/`<VIRTUAL>` attributes per LAB as in (h).
- **The image is the intended netlist**. `designs/blinky_a5db/netcheck.py`
  starts from the image, not from the build script. It reads the edges
  `a5db routed` finds, the hardwired graph edges and the decoded LAB
  attributes. From those it rebuilds 29 cells and 118 connections. Clocked,
  they make a 25-bit binary counter: 5,000 random states each step to
  state + 1. LED0's `IO96_RE` pin hangs off the top bit (1.49 Hz at
  50 MHz), and all three LAB clocks trace back to V16's `HVIOWR_RE`. The
  381 edges the image selects are exactly `net`'s configured hops, with
  nothing else and nothing missing.
- The idle image alone (`0_blank.sof`) goes through `quartus_pfg -c` too,
  and `a5db rbf` of it equals pfg's.
- **pfg converts it**: `quartus_pfg -c` succeeds. **a5db's `.rbf` is
  byte-identical to pfg's** (SHA-256 `515440fe…`), with the (o) encoder.
  `quartus_pfg --check_integrity` says OK on both, and `bitstream.py check`
  passes.
- **Against Quartus' own blinky**: no bit differs outside the two designs'
  blocks. `imgdiff` gives 2,691 bits in 44 instances. Every one of them is
  in blocks that blinky or blinky_a5db uses: each image's idle-diff is
  its own design and nothing else. The image parts:
  - `perictrl0`, `perictrl2`, both IO96 objects and sector 0 are
    identical. So the LED0 output and the V16 input match Quartus' bit for
    bit, and so does the IO hash (`10CEA95D…`).
  - `perictrl1` differs in 2 bytes: KEY0 (`hvio_3_1 xx20_u0`), Quartus'
    reset input, which blinky_a5db doesn't have.
  - Sector 1 differs in 1,253 bytes, all from placement and routing:
    - Quartus uses LAB X109 Y48 and MLABs X108 Y47–48; a5db uses LABs
      X107 Y6–8.
    - Quartus' LED net runs down V wires at X95 to row 5; a5db's is a
      short path.
    - The clock branches to different rows: Quartus' spine tap is cell 8
      to row clocks Y47–48, a5db's cell 6 to Y6–8. The path up to the
      seam tap is the same.
    - KEY0's fabric routing goes through HIOL X121 Y48–49.

  The `.rbf`s: `CMF` identical; `IO` and `CORE` differ in their data and
  hashes as above.

### (o) Phase 4: byte-identical CORE

**S4 flags, by watching pfg** [obs]. `quartus_pfg -c` was run under gdb (an
`LD_PRELOAD` shim calls `prctl(PR_SET_PTRACER)` and waits, since
`ptrace_scope` is 1; scripts in `specimens/core/gdb`, git-ignored). The frame
packets come from `FBGEN_ND_CRAM_FRAME_MGR`, not the `KM` manager or the
repairer:

- `create_partial_frames(dblock, cfg, 0, 4605)` walks the 4,606 stored frames
  of a sector. Stream frame k maps to image frame k (k < 98) or k + 98
  (`m_rbf_to_sof_map`), and that image frame's `S4_SII` list goes to
  `fill_core_frame_data`. The lists gdb read out of the live process equal
  the RIF parse, slot for slot.
- `fill_core_frame_data` builds the 656-byte physical frame nibble by
  nibble from a 1,312-entry table (dumped from the process): an entry p
  copies bits p, p−1, p−2, p−3 of pfg's frame; `0xffffffff` is a zero pad;
  an entry with bit 31 set is an S4 pair at s = (p & 0x7fffffff) ^ 1, read
  upwards (s, s+1). pfg's frame is the dr with each `s4` group g permuted:
  its bits g+7 … g hold dr g+5, g+1, g+4, g, g+7, g+3, g+6, g+2. So the
  pair (g+3, g+7) is pfg's (g+2, g+3) and (g+2, g+6) is (g, g+1).
- When exactly one bit of a pair is set, its pfg-frame index is the key. If
  the frame's list holds that key, the nibble is `value·4 | 1 << flag | 1`
  (bit 0 first); otherwise it is the plain `0 0 a b`.
- Check: `a5db rbf` with this rule gives physical frames equal to pfg's in
  all 9,212 stored frames of the all-random specimen, and the Python model of
  the rule matches all 2,331,755 one-hot pairs there and all 20 in blinky.
  `a5db` reads the lists from the user's install at run time
  (`install::rif_lists`): the `S4_SI` records are stored raw, 10 bytes each,
  one ascending run per slot.

**pfg's 2D compression** [obs; `FBGEN_ND_2D_ENGINE::run`,
`FBGEN_GEN::merge_and_create_2d_libs`, `update_with_libs`,
`find_and_clear_top_hash` and `run_combined_target` decompiled, then
checked]. A block is one of the 60 BBM columns over one stored 98-frame
group, taken from the physical frames (S4 flags included):

- **Zero**: an all-zero block gets code 1. pfg resets a column's zero flag
  and CRC after each group only for IP ids 1–4, so for ids 5, 6 and 15 the
  flag sticks: their code 1 needs the column zero in every earlier stored
  group of the sector too (the `walk` specimen has id-6 columns that become
  literal for good after one non-zero group).
- **Libs**, ids 1–4 only: each non-zero block's CRC (reflected, polynomial
  0x88010000, no inversion, `FBGEN_UTIL::lsb_calculate_crc32`, chained over
  the group's frames from 0) is counted over both sectors. Lib 0 is the
  block with the highest count, ties to the smaller CRC (the `std::map`
  walk keeps the first maximum); its count is cleared and lib 1 is picked
  the same way. A lib needs a count of at least 1, so an id has 0, 1 or 2
  libs. Blocks equal to lib k get code 2 + k.
- **Lib storage**: per id, its libs back to back, each frame's bytes
  padded to the stride (4 bytes under 12, else 8); ids 1–4 in order. The
  `0x1f` record holds the id offsets in 8-byte units and the lib area is
  cut into pages; with no lib at all there are no lib pages and no
  `0x15 (n, s, 4)` records.
- **Everything else** of ids 1–4 takes the candidate leaving the most zero
  bytes in the 98 × width residual: literal (0), XOR lib 0 (10), XOR lib 1
  (11), row history class 1 (8) and class 2 (9) (bytes of the class XOR the
  frame 18 back, frames 26–97), tried in that order; a later one needs
  strictly more zeros to win. Ids 5, 6 and 15 stay literal. Literal and
  residual columns go into the frame packets as before.

**Frame page interleave** [obs; `BITASM_FAMILY_VAR1::get_group_data`
decompiled]. Lib pages go sector by sector and the four setup pages per
sector alternate. Each sector's deflated frame stream is then cut into
pages and merged: with n₀, n₁ pages, r = max/min and R the sector with
more pages (sector 0 on a tie), turns alternate 0, 1, 0, … starting at R;
R sends a page on each of its turns, the other only when its bytes sent
times r minus R's bytes sent is at most 4,096. This gives strict
alternation for similar sizes and spreads the smaller stream evenly
otherwise (95 against 17 pages in `n_dup`). `merge_km_bitstream_data`,
the obvious candidate, never runs for this part.

**Sector-header stream** [obs]: five specimens with random, all-zero and
all-one sector headers classify every bit of both passes: in the first
pass bits 0–223 of each send are forced (to zero, except bits 2, 3, 6, 19,
21, 23, 26, 27 of sector 0 and 2, 3, 6, 19, 21, 23, 27 of sector 1) and
bits 832, 1885, 2134, 3183 (sector 0) and 768, 921, 1494, 2095 (sector 1)
are forced to one; in the second pass bits 0–31 are forced the same way
(ones at 6, 19, 21, 23, 26, 27 and 6, 19, 21, 23, 27). Every other bit is
the header's. rt0, the first specimen whose clock network sets the top
header bits, broke the phase-3 constants.

**Verified** [obs]: `a5db rbf` writes a whole `.rbf` byte-identical to
pfg's for all 78 `.sof`/`.rbf` pairs at hand: the 63 from phases 2
and 3 (blinky, fz0, the random-header, random-IO, pattern, random-CRAM,
bit-flip, walk and dense specimens) and 15 new ones: rt0 (a real design
with clock routing), nine CRAM edits of fz0 and blinky (2,000 and 50,000
random flips, 1 % random noise, half-random sectors, duplicated random
groups that tie two libs, rows repeating every 18 frames, an all-zero and
an all-one CRAM, a walking bit in sector 1) and the five sector-header
specimens above. The 78 pairs are in `specimens/core/pairs/` (git-ignored),
and `tools/rbfpairs.sh specimens/core/pairs.txt` reruns the comparison (8 min;
78 of 78 identical after the merge into `phase4`). The board question of phase 3 (does the SDM take a literal
`CORE`) no longer matters for a5db's own output.

### (p) Phase 5: pin tables from a template

The phase-4 tables (V16 in, DF35 and DJ32 out) are replaced by one template,
`db/sm4revb/io/template.tsv`, that `tools/pins/derive.py` writes from the I/O
bits of three training images (`designs/pt`, blinky, ho). `a5db pintable`
expands it for any set of pins, `a5db pins` writes the result, and
`db/sm4revb/io/de25_pins.tsv` holds the expansion for every DE25-Nano user pin
as input and as output (`tools/pins/de25.sh`). `db/sm4revb/io/pins.tsv` lists
the package's 351 HVIO and IO96 balls with their block and pad (`ddb pins`).
The DE25-Nano pin list with the manual's standards is
`designs/pins/de25_pins.tsv` (from the Terasic System Builder scripts in
[spersvold/de25_nano](https://github.com/spersvold/de25_nano)): 159 HVIO pins
in banks 6A–6H (6G and 6H at 1.8 V, the rest 3.3 V) and 22 IO96 pins in bank
2A at 1.1 V, 6 of them the camera's D-PHY.

- **Pad name → bits** [obs]: an HVIO pad `HVIO_B<b>_<g>_<k>` is
  `xx20_u<b>/xx4_u<g>/xx1_u<k>` of its `hvio_*` instance (V16 is
  `HVIO_B1_2_1` = `xx20_u1/xx4_u2/xx1_u1`), and an IO96 pad `P<n>` is lane
  n / 12, pin n % 12 (DF35 is P61, lane 5 pin 1). The two hold for every pin
  checked below.
- **HVIO pins are self-contained** [obs]: a used pin changes only bits under
  its own `xx1_u<k>`, with no bank or group bits. Output: `itxen` = 1, the
  output and OE register muxes in bypass (`oreg` sel 00, `din3` 0). Input:
  `irxen` = 1 and one input register mux in bypass (`ireg_4to1_mux1` sel 00
  for data). The standard is `iio_config_[1:0]`: idle 11, 3.3-V LVCMOS 00,
  1.8-V LVCMOS 10 (the rx and tx `iio_config_` bits are one image bit). A
  clock input uses `ireg_4to1_mux0`, the bank's `clkordat` demux
  `u<4g + k>` and an `xif` ioclk mux.
- **IO96 pins have pin, lane and bank parts** [obs]:
  - pin: output 12 bits (`txdigitop`/`txtop`/`drvstatctrl`/`x_gpio` of pin p:
    data and OE bypass, slew, drive code), input 3 (`i_rx_tggpiorxen`,
    `ireg_4to1_mux0` sel 00), plus their IOSSM bit below;
  - lane: 56 bits once a lane has any used pin (ODT/sense-amp timing,
    vref enables), plus 11 vref bits whose values depend on whether the lane
    has an input: output-only lanes get `rxdatavref{l,u}` = 358 and
    `i_vdac_sel` = 01, a lane with an input `rxdatavrefu` = 384,
    `rxdatavrefl` = 0, `i_vdac_sel` idle, and all 12 `xp2c` bead selects of
    its fabric adapter. So pin tables don't simply add up; `pintable`
    composes them per lane;
  - bank: 23 bits once per IO96 with any pin (`rd_training` in all 8 lanes,
    the power controller, 8 IOSSM bits);
  - the IOSSM init memory (`iossm_mem_init_122_bit[]`) is a pin map: lane L
    has a 32-bit word at 28,992 + 32 (L − 4), inputs at bit p, outputs at
    12 + p, and bit L of the bytes at 28,800, 28,808, 28,816 and 28,824 marks
    a used lane;
  - an output also clears one unnamed `io96_0` header bit,
    45,136 + 229 L + p (seen in lanes 4 and 5; the stride is [inf] for the
    others).
- **Held out** [obs]: `designs/pins/gen.py` writes `pt` (training: 7 inputs,
  7 outputs in all 8 HVIO banks and IO96 lanes 4–5, both HVIO voltages) and
  `ph1`/`ph2`, every other DE25-Nano pin, ph1 with the even pins of the list
  as outputs and the odd ones as inputs and ph2 the reverse (167 pins each,
  outputs XOR two inputs, no clock). `tools/pins/check.sh` writes the idle
  image plus `a5db pins` for the design's pins and compares every hviowr,
  io96 and unnamed io96 bit with Quartus' image. pt: 0 bits differ. Of 334
  held-out pin uses, **6 bits differ**, all in the input register mux:
  - ph2's `DRAM_RAS_n` input has no load (the generator never used it), so
    Quartus enabled the buffer but no `ireg` mux (2 bits);
  - ph1's SW3 (lane 5 pin 7) leaves through `ireg_4to1_mux1` instead of mux0
    (4 bits). Which of the two muxes an input uses goes with the fabric route
    Quartus chose, so a pin spec can name it: `in0`/`in1` (`in` is the
    usual one). Which graph node each mux reaches is [unk].
- **Not covered**: the D-PHY standard (used here single-ended at 1.1 V),
  bidirectional pins, pull-ups, bus hold, current strength and slew
  settings (the DE25-Nano manual sets none), IO96 lanes 0–3 and 6–7 and the
  second IO96 bank (no DE25-Nano pins), differential I/O, and the pin ↔
  routing-graph node map (which fabric node carries a pin's input or
  output; phase 4 took V16's and DF35's from Quartus routes).

### (q) Phase 5: core features, rebuilt from a design file

`a5db extract` turns an image into a **design file**, the vocabulary a placer
and router would write through a5db (`src/rebuild.rs`): `pin` lines (the
template of (p)), `edge` lines (routing graph edges, decoded with `a5db
routed`'s rules), then per instance `attr` (a CUSTOM setting) and `direct`
(a DIRECT bit string: LUT masks, M20K contents, DSP inversion masks) for
whatever the pins and edges leave different from the idle image. What that
vocabulary can't express comes out as `bit` (a named bit) and `raw` (an
image bit no named bit covers) lines. `a5db build` writes a design file over
the idle image; `a5db rebuild` does both and diffs the result against the
input. A design file names nothing from Quartus' image: every line is a
setting, an edge or a table expansion.

Designs (all compiled for this phase; `specimens/runs/*0`):

- `designs/ar` (development): a 32-bit accumulator with enable and
  synchronous clear (carry chain), a ternary adder (shared arithmetic), a
  12-bit down counter with asynchronous clear and synchronous load, a
  comparator and subtractor, an 8-bit register-only shift register and a
  32 x 16 RAM in an MLAB;
- `designs/mm` (development): a 512 x 32 simple dual-port M20K, a 1024 x 16
  initialised ROM in M20K, a registered 18x18 signed multiply and a 27x27
  multiply-accumulate in DSP blocks;
- `designs/hx` (**held out**: written before the tools above and never used
  to fix them): a 20-bit up/down counter with enable, asynchronous preset
  value and synchronous clear, a four-input adder, a 16 x 8 MLAB RAM, a
  2,048 x 10 true dual-port M20K with initial contents, and a 9x9 + 18x19
  multiply-add, on another bank and clock (CLOCK2_50) than ar and mm.

Results [obs]:

| design | pins | edges | attr | direct | bit | raw | bits differing after the rebuild |
|---|---|---|---|---|---|---|---|
| ar | 9 | 1,888 | 1,198 | 98 | 0 | 0 | **0** |
| mm | 5 | 1,760 | 712 | 577 | 1 → 0 | 0 | **0** |
| hx (held out) | 10 | 1,675 | 917 | 591 | 1 → 0 | 0 | **0** |

- Each image is rebuilt **bit for bit** in every part (CRAM, M20K content
  RAM, I/O headers): the rebuild leaves 0 differing bits, so there are no
  unmapped bits to exclude here.
- The one `bit` line in mm and hx was `rmode[0]` of a DSP: `dsp_mode`, like
  four others below, is a CUSTOM attribute that the Bcm_device dump has no
  bits for. With it added (below), both are pure attr/direct.
- The attributes these designs need, by feature [obs, names from the design
  files]: LUT masks (`lut_mask`, DIRECT); carry chains and shared arithmetic
  (`adder_mode` ENABLE_ADDER on the LUT, `adder_or_packed_mode` on the
  adder; the register's `reg_input` picks ARITH_SUMOUT, LUTOUT_6LUT,
  LUTOUT_5LUT or LEIM, the last one a register packed with unrelated logic,
  fed straight from routing); registers (`register_comb` DFF_LATCH, REGIN,
  or WIRE_MODE to bypass; four per ALM in `xcw_la_le_reg{0,1}/xreg{0,1}`);
  register controls (`regclk` DFF_LABCLK0/1 picks one of the LAB's two
  clocks, `regclr` CLR1/CLR2 one of its two clears, `ce_sel`/`clr_sel` per
  half-LAB select the LAB's enable and clear lines, `sclr_sel` SCLR per
  register, and the programmable inverter `iss/xclrb_rpi/sel` INV in the
  routing tile for an active-low clear); MLAB RAM (`ram_mode`
  LUTRAM_32DEEP/LUT_MODE on `xce_ml_le_din`, `xcw_ml_le_csel` and the LAB's
  `lram_sigctrl`); M20K (42 CUSTOM settings for widths, ports and registers,
  plus `m20k_eram_add<k>`: row k/2, bits 40(k mod 2) .. +39 of the 256 x 80
  `eram_array`); DSP (`dsp_mode`, input/output register and clock selects,
  signedness, `dsp_proginv_{l,h}` DIRECT masks).
- **Routing needs two more things than the mux settings** [obs]:
  - every used routing mux with a hyper-register (LEIM, LEIM_WB, EDIM, the
    H/V drivers, HIOL `xleim`/`xedim`) is set to `flop_mode = FAST_WIRE`
    (bypass) by Quartus, 580 of 580 in ar; idle is DIS. The exception are
    the LEIMs into a LAB's control inputs (`LEIM` I80 and up of a LAB
    routing tile), which stay DIS. `a5db` now writes FAST_WIRE with every
    edge it routes (`Join::edge`). **The phase-4 from-scratch blinky has
    every routing mux at DIS**, which no Quartus image has; whether DIS
    passes the signal is [unk], so its board files are rebuilt with
    FAST_WIRE (see "Board test");
  - the HIOL tiles' `xleim` input muxes (pin data into the fabric) had no
    `sel` bits in the Bcm_device dump, so 1,565,476 graph edges resolved to
    nothing. asm2's `set_attribute` gives them: MUXINk is the one-hot
    `cram_mux{k/4}_hg[k mod 4]`, MUX_DIS all zero (`tools/bcm/
    asm2_settings.tcl`). Data inputs from HVIO pins pass through them.
- **CUSTOM attributes without bits in the Bcm_device dump** [obs]:
  `tools/bcm/asm2_probe.sh` sets every value of each such attribute on one
  instance through asm2 and records the named bits that change
  (`probe2settings.py` writes them into `bcm_settings.tsv`). Of 1,008
  probed (LAB, MLAB, M20K, DSP, routing, clock, HVIO), only three change
  named bits: DSP `dsp_mode` (`rmode[4:0]`, a 5-bit code: TWO_18X19 0,
  ONE_27X27 1, SUM_OF_2_18X19 2, … COMPLEX_MULT 22) and the HVIO input and
  OE delay chains (`a_rb_dlychn_sel`, 64 and 16 steps). The rest (LAB
  `lut_mode`, `carry_out`, `latch_mode`, MLAB `fastwrite`, M20K port
  selects, the HVIO buffer's `io_standard`, `weak_pull_up`, `bus_hold`,
  `slew_rate`, `open_drain`, `schmitt_trigger` …) change no named bit in
  asm2: they are `<VIRTUAL>` there, so whatever they mean on silicon comes
  through other attributes or the fitter [inf]. Three LAB/MLAB `lut_mode`
  probes crash asm2 (`asm2_state.cpp:1140`).
- **DIRECT attributes** (`db/sm4revb/direct.tsv`, `tools/bcm/direct.tcl`):
  the bit order of every DIRECT attribute of the LAB, MLAB, M20K, DSP and
  HVIO blocks (775), and of the IO96 (4,775 attributes, 4.28 M bits,
  `direct_io96.tsv.xz`, not loaded by a5db).
- **Readback** [obs]: `tools/readback.sh` on each rebuilt image (a5db's
  decode against asm2's `get_attribute`, every attribute of every instance
  that differs from idle): hx 35,563 attributes in 58 instances, 34,626
  equal, 0 differ; ar 33,409 in 55, 32,557 equal, 0 differ; mm: see the
  commit after. The attributes a5db can't decode are DFT and scan settings
  (`a_regscan`, `a_frzlogic`, `dft_mode`, `secmux_dftclk*`), whose bits are
  among the unmapped ones of (h).

### quartus_sta fails under chipsmith [obs]

The full `--flow compile` dies in signoff timing analysis: `quartus_sta`
cannot load `quartus/linux64/libsys_pdpw.so`, because the install's own
`qcore/linux64/libQt6Network.so.6` has `NEEDED libbrotlidec.so.1` and
`RUNPATH $ORIGIN`, and Quartus doesn't bundle brotli (it bundles `libzstd`,
the other compression dependency). chipsmith's
`crates/chipsmith-quartus-common/compat.nix` provides no brotli. The fix
belongs in chipsmith: add `brotli = pkgs.brotli.lib.outPath;` to that
list. Fitter and assembler are unaffected, so blinky was assembled with a
separate `quartus_asm blinky`.

### Board test (prepared, not run: the board is off)

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
tools/cap.sh 12G python3 -I designs/blinky_a5db/build.py specimens/b5/r4  # 10 min
cp specimens/b5/r4/blinky_a5db.sof $B/scratch.sof
$A rbf db/sm4revb $B/scratch.sof $B/scratch_a5db.rbf $Q specimens/rbf_keyed_hash.key
tools/quartus.sh quartus_pfg -c $B/scratch.sof $B/scratch_pfg.rbf        # equals scratch_a5db.rbf
for v in zero-io-hash zero-design-hash corrupt-core; do
    python3 -I tools/rbf_patch.py $v $B/scratch_a5db.rbf $B/scratch_a5db_$v.rbf; done
for f in $B/*.rbf; do python3 -I tools/bitstream.py check $f; done      # all pass but corrupt-core
```

SHA-256 of the files as built here: `blinky_pfg.rbf` 8b490e78…,
`scratch_a5db.rbf` = `scratch_pfg.rbf` 515440fe…,
`scratch_a5db_zero-io-hash.rbf` 4933335c…,
`scratch_a5db_zero-design-hash.rbf` 5203f7c4…,
`scratch_a5db_corrupt-core.rbf` 8b8ac007…. `quartus_pfg --check_integrity`
says OK for all but corrupt-core (`Section hash mismatch`). The phase-3
files (the all-literal `CORE` of blinky) are kept in `specimens/board/old/`
in case 4 fails.

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

## Oracle

`tools/quartus.sh <tool> <args…>` runs one Quartus Pro tool through
chipsmith inside a fresh user+network namespace. It refuses to run if any
interface besides `lo` (and the optional unaddressed `dummy` licence
interface) is visible. It reads the namespace's own link list; note that
`/sys/class/net` inside the namespace still shows the host's NICs. It needs
`CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro`, and `CHIPSMITH` if the
`quartus-pro` chipsmith isn't on `PATH`.

The machine is shared and has no swap. Anything that can grow past about
1 GB (Quartus, `a5db` over whole images or the graph, Python over tables)
runs under a memory cap: `tools/cap.sh <limit> <command…>`, a
`systemd-run --user --scope` with `MemoryMax`. An uncapped one-off Python
loop over `.rbf` sections reached 20 GB and the kernel OOM killer on
2026-10-31: the first section's size word is at +0x408, not +8, so it read
size 0 and never advanced. `bitstream.py`'s `sections` now refuses a
size-0 section, and `bitstream.py cmp a.rbf b.rbf` compares two files
section by section, so there's no reason to hand-roll that loop again.

`designs/blinky` (LED[0] at about 1.5 Hz from CLOCK1_50, KEY[0] resets)
**compiles offline to `.sof`** with the licence [obs]: fitter 1 h 14 min on
the loaded machine, then a separate `quartus_asm` (6 min; the flow's
`quartus_sta` step fails, see (f)). There is also a "use the Reset Release IP"
critical warning, which a real design should follow.

## Plan

**Phase 4 status** (branch `phase4`):

- Step 1 (whole image from the database): **done** (n). `a5db blank`
  builds the idle image from `db/sm4revb/idle*.tsv`, in a `.sof` container
  built from the install. A blinky (V16 clock, DF35 LED, 29 hand-placed
  cells, 31 routed nets) on top of it is read back by asm2 as intended,
  rebuilds to a counting netlist, and goes through pfg and
  `--check_integrity`. Against Quartus' blinky no bit differs outside the
  two designs' blocks.
- Step 2 (byte-identical CORE): **done** (o). S4 flags, lib/BBM choice,
  page interleave and the sector-header forced bits. `a5db rbf` equals
  pfg's whole `.rbf` on 78 specimen pairs and on the from-scratch blinky.
- Step 3 (leftover [unk]s): `0xc362c0dd` is the JTAG IDCODE (n); the DCM
  P2C mux order has a rule, checked on two muxes (n); the sector-header
  forced bits are classified (o). Still open: redundancy/spare-row tiles
  and the 8 skipped perictrl0 words.
- Step 4 (board test): files and steps for the from-scratch blinky ready
  ("Board test"); needs the board.

Open after phase 4 [unk]:

- On the board: whether the from-scratch image configures and blinks, and
  whether the SDM checks the IO and design hashes ("Board test").
- Spare-row and redundancy tiles still have no bit maps. They are zero in
  every image here, so they only matter for a design that uses them.
- The other 30 DCM P2C muxes follow the rule only by inference. IO96
  outputs on other lanes, IO96 inputs, HVIO outputs and other I/O
  standards have no table yet: only V16 in, DF35 and DJ32 out.
- What the 8 perictrl0 words pfg skips, the sector-header forced bits and
  record 0x13 mean; OPTIONS is one constant for the default device
  options.
- Where Quartus' design hash comes from (a5db uses its own, which pfg
  accepts).

Next: the board test; pin tables generated from the lane/pin template
rather than captured one by one; then nextpnr (Plan step 7) writing through
`a5db blank`, `apply`, `set`, `net` and `rbf`.

**Phase 3 status** (branch `phase3`):

- Step 1 (big tables compressed): **done** (j). `io96` and `sm_hssi_top`
  layouts are `.tsv.xz` (363 MB → 3.6 MB); every reader takes either form
  and outputs are byte-identical.
- Step 2 (routing graph → bits): **done** (k). `a5db join/route/routed/
  routecheck`; on fz0 and `designs/rt` every routing bit Quartus set is the
  join's from `report_routing`'s paths or a decoded FF data-in edge that
  `report_routing` can't print, and the join sets none Quartus didn't.
  "Direct links" are LAB_RE → LL.
- Step 3 (`.rbf` writer): **done, CORE not byte-identical** (l). `a5db rbf`
  writes CMF and IO byte-identical to pfg on 60 specimens and an all-literal
  CORE that decodes to the same CRAM; pfg's `--check_integrity` accepts it.
  Byte identity would need pfg's lib/code choice and the S4 flag rule
  [unk]. SDM acceptance is [unk] until the board test below.
- Step 4 (a design from a5db): **done on top of fz0's image** (m). A LUT,
  an FF and two routed nets (one a clock through spine and row-clock muxes)
  written by `a5db set`/`net`; Quartus's asm2 reads every setting back as
  intended and pfg converts the `.sof`.

Open after phase 3 [unk]:

- SDM acceptance of an a5db `.rbf` (all-literal `CORE`, no S4 flags), and
  whether the SDM checks the IO and design hashes: the board test above.
- The S4 flag's image-frame → RIF-slot mapping (l), and pfg's lib and BBM
  code choice; both are needed for a byte-identical `CORE`.
- The DCM periphery clock muxes (`cw_cc_dcm_hssi_p2c_bf`, no edge API):
  one instance mapped, the order of the other 31 isn't (k).
- Redundancy and spare-row tiles have no bit maps (k).
- The constant `0xc362c0dd` at +0x7C/+0x80 of the section header, the
  eight perictrl0 control words pfg skips, and the first-pass bits of the
  sector-header stream: copied, not understood (l).
- A whole image from defaults rather than on top of a Quartus `.sof` (m).

Next: the board test; then nextpnr (Plan step 7) on `a5db net`'s graph and
join, with `a5db rbf` as the writer.

**Phase 2 status** (branch `phase2`):

- Step 1 (container hashes): **done** (g2).
- Step 2 (design-section payload): `CORE` 2D compression decoded
  (`tools/core_decode.py`; blinky exact, random images off only in S4-flagged
  nibbles [unk]); `IO` section decoded and re-encoded byte for byte
  (`tools/io_decode.py`). Deflate is zlib level 9, window 13, `Z_FIXED`. Still
  missing: a CORE encoder (lib/code choice, or an all-literal stream the SDM
  may accept), and assembling a full IO section.
- Step 3 (routing graph): **done** through Quartus's DYGR API (i): 7.6 M
  nodes, 26.3 M edges, 22.7 M with their mux setting; checked on fz0's route
  and `.sof`.
- Step 4 (config bits): **done** (h): bit maps for all 26 block types in
  `db/sm4revb/bits` (one layout each, aliases resolved), CUSTOM settings and
  the BCM type tree in `db/sm4revb`, and `tools/a5db` reads and writes
  attributes (encode byte-identical to asm2's `set_attribute`). Every set
  CRAM bit in fz0 is a named bit.
- Step 5 (fuzzing): replaced by asm2 `set_bit`/`write_sof` diffs; ECO runs
  through the fitter used as an independent check (LUT bits, mask order).

Next: join the graph's edge settings to the bit maps in `a5db` (route a
net → bits); a CORE encoder; a full `.rbf` writer (step 6); then nextpnr
(step 7). Board tests (does the SDM accept an all-literal CORE, do the
IO/design hashes matter) need the user.

Done in phase 1: licence and first `.sof`/`.rbf`; the v25 reader with
trace-verified `pkg`, `pad`, `hdr` and `place` schemas; the DE25-Nano pin
map; the container layout. Next, in order:

1. **Container hashes**: find what the `IO`/`CORE` hashes and the keyed
   hashes cover (REA on `quartus_pfg`'s writer, or pair-diff two designs),
   and what the CMF word at 0x1F00 means. Compile a second design to check
   that the patched CMF only depends on the section count.
2. **Design section payload**: how the 19.75 MB `.sof` image becomes 120 KB
   of `CORE` (compression or frame selection), using `sof_meta.pb`'s
   sector/frame/CRAM table.
3. **Routing graph**: decode the `.udm`/`.mdb` mapped-database format
   (header offsets, `SCHEMA_ID` table) for sm4revb, or find the DYGR code
   that builds the graph from it (REA).
4. **Config bits**: the second half of `asmm` (198 MB after the
   4,911,334-name table), by REA on its reader since the trace only
   gives widths; `bcm2bits` decryption.
5. **Fuzzing**: pair-diff `.sof` images per feature (LUT, FF, routing,
   I/O), as for Cyclone II. The `.sof` image is unhashed, so diff it rather
   than the `.rbf`.
6. **Writer and loader**: emit fabric sections with correct hashes, splice
   in the user's own install's `main.cmf` (patched as in (f)), and load
   with openFPGALoader after adding the A5EB013B IDCODE. On-board testing
   needs the user.
7. **Open flow**: yosys and nextpnr (himbaechel), fed from `db/`, the same
   way chipsmith `open` works for Cyclone II.

## License

MIT.
