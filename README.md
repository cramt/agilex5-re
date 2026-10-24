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

  Each section's first two 4 KiB blocks end in a CRC-32 (MSB first, poly
  0x04C11DB7, init and xorout 0xFFFFFFFF, i.e. CRC-32/BZIP2) over
  `[block, block+0xFFC)`. The pfg log's two "Keyed hash" values are at
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
