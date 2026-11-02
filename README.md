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
  stored value too [inf]. (Phase 8, README (z3): that is the WYSIWYG port
  convention; what the silicon computes through `LUTOUT_6LUT` is
  ¬BCM[i XOR 32], checked against Quartus' own image of a design.) fz0's original LUTs don't decode to their
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
- Edge delays and timing (`DYGR_ROUTE_TIMING_INFO`, not dumped; a model
  fitted to quartus_sta's paths is in (u)).
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
  - The whole ph1 image, with SW3 as `in1`, rebuilds bit for bit from a
    design file (q): 167 pins, 1,448 edges (the pin data through the HIOL
    and VIO input muxes), 443 LAB/MLAB attributes and 60 LUT masks, 0 bits
    differ.
- **Not covered**: the D-PHY standard (used here single-ended at 1.1 V),
  bidirectional pins, pull-ups, bus hold, current strength and slew
  settings (the DE25-Nano manual sets none), IO96 lanes 0–3 and 6–7 and the
  second IO96 bank (no DE25-Nano pins), differential I/O. The pin ↔
  routing-graph node map (which fabric node carries a pin's input or
  output; phase 4 took V16's and DF35's from Quartus routes) is now (t).

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
  `lram_sigctrl`); M20K (CUSTOM settings for widths, ports and registers, 42 in mm,
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
  equal, 0 differ; ar 33,409 in 55, 32,557 equal, 0 differ; mm 37,021 in
  61, 36,339 equal, 0 differ. The attributes a5db can't decode are DFT and scan settings
  (`a_regscan`, `a_frzlogic`, `dft_mode`, `secmux_dftclk*`), whose bits are
  among the unmapped ones of (h).

### (r) Phase 5: clocks

Designs: `designs/clocks/gen.py` writes `ck1` and `ck2`, which between them
use every clock-capable HVIO pin of the DE25-Nano (pads `HVIO_B<b>_2_<k>`,
4 per bank, 32 in all, CLOCK1_50 and CLOCK2_50 among them) as a global
clock, each driving a 2-bit counter with a clock enable. ck1 adds CLOCK0_50
(IO96, DJ35) and three IO registers. `designs/ck_all`, all 32 pins at once,
doesn't fit (Error 24404: a pin's periphery-to-core clock path is taken by
another pin), and that failure split the pins: ck1 got the 16 that placed,
ck2 the other 16.

- **How an HVIO clock pin enters the clock network** [obs on all 32 pins]:
  each bank half b of an `hviowr` has two clock outputs toward the DCM, a
  2:1 `xif_p2c_ioclk_2to1_mux_u<b>` that takes pin k = 0 or 1 of the
  `xx4_u2` group (P0, P1) and a 4:1 `xif_p2c_ssclk_4to1_mux_u<b>` that takes
  pin 2 or 3 of either half (`O_SSCLK_BANK<half>_P<k>`; Quartus always used
  the one numbered like the pin's half). So at most two clock pins per bank
  half reach the clock network at once, 16 per design on this board, which
  is why ck_all can't fit. Besides that the pin needs its pin-level bits
  (input buffer, `ireg_4to1_mux0` in bypass, the bank's `clkordat` demux
  `u<4g + k>` toward the clock path). `a5db pins` writes all of it for
  `<ball>=clock_in` (`src/pins.rs`).
  - Fitted on ck1 (with pt, blinky and ho), **held out on ck2**: its 16
    clock pins are exactly the other pin of each ioclk and ss clock mux.
    `tools/pins/check.sh` on ck2: **0 I/O bits differ**, as on ck1.
  - 1.8-V clock pins (6G, 6H) clear only `iio_config_[0]`, as 1.8-V data
    pins do (p).
- **CLOCK0_50 (IO96)** [obs]: an IO96 clock input sets the input's
  `i_rx_tggpiorxen` and IOSSM bit but leaves `ireg_4to1_mux0` at its idle
  SEL3; the lane gets the lane-with-input rows of (p). One pin seen
  (`<ball>=clock_in` on IO96).
- **The DCM P2C muxes** [obs]: the rule of (n) was checked on 2 muxes. ck1
  and ck2 use **22 distinct P2C muxes** (10 in the DCM at X121 Y4, 12 at
  X121 Y51) with 32 (mux, input) pairs covering inputs 0–3, and every one
  decodes to an edge whose setting equals Quartus'. The DCMs at X0 Y4 and
  X0 Y51 serve the left side, which has no DE25-Nano clock pins; the rule
  there is [inf].
- **Whole images** [obs]: `a5db rebuild` on ck1 (23 pins, 426 edges) and ck2
  (18 pins, 395 edges) leaves **0 differing bits**. No clock-network block
  needs an `attr` line: the P2C muxes, switchboxes, vertical-seam and spine
  taps and row clocks all come from routing edges and the clock rules
  (graph.rs). The LAB clock enables are LAB attributes (`ce_sel` CE and the
  LAB's enable lines, (q)).
- **Global and regional** [obs]: there is one clock network. Quartus builds
  each clock's tree only into the sectors that have loads: every clock in
  ck1, global or not, reports "Clock Region: Sector (1, 0)" (the right half,
  `(61, 5)` to `(121, 51)`). `GLOBAL_SIGNAL "REGIONAL CLOCK"` is not a valid
  value for Agilex 5 (Warning 125035); `CLOCK_REGION "SX1 SY0 SX1 SY0"` on
  CLOCK2_50 is honoured ("Clock Region Constraint") and changes nothing
  since the tree was one sector anyway. A placer chooses the sectors by
  where it puts the loads; the routing is the same clock-network edges.
- **Row-clock delay chains** [obs in mm, rule inf]: mm's DSP/M20K rows use a
  row clock through its delay chain: `delay_chain_<side><j>/mux41` CLKIN0
  and the predrive mux on input 0 instead of input 1. graph.rs now has rules
  for the delay chain edges (tap k → CLKIN<k>, only k = 0 seen).
- **IO registers** [obs, ck1]: FAST_INPUT_REGISTER and FAST_OUTPUT_REGISTER
  on 1.8-V pins are the uses `in_reg` and `out_reg`: the input register's
  clock inversion, clock and enable enables and `ireg_4to1_mux1` SEL1, and
  the output's `oreg` clock and enable enables and `oreg` mux SEL1. The
  register clock comes from the fabric through the HIOL tile, where
  Quartus also set three `rpi_sel` INV programmable inverters (`attr`
  lines). One design, so [obs] for those three pins and [inf] for others.

- **IOPLL** [obs, one configuration]: `designs/pl` (CLOCK1_50 into an
  IOPLL made by `ip-deploy`/`qsys-generate` in `prep.tcl`, 100 MHz out to a
  24-bit counter on LED0, `locked` on LED1). Quartus placed it in
  `hvio_3_1`'s `xpllwrap_top`, next to V16. `a5db rebuild` (4 pins, 95
  edges) leaves **0 differing bits**; the PLL is 40 `attr` lines (enable,
  reference from the I/O clock `PLL_CLKMUX_IOCLKIN_3`, VCO phases, charge
  pump and loop filter trims, counter sources and bypasses, output enable),
  10 `direct` lines (the C0/C1/M counters' hi/lo dividers, DCC timers) and
  19 `bit` lines: lock-detector and VCO-calibration controls
  (`cr_lockf_*`, `cr_vcocalsel_clk*`, `cr_dcs_clksel`) that no attribute
  covers. V16 as a PLL reference leaves the ioclk mux at P0: the reference
  doesn't use the clock-pin path. The PLL output enters the clock network
  as an ordinary graph edge (`HVIOWR_RE … I16 → DCM_mux`). How counters and
  trims follow from the requested frequencies was [unk] here (one PLL
  compiled); phase 6 has the rule, held out on four more PLLs (v).

### (s) Phase 5: leftover unknowns

- **Record 0x13 is the JTAG USERCODE** [obs]: its 16 bytes are `u32 0,
  0x00200020, 0x00010000` and the USERCODE (0xFFFFFFFF by default). An
  assembler-only rerun of ar with `STRATIX_JTAG_USER_CODE 1234ABCD` changed
  the last word to 0x1234ABCD and nothing else in the record. The first
  three words are the same in every image here [unk]. `a5db`'s container
  keeps 0xFFFFFFFF.
- **Where the design hash comes from** [obs, decompiled]: the fitter, not the
  assembler. `FITCC_DESIGN_HASH_UTILITY::apply_design_hash`
  (`libfitter_fitcc.so`) runs `FITCC_PR_POF_HASH_GENERATOR` per
  partition: SHA-256 over the partition's 8-byte netlist cookie
  (`cdb_get_atom_netlist_cookie`, its child partitions' after it) and then
  its LUT masks, and stores it as the partition's DESIGN_HASH assignment;
  the assembler copies it into record 0x36 and the `.rbf`. It is the
  partial-reconfiguration compatibility hash: a PR image must carry the
  same hash as the static region it was compiled against [inf, from the
  names]. The netlist cookie is a database value with no counterpart in the
  image, so an open flow can't reproduce Quartus' hash and needn't: it has
  to be consistent between its own images, which a5db's SHA-256 over the
  sector objects is. An assembler rerun keeps the hash (ar: `3C76F86D…` in
  both, also in `ar.asm.rpt`).
- **The spare row is a spare row 5, outside the image** [obs/inf]: asm2's
  `get_uids` lists `_rsp5_` instances for 8 block types: 52 LABs, 52 MLABs,
  8 M20Ks, 4 DSPs, 104 LAB and 16 M20K/DSP routing tiles, 2 HIOL tiles and 2
  row-clock tiles, a whole fabric row named after row 5. `get_bit` on any of
  them aborts (`data < data_width`), so they have no image address [obs].
  Every fabric row Y = 5..51 has a 98-frame group (Y = 5 group 0, Y = 6..51
  groups 2..47) and group 1 holds no tile and is never stored by pfg (f),
  so the spare row is that group: the SDM fills it itself when it repairs a
  row through the redundancy muxes [inf]. The 94 `cw_rd_redtl_bf` tiles
  (one per row at X = 0) and the `*_redt_bf` vertical-seam tiles are not
  asm2 block types at all (asm2 has 26 types), so their settings don't come
  from the user image either; `lsm_eram_redundancy.fw` is the likely owner
  [inf]. A writer leaves both alone, as every Quartus image here does.
- **The 8 perictrl0 words pfg skips** [unk, narrowed]: none of their CNOC
  addresses (0xA1000, 0x20F000…0x20F128, alone or plus 0x300000 or
  0x10300000) appears as a 32-bit word in `cssm.fw`, `cssm_ux_phy.fw`,
  `io_config.fw`, `cssm_reset_release.fw` or `enable_pm.fw`, raw or
  inflated. The "firmware owns them" inference of (l) stands untested.
- **Sector-header forced bits**: no change since (o); classified, meaning
  [unk].

### (t) Phase 6: pin ↔ routing-graph node map

`a5db pinmap <db> <graph>` writes
[`db/sm4revb/io/pin_nodes.tsv`](db/sm4revb/io/pin_nodes.tsv): for each of the
357 HVIO and IO96 balls, the graph node where fabric routing starts from the
pin (roles `in0`, `in1`, `clock`, `direct`) or ends at it (`out`, `oe`,
`out2`). A router starts an input net and ends an output net there, with no
Quartus route to copy. `a5db pincheck <db> <graph> <design>…` checks the
table against design files extracted from Quartus images.

- **A pad's nodes** [obs]: every graph node at the pad's fitter location,
  `<type>:X<x>Y<y>Z<sub_loc>I<i>`, where (x, y, sub_loc) is the pad's
  `X…Y…SUB_LOC…` from `ddb pins` (now columns of `io/pins.tsv`). The input
  buffer (`IO_INPUT_BUFFER_OUTPUT`) and output buffer (`IO_OUTPUT_BUFFER_INPUT`
  I0 data, I1 OE, I2 a third input on HVIO) are among those nodes or one hop
  from them.
- **The rule** [obs on the designs below]: from the buffer, follow every edge
  the router doesn't set (forward for inputs, backward for outputs), and stop
  at the first node with an edge the router does set: a mux in a routing tile
  (`cw_rt_*`) or the clock network (`cw_cc_*`, or a clock rule of (k)). Edges
  inside I/O blocks have no edge API (the pin template of (p) sets those
  muxes), and the redundancy muxes (`REDMUX`, `cw_rd_*`, no bit map) pass
  their data input when idle, so the search goes through both. The search
  ends 27–83 hops from the buffer. Then:
  - HVIO input: two `REDMUX` nodes, one through each input register 4:1 mux
    (`IOGPIO_REG_4TO1_MUX_RE` at Z = sub_loc − 15 for mux 0, − 14 for mux 1).
    Pin use `in` (mux 1, as (p)) leaves through the second. The REDMUX feeds
    one `HIOB` through the HIOL's `i_4mux1`, which has no `sel` bits, so it
    is a pass-through.
  - HVIO clock: the `HVIOWR_RE` clock output reached through mux 0, the
    bank's clock-or-data demux and the ioclk 2:1 or ss clock 4:1 mux (r).
    Two pins share each such node, which is the phase-5 limit of two clock
    pins per bank half seen from the graph side.
  - IO96 input: `in0` (the usual `in`, mux 0) leaves through the fabric
    adapter's flyby path into the VIO tile's `VIO_TO_CORE` mux, `in1` through
    the IOGPIO 4:1 mux and the adapter's 2:1 mux, `clock` through the
    adapter's `P2C_FRZ` into the DCM; `direct` (adapter 2:1 mux only) is never
    used by Quartus here [unk].
  - Outputs: the `RPI` (programmable inverter in the HIOL or VIO routing
    tile) that leads to the buffer's data input. The OE input has its own RPI
    but no Quartus design routes it: the template ties OE for output pins.
  - `pio_1_0` (top IO96 bank, no DE25-Nano pins) goes through
    `SOC_MPFE_WRAPPER_RE` and is in the table but unchecked [inf].
- **Checked** [obs] (`pincheck`): a pin "uses" a node if the design file's
  first (last) routing edge of the pin's net leaves (enters) it, allowing up
  to 3 non-wire hops for edges a design file can't list (RPI inputs and the
  HIOL 4:1 data mux are pass-throughs; MUXIN settings equal to the idle state
  aren't decoded). The rule was worked out on ph1 alone; 429 of 432 pin
  uses in 14 designs match, and the 3 misses are explained.

  | design | pin uses | at the predicted node |
  |---|---|---|
  | ph1 (development) | 167 | 167 |
  | ph2 | 167 | 166: `DRAM_RAS_n` is an unloaded input (p), no route |
  | ck1 | 23 (16 HVIO clocks, IO96 clock, IO registers) | 23 |
  | ck2 | 18 | 18 |
  | ar, mm, hx | 9, 5, 10 | all |
  | pl | 4 | 3: V16 is the IOPLL's reference and never enters the clock network (r) |
  | pt, ho, blinky, gc, rt (extracted for this) | 27 | 27 |
  | idle | 2 | 1: its CLOCK2_50 drives one flop and Quartus routed it as data (`in1`, which it uses), not over the clock network the `clock_in` label assumed |

  `in_reg` (an HVIO input register) leaves through mux 1 like `in`; an output
  register (`out_reg`) uses the `out` node. In ar one data input also feeds
  the clock network: Quartus used its clock node as well as its `in1` node.
- **Not covered**: `direct` and `out2` are unexplained; `oe` is unchecked
  (no design drives an OE); IO96 lanes without DE25-Nano pins follow the same
  search but no design uses them [inf].

### (u) Phase 6: delays for routing

[`db/sm4revb/timing/delays.tsv`](db/sm4revb/timing/delays.tsv) is a delay
model a router can use as node costs: the mean delay into each routing node
type, and per cell the LUT input-to-output, arithmetic, clock-to-out and
register-input delays. It is fitted to quartus_sta's own timing paths, not
read from the device database.

- **quartus_sta runs** [obs] once `libbrotlidec` is on `LD_LIBRARY_PATH`
  (nixpkgs `brotli.lib`): `tools/timing/sta.sh <run> <out>` copies a
  compiled run and dumps `report_timing -setup -detail full_path
  -show_routing` (slow model, up to 4,000 paths per clock). That is the
  workaround for "quartus_sta fails under chipsmith" below until chipsmith
  carries brotli. Every path lists each routing hop by fitter node name
  (DYGR name after the (k) renaming) with its incremental delay, and each
  cell's input port, output port and delay.
- **Where Quartus keeps the model** [obs, not decoded]: `DYGR_ROUTE_TIMING_INFO`
  in `qcore/linux64/libddb_dygr.so` holds per-node driver data
  (`set_driver_data(unsigned short, float*, float*, int, int, int*,
  DEV_ROUTING_SWITCH_ENUM*)`) and loads (`set_load`), loaded from
  `devices/10nm/sm4revb/ddb_sm4revb_timing_routing_delays_*` (one file per
  process/voltage/temperature corner, plus crosstalk and DSPF RC models in
  `*_routing_models_*.mdb`). It is an RC model with per-load terms and
  `dump_timing_nodes()` exists, but turning it into delays needs the
  evaluation code too [unk]. The path-based model below was the tractable
  route.
- **The model** [obs] (`tools/timing/paths.py` parses, `fit.py fit` fits):
  a hop's class is its destination node type (`re:H4`, `re:LEIM`, …) or,
  for cells, `lut:<input>` (a..f → combout), `arith:<input>>sumout|cout`,
  `arith:cin>cout|sumout` (along a carry chain), `tco`, `reg_d`, `tsu`.
  The value is the class mean over rt and ar (5,443 paths). Some numbers,
  slow 0 °C corner: tco 0.320 ns; LUT input A 0.209, B 0.183, C 0.150,
  D 0.130, E 0.068, F 0.043; carry cin→cout 0.059, cin→sumout 0.169; H0
  0.077, H1 0.065, H2 0.067, H4 0.101, H6 0.110, H12 0.135, V1 0.128, V4
  0.120, V8 0.124, LL 0.054, LEIM 0.050 per hop.
- **Checked against quartus_sta** [obs] (`fit.py eval`): each path's hops
  summed under the model against quartus_sta's data delay (same route,
  Quartus' placement):

  | design | paths | mean error | median abs error | 95th percentile |
  |---|---|---|---|---|
  | rt (fit) | 1,443 | −0.9 % | 2.7 % | 7.3 % |
  | ar (fit) | 4,000 | +0.5 % | 7.0 % | 19.2 % |
  | hx (held out) | 2,817 | +2.4 % | 5.2 % | 11.7 % |
  | mm (held out) | 1,164 | −2.2 % | 4.8 % | 10.5 % |
  | ck1 (held out) | 53 | +2.2 % | 3.8 % | 16.2 % |
  | blinky (held out) | 405 | −0.1 % | 11.9 % | 22.0 % |

  M20K and DSP cells (`MEDIUM_EAB_RE`, `MP_MAC_RE`) and IO registers are
  not modelled; those hops use quartus_sta's own delay in the check.
  Per-hop spread is real (an H4 hop runs 0.073–0.168 ns with its load and
  position), so the model is for costs, not sign-off. The biggest residual
  is a carry chain's data input to cout (0.17–0.59 ns, depending on the
  ALM) [unk].
- a5pnr (w) uses the `re:` means as PathFinder base costs, and after
  routing prints the model's longest path into a flop (driver tco or 0 at a
  pin, each routed hop's mean, LUT and carry arcs, `reg_d`, `tsu` 0.084 ns):
  blinky 2.44 ns, leds 3.51, acc 5.75, ram 2.85 [inf: model estimates; no
  clock skew, no hold]. For scale, quartus_sta gives Quartus' own 25-bit
  counter blinky a worst data delay of 1.37 ns. Placement is not
  timing-driven.

### (v) Phase 6: IOPLL settings from frequencies

`python3 -I tools/pll/iopll.py <ref ball> <ref MHz> <out MHz>[,…]` writes an
IOPLL's a5db design-file lines for any reference and output frequencies:
the template every PLL here shares
([`db/sm4revb/io/iopll.tsv`](db/sm4revb/io/iopll.tsv): 38 attributes, 5
DIRECT values, 12 named bits, all equal in the five PLLs compiled) plus the
counters, reference select, output enables and charge-pump trims below.
`tools/pll/check.sh` builds them over the idle image and compares every
named bit under `xpllwrap_top` with a Quartus image.

- **Frequencies → N, M, C** (`tools/pll/compute.py`) [obs]: the
  altera_iopll IP's own choice, learnt from a sweep that generates the IP
  for many frequency sets with no fit (`tools/pll/ipsweep.py`: ip-deploy
  and qsys-generate, then the `tennm_iopll` atom parameters; about 5 min per
  set on this machine). The rule: the VCO candidates are the multiples of
  the outputs' least common multiple from 3,200 MHz down to 600 MHz; for
  each, the smallest N whose PFD frequency (reference / N) is at least
  10 MHz and makes M = VCO / PFD an integer; the first candidate with such
  an N wins, and C_i = VCO / out_i. For example 50 → 120 MHz gives VCO
  3,120 MHz, N = 5, M = 312, C = 26 (3,000 MHz with N = 1 would also work,
  but a higher VCO wins), and 50 → 13.5 MHz gives 3,037.5 MHz with N = 4.
  - Training: 32 frequency sets (references 10–156.25 MHz, outputs 10 MHz
    to 1 GHz, up to four outputs). 31 match; the other, 1 MHz, has no exact
    solution, and the IP falls back to the nearest it can reach (600 MHz /
    510).
  - **Held out**: predictions written to `tools/pll/predictions.tsv` and
    committed before the IPs were generated, for 6 sweep sets (50 → 125,
    180, 48, 65, 16 and 200/100/50/25 MHz) and pb's four PLLs (50 → 64/160,
    100/12.5, 270; 25 → 40/80): **10 of 10 match** the IP's N, M and C. A
    seventh set, 50 → 1,200 MHz, is refused by ip-deploy itself, so there
    is an output-frequency ceiling the rule doesn't model [unk].
- **Counters → bits** [obs on pl and pa, five PLLs]: across the five, only
  these PLL settings differ:
  - C_k: `xccnt_iopll<k>` hi = ⌈C/2⌉ and lo = ⌊C/2⌋ (8-bit DIRECT), and
    `dprio_c_m_cnt_even_duty_en` = ENABLED for odd C (C = 105 → 53 / 52);
    an unused C1 is 255 / 255; counters 2–6 stay bypassed unless used;
  - M: `gdiv/dprio_c_m_cnt_hi_div_user`, 9 bits, is M itself (63, 64, 125,
    312);
  - N: bypassed for N = 1; otherwise `pll_n_cnt_bypass_en` DIV_ENABLED, hi
    and lo as for C, odd duty for odd N (N = 5 → 3 / 2);
  - `xpllcoutbuf<k>/cnt_out_en` for each used output above 0;
  - the reference mux `hviowr_pllwrap_clk_mux_sel` = PLL_CLKMUX_IOCLKIN_(2 + k)
    for the bank-half 1 reference pad `HVIO_B1_2_<k>` (CE19 and V16 → 3, BF23
    and AE11 → 2); bank-half 0 pads are [unk];
  - charge-pump and lock-filter trims follow the PFD frequency [inf, three
    values seen]: 50 MHz → `cpi_trim` 00110, `cpp_trim` 01001,
    `cr_lockf_early` 1000; 25 MHz → 00011, 00011, 1000; 10 MHz → 00010,
    00010, 1011. Other PFD frequencies are [unk] and iopll.py refuses them.
- **Check** (`tools/pll/check.sh`): pa (training, four PLLs: 50 → 75/30,
  120, 33.33/200; 25 → 125): 2,776 of 2,776 named bits equal Quartus'.
  **pb (held out**: compiled after the rule and template were fixed; four
  PLLs: 50 → 64/160, 50 → 100/12.5, 50 → 270 with N = 5 and M = 297,
  25 → 40/80 with M = 128): **2,776 of 2,776 named bits equal Quartus'**,
  the PFD-based trims included (10 MHz with a different M than training,
  25 MHz likewise).
- The PLL's input pin and output routing are not in these lines: the
  reference pin needs its pin bits, and each output enters the clock
  network as a graph edge (r).

### (w) Phase 6: an open flow (yosys, a5pnr, a5db)

`tools/a5pnr/flow.sh <top> <pins.txt> <out> <design.v>` takes Verilog to a
`.rbf` with no Quartus image or route anywhere: yosys from nixpkgs
(`synth.sh`), `tools/a5pnr` (Rust: packing, placement, routing; writes an
a5db design file), `a5db build` and `a5db rbf`. `tools/a5pnr/check.sh` runs
every check below on the result.

- **Synthesis** (`synth.sh`, `yosys/arith_map.v`, `yosys/cells.v`): yosys'
  generic flow down to `$lut` (≤ 6 inputs), plain `$_DFF_P_` flops (enables
  and synchronous resets become LUT logic; flops power up at 0), and `$alu`
  mapped to `A5_ARITH` carry chains with tennm_lcell_comb's arithmetic
  semantics. A chain starts with a cell whose carry out is the carry in
  (p = 0, g = CI). `A5_M20K` is instantiated by hand.
- **The ALM's arithmetic mode** [obs]: Quartus' own simulation model
  (`quartus/eda/fv_lib/verilog/tennm/tennm_atoms.sv`, `tennm_lcell_comb`)
  defines it: p = mask[15:0] and g = mask[31:16] over {D, C, B, A},
  sum = cin ^ p, cout = p ? cin : g. From ar's fitted atoms (quartus_fit
  ECO, `tools/alm/atoms.tcl`), its stored masks and quartus_sta's port-to-pin
  paths (`tools/timing/pins.py`), on all 87 arithmetic half-ALMs:
  - an ALM holds two arithmetic halves; half h's p is stored bits
    32h .. 32h + 15 and g 32h + 16 .. 32h + 31 of the DIRECT `lut_mask`, as
    the WYSIWYG masks are (no inversion, unlike the 6-LUT's
    `stored[j] = ¬W[j ^ 31]`); each cell's p depends on exactly the inputs its
    ports connect, and is XOR, g AND, in every adder ALM;
  - inputs A and B are ALM pins 3 and 4 (shared), C and D are pins 2 and 1
    for half 0 and 5 and 6 for half 1;
  - the chain runs N0 → N3 → … → N57 through a LAB (two halves per ALM, 20
    per LAB) and on to N0 of the LAB one row down (Y − 1); cin and cout are
    LAB_RE 153 and 154;
  - half h's flops are `xcw_la_le_reg{h}`, xreg0 driving LAB_RE
    89 + 4k + 2h and xreg1 90 + 4k + 2h; a sum flop keeps `reg_input` at its
    idle ARITH_SUMOUT;
  - Quartus sets `secmux_bto = DISABLE_BTO` and `hipi_clken = COMB_PATH` in
    the LAB where a chain starts (ar, ho); what they select is [inf], and
    a5pnr copies them.
- **a5pnr** packs a 6-LUT and its flop into ALM half 0 (combinational
  output through `reg0/xreg1` in WIRE_MODE, as phase 4's blinky did), or two
  arithmetic halves into an ALM; a lone flop gets a pass-through LUT. It
  places chains at ALM 0 of a LAB column and the other ALMs by simulated
  annealing on half-perimeter wire length, and routes with PathFinder
  (A* from the net's tree to each sink, one net per node, present and
  history congestion costs). Node costs are the delay model's means (u).
  Pins start and end nets at (t)'s nodes. One clock domain; the clock net
  stays on the clock network.
- **The router's edges** (`a5db usable`, one byte per graph edge in
  `specimens/rg/full/usable.bin`): routing-tile and clock-network muxes that
  resolve to bits (21,093,894 and 48,968), pass-throughs and hardwired edges
  between fabric node types (938,661 plus the DCM's hardwired clock paths).
  I/O block internals and redundancy muxes are never routed through.
- **Checks** [obs] (`check.sh`), each independent of a5pnr's intent:
  1. `a5db extract` decodes the image back into a design file; it must hold
     exactly a5pnr's pins, edges, attributes and DIRECT values, and nothing
     else (no `bit` or `raw` lines);
  2. `a5pnr readback` rebuilds a netlist from that decode alone: every sink
     traced back over the decoded mux edges and the graph's pass-through
     edges (a node reached from two sources is an error), every ALM from its
     decoded mode, masks and register settings. `simcheck.py` simulates it
     against the RTL in iverilog: 2,000 cycles each starting from a fresh
     random register state (the same on both sides, biased towards all ones
     and all zeros so long carries happen), then 2,000 free-running cycles,
     comparing every output and every register. Mutating one mask bit, one
     flop's input or one routing edge makes it fail (400 to 6,000
     mismatches on blinky);
  3. `tools/readback.sh`: Quartus' assembler (asm2) reads every attribute of
     every instance that differs from idle, against a5db's decode;
  4. `quartus_pfg -c` on the `.sof` (its `.rbf` must equal a5db's),
     `quartus_pfg --check_integrity` and `bitstream.py check` on a5db's.

Results (`designs/p6`, pins in `designs/p6/*.pins`):

| design | cells | ALMs (arith) / LABs | M20K | nets | edges | route iterations | a5pnr time | extract = design | simcheck | asm2 | pfg |
|---|---|---|---|---|---|---|---|---|---|---|---|
| blinky (25-bit counter → LED0) | 26 A5_ARITH, 25 FF | 13 (13) / 2 | 0 | 26 | 108 | 1 | 20 s | yes | 0 mismatches, 25 registers | 0 of 25,821 differ (18 instances) | `.rbf` identical, integrity OK |
| leds (27-bit counter, KEY0 clear, KEY1 invert, SW0 reverse → LED[7:0]) | 35 LUT, 28 A5_ARITH, 27 FF | 49 (14) / 28 | 0 | 66 | 700 | 2 | 35 s | yes | 0, 27 registers | 0 of 102,056 (152) | identical, OK |
| acc (56-bit accumulator + 32-bit LFSR, chain over 3 LABs) | 58 LUT, 57 A5_ARITH, 88 FF | 117 (29) / 30 | 0 | 147 | 1,287 | 3 | 36 s | yes | 0, 87 registers | 0 of 86,320 (130) | identical, OK |
| ram (512 x 32 M20K, counter writes, slow read → LED[7:0]) | 33 A5_ARITH, 32 FF, 1 A5_M20K | 17 (17) / 2 | 1 | 41 | 391 | 3 | 29 s | yes | 0, 32 registers | 0 of 46,723 (51) | identical, OK |

"cells" are yosys' after mapping, "edges" the configured routing edges in
the design file (pass-throughs not counted), "a5pnr time" includes about
18 s of loading the graph. The other steps, on the shared machine (load
20–40): synth 12–38 s, `a5db build` 190–276 s, `a5db rbf` 4–9 s; `check.sh`
about 45 min, most of it `a5db extract` and asm2.

- The M20K [obs, mm's quartus_sta paths, `report_path` for the read side]:
  in its 512 x 32 simple dual-port use Quartus splits the RAM into 32 one-bit
  `ram_block2a<k>` atoms in one block, and the block's `MEDIUM_EAB_RE` pins
  are: clock 77, write address bit i 25 + i, read address bit i 52 − i,
  write data bit k 57 + k (k < 20) or 40 − k, read data bit k 83 + k. The
  block's settings are the 20 `xdigicore` delay and enable settings mm's
  such block differs from idle in; widths and modes stay idle. Write enable
  is tied high (mm writes every cycle); a5pnr refuses any other. The clock
  enters through an RPI from the row clock, as a LAB's does.

### (x) Phase 7: a5db fast enough to iterate

Phase 6's `a5db build` took 190–276 s and `check.sh` about 45 min, so every
change to a design cost the better part of an hour to check. Profiled with
`perf` on acc (`specimens/p7/prof`) [obs]:

- **build**: 245 s, of which the graph load was 10 s. The rest was the idle
  image: `BlockMap::bit_names` sorted every instance's names on each call,
  and every bit position was two SipHash lookups of `String` keys (io96's
  318 MB layout alone is 5 M rows). The bit maps now keep each layout's rows
  sorted once, index names with FxHash over slices of the (leaked) table
  text, and hand out `(name, position)` per instance without lookups; the
  graph and the database load on two threads; a design's few thousand node
  names are found in one pass instead of indexing all 7.6 M; attribute and
  type lookups are cached. **245 s → 38–41 s**, the `.sof` byte-identical.
  What is left: 17–24 s of loading (the graph's 630 MB of text, io96's xz),
  7–10 s for the idle image.
- **extract**: 313 s → **58 s**, the design file identical. It decoded all
  26.3 M graph edges and hashed every named bit of every instance into a
  `HashSet`. Now it first finds the instances whose bits differ from the
  idle image (130 for acc) and decodes only their muxes and attributes;
  that is exact because the idle image selects no edge (every extract of a
  design lists only its own edges).
- **check.sh**: about 45 min → **3.5 min** on acc, every result identical
  (now with asm2 and pfg running beside extract and the simulation: 2–3 min
  for the small designs).
  `tools/readback.sh` ran `a5db attrs` once per instance (130 database loads
  for acc); `a5db attrs-of` does them in one run. asm2's own part is about
  80 s for 86 K attributes, not the bottleneck it looked like.
- **a5pnr on a CPU** (2,628 ALMs, 3,301 nets; README (z2)): the first
  timing-driven run routed in 372 s. Three changes, each measured on that
  design: an A* bound from the fastest wire per column and row (twice the
  lower bound) instead of 0.006 ns per unit; searching a net's bounding box
  plus 3 columns and rows first (VPR's `bb_factor`; unbounded only on a
  miss); and seeding a sink's search with the tree's nodes near it, not
  the whole tree. With rerouting only the nets on an overused node after
  the first iteration: **372 s → 149–192 s**, same iteration count (7–9).
  Placement (two anneals, wire length then timing-driven) takes 120–145 s.
- **Phase 7's flow and checks** [obs]: a small design is 25–50 s of a5pnr,
  25–50 s of `a5db build`, 3–9 s of `a5db rbf` and 2–3 min of checks; the
  CPU is 7.5 min of flow (synth 30 s, a5pnr 358 s, build 51 s, rbf 9 s) and
  17 min of checks (extract 2.3 min, asm2 5 min beside it, simulation of
  20,600 cycles 15 min).

### (y) Phase 7: what real designs need from the flow

Two Quartus designs were compiled for this (`designs/ct`: two clock
enables, a synchronous clear, an active-high and an active-low
asynchronous clear, both priorities of enable and clear, a second clock, a
carry-in from a pin, a subtraction, I/O registers; `designs/mq`: an M20K
with contents, write and byte enables, another with a read enable and an
output register), extracted with `a5db extract` (no `bit` or `raw` lines)
and read with `a5pnr drivers` (which source reaches each node) and
quartus_sta's port-named paths (`tools/timing/sta.sh`).

- **LAB control lines** [obs, ct]: LAB_RE 80 and 81 are the enable lines
  CLKEN0 and CLKEN1 (an ALM picks one with
  `xcw_la_le_regctrl/xce_la_le_regctrl_mux/regclk_ens`), 82 the
  synchronous clear, 83 LAB clock 0, 86 the clear `regclr` CLR1 selects
  (its idle setting); CLR2 is LAB_RE 85 [inf: the other line], which
  Quartus leaves unrouted for every flop without a clear. 86 comes from the
  routing tile's `iss/xclrb` mux through the inverter `iss/xclrb_rpi`: an
  active-low clear is `iss/xclrb_rpi/sel = INV` instead of the edge's
  non-inverting setting (RPII I5 → RPI I5). Per flop, `xreg0` has `ce_sel`
  and `sclr_sel` (opt in to the ALM's enable line and the LAB's clear);
  `xreg1` has neither in the BCM, and Quartus puts no enabled or cleared
  flop there. Clock and clear are per half (`xcw_la_le_reg{h}`).
- **Enable over synchronous clear** [obs, ct]: `if (CE) q <= SCLR ? 0 : d`
  maps onto the lines directly (r7: CEA on 80, SCLR on 82); for
  `if (SCLR) q <= 0; else if (CE) q <= d` Quartus routes a LUT's CE | SCLR
  to the enable line. So the enable gates the clear (yosys
  `$_SDFFCE_`).
- **Two clock domains** [obs, ct]: each LAB on one domain, both on LAB
  clock 0 (83, row clock → GLOBAL_LOCAL_SEL I2); Quartus didn't use clock
  1 (84), and a5pnr allows one clock per LAB.
- **LEIM flops** [obs, hx and mm's quartus_sta paths]: a flop with
  `reg_input = LEIM` takes D from the ALM's input A (LAB_RE 8k + 3) in
  half 0, B (8k + 4) in half 1. A flop with no logic of its own shares an
  ALM with unrelated logic this way.
- **Where a carry chain starts** [obs, 13 LABs of ar, hx, ct, mq]:
  `secmux_bto = DISABLE_BTO` is set exactly in the LABs where a chain runs
  from ALM 4 into ALM 5 (top half into bottom half), and `hipi_clken =
  COMB_PATH` where a chain leaves ALM 9 for the LAB below (ar, 1 LAB). A
  chain-start half Quartus makes for a carry-in has p = ¬D, g = D, so
  cout = D | cin: it relies on the hardware carry into a chain's first
  half being 0 [inf: chains start at ALM 0 or 5 with the settings idle].
  Phase 6 set both settings only in the LAB where a chain starts, so its
  ram (17 arithmetic ALMs: the second LAB runs ALM 4 into 5) and acc (the
  middle of three LABs) images lack them where needed and are probably
  wrong on silicon [inf]; phase 7 rebuilds them.
- **M20K pins** [obs, mq's quartus_sta paths]: write enable 37
  (`portawe`), read enable 39, byte enables 55, 56, 22, 21 for bytes 0–3
  (`portabyteenamasks`). With byte enables Quartus lays the 32 data bits in
  four 10-bit lanes: bit k at position j = 10 (k / 8) + k % 8, written at
  57 + j (j < 20) or 40 − j and read at 83 + j (mq's block A reads 83–90,
  93–100, 103–110, 113–120); without, contiguously (phase 6). An unrouted
  enable is high. The output register is on when both `m20k_*_outreg_byp`
  stay idle (mq's block B); a ROM's write enable is the unrouted pin through
  `xsecmux/xwe0/m20k_invsel = INV_LEIM_SEL` (mm).
- **M20K contents** [obs, mq: 512 of 512 words]: `m20k_eram_add<a>` is word
  a in the lane layout, MSB first.
- **M20K contents in the `.rbf`** [obs]: what README (g) called a deflated
  all-zero block in each sector's CORE stream is the sector's M20K
  contents: per stored row group (0, 2, …, 47) and each of the sector's
  four M20K columns a record `(8, w, 128, 2)`, four zero bytes (w = 12,
  16, 20, 25) and 2,560 bytes holding address a's 40-bit word at bits
  40 (511 − a) onwards, LSB first; its pages go in the frame pages'
  interleave, and the header byte sum at +0x78 covers the contents too.
  **a5db's `.rbf` dropped M20K contents until now** (every phase-4 to -6
  specimen pair had none). Now byte-identical to pfg on mq (used to work
  it out), blinky, and held-out mm (1024 x 16 ROM) and hx (2048 x 10 true
  dual port with contents).
- **Edges a router must not take** [obs]: 896 graph edges (416 fabric,
  480 clock) select their mux's idle setting. They write no bit, so an
  image can't show them, and whether an unconfigured mux passes that
  input is [unk]: rom's first route took one (a vertical seam tap) and its
  clock never reached the LABs in the readback. `a5db usable` now gives
  them class 4, which a5pnr doesn't route over. And 18,000 nodes outside
  LABs have more than one pass-through fanin (`a5pnr ambiguous`: 16,640
  `HIPI_M_MUX`, 1,128 `SCLKMUX_RE`, …): which one drives them is a
  setting nobody knows [unk], so the router doesn't enter them over a
  pass-through, and the readback treats a node two sources reach as
  unknown (an error only if a used pin depends on it).

- **a5pnr now** (`tools/a5pnr`): flops with an enable, a synchronous clear
  (gated by the enable) and an asynchronous clear of either polarity map onto
  the LAB lines (`synth.sh` legalises to `$_DFFE_`, `$_SDFF_`,
  `$_SDFFCE_`, `$_DFF_P?0_`, `$_DFFE_P?0P_`); every LAB is checked for one
  clock, two enables, one synchronous and one asynchronous clear. An ALM
  is its logic plus four register slots: a LUT's flop and its
  combinational output, two arithmetic halves with their sum flops, and
  any free slot takes a flop through LEIM. Flops with an enable go into
  flop-only ALMs grouped by enable (scattered over logic ALMs, a register
  file's 31 enables left the CPU at four ALMs per LAB). Chains start at ALM
  0 or 5, with no start cell when the carry-in is 0, and get the two LAB
  settings where they cross. M20Ks take WE, RE, byte enables, the output
  register and contents (`A5_M20K #(.INIT, .OUTREG)`). `A5_IOPLL
  #(.REF_KHZ, .OUT_KHZ)` gives a PLL clock: V16's PLL, output 0, with
  `tools/pll/iopll.py`'s lines in the design file. The annealer charges
  1.5 wire-length units per LAB in use.
- **Packing** [obs]: phase 6's leds took 49 ALMs in 28 LABs; it now takes
  24 ALMs in 4 LABs (its clear became the LAB's synchronous clear, its
  LUTs went into LEIM slots), acc 117 ALMs in 30 LABs → 31 in 6.
- **Not done** [unk]: I/O registers from a5pnr (ct: the register's clock
  enters through the HIOL's `GLOBAL_LOCAL_SEL` I5 → RPII/RPI I29 into a
  `REDMUX` no router edge reaches, so the pin map needs an IO-register
  clock role; and Quartus sets the HIOL data inverters `irpi1`/`irpi2` to
  INV around both registers, which isn't understood); MLAB RAM (yosys's
  generic flow never maps to it; it needs a `memory_libmap` library, the
  MLAB's `ram_mode` attributes (ar, hx) and a readback model); LAB clock 1
  and the CLR2 line carrying a signal (never seen used); PLLs other than
  V16's output 0 (pa shows the others' output nodes, I34, I46/47, I58/59).

Every design through `tools/a5pnr/suite.sh` (flow and all four checks;
`specimens/p7/s2`, pins in `designs/p6`, `designs/p7`):

| design | ALMs / LABs | M20K | route iterations | model Fmax | a5pnr / build / rbf | extract = design | simcheck | asm2 differ | pfg |
|---|---|---|---|---|---|---|---|---|---|
| blinky (p6) | 13 / 2 | 0 | 1 | 393 MHz | 34 / 39 / 4 s | yes | 0 mismatches | 0 | identical, OK |
| leds (p6) | 24 / 4 | 0 | 3 | 313 MHz | 34 / 37 / 5 s | yes | 0 | 0 | identical, OK |
| acc (p6) | 31 / 6 | 0 | 6 | 219 MHz | 30 / 39 / 5 s | yes | 0 | 0 | identical, OK |
| ram (p6) | 16 / 3 | 1 | 4 | 342 MHz | 30 / 40 / 4 s | yes | 0 | 0 | identical, OK |
| ctl: enables, clears, two clocks | 45 / 9 | 0 | 4 | 397 MHz | 27 / 27 / 4 s | yes | 0 | 0 | identical, OK |
| arith: chain over halves and LABs, carry-in | 25 / 3 | 0 | 2 | 352 MHz | 27 / 27 / 4 s | yes | 0 | 0 | identical, OK |
| rom: M20K contents, WE held low | 17 / 2 | 1 | 3 | 345 MHz | 31 / 36 / 4 s | yes | 0 | 0 | identical, OK |
| ramx: WE, byte enables, RE, output register | 17 / 3 | 1 | 3 | 342 MHz | 31 / 37 / 3 s | yes | 0 | 0 | identical, OK |
| pll: 50 → 100 MHz IOPLL | 27 / 4 | 0 | 1 | 379 MHz | 25 / 25 / 3 s | yes (19 PLL `bit` lines) | 0 | 0 | identical, OK |

simcheck is 2,000 cycles from random register states plus 2,000
free-running (all clocks tick together; a PLL output follows its
reference). It now names each image flop by the RTL's own register (a
yosys net may carry a wire's name), writes RTL memories a word at a time,
keeps one-hot registers one-hot, and knows flops yosys keeps inverted
(`!name` in the `.ffmap`). The extract comparison ignores `bit` lines at
their idle value (iopll.py writes whole fields).

### (z) Phase 7: timing-driven placement and routing

- **STA** (`tools/a5pnr/src/sta.rs`): a timing graph of the packed design:
  launch at flop outputs (tco), input ports and M20K read data (1.43 ns);
  per connection a routed or estimated delay; LUT arcs by the input's
  position (A–F), arithmetic arcs, the carry chain; capture at flop D
  (`reg_d` + `tsu`), enables and synchronous clears, M20K inputs (setup
  0.25 ns [inf], not in the fit) and output ports. Arrival forward,
  required backward against the worst arrival, and per connection a slack
  and criticality. One clock domain at a time; no skew, no hold. The
  numbers are the model's (README (u)), not sign-off.
- **Delay estimates for placement**: a table of routed delay against
  (|dx|, |dy|) from one Dijkstra over the router's edges from a LAB near
  the middle (309,600 LAB input pins reached in 7–14 s): same LAB 0.109
  ns, one column 0.168, ten columns 0.408, ten rows 0.572, (20, 20) 1.035.
- **Placement**: a wire-length anneal, then STA on its estimates, then a
  second anneal from scratch on (1 − λ) wire length + λ Σ crit⁸ · delay
  (λ = 0.5, both terms normalised to their starting values, as VPR).
- **Routing**: PathFinder where a sink's node cost is crit · delay +
  (1 − crit) · (delay + history) · (1 + present · occupancy), the most
  critical sinks routed first (crit capped at 0.98).
- **Results** [obs, model]: the CPU from 96 MHz (wire-length placement,
  routed) to 124–128 MHz (timing-driven); small designs 219–397 MHz. Routed
  path delay per design is in the table above.
- **quartus_sta on an a5pnr result: out of reach in this phase.**
  quartus_sta needs Quartus' own compiled database, not a `.sof`; feeding it
  a5pnr's result would mean writing a5pnr's packed netlist as Quartus atoms
  (`tennm_lcell_comb`, `tennm_ff`, `tennm_ram_block` in a VQM), every
  placement as location assignments and every route as routing
  constraints, which Quartus Pro takes in a different form than Quartus
  II's `.rcf` [unk]. The model's accuracy on Quartus' own routes is (u)'s
  (median 3.8–11.9 % on held-out paths); on a5pnr's it is unmeasured.

### (z2) Phase 7: a RISC-V CPU from the open flow

`designs/p7/cpu`: Bruno Levy's FemtoRV32 Quark (RV32I, `femtorv32_quark.v`
from chipsmith's example, unmodified, BSD-3-Clause) with its program in an
M20K. `asm.py` assembles the 13-instruction program (hand-written encoders
for lui, addi, andi, lw, sw, bne, jal) into `prog.vh`, the M20K's `INIT`:
it counts in x2 and stores the count to the LEDs (0x400000), waiting 2^21
turns of a 6-cycle loop per count, or 3 turns while SW0 (read at
0x400004) is up. The ROM's read enable is the CPU's read strobe. KEY0
resets. In RTL simulation (`run.v`) the count steps every 44 cycles with
SW0 up; with SW0 down that is 12.6 M cycles, 0.25 s per count at 50 MHz.

- **Result** [obs] (`specimens/p7/cpu/run3`, `designs/p7/cpu/go.sh`):
  yosys 3,450 cells (2,036 LUTs, 1,229 flops, 176 carry halves, 1 M20K) →
  **2,628 ALMs (2,038 LUT, 89 arithmetic in 8 chains, 501 flop-only) in
  395 LABs, 1 M20K**, 3,301 nets, 43,295 configured edges, **8 routing
  iterations**, **model Fmax 128 MHz** (longest path into the register
  file, 7.8 ns). Times: synth 30 s, a5pnr 358 s (place 146 s, route
  192 s), `a5db build` 51 s, `a5db rbf` 9 s.
- **Checks** [obs]:
  1. `a5db extract` of the image: exactly a5pnr's 11 pins, 43,295 edges,
     23,418 attributes and 2,637 DIRECT values (512 of them the program),
     no `bit` or `raw` lines;
  2. the netlist rebuilt from that decode against the RTL: 300 cycles from
     random states and 300 free-running, 1,189 registers compared, **0
     mismatches**; and **20,000 cycles from power-up** (every register 0,
     KEY0 up, SW0 up) with the program running: the LEDs count 1, 2, 3, …
     (454 changes, last value 198), **0 mismatches**;
  3. asm2: 0 of 972,173 attributes in 1,694 instances differ;
  4. pfg's `.rbf` is byte-identical to a5db's (with the program in CORE's
     M20K records), integrity OK.
- **On the way** [obs]: iverilog 13 rejects femtorv32 (uses before
  declarations; its lexical-position name search), so simcheck pins
  iverilog 12; `opt_merge -share_all` merged identical carry-chain start
  cells into one shared by six chains, so it now merges LUTs only; yosys
  keeps one of the four one-hot state bits as an inverted flop
  (`dfflegalize` flip_bits) and derives the RTL bit from it.

### (z3) Phase 8: what an ALM computes, checked against Quartus' own image

Every check so far compared a5pnr's images with a5pnr's intent: a5db
extract, the readback, the simulation and asm2 all agree when writer and
reader share a wrong rule. Phase 8's oracle is Quartus' own: `designs/fr`
(`gen.py`: 192 registers, each the next state of a random 2- to 7-input
function, a shift register, async clears; Quartus Pro packs it into 233
ALMs, 31 LABs/MLABs, two LUTs per ALM where it can) is compiled by
Quartus, its image decoded by `a5db extract`, rebuilt by `a5pnr readback`
and simulated against `fr.v`. The fitted atoms (`tools/alm/atoms.tcl`, now
with the right key `LUTMASK_LUT_MASK`; it still returns no mask) say which
register sits where (`tools/alm/ffmap.py`) and which ports each LUT reads.
Specimens: `specimens/p8/alm/fr0` (Quartus run), `specimens/p8/ex/fr.txt`
(extract), `specimens/p8/oracle/fr` (readbacks, scripts).

- **The ALM's pins** [obs, fr's atoms, `tools/alm/halves.py`]: of the
  eight pins E0, D0, C0, A, B, C1, D1, E1 (offsets 0–7 of the ALM's
  LAB_RE pins), half 0's 5-LUT reads A, B, C0, D0, E0 and half 1's A, B,
  C1, D1, E1; a 6-input atom reads E on E0 and F on E1, and Quartus routes
  its C and D to both C0, C1 and D0, D1. Every atom's flop fanins land on
  exactly these pins.
- **What the stored mask computes** [obs: every LUT of fr between two
  registers, `tools/alm/lutrule.py`: 54 through `LUTOUT_5LUT` and 27
  through `LUTOUT_6LUT` agree, none differ]: with S the DIRECT `lut_mask` (S[j] is
  the j-th bit from the right of asm2's string) and idx = {E, D, C, B, A}
  of the half,
  - `LUTOUT_5LUT` of half h is **S[32h + idx]**, as stored;
  - `LUTOUT_6LUT` is **¬S[32·¬F + idx]** (F = E1, E = E0): both the half
    select and the output are inverted. Which half reads C0/D0 and which
    C1/D1 is [inf] (the low stored half C0/D0); Quartus routes both, and
    a5pnr now does too.

  **Phase 2's rule W[i] = ¬S[i ^ 31] (README (h)) was wrong for the
  function the silicon computes.** It came from ECO `-mask` values, i.e.
  Quartus' WYSIWYG port convention, and differs from the truth by
  complementing all six inputs: a 6-LUT written under it computes
  W(¬inputs). a5pnr's writer and readback used it from phase 6 on, so
  every check passed, but **every phase 6–7 image with a LUT in it
  computes the wrong function on silicon** (blinky, a pure carry chain,
  and the arithmetic halves are unaffected: their rule (w) came from
  Quartus' stored masks directly). Phase 4's hand-made scratch blinky
  (`designs/blinky_a5db`) has the same fault: its first counter bit never
  toggles. Both are rebuilt with the rule above.
- **The direct LUT outputs** [obs, the atoms behind 52 direct pins]:
  `lut5_powerdown = LUT5_OUT` drives LAB_RE 129 + k (ALM k < 5) or
  139 + k − 5 from the ALM's **half-1** 5-LUT, `lut6_powerdown = LUT6_OUT`
  the pin 5 above from the 6-LUT. Quartus uses them for LUT outputs read
  only by other LUTs, saving a register slot. The first readback guess
  (half 0) built 24 combinational loops out of fr; that was the OOM below.
- **The enable and synchronous clear serve the whole half** [obs, fr and
  cql]: `xreg0`'s `ce_sel` and `sclr_sel` are the only selects, and they
  gate `xreg1` too. In cql (designs/p8/cql, Quartus' fit of the CPU) all 391
  halves with an enabled flop in `xreg1` alone have `ce_sel = CE` on
  `xreg0` (empty or a wire), 10 of 10 likewise for `sclr_sel`, and the 255
  halves with flops in both slots have the same enable in both; fr's
  duplicated r[25] and r[38] keep their synchronous clear in `xreg1`.
  a5pnr used to put a plain flop beside an enabled or cleared one, which
  on silicon would have been gated too (acc, ctl, the CPU); now a half's
  two flops share their enable and clear, and two flops with the same
  enable may share a half, which is what lets a register file pack four
  bits to an ALM.
- **Register positions** [obs]: FF atom `N` = 6k + 3h + 1 + r is ALM k,
  half h, `xreg{r}` (338 of 348 fr flops agree on `reg_input`; the rest are
  pass-through LUTs the atom netlist doesn't show).
- **The spare row** [obs]: Quartus routes vertical wires across row 5,
  where the redundancy tiles sit (`*_rsp*`, `cw_rd_*`, README (s)), over
  graph edges with no bit map (`V1:X107Y5Z0I57 → I98`); a5db's join
  classes them NO. Each enters a node with no other fanin, so an
  unrepaired device has nothing to select there, and the readback now
  follows such edges (8 of fr's LUT inputs came out undriven without it).
  The router still doesn't take them.
- **Hyper-Retiming** [obs]: Quartus Pro retimed fr (`~ERTM` flops: state
  moved across logic, not the named register) and made `lfsr`'s power-up
  1 its own way. So the oracle runs from power-up, the RTL at its declared
  initial values and the image at its 0s, with the retimed flops out of
  the map.
- **Result** [obs] (`tools/alm/oracle.sh specimens/p8/alm/fr0 fr
  designs/fr/fr.pins <out> 5000 designs/fr/fr.v`, `SIMCHECK_HOLD=KEY0=1`):
  the netlist rebuilt from Quartus' fr image runs like
  `fr.v` for **5,000 cycles from power-up, 337 registers and 4 outputs
  compared, 0 mismatches**, and 1,000 cycles with KEY0 held (the
  asynchronous clear) likewise; under phase 2's rule it was 627,863
  mismatches. This is the first check of the ALM model that a5pnr's own
  assumptions can't pass by agreeing with themselves.

**The OOM that killed the previous phase-8 run**: vvp reached 11 GB on fr's
readback. The rebuilt netlist had combinational loops (the wrong direct-
output guess above), so simulated time never advanced while vvp's event
queue grew. simcheck now runs iverilog and vvp in their own memory cgroup
(`tools/cap.sh`, `SIMCHECK_MEM`, default 8G) under a wall-clock limit
(`SIMCHECK_TIMEOUT`, 3,600 s) and reports which one hit; the same run now
dies at 4 GB in 90 s, alone. Two more simcheck fixes came out of the
oracle: a register Quartus duplicated gets the same random value in every
copy, and with 0 random cycles the power-up run leaves the RTL at its own
initial values.

### (z4) Phase 8: density, and Quartus timing a5pnr's placement

**Density** [obs]. The CPU (designs/p7/cpu: FemtoRV32 Quark with its
register file in flops, as a5pnr's yosys flow maps it), in ALMs, against
Quartus Pro's fit of the same RTL (`designs/p8/cql`, register file forced
into logic; `designs/p8/cq` lets Quartus choose, and it puts the register
file in two M20Ks):

| | ALMs / LABs | LUTs from yosys | model Fmax | all four checks |
|---|---|---|---|---|
| phase 7 (one LUT per ALM) | 2,628 / 395 | 2,036 | 128 MHz | yes |
| two 5-LUTs per ALM, half's flops share enable and clear (r3) | 1,557 / 228 | 2,036 | 121 MHz | yes |
| + 4:1 muxes as 6-LUTs (`muxcover`), chains from ALM 0 (r6) | **1,393 / 215** | 1,392 | 120 MHz | yes |
| Quartus Pro, register file in flops (cql) | 1,082 | (1,171 LUT + 195 arithmetic cells) | 300.7 MHz (quartus_sta) | |
| Quartus Pro, its own choice (cq) | 334 + 2 M20K | | | |

- **Two LUTs per ALM**: a5pnr pairs LUTs of up to five inputs that fit
  one ALM's pins (A and B shared, each half's C, D, E its own; README
  (z3)), the pair sharing the most inputs or a connection first. The
  register file's 992 enabled flops now go four to an ALM (two through
  LEIM, two through pass-through 5-LUTs), since a half's two flops may
  share an enable (z3): flop-only ALMs 501 → 251.
- **Synthesis was the other half of the gap**: yosys' `abc -lut 6` built
  each 32:1 register-file read mux from 2:1 muxes first (1,024 3-LUTs, 21
  LUTs per bit and port instead of 11). `muxcover -mux4 -nodecode` before
  abc, with each `$_MUX4_` a 6-LUT (`yosys/mux4_map.v`), takes the CPU
  from 2,036 to 1,395 LUTs; abc's area scripts (`dch`, `if -a`, `mfs2`)
  change it by < 1 % either way [obs, /tmp bench of 6 variants].
- What is left (1,393 vs 1,082): Quartus' LUT count is still lower
  (extended 7-input LUTs, `shared_arith`, its own synthesis), and a5pnr
  doesn't yet put a LUT with its unrelated flops in one ALM half when the
  flop needs the half's LEIM pin.
- Every design through `tools/a5pnr/suite.sh` (`specimens/p8/s4`; flow
  and all four checks, current code):

  | design | ALMs / LABs (phase 7) | M20K | route iterations | model Fmax | extract = design | simcheck | asm2 differ | pfg |
  |---|---|---|---|---|---|---|---|---|
  | blinky | 13 / 2 (13) | 0 | 1 | 393 MHz | yes | 0 mismatches | 0 | identical, OK |
  | leds | 19 / 2 (24) | 0 | 5 | 306 MHz | yes | 0 | 0 | identical, OK |
  | acc | 37 / 6 (31) | 0 | 5 | 193 MHz | yes | 0 | 0 | identical, OK |
  | ram | 16 / 2 (16) | 1 | 3 | 338 MHz | yes | 0 | 0 | identical, OK |
  | ctl | 39 / 8 (45) | 0 | 4 | 389 MHz | yes | 0 | 0 | identical, OK |
  | arith | 23 / 4 (25) | 0 | 3 | 352 MHz | yes | 0 | 0 | identical, OK |
  | rom | 17 / 2 (17) | 1 | 3 | 345 MHz | yes | 0 | 0 | identical, OK |
  | ramx | 17 / 3 (17) | 1 | 4 | 338 MHz | yes | 0 | 0 | identical, OK |
  | pll | 27 / 4 (27) | 0 | 1 | 375 MHz | yes | 0 | 0 | identical, OK |
  | fr (Quartus: 233 ALMs) | 505 / 75 | 0 | 12 | 179 MHz | yes | 0 | 0 | identical, OK |

  acc grew because its accumulator's flops no longer share a half with a
  flop the LAB's clear or an enable doesn't apply to (z3). fr is random
  2- to 7-input functions: yosys makes 879 LUTs of them, and `muxcover`
  would make 1,458 (848 ALMs), so synth.sh maps both ways and keeps the
  smaller (the CPU takes muxcover, every other design abc alone).

**Quartus timing a5pnr's placement** (`a5pnr --quartus <dir>`,
`src/export.rs`). Quartus Pro takes no routing constraints: its ECO Tcl
reads a route (`eco get_info routing_path <iterm>` lists it as routing
GIDs, and GID − 0x80000000 is the node's line in
`specimens/rg/full/nodes.tsv`: exactly our graph's names [obs]), but
`eco reroute <iterm>` ignores any path given and just rips the route up,
and the other editing commands answer "Unsupported command" [obs, three ECO
probes]. So the import is **placement only**: a WYSIWYG netlist
(`tennm_lcell_comb` per LUT and arithmetic half, `tennm_ff` per flop, the
M20K as inferred RTL) with a location assignment for every atom
(`LABCELL_X_Y_N(6k + 3h)`, `FF_X_Y_N(6k + 3h + 1 + r)`), retiming off, and
Quartus routes it. quartus_sta then times a5pnr's placement on Quartus'
routes, and a5pnr writes its own model's view of its own routes beside the
project (`a5pnr_timing.txt`); `tools/timing/compare.py` puts them side by
side.

- **Quartus keeps the placement** [obs]: on ctl every atom on a reported
  path (69 of 69) sits where a5pnr put it. Two fixes came out of it: Quartus
  rejects a carry chain that starts at ALM 5 and runs on ("Illegal location
  assignments for LEs in carry chain"), and its own chains all start at ALM
  0 (cql's six), so a5pnr's do now; a chain's first `cin` is left open.
- **The model on Quartus' routes** [obs, `tools/timing/fit.py eval`,
  held out]: Quartus' own CPU (cql, 1,508 paths launched by flops): mean
  −2.4 %, |error| median 3.3 %, 95th percentile 12.5 %; a5pnr's CPU
  placement routed by Quartus: +0.7 %, 3.0 %, 8.9 %; a5pnr's ctl
  placement routed by Quartus (442 paths): mean +1.5 %, median 12.5 %, 95th
  21.6 %, nearly all of it carry chains. Refitting on rt, ar, hx and mm
  (twice phase 6's training set) moves these by under a point, so the
  per-hop means stand; what is off is how a5pnr's STA strings the carry
  arcs together: quartus_sta charges `cin → cout` once per ALM (0.05–0.08
  ns) and a data input to the chain runs to the half-LAB's end in one
  look-ahead step (0.56 ns over 6 halves and over 10), where a5pnr charges
  `cin → cout` per half and ripples [obs, ctl2's paths]. The paths cql's
  M20K launches (tco 1.43 ns) show as a −40 % tail in `fit.py eval`,
  which files that launch under `tco`; a5pnr's STA has the M20K's own.
- **a5pnr's routes against Quartus'** on the same placement [obs, ctl]:
  per flop endpoint the model on a5pnr's routes is +0.56 ns (+57 %) over
  quartus_sta on Quartus' (median of 32); register to register 2.55 ns
  (393 MHz) against quartus_sta's 664 MHz (slow 100 °C corner; 365 MHz
  "restricted" by the clock's minimum pulse width). The model prices
  Quartus' own path into the worst endpoint (a carry chain fed from its own
  LAB: LL, LEIM, then the chain) at 1.54 ns against quartus_sta's 1.37, so
  about 0.9 ns of a5pnr's 2.46 there is routing [inf]: Quartus' critical
  paths run LAB_RE → LL → LEIM inside a LAB, where a5pnr's routes for the
  design use 160 H0, 79 H1 and 77 V1 hops.
- **The CPU** [obs, r5's placement, `specimens/p8/qx/cpu3`, `sta/cpu3`]:
  Quartus' legalizer rejects LABs of a5pnr's placement with "too many LAB
  line inputs": a5pnr routed 76 and 71 local lines into LABs X75 Y25 and
  X73 Y25, where Quartus' own LABs use at most 73 (cql, fr); its budget
  counts something a little different [unk], and a5pnr has none. With the
  17 LABs that use 60 or more local lines left to Quartus (324 of 2,802
  cells), it keeps the rest (all 261 locked atoms on reported paths where
  a5pnr put them). quartus_sta: **181.0 MHz** on Quartus' routes of
  a5pnr's placement, against **300.7 MHz for Quartus' own fit** (cql) and
  the model's **125 MHz on a5pnr's own routes**. The model on Quartus'
  routes of this placement: mean +0.7 %, |error| median 3.0 %, 95th
  percentile 8.9 %, max 13.1 % (2,000 paths launched by flops); per flop
  endpoint a5pnr's routes cost +1.57 ns (+28.6 %, median of 160) more than
  Quartus' on the same placement. So the CPU's 125 → 301 MHz splits about
  evenly into routing (125 → 181 on one placement) and placement and
  synthesis (181 → 301) [inf: the two runs differ in both].

Other a5pnr changes: hash maps with a fixed hasher (`src/det.rs`), so a seed
gives the same placement every run (std's RandomState didn't: the CPU's r2
and a rerun differed); PathFinder's present-congestion factor capped at
1,000 (×1.8 per iteration reached 10²⁰ by iteration 80, where every path
costs the same and the last two nets on a node never part); a placement
whose routing doesn't converge is placed again with the next seed (the
CPU's r4: two nets that each needed the same local line, LUT inputs being
fixed to pins at packing).

### (z5) Phase 8: MLAB RAM, read from Quartus' own image

`designs/p8/ml`: two RAMs Quartus Pro is told to put in MLABs (a 32 x 20
with an asynchronous read and a 16 x 8, written and read from an LFSR and a
counter so that each pin carries its own signal; retiming off), compiled
(`specimens/runs/ml0`), its atoms dumped (`specimens/p8/alm/ml0`), its image
decoded (`specimens/p8/ex/ml.txt`). Each `LUTRAM` atom is one data bit; its
ports name every signal, and `a5pnr drivers` names what the decoded image
routes to each LAB_RE pin [obs]:

- **One bit per half-ALM, 32 words in its 32 mask bits.** A 32 x 20 RAM is
  one MLAB (10 ALMs, 20 halves), a 16 x 8 four ALMs of another.
- **Read**: address bits 0–4 on A, B, C1, D1 and E (E0 and E1 both, the
  same signal) in every ALM, i.e. the index of the half-1 5-LUT; the data
  comes out through `LUTOUT_5LUT` into the half's register slots, as a
  logic 5-LUT's does (WIRE_MODE for an asynchronous read, a flop on **LAB
  clock 1**, `regclk = DFF_LABCLK1`, where Quartus pulls the read register
  into the MLAB).
- **Write**: address bits 0–4 on LAB_RE 156, 157, 160, 161, 155 (a 16-word
  RAM leaves 155 and E unrouted); write enable on 80 (the LAB's CLKEN0
  line); half 0's data on C0, half 1's on D0; the clock on the LAB clock
  lines (83, 84 and 149–171 all carry it).
- **RAM or logic**: every MLAB Quartus uses as logic sets
  `xpwrmgmt/lutram_mode_vcchg_power_saving = ENABLE_POWER_SAVING` (fr 16,
  cq 24, cql 167 of 167) and `ram_mode = LUT_MODE` on its bottom-half
  ALMs; the two RAM MLABs set neither (RAM is the idle mode). Their masks
  are all 0 (the RAMs start empty).

`a5pnr readback` now models it: an MLAB without the power-saving setting is
RAM, each half a 32-bit memory written on LAB clock 0 when CLKEN0 is high
and read at {E, D1, C1, B, A}; unrouted address pins read 0 on both ports
[inf]. Registers on LAB clock 1 are modelled too. **The netlist rebuilt
from Quartus' ml image runs like `ml.v` for 3,000 cycles from power-up (49
registers, 4 outputs, 0 mismatches) and 1,000 with KEY0 low (the 16 x 8's
write enable gated off), 0 mismatches** (`tools/alm/oracle.sh
specimens/p8/alm/ml0 ml designs/p8/ml/ml.pins <out> 3000
designs/p8/ml/ml.v`, `SIMCHECK_HOLD=KEY0=1`). `tools/alm/ffmap.py` now
maps a register that starts at 1 inverted (Quartus keeps `lfsr[0]`, which
`lfsr = 32'h1` starts at 1, as an inverted flop: the device's flops power
up 0).

Not done [unk]: a5pnr doesn't place MLAB RAM yet (a yosys `memory_libmap`
library for 32 x 1 per half with one synchronous write and one
asynchronous read port, MLAB placement in a5pnr, emit); which mask bit is
which word for an initial content other than 0 (the read index is the
5-LUT's, so stored[32h + address] is [inf]); whether the write clock is
83 or the 149–171 lines. Quartus didn't put the CPU's register file in an
MLAB by itself ("Unsupported Mixed Feed Through Setting in Auto MLAB
Conversion"): in cq it used two M20Ks, one per read port.

### (z6) Phase 9: every rule a5pnr writes, against Quartus' own images

Phase 8's lesson, applied to every rule: for each rule a5pnr's writer and
its readback share, a Quartus-compiled design that uses it is decoded by
`a5db extract`, rebuilt by `a5pnr readback` and simulated against its RTL
from power-up (`tools/alm/oracle.sh`), under every hold of its control
inputs. The readback's own rules are what is under test; where it met
something it didn't know it stopped (it bails rather than guesses), and
each such stop is a row below. New tools: `tools/alm/atoms.sh` (the atom
dump of an existing run, 15–20 min and 4 GB for a small design),
`tools/alm/qsf2pins.py`, `ORACLE_REUSE=1` (re-simulate after a readback
change). Runs in `specimens/p9/alm`, readbacks in `specimens/p9/oracle`.

| rule (what a5pnr writes or reads) | oracle design | result | fix |
|---|---|---|---|
| carry chain: sum, carry, chain start at 0, ALM 4 → 5 (`DISABLE_BTO`), into the LAB below (`hipi_clken`) | ar (32-bit accumulator, ternary adder, comparator, subtractor; 6 `DISABLE_BTO`, 1 LAB crossing) | **pass** | |
| enable, synchronous clear (the enable gates it), active-low asynchronous clear, power-up 0 | ar under KEY0, KEY1, EN held each way | **pass** (2,000–3,000 cycles, 127 registers, 0 mismatches each) | |
| M20K contents and word order, write and byte enables, output register | mq (512 x 32 with contents and byte enables; 512 x 32 with RE and output register) under WE, RE held each way | **pass** after two fixes | readback: the physical 40-bit word |
| M20K read enable | mq, RE low | **fail**: `A5_M20K` (cells_sim.v) registered only the read address under RE, so a later write to that address showed through; Quartus implements `if (RE) q <= mem[a]` with the RE pin, i.e. RE holds the read data | model: RE latches the data (old data on a same-address write) |
| M20K data pins | mq | **readback stop**: Quartus packs a 32-bit word without byte enables into bits 0–31 of the 40 (pins 83–114), a5pnr into 8 of each 10-bit lane; both are the same pin rule (bit j in at 57 + j / 40 − j, out at 83 + j) | readback models the 40-bit word (`A5_M20K40`) |
| M20K clock enable | cqr (Quartus' CPU ROM) | **readback stop**: Quartus gates the ROM's read with `xsecmux/xclk0/m20k_clkensel = CLKEN_SEL` and pin 41, not RE (39) | modelled (pin 41, `CLKEN_SEL_INV` inverted); a5pnr doesn't write it |
| MLAB RAM with initial contents | mi (32 x 20 asynchronous read, 16 x 8 registered, every word its own initial value) under KEY0 (write enable) held each way | **pass** (3,000 cycles, 82 registers, 0 mismatches) after one fix | |
| MLAB word order | mi | 32-word RAM: word a, bit b at stored[32h + a] [obs, 20 of 20 halves]; **16-word RAM: word a at stored[32h + 16 + a]** [obs, 8 of 8]: the unrouted address bit 4 reads **1**, not the 0 phase 8 inferred | readback: unrouted MLAB address pins read 1 |
| a flop in a chain's spare half | mi (`rdaddr_reg[1]` in the last ALM of a counter chain) | **readback stop**: in arithmetic mode `LUTOUT_5LUT` is the half's 5-LUT over {E, D, C, B, A} (p in the low 16 bits, g in the high), used as a pass-through (p = 0, g = 1: E) | modelled; a5pnr doesn't use it |
| IOPLL (V16, output 0), its clock into the fabric | pl (24-bit counter on the PLL clock) | **pass** (25 registers, 0 mismatches); `locked` not modelled (x on both sides) | |
| V16's ioclk mux | pl | Quartus sets `xif_p2c_ioclk_2to1_mux_u1` P0 with V16 only a PLL reference; pa (V16 a PLL reference too) leaves it idle, as a5pnr does [obs] | readback ignores pin-level clock muxes |
| LAB clock delay | cqr | `smxclk_dly` DELAY_2..4, `smxclk_dly_off` ENABLE_DELAY on 219 LABs and `regclk = DFF_LABCLK_PHI1_0` on 476 flops: the LAB clock through a programmable delay [inf: timing only] | readback treats PHI1 as the same clock |
| hyper-registers | cql (phase 8's CPU fit, retiming on) | **readback stop**: 1,556 routing muxes at `flop_mode = FLOP` (registers in the routing); a5pnr never writes them, so the CPU was recompiled with retiming off (`designs/p9/cqr`) | none needed |
| chains over four LABs, carry out as data, a pin as carry-in, subtract; active-high and active-low clears; a preset; power-up 1; enable and clear in both priorities | oc (64-bit accumulator, comparators, ct's controls without I/O registers) under seven holds of CEA, CEB, SCLR, ACLR, ACLRN, CIN | **pass** (3,000 cycles, 213 registers, 0 mismatches each) after one fix | |
| a second clock domain | oc (CLOCK2_50, BF23) | **fail in the readback**: CLOCK2_50 reaches the spine through `Vertical_seam_tap_mux` I161 → I125, a mux at its idle setting (`usable` class 4), which no decode shows, so the LAB clock read x | readback: a clock-network mux nothing configures passes its idle input [obs]; a5pnr doesn't route over such edges |
| a preset's power-up | oc | Quartus ignores a declared power-up 0 on a preset register ("will power up to High"): it is an inverted flop on the clear; a5pnr's synthesis refuses that combination | the oracle's RTL says 1 |
| every LAB/MLAB ALM setting | all of the above | the readback ignored unknown settings outside MLABs; now it stops on any it doesn't read or know to be inert (`leout`, the ALM's `regctrl_reg` `ce_sel`/`clr_sel`), and every image above still reads back | strict |
| the whole ALM, M20K ROM, clock enable, LAB clock delays | cqr (Quartus' CPU, register file in flops, retiming off) from power-up, SW0 held each way | **pass** (3,000 and 10,000 cycles, 230 registers and the LEDs, 0 mismatches) | |
| routing muxes, `FAST_WIRE`, the clock network, DCM/P2C, pins in and out | every design above: each decoded edge is a mux setting; V16 and BF23 through the DCM, switchbox, seam, spine and row clocks; HVIO and IO96 inputs and outputs | **pass** (as part of each) | |
| I/O registers | ct (FAST_INPUT/OUTPUT_REGISTER on 3.3-V pins) | **readback stop**: not modelled. The register clock enters at HIOL `GLOBAL_LOCAL_SEL` I5 → RPI I29 → REDMUX I372, a LUT constant through an inverted HIOL RPI feeds each register's (active-low) clock enable, and `xinv_fr_in_clk` = IN_INV [obs, ct] | step 5 (not reached) |

The oracle designs' RTL got the device's power-up 0s where it had none
(mq's second RAM, ar's registers, cqr's CPU in `specimens/p9/oracle/cqr/rtl`
only), which the simulation needs and Quartus already assumed.
`tools/alm/oracles_p9.sh` reruns all 18 holds of the six oracle designs.

**What failed, and what a5pnr wrote**: of the rules a5pnr's writer shares
with the readback, none was wrong in a5pnr's images. The M20K read enable
was wrong in the *model* both checks share (`A5_M20K` in cells_sim.v);
a5pnr's bits for it are Quartus' (RE pin 39), so the images are unchanged
and so is what the board should show (ramx with KEY1 held: frozen either
way, since its write address moves on). The unrouted MLAB address pin and
the idle clock mux are readback rules a5pnr doesn't exercise (no MLAB RAM
yet; it never routes over an idle mux). So no phase-8 board file becomes
wrong through the audit; phase 9's files differ because placement and
routing changed ((z8), "Board test").

### (z7) Phase 9: the STA's carry arcs

quartus_sta's carry-chain arcs, from every reported path of phases 6–8
(`tools/timing/paths.py` now keeps each cell's location, so an arc's
ALMs are known) [obs]:

| arc | n | mean (ns) |
|---|---|---|
| `cin → cout` within one ALM (both halves) | 17,577 | 0.050 |
| the same at a half-LAB's last ALM (4 or 9) | 1,585 | 0.118 |
| `cin → sumout` (either half) | 6,537 | 0.178 |
| data → its own ALM's `cout` | 2,175 | 0.226 (A 0.29 … D 0.21) |
| data → `cout` of the half-LAB's last ALM, d ALMs on (look-ahead) | 3,559 | 0.41 (d = 0–1), 0.43, 0.48, 0.50 (d = 4) |
| data → `cout` of ALM 9 from the top half-LAB (d = 5–9) | 803 | 0.46–0.53 |
| data → `sumout` | 335 | 0.27 |
| ALM 9's `cout` into the next LAB (`LAB_RE` I88) | 744 | 0.105 |

So a carry ripples per ALM, not per half, and a data input reaches a
half-LAB's end in one step, nowhere near a ripple's cost; no reported path
ripples through ALM 9 into the next LAB. a5pnr's STA charged `cin → cout`
(0.059, a mean over both kinds) per half and one 0.29–0.38 ns data → cout
at the entry half before rippling.

- **The fit** (`tools/timing/fit.py`): carry arcs are classed by where they
  run (`arith:<in>>cout`, `:e` at ALM 4 or 9, `arith:data>cout:la<d>`,
  `arith:cin>cout:skip`, `:x` for half 0's data into half 1's sum,
  `carry:lab`). Trained on rt and ar as before; `carry:lab` (0.105 ns)
  from cql0, the only Quartus fit with a reported LAB crossing.
- **a5pnr's STA** (`src/sta.rs`) now strings them that way: per ALM a carry
  out, `cin → cout` once per ALM, each data input to its own ALM's carry
  out and by look-ahead to the half-LAB's and the LAB's last ALM,
  `carry:lab` into the next LAB.
- **The error, re-measured** [obs]: `fit.py eval --sta old|new --worst`
  prices every carry section of a quartus_sta path (data input to the sum
  it ends in) the way each STA strings it, routing hops at their class
  means, on each endpoint's worst path; held out except where noted:

  | set (worst path per endpoint) | old STA: mean, median, 95th, max | new STA |
  |---|---|---|
  | ctl2 (a5pnr's ctl placement, Quartus' routes; 66) | +33.6 %, 37.1 %, 67.8 %, 71.8 % | **+6.7 %, 6.1 %, 21.9 %, 22.3 %** |
  | cpu3 (a5pnr's CPU placement; 2,000) | +11.0 %, 10.1 %, 21.6 %, 29.2 % | **+3.1 %, 2.5 %, 10.1 %, 14.2 %** |
  | hx (473) | +0.2 %, 2.8 %, 14.3 %, 44.5 % | −1.3 %, 2.3 %, 7.2 %, 10.1 % |
  | ct (126) | +3.9 %, 2.6 %, 29.7 %, 36.0 % | +0.5 %, 2.1 %, 7.3 %, 30.9 % |
  | mm, mq (1,092, 1,337) | 4.6 %, 2.7 % median; max 34.8 %, 43.5 % | 4.4 %, 2.7 %; max 13.5 %, 10.2 % |

  Hop by hop (each quartus_sta arc at its class mean, all paths), the new
  classes take ctl2 from 12.5 % to 5.4 % median, hx 5.2 → 1.5 %, ct 8.2 →
  2.7 %, cpu3 3.0 → 2.1 %. What is left on ctl2 (+5–7 %) is routing hops,
  not carry [inf].

### (z8) Phase 9: routing quality

Measured as in (z4): a5pnr's placement goes to Quartus as locked atoms
(`--quartus`), Quartus routes it, quartus_sta times it; a5pnr's model times
a5pnr's own routes of the *same* placement (`tools/timing/compare.py`, per
flop endpoint, the worst data delay). With the STA's carry arcs fixed (z7)
the baseline is smaller than phase 8's figures, which partly measured the
STA's own chain error.

- **The LAB-input budget** [obs, inf]: a LAB's data pins take their nets
  over its 80 local lines, and Quartus' legalizer rejects a LAB with "too
  many LAB line inputs" by a rule that is [unk]. Counting what reached the
  pins: it rejected a5pnr LABs with 61, 59 (phase 8's CPU r5) and 55 (this
  phase's first try, with a budget of 56) distinct nets, 53–60 of them
  from outside the LAB; it took a5pnr LABs with up to 51; its own CPU fit
  (cqr) has at most 52 (48 from outside). All 80 lines take both wires and
  LAB outputs (graph), so it isn't a plain split. a5pnr's placer now keeps
  every LAB at **50 nets** or fewer (`place::LAB_INPUTS`), and Quartus took
  the whole CPU placement (`specimens/runs/qcpu1`, 209 LABs, every atom
  locked; phase 8 had to leave 17 LABs to Quartus).
- **LUT input swapping** (`pack::swap_group`, `route.rs`): a LUT input's
  sink may end on any pin it can move to (a 6-LUT's A, B, E, F; a 5-LUT
  half's own C, D, E; never a pin a LEIM flop reads), PathFinder settles
  which net gets which pin, and the masks are rebuilt from the pins chosen.
  On the CPU 3,566 inputs in 1,209 ALMs moved; every check still passes
  (simulation catches a wrong permutation: ctl and arith, 0 mismatches).
- **Local feedback**: a5pnr's router could always take LAB_RE → LL → LEIM
  inside a LAB, but with fixed pins a source's two local lines rarely
  reached the sink pin's LEIM. Swapping makes it more often possible: of
  the CPU's 1,146 connections inside one LAB, 244 → 358 run over the local
  lines alone.
- **Measured** [obs] on the CPU placement Quartus routed (`qcpu1`,
  quartus_sta **174.6 MHz**; the model on Quartus' routes of it is 4.4 %
  median high, `fit.py eval --sta new`), per flop endpoint, 195 endpoints,
  model on a5pnr's routes minus quartus_sta on Quartus':

  | router | median (relative) | mean | model, register to register | iterations |
  |---|---|---|---|---|
  | phase 8's (pins fixed at packing) | +0.91 ns (+16.3 %) | +0.93 ns | 129 MHz | 17 |
  | + LUT input swapping | +0.75 ns (+13.0 %) | +0.78 ns | 136 MHz | 10 |
  | + criticality squared in the router | **+0.68 ns (+12.0 %)** | +0.71 ns | **141 MHz** | 9 |
  | (rejected) criticality to the 4th | +0.71 ns (+12.4 %) | +0.74 ns | 137 MHz | 24 |
  | (rejected) a second pass with criticalities from the routes | +0.78 ns (+13.8 %) | +0.81 ns | 126 MHz | 12 |

  On fr (no Quartus reference, model only) criticality squared gives 249
  MHz register to register against 246 and routes in 7 iterations instead
  of 52. On ctl's placement (`qctl0`, quartus_sta 572 MHz on Quartus'
  routes) the gap is +0.21 ns (+20 %) with or without swapping: its
  critical paths are carry chains, whose pins don't move. So of the CPU's
  141 → 175 MHz, about a third is the model reading high (4.4 %) and the
  rest a5pnr's routes [inf]. Runs: `specimens/p9/route`, `specimens/p9/sta`.

### (z9) Phase 9: MLAB RAM in a5pnr

- **Synthesis** (`tools/a5pnr/synth.sh`, `yosys/mlab.txt`): yosys'
  `memory_libmap` maps every RTL memory that fits to `$__A5_MLAB_` cells,
  one bit of a 32-word RAM each, with a synchronous write and an
  asynchronous read; a register on the read stays the RTL's own flop
  (`synth -nordff`: merged into the read port, yosys rebuilt it under a
  name of its own and the CPU's `rs1` lost its name). `A5_MLAB=0` keeps
  memories in flops, as before.
- **Packing** (`Logic::Ram`): two bits with the same read address share an
  ALM; up to ten ALMs with the same clock, write enable and write address
  share an MLAB, which holds nothing else. The read address goes on A, B,
  C1, D1 and E0/E1, the write data on C0 (half 0) and D0 (half 1), the
  write address on LAB lines 156, 157, 160, 161, 155 (`Ctl::Wa`), the write
  enable on CLKEN0, the read data out through `LUTOUT_5LUT` in WIRE_MODE,
  as in Quartus' ml and mi (z5), (z6). A constant address bit stays
  unrouted and reads 1, so its words are stored where that bit is 1
  (`emit::ram_stored`).
- **Placement**: MLAB RAMs go to the MLABs nearest the ports' centre and
  stay there, like the M20Ks; logic never goes into an MLAB.
- **Checks** [obs]: `designs/p9/rf` (a 32 x 8 RAM with initial contents and
  a 16 x 4, whose constant address bit exercises the unrouted pin): 6 RAM
  ALMs in 2 MLABs, the image decodes to the design, 352 registers
  including all 320 mapped RAM bits from power-up and random states, 0
  mismatches. **The CPU with its register file in MLAB** (two read ports,
  so two copies, 64 bits in 32 ALMs and 4 MLABs): **453 ALMs** instead of
  1,393 (Quartus: 1,082 with the register file in flops, 334 + 2 M20Ks
  by its own choice), extract = design, 20,000 cycles from power-up and
  300 from random states with 2,245 registers compared, 0 mismatches,
  asm2 0 of 333,910 attributes differing, pfg identical, integrity OK. The
  model puts it at 113 MHz register to register (the flop version 141):
  the critical path now ends at an MLAB's write data.
- simcheck now maps RTL memory words onto MLAB contents (`mem[word][bit]`
  lines in the ffmap) and follows `wire X = reg;` aliases back to the
  register: femtorv32's `rs1` only ever appeared as `aluIn1`, so its RTL
  copy was never given its random state (a gap phase 8's runs didn't hit).
- Every design through `tools/a5pnr/suite.sh` on phase 9's final code
  (`specimens/p9/s1`, the CPUs `specimens/p9/cpu`; the model's Fmax is the
  longest path including the pins, as before):

  | design | ALMs / LABs (phase 8) | M20K | route iterations | model Fmax | extract = design | simcheck | asm2 differ | pfg |
  |---|---|---|---|---|---|---|---|---|
  | blinky | 13 / 2 (13 / 2) | 0 | 1 | 528 MHz | yes | 0 mismatches | 0 | identical, OK |
  | leds | 19 / 2 (19 / 2) | 0 | 6 | 312 MHz | yes | 0 mismatches | 0 | identical, OK |
  | ram | 16 / 2 (16 / 2) | 1 | 4 | 424 MHz | yes | 0 mismatches | 0 | identical, OK |
  | acc | 37 / 6 (37 / 6) | 0 | 5 | 245 MHz | yes | 0 mismatches | 0 | identical, OK |
  | arith | 23 / 4 (23 / 4) | 0 | 3 | 497 MHz | yes | 0 mismatches | 0 | identical, OK |
  | ctl | 39 / 9 (39 / 8) | 0 | 5 | 377 MHz | yes | 0 mismatches | 0 | identical, OK |
  | rom | 17 / 2 (17 / 2) | 1 | 4 | 424 MHz | yes | 0 mismatches | 0 | identical, OK |
  | ramx | 17 / 3 (17 / 3) | 1 | 4 | 469 MHz | yes | 0 mismatches | 0 | identical, OK |
  | pll | 27 / 4 (27 / 4) | 0 | 1 | 515 MHz | yes | 0 mismatches | 0 | identical, OK |
  | fr | 505 / 69 (505 / 75) | 0 | 8 | 113 MHz | yes | 0 mismatches | 0 | identical, OK |
  | rf | 23 / 5 (new) | 0 | 4 | 475 MHz | yes | 0 mismatches | 0 | identical, OK |
  | cpu (register file in MLAB) | 453 / 78 (1,393 / 215) | 1 | 32 | 113 MHz | yes | 0 mismatches | 0 | identical, OK |
  | cpu_ff (register file in flops) | 1,393 / 206 | 1 | 15 | 136 MHz | yes | 0 mismatches | 0 | identical, OK |

  The model's figures aren't comparable with phase 8's for designs with
  carry chains (blinky 393 → 528 MHz, arith 352 → 497): its carry arcs
  changed (z7). fr has 69 LABs instead of 75 under the input budget.
- Not done [unk]: logic in an MLAB beside RAM, RAMs deeper than 32 words
  (memory_libmap would build them from several with a read mux; not tried),
  the read register inside the MLAB (Quartus uses LAB clock 1 for it, (z5)),
  MLAB RAM in the Quartus export.

### (z10) Phase 10: I/O registers

`designs/p10/iq` (Quartus' oracle for this): input, output and OE registers
on 13 HVIO pins in two blocks (hvio_3_1, hvio_0_1) and 9 IO96 pins (bank 2A's
switches and LEDs), plain, with a clock enable, an active-high and an
active-low asynchronous clear, a preset (a clear to 1) and a synchronous
clear, each control on its own pin (`FAST_INPUT_REGISTER`,
`FAST_OUTPUT_REGISTER`, `FAST_OUTPUT_ENABLE_REGISTER`, retiming off). Quartus
packed all 23 into the I/O cells (`iq.fit.rpt`, "Fitter Netlist
Optimizations"). Run `specimens/runs/iq0`, atoms `specimens/p10/alm/iq0`,
readback `specimens/p10/oracle/iq`.

- **Where the controls enter** [obs, `a5pnr drivers` on iq's decode, every
  register]: each control is a pin of the I/O block next to the pin's own
  data pins. On an HVIO pin, with O the block pin its OE path enters
  (`pin_nodes.tsv` role `oe`): O − 4 the input register's synchronous
  clear, O − 3 the asynchronous clear or preset all three registers share,
  O − 2 the output and OE registers' clock enable, O − 1 the input
  register's clock enable, O + 1 the output data (16 HVIO controls in
  iq). On an IO96 pin, with U the block pin its output data enters (role
  `out`): U + 2 the asynchronous clear, U + 4 the input register's clock
  enable, U + 5 the output and OE registers' clock enable (13 controls;
  the OE data enters at the `oe` boundary, which on DN25 is another block
  pin than U's neighbours). An IO96 synchronous clear wasn't seen [unk].
  The fabric ends at an I/O routing tile's RPI; the hops after it
  (REDMUX, the block pin) have no bits.
- **The clock** [obs]: a register clock pin of the block, entered from the
  HIOL tile's `GLOBAL_LOCAL_SEL` I5 (each HIOL row has three, I5–I7, into
  three block pins); every register of hvio_3_1 used the one at row 49
  (`HVIOWR_RE` I2009), whichever row its pin sits in.
- **Every control is active low** [obs]: the settings say so
  (`a_rb_ncein_ireg_ena`, `a_rb_nceout_oreg_ena`, `a_rb_naclr_*_ena`,
  `a_rb_nsclr_ireg_ena` turn each one on), and Quartus drives every clock
  enable, every active-high clear and the synchronous clear through the
  I/O routing tile's programmable inverter (`<path>/rpi_sel = INV` on a
  HIOL or VIO tile), the active-low clear (ACLRN) straight. A register with
  no enable still has its enable on, driven by a constant LUT through an
  inverter. **So ct's two HIOL inverters (README (y)) are its two I/O
  registers' clock enables, not "data inverters around both registers".**
  `a5db rpimap` lists every HIOL and VIO inverter with its RPI node
  (`db/sm4revb/io/rpi.tsv`, 20,982: the RPII → RPI edge's config is the
  attribute's path).
- **A preset is `a_rb_naclr_*_sel = *_NPRE_SEL`** on the shared clear
  [obs]; an input register's synchronous clear is its own
  (`a_rb_nsclr_ireg_ena`, O − 4); an output register's synchronous clear
  Quartus folds into the data (qf).
- **The OE register holds the inverse** [obs]: Quartus reports all three as
  "Inverted", drives their data through an inverted RPI, and the buffer
  drives the pad while the register holds 0. With a clear, an OE
  register's shared clear is then a preset (`EREG_NPRE_SEL`, ZB, whose
  RTL OE clears to 0).
- **The input register's clock inverter** `xinv_fr_in_clk` is IN_INV on
  every input register (ct and iq, 12 of 12); its idle setting is IN_BUF.
  So IN_INV is what a rising-edge input register needs [obs]; that IN_BUF
  would capture on the falling edge is [inf].
- A registered input leaves on the pin's `in1` node, IO96 too; on IO96 its
  4:1 `ireg_4to1_mux0` stays at its idle SEL3, and its header bit
  45,136 + 229 L + p is 0, as for an output [obs: SW0–3 decoded as 4 `raw`
  bits until the template got an IO96 `in_reg` use with that row]. The
  IO96 lane's input flyby muxes (`xflyby/x<k>/sel_f`) are GPIO in the lane
  with the registered inputs [obs; what they select is inf].

`a5pnr readback` now models them (`src/ioreg.rs`): per pin the three
registers as the block's flops on its register clock, their data, enables,
clears, preset and synchronous clear at the block pins above, a signal
through an inverted RPI arriving inverted, the OE register's 0 driving the
pad. **Quartus' iq image runs like `iq.v` for 3,000 cycles from power-up
under six holds of EN, ACLR, ACLRN, SCLR (the other inputs random every
cycle, `SIMCHECK_RUN_RANDOM=1`, new in simcheck), 0 mismatches each; ct
(which stopped the phase-9 readback) likewise, 0 mismatches** [obs]. The
check has teeth: without the RPI inversions 27,094 mismatches, without the
preset 4,979, without the synchronous clear 740 (`A5_RB_MUTATE`), the OE
enable the other way round 6,000. Two oracle-side fixes: `tools/alm/ffmap.py`
leaves I/O register atoms out (the pins observe them), `oracle.sh` takes a
pin's use from a fourth column of the pins file.

**a5pnr writes them** (`src/ioreg.rs`, `pack.rs`, `main.rs`): a fourth
column `reg` in pins.txt puts the port's flop into the pin's I/O cell: an
input's one flop on the pin, an output's flop driving it, and for a
tristate output (yosys `$_TBUF_`, kept by `tribuf` in synth.sh when the
RTL has a `'bz`) its OE flop as well. The flops leave the ALMs; their data
and controls are routed to the RPIs that feed the block pins above
(`End::Io`), the clock to the block's register clock entry (hvio_3_1:
HIOL row 49 `GLOBAL_LOCAL_SEL` I5, hvio_0_1: row 9; IO96 lane 4: VIO X106
I3, lane 5: X107 I3; other blocks and lanes are refused, [unk]). Every
active-high control, and the OE register's data, gets its RPI's
inverter; a flop without an enable gets a constant-1 LUT on it, as Quartus
does. The block settings are `attr` lines, as `a5db extract` decodes them
with the pins' plain uses (an IO96 registered input is `in_reg`); an IO96
lane with a registered input sets its twelve flyby muxes. Not supported:
a preset (a5pnr's flops have none), an output register's synchronous clear
(it belongs in the data), an IO96 synchronous clear, the Quartus export.

`designs/p10/io` (the board's KEY0, KEY1 and SW[3:0] in input registers,
LED[7:0] from output registers with enables and both clear polarities,
three GPIO pins with an input synchronous clear, an input asynchronous
clear, an output register and an OE register): 24 ALMs in 8 LABs, 18 I/O
registers. **All four checks pass**: extract = design (19 pins, 559 edges,
369 attributes), the rebuilt netlist against `io.v` 2,000 cycles from random
states and 2,000 free-running with 36 registers compared (the 18 I/O
registers mapped), and 3,000 cycles from power-up with random inputs, 0
mismatches; asm2 0 of 106,145 attributes differ; pfg's `.rbf` identical,
integrity OK [obs, `specimens/p10/io/r1`].

The audit (`tools/alm/oracles_p10.sh`): iq under six holds and ct under
two, every other input random each cycle, 3,000 cycles each, **0
mismatches in all eight** [obs].

### (z11) Phase 10: DSP blocks

Phase 5 rebuilt the DSP modes' bits (18x18, 27x27 MAC, 9x9 + 18x19), but
which `MP_MAC_RE` pin carries which operand or product bit, and what the
settings do, wasn't known. Two Quartus oracles: `designs/p10/dq` (18x18
signed with registered operands, 18x18 unsigned, 27x27, a 27x27
multiply-accumulate with enable and clear, a sum of two products; XOR-reduced,
so only the low product bits are identifiable) and `designs/p10/dq2`: 18x18
and 27x27, each signed and unsigned, and a sum of two 18x18 products, every
operand bit straight from its own LFSR flop and every product bit into a
fabric flop through a LUT of its own (`q[k] = p[k] ^ s[k]`). Runs
`specimens/runs/dq0`, `dq20`; atoms `specimens/p10/alm/dq20`.

- **The pins** [obs, `a5pnr drivers` on dq2's decode and the atoms' flop
  locations, all operand and product bits of five blocks]: pin 0 is the
  clock. An 18x18 product (`dsp_mode` TWO_18X19, the idle value) takes A on
  pins 335 + i and B on 353 + j; a signed one routes A's sign bit to pin 280
  as well (A is the 19-bit operand). A 27x27 product (ONE_27X27) takes A on
  353 + i (i < 9) or 362 + i, B on 335 + i (i < 9) or 380 + i. Product bit k
  leaves on pin 134 + k (k < 38) or 60 + k.
- **The settings of a registered multiply** [obs, dq2 r47–r50]: Quartus puts
  `p <= a * b` with operands straight from flops in the output register
  alone (`dsp_oreg_ctrl = REGISTERED_OREG`, every other register's clock
  `NO_CLK_OPTION_*`), plus `dsp_sign_{a,b}{x,y}` for signed products, the
  four `dsp_pgm_invert_for_*` set, `dsp_clr_ctrl = NCLR_SELECTION`, power
  gating off: 28 to 31 attributes per mode, kept as they are in
  `db/sm4revb/dsp/modes.tsv`.
- **`dsp_proginv_l` and `_h` are per-pin input inverters** [obs]: Quartus
  keeps the LFSR's bit 0 as an inverted flop (it starts at 1), and in each of
  the four blocks reading it the masks differ from the other blocks of the
  same mode in exactly one bit, the bit of the pin that reads it: l[36] for
  pin 353, l[18] for 335, h[27] for 398. So l[k] is pin 263 + k (k < 18) or
  317 + k, h[k] pin 371 + k (k < 36). The masks set the bits of the pins a
  mode leaves unrouted, and in 27x27 mode h[0..17] too, pins 371–388, which
  carry A's bits 9–26 [inf: inverted on their way in, and an unrouted pin
  reads 1]. A mode's baseline is Quartus' mask with that one bit cleared
  (the signed and unsigned 27x27 baselines are equal); the pins of
  h[36..53] are [unk].
- Not modelled: input and pipeline registers (dq's blocks and dq2's sum of
  two products use them), the sum, MAC, 9x9 and floating-point modes,
  enables and clears, the dynamic controls, cascades.

`a5pnr readback` models a block whose settings are exactly one of the four
modes': its product, `$signed` or not, on the block's clock into a 54-bit
output register, each operand pin through its inverter (a bit differing
from the mode's baseline). **Quartus' dq2 image, its sum-of-two block left
out, runs like `dq2.v` for 3,000 cycles from power-up: q1–q4 (235
registers), 0 mismatches** [obs]; before the inverters were understood the
two signed blocks (whose pins read the inverted flop) mismatched from cycle
19. `designs/p10/dq3` (dq2 without the sum of two, compiled after the rule was
written; its blocks sit at other rows and other pins read the inverted
flop) **runs like its RTL for 3,000 cycles, 196 registers, 0 mismatches**
[obs, held out], once the readback also knew that Quartus drives its
constant-0 output by leaving the pin's data unrouted and inverting its RPI
[obs]: so an unrouted I/O data pin reads 1 [inf], as an unrouted MLAB
address pin does (and, inferred, a DSP operand pin). The readback models
only that case; an output nothing it models reaches stays unknown (phase
9's pl and its PLL `locked`). The phase-9 audit (`oracles_p9.sh`, 18 holds
of six designs) still passes with every phase-10 readback change, 0
mismatches [obs].

**a5pnr maps multipliers** (`tools/a5pnr/yosys/mul_map.v`, `src/dsp.rs`):
before synth, yosys narrows each `$mul` (`wreduce`) and maps a product of
two operands of up to 27 bits, both signed or both unsigned, to `A5_MUL`
(18x18 when both fit), the operands extended to the multiplier's width so
that every operand pin carries a signal (`A5_DSP=0` leaves products to
LUTs). a5pnr takes the flops that register the product (each product bit
must go straight into one flop with no enable or clear, one clock) as the
block's output register, places blocks on the DSP sites nearest the ports'
centre (as M20Ks), routes the operands and the clock to the pins above,
and writes the mode's settings with its baseline masks. The STA times a
block as an M20K [inf]; the Quartus export refuses DSPs. On the way:
yosys' carry out of a comparison (`step <= 64`) read the chain's internal
carry, which no ALM pin exports; `arith_map.v` now takes the top carry from
the sum of one more half and the others from the next sum, which leaves
every earlier design's cells unchanged (CPU 587 LUTs, 182 carry halves,
453 ALMs).

`designs/p10/mul` (a self-test after chipsmith's mult-selftest: eight
products, 9x9 to 27x27, signed and unsigned, each checked against a
shift-and-add reference in the fabric; its slot multiplexer is written as
continuous assignments, since iverilog never ran an `always @*` version at
time 0): 407 ALMs in 89 LABs, 8 DSP blocks, model 222 MHz. **All four
checks pass** (`specimens/p10/mul/r2`): extract = design (11,745 edges,
4,692 attributes, 431 DIRECT values), 2,000 cycles from random states and
2,000 free-running with 627 registers (including the 336 DSP
output-register bits), 0 mismatches, and 20,000 cycles from power-up with
the self-test's error count 0 on both sides, 0 mismatches; asm2 0 of
414,664 attributes differ; pfg identical, integrity OK.

### (z12) Phase 10: chipsmith's synthesis as a5pnr's front end

`tools/a5synth` (Rust) runs chipsmith's Verilog front end and its
device-independent synthesis (`chipsmith-verilog`, `chipsmith-synth`,
`chipsmith-netlist`, cargo git dependencies pinned to chipsmith `open`
0fd5991, as cyclone2-re's cram pins chipsmith-cyclone2) with its LUT
mapper at K = 6 (`lutmap::map_module_k`, chipsmith ADR-0013), in the pass
order of chipsmith's Cyclone II synthesis, and writes the cells
`tools/a5pnr/synth.sh` gets from yosys, in yosys' JSON: `$lut`,
`A5_ARITH` chains (its own `$alu` mapping: a start half for the carry-in,
p = A ^ B ^ BI, g = A & (B ^ BI), a carry out read as data from the next
sum or one more half), flops legalised to the LAB's controls (an enable,
a synchronous clear the enable gates, an asynchronous clear of either
polarity; a reset or power-up value of 1 kept inverted; a reset that
contradicts the power-up value becomes logic; constant enables and resets
folded, also after LUT mapping), `$__A5_MLAB_` RAM for memories of up to
32 words with one clocked write port (a synchronous read becomes an
asynchronous one and a register; per-bit write enables that are one mux
are one enable), and the hand-instantiated `A5_M20K`/`A5_IOPLL` as they
are. `A5_SYNTH=chipsmith tools/a5pnr/flow.sh …` uses it; a5pnr reads it
unchanged (it now gives nets no netname covers a slot, and reads
`$tribuf`).

**Checks** [obs]: every phase 6–10 design through the flow with chipsmith's
synthesis and the simulation check (`specimens/p10/synth/suite`: blinky,
leds, acc, ram, ctl, arith, rom, ramx, pll, fr, rf, io; the CPU with its
register file in MLAB, `specimens/p10/synth/cs1`: 20,000 cycles from
power-up and 300 from random states, 2,048 registers; picorv32: 300 cycles
from random states with 1,536 registers and 5,000 from power-up with the
CPU running, outputs compared): **extract = design and 0 mismatches
everywhere**.

**LUTs and ALMs** (`tools/a5synth/compare.sh`; ALMs are a5pnr's packing,
`--pack-only`; Quartus Pro's from its own fit of the same RTL):

| design | yosys: LUTs / ALMs | chipsmith: LUTs / ALMs | Quartus Pro ALMs | synthesis time, yosys / chipsmith |
|---|---|---|---|---|
| blinky, leds, acc, ram, rom, ramx, ctl | equal within 1 LUT and 1 ALM | | | 30–70 s / ≤ 1 s |
| arith | 4 / 23 | 4 / 24 | | 47 s / 0.4 s |
| io | 19 / 24 | 11 / 19 | | 46 s / 0.3 s |
| rf (MLAB RAM) | 1 / 23 (12 RAM bits) | 1 / 21 (8: the unread ones dropped) | | 34 s / 0.6 s |
| fr (random 2–7-input functions) | 879 / 505 | **320 / 209** | 233 (phase 8) | 110 s / 4 s |
| CPU, register file in MLAB | 587 / 453 | 442 / 450 | | 68 s / 1 s |
| CPU, register file in flops | 1,392 / 1,393 | **1,174 / 1,343** | 1,082 (1,171 LUTs + 195 carry cells, phase 8) | 73 s / 4 s |
| sha256 | 4,066 / 2,620 | **2,058 / 2,111** | (bench) | 102 s / 17 s |
| picorv32 | 1,218 / 805 | **473 / 506** | (bench) | 98 s / 2 s |

- **chipsmith's mapper needs fewer LUTs everywhere it matters**: the CPU's
  1,392 → 1,174 (Quartus' synthesis: about 1,170, so the LUT gap phase 8
  put down to synthesis is closed), fr 879 → 320, sha256 4,066 → 2,058,
  picorv32 1,218 → 473 [obs]. picorv32's flops also drop 381 → 345: chipsmith
  removes the instruction-word register bits its five-word ROM can only
  ever load as constants [inf]; the random-state check (which leaves
  registers the netlist doesn't have at the RTL's own values) passes.
- **ALMs follow LUTs less closely than LUT counts suggest**: the mapper
  minimises LUTs with K = 6 and makes many 6-input LUTs, which take a
  whole ALM, where two LUTs of up to five inputs share one. The MLAB CPU
  shows it: 442 LUTs (143 6-input) pack into 450 ALMs, yosys' 587 (36
  6-input) into 453; picorv32 219 of 473. **What chipsmith would need**
  (not changed here; chipsmith `open` untouched): a LUT mapper cost that
  knows the fracturable 6-LUT, i.e. an area per cut of K = 6 that charges a
  6-input cut one ALM and a cut of ≤ 5 inputs half of one (two of them pair
  when they share A and B: ≤ 8 distinct inputs, 2 shared), e.g. a
  `LutSize`-like parameter `LutCost`/area table for `map_module_k`; and a
  device-independent multiplier hook (chipsmith-synth maps `$mul` to blocks
  only in chipsmith-cyclone2's `mult`), so a5synth could place products in
  DSP blocks instead of gates (a5synth lowers `$mul` to LUTs today; the
  yosys flow maps them, (z11)).
- Synthesis time drops from 30–110 s (yosys from nixpkgs, two `abc` runs)
  to under 20 s.

### quartus_sta fails under chipsmith [obs]

The full `--flow compile` dies in signoff timing analysis: `quartus_sta`
cannot load `quartus/linux64/libsys_pdpw.so`, because the install's own
`qcore/linux64/libQt6Network.so.6` has `NEEDED libbrotlidec.so.1` and
`RUNPATH $ORIGIN`, and Quartus doesn't bundle brotli (it bundles `libzstd`,
the other compression dependency). chipsmith's
`crates/chipsmith-quartus-common/compat.nix` provides no brotli. The fix
belongs in chipsmith: add `brotli = pkgs.brotli.lib.outPath;` to that
list. Fitter and assembler are unaffected, so blinky was assembled with a
separate `quartus_asm blinky`. With `brotli.lib` on `LD_LIBRARY_PATH`
quartus_sta runs (u) [obs].

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

**Phase 9 status** (branch `phase9`): every rule a5pnr writes checked
against Quartus' own images; the router, the STA's carry arcs and MLAB RAM.

- Step 1 (oracle audit): **done** (z6). Six Quartus designs (ar, mq, pl,
  mi, oc, cqr) decoded, rebuilt and simulated against their RTL under 18
  holds of their controls (`tools/alm/oracles_p9.sh`; ct stops at its I/O
  registers, step 5): every rule a5pnr
  writes passes (carry chains over four LABs, chain starts, `DISABLE_BTO`;
  enable, synchronous clear, both clear polarities, preset, power-up; M20K
  contents, word order, enables, output register; MLAB RAM with contents;
  routing, the clock network, both clock pins, the PLL; pins). The shared
  *model* was wrong once: the M20K's read enable holds the read data
  (`A5_M20K` registered only the address); a5pnr's bits are unchanged.
  The readback learnt Quartus' other uses (40-bit M20K words, the M20K
  clock enable, arithmetic-mode `LUTOUT_5LUT`, LAB clock delays, idle clock
  muxes, unrouted MLAB address pins reading 1) and now stops on any ALM
  setting it doesn't know. No phase-8 board file becomes wrong.
- Step 2 (routing): **done** (z8). A LAB-input budget (Quartus now takes
  every LAB of a5pnr's CPU), LUT input swapping and criticality squared:
  the CPU per flop endpoint against Quartus' routes of the same placement
  +16.3 % → +12.0 % (median), model 129 → 141 MHz; local feedback 244 → 358
  of 1,146 connections inside a LAB.
- Step 3 (STA carry arcs): **done** (z7). Per-ALM carry with look-ahead:
  the STA's error on Quartus' routes of a5pnr's placements, median, ctl2
  37.1 % → 6.1 %, cpu3 10.1 % → 2.5 %.
- Step 4 (MLAB RAM in a5pnr): **done** (z9). The CPU with its register file
  in MLAB: 453 ALMs (was 1,393), every check passes.
- Step 5 (I/O registers): **not done**; ct shows their clock and clock
  enable paths (z6).
- Board: steps 23–26 of "Board test", files in `specimens/board/p9/`.

Open after phase 9 [unk]:

- Everything on silicon, still (the board is off); the phase 7 → 8 LUT fix
  and now MLAB RAM (steps 24, 25) are the first things to confirm.
- Quartus' LAB-input rule (a5pnr keeps 50 nets; rejected 55, taken 51).
- What an idle *fabric* mux passes (the clock network's do pass their idle
  input, z6).
- I/O registers: the clock and clock-enable nodes of pins other than ct's
  two; the input register's `IN_INV`.
- The rest of a5pnr's routing gap (+12 %, of which the model reads about 4
  points high); local feedback when the driver's two local lines can't
  reach the sink's LEIMs (pin choice at packing, or slot choice).
- MLAB: logic beside RAM, deeper RAMs, the read register in the MLAB, MLAB
  RAM in the Quartus export.
- Hyper-registers (Quartus uses them with retiming on; a5pnr doesn't).

Next: the board test (phase 9 files, steps 23–26, after phase 8's A/B
pairs); I/O registers in the readback and a5pnr, with ct as the oracle and
a second design with registers on other pins; local feedback through pin
choice at packing.

**Phase 8 status** (branch `phase8`): the ALM checked against Quartus' own
images, two LUTs per ALM, and quartus_sta on a5pnr's placement.

- The oracle (z3): Quartus' fr and ml images, decoded and rebuilt, run like
  their RTL. It found that **phases 2–7 wrote every LUT on inverted
  inputs** (the LUT rule came from Quartus' port convention, and a5pnr's
  writer and reader shared it), that a half's enable and clear cover both
  its flops, the direct LUT outputs and the spare-row hops. Every phase 6–7
  board file with a LUT in it is wrong; phase 8's replace them.
- Step 1 (density): **done for LUTs and flops** (z4). Two 5-LUTs per ALM,
  four register-file bits per ALM, 4:1 muxes as 6-LUTs: the CPU 2,628 →
  1,393 ALMs (Quartus: 1,082 for the same RTL). Every design passes every
  check.
- Step 2 (timing truth): **placement only** (z4). `a5pnr --quartus` writes
  the placement as Quartus atoms with locations, and Quartus keeps it
  (but for LABs past its LAB-input budget); it takes no routing
  constraints. The model on Quartus' routes: 3.0–3.3 % median error on the
  CPU (Quartus' placement and a5pnr's), 12.5 % on ctl (carry chains, whose
  arcs a5pnr's STA strings wrongly). The CPU: quartus_sta 181 MHz on
  a5pnr's placement, 301 MHz on Quartus' own fit, the model 125 MHz on
  a5pnr's routes; a5pnr's routes cost +29 % (CPU) and +57 % (ctl) per
  endpoint over Quartus' on the same placement.
- Step 3 (I/O registers): **not done**.
- Step 4 (MLAB RAM): **the bits and the readback, checked on Quartus' ml
  image** (z5); a5pnr doesn't map RAMs to MLABs yet.
- Step 5: **LAB clock 1** (in the readback, from ml). CLR2 carrying a
  signal, other PLL outputs, PLL reset and lock, `secmux_bto` and
  `hipi_clken`: not done.
- Board: steps 18–22 of "Board test", files in `specimens/board/p8/`.

Open after phase 8 [unk]:

- Everything on silicon, still; the phase 7 → 8 LUT fix is the first
  thing the board can confirm (step 19's A/B pair).
- a5pnr's routes: why they cost more than Quartus' on the same placement
  (LAB-local feedback through LL/LEIM; LUT input pins are fixed at
  packing, so the router can't swap them).
- Quartus' LAB-input budget (it rejects 71 local lines in a5pnr's LAB but
  uses 73 in its own); a5pnr should respect one.
- The STA's carry arcs: `cin → cout` per ALM and data → `cout` to the
  half-LAB's end in one step, as quartus_sta reports them.
- Exact routes in Quartus: no command found that sets one.
- MLAB RAM in a5pnr (a yosys `memory_libmap` library, placement, emit) and
  the word order of an initial content; I/O registers; CLR2; PLL outputs
  other than V16's 0, PLL reset and lock.
- Synthesis: Quartus' CPU needs 1,171 LUTs to yosys' 1,392 (extended
  7-input LUTs, `shared_arith`).

Next: the board test (phase 8 files first); then the router (LUT input
swapping, local feedback) and the STA's carry arcs, measured with
`--quartus` and `tools/timing/compare.py`; MLAB RAM in a5pnr.

**Phase 7 status** (branch `phase7`): from four toy designs to a RISC-V
CPU through the open flow.

- Step 1 (what real designs need): **done but for I/O registers and MLAB
  RAM** (y). LAB enables, clears and two clock domains; LEIM flops and LAB
  packing (leds 28 → 4 LABs); chain starts and the two LAB settings
  (phase 6's acc and ram lacked one); M20K write, byte and read enables,
  output register and contents, and the contents in the `.rbf`, which
  a5db had been dropping; a PLL clock from V16's IOPLL. Nine designs pass
  every check. Two Quartus oracle designs (ct, mq).
- Step 2 (speed): **done** (x). `a5db build` 245 → 40 s, extract 313 → 58
  s, `check.sh` 45 → 2–3 min; the router 372 → 149–192 s on the CPU.
- Step 3 (timing-driven): **done in the model** (z). STA, a delay table,
  timing-driven placement and routing (CPU 96 → 128 MHz by the model).
  quartus_sta on an a5pnr result is out of reach for now.
- Step 4 (CPU): **done** (z2). FemtoRV32 Quark with its program in an M20K:
  2,628 ALMs in 395 LABs, 8 routing iterations, model 128 MHz; the image
  decodes to exactly a5pnr's design, the rebuilt netlist runs the program
  like the RTL for 20,000 cycles and from random states, asm2 agrees, pfg
  is identical.
- Step 5 (board): steps 11–17 of "Board test", files in
  `specimens/board/p7/`.

Open after phase 7 [unk]:

- Everything on silicon, still: the board is off.
- I/O registers from a5pnr (their clock path and Quartus' data inverters),
  MLAB RAM, LAB clock 1, a signal on CLR2, PLL outputs other than V16's 0,
  PLL reset and lock.
- What an unconfigured mux passes (896 edges at idle settings), and which
  pass-through drives a node with several (18,000 such nodes).
- `secmux_bto` and `hipi_clken` are placed by a rule that fits every
  Quartus LAB seen; what they do in silicon is [inf].
- The model's error on a5pnr's own routes, and quartus_sta on them; hold
  and clock skew.
- Fracturable LUTs (two 5-LUTs per ALM: `LUTOUT_5LUT` exists and Quartus
  uses it) would roughly halve a logic-heavy design's ALMs.

Next: the board test (phase 4–7 files); then I/O registers, MLAB RAM and
fractured LUTs in a5pnr, and the nextpnr (himbaechel) target of Plan step 7.

**Phase 6 status** (branch `phase6`): from "a5db rebuilds Quartus images" to
an open flow that places and routes Verilog itself.

- Step 1 (pin ↔ routing-graph nodes): **done** (t). `a5db pinmap` derives
  every ball's boundary nodes from the graph; 429 of 432 pin uses in 14
  Quartus designs start or end at the predicted node, rule fitted on ph1
  alone; the 3 misses are explained (an unloaded input, a PLL reference, a
  clock routed as data).
- Step 2 (timing): **done as a fitted model** (u). quartus_sta runs with
  brotli on `LD_LIBRARY_PATH`; per-node-type and per-cell-arc means from
  its paths predict held-out path delays within 3.8–11.9 % median (95th
  percentile 8.7–22 %). Quartus' own RC model is located, not decoded.
  a5pnr routes on these delays as costs; there is no timing report.
- Step 3 (IOPLL): **done for PFDs of 10, 25, 50 MHz** (v). The IP's counter
  choice is a rule (10 of 10 held-out frequency sets match), the bits are a
  template plus counters and trims (held-out pb: 2,776 of 2,776 bits).
- Step 4 (open flow): **done** (w). yosys → a5pnr → a5db build → a5db rbf
  for four designs (counter, keys and LEDs, a carry chain over three LABs,
  an M20K); each image decodes back to exactly a5pnr's design file, its
  rebuilt netlist simulates like the RTL, asm2 agrees with every decodable
  attribute, and pfg's `.rbf` is byte-identical with integrity OK.

Open after phase 6 [unk]:

- Everything on silicon: the board is still off. The phase-6 files add
  four open-flow images to "Board test".
- Two LAB settings Quartus writes where a chain starts (`secmux_bto`,
  `hipi_clken`): copied, not understood. The hardware carry-in at a chain's
  start (a5pnr's chains begin with a p = 0 cell, so it doesn't matter).
- Pin map: IO96 `direct` and HVIO `out2` roles, output enables, pio_1_0.
- IOPLL: other PFD frequencies, bank-half 0 reference pads, the IP's output
  ceiling (1.2 GHz refused), phase shifts, cascading.
- M20K: only the 512 x 32 simple dual-port use with write enable tied high;
  other widths, enables, output registers and true dual port need their
  pins and settings (hx, mm have Quartus examples).
- Timing: Quartus' RC model, hold and clock skew; an STA for a5pnr's output.
- a5pnr: one clock domain; no LAB control signals (enables and resets
  become LUT logic), MLAB RAM, DSP, IO registers, PLLs; placement packs
  loosely (leds: 49 ALMs in 28 LABs).

Next: the board test (phase 4–6 files); then widen a5pnr (enables and
clears on the LAB lines, more M20K modes, PLLs, DSP) and grow it into a
nextpnr (himbaechel) target fed from `db/`, as Plan step 7 said.

**Phase 5 status** (branch `phase5`): coverage for a real toolchain. The
summary for a toolchain author is [docs/toolchain.md](docs/toolchain.md).

- Step 1 (pin tables): **done** (p). One template, fitted on 3 images and
  held out on 334 pin uses (ph1, ph2: every other DE25-Nano pin as input and
  output): 6 bits differ, all explained (input register mux choice, an
  unloaded input). `db/sm4revb/io/de25_pins.tsv` for every DE25-Nano pin;
  `a5db pins` for any set of pins on the package.
- Step 2 (clocks): **done** (r). The HVIO clock entry (2 clock pins per bank
  half) fitted on ck1 and held out on ck2 (all 32 clock pins, 0 bits
  differ); 22 DCM P2C muxes decode by the rule; ck1, ck2 rebuilt bit for
  bit; one clock network with per-sector trees (no separate regional
  network); clock enables are LAB attributes; one IOPLL (50 → 100 MHz)
  rebuilt bit for bit.
- Step 3 (core features): **done** (q). Carry chains, shared arithmetic,
  register controls, register-only cells, MLAB RAM, M20K and DSP: the
  held-out hx and the development designs ar and mm are rebuilt bit for bit
  from design files (pins, edges, attributes, DIRECT values), and asm2
  reads every rebuilt image back with 0 attributes differing. Two
  router-level rules came out of it: `flop_mode = FAST_WIRE` on used muxes,
  and the HIOL `xleim` mux settings.
- Step 4 (leftovers): record 0x13 is the JTAG USERCODE; the design hash is
  the fitter's PR hash over the netlist cookie and LUT masks; the spare row
  is a spare row 5 with no image address (s). The 8 perictrl0 words and
  the sector-header forced bits stay [unk].
- Step 5: [docs/toolchain.md](docs/toolchain.md).

Open after phase 5 [unk]:

- On the board (still off): everything; in particular whether a5db images
  configure, and whether a used mux at `flop_mode` DIS (phase 4's
  from-scratch blinky) passes its signal. The board files are rebuilt with
  FAST_WIRE ("Board test").
- The pin ↔ graph node map (which fabric node each pin's input register mux
  and output feed), which a router needs to start and end nets at pins
  without a Quartus route to copy.
- IOPLL settings as a function of frequencies; I/O buffer options
  (pull-ups, slew, drive, bus hold, open drain) and bidirectional pins;
  differential I/O; IO96 lanes 0–3 and 6–7 and `pio_1_0`; the left DCMs.
- Edge delays for timing-driven routing.
- The 8 perictrl0 words pfg skips, the sector-header forced bits, the first
  three words of record 0x13, the netlist cookie's origin.

Next: the board test, then nextpnr (Plan step 7) writing design files
through `a5db build` and `a5db rbf`, with the pin ↔ node map first.

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
