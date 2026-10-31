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
- **Not yet an encoder** [unk]: pfg's raw deflate isn't zlib's at any
  level, memLevel or strategy. The encoder's lib and code choice
  (`run_row_target`, `update_with_libs`, `run_combined_target`) isn't
  reimplemented. Whether the SDM accepts an all-literal stream (every code
  0, plain `00ab` nibbles) is the obvious first board test.

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
  `pack.py` stores one relative layout plus a per-instance anchor in
  [`db/sm4revb/bits/`](db/sm4revb/bits/).

  | type | instances | bits/instance | mapped | layouts | where |
  |---|---|---|---|---|---|
  | `cw_la_lab` | 2,340 (spare rows skipped) | 1,165 | 1,140 (2,667,600 pairs; 7,020 clash bits) | **1** | CRAM, 94 frames x 16 bdata |
  | `hviowr` | 5 | 5,927 | 4,748 | 1 | `perictrl{0,1,2}` object headers |

  The LAB anchors decompose exactly [obs]: frame = 200 + 98·(Y − 6),
  so one LAB row per 98-frame group (`sof_meta`'s `_frames` unit), and
  bdata depends only on X, at 64 bdata per column. Sector 0 holds X ≤ 59 with
  bdata growing toward X = 1 (4176); sector 1 holds X ≥ 62 mirrored (X =
  62 → 112, X = 120 → 4176). The two sectors are mirror images about the
  centre seam, but the LAB's internal layout is the same in both.
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

## Oracle

`tools/quartus.sh <tool> <args…>` runs one Quartus Pro tool through
chipsmith inside a fresh user+network namespace. It refuses to run if any
interface besides `lo` (and the optional unaddressed `dummy` licence
interface) is visible. It reads the namespace's own link list; note that
`/sys/class/net` inside the namespace still shows the host's NICs. It needs
`CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro`, and `CHIPSMITH` if the
`quartus-pro` chipsmith isn't on `PATH`.

`designs/blinky` (LED[0] at about 1.5 Hz from CLOCK1_50, KEY[0] resets)
**compiles offline to `.sof`** with the licence [obs]: fitter 1 h 14 min on
the loaded machine, then a separate `quartus_asm` (6 min; the flow's
`quartus_sta` step fails, see (f)). There is also a "use the Reset Release IP"
critical warning, which a real design should follow.

## Plan

**Phase 2 status** (branch `phase2`): step 1 done (g2): every `.rbf` hash
and CRC is reproducible. `.sof` re-signing works (g). Quartus's
own assembler and configuration-model Tcl APIs are open (h), so step 4
(config bits) becomes a dump rather than a fuzz. The `CORE` 2D compression
(step 2) is being decoded separately. Next: a full bit map from asm2, then
the routing graph.

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
