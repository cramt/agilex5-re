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
- Where the routing graph lives is [unk]. No `routing`/`icname` ddb exists
  per die. Candidates are `asmm`, `.mdb`, `gid_database` and the
  `agilex_common` package.

### (c) `PDB_ASCII_DUMP` still works

- [obs] `PDB_SEGMENT_IMPL::PDB_SEGMENT_IMPL` in `libccl_pdb.so` does
  `this[0x58] = cfg_is_on("PDB_ASCII_DUMP")` (the string is built inline
  from two immediates at file offset 0x28ea9, which is why `strings` only
  shows `PDB_ASCI`).
- [obs] With `PDB_ASCII_DUMP=on` in `quartus.ini` in the working directory
  (Quartus prints `Info: Using INI file …`), every database a tool reads is
  traced to **`<database>.ddb.dmp` next to the database itself**, not to
  stdout as in Quartus 13. The format is the same annotated one: value,
  raw delta, C++ member name. For example
  `ptr.m_ids_by_string … = "DF35"` in the sm4revb package dump. A
  `quartus_syn` run plus one `get_part_info` call wrote 9 dumps (about
  290 MB, mostly `ddb_cumulative_info`).
- Consequence: the dumps land in the install tree. Move them to
  `specimens/pdbdump/` after each run, as was done here. A bind-mounted
  overlay of `devices/` would be cleaner.

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

## Oracle

`tools/quartus.sh <tool> <args…>` runs one Quartus Pro tool through
chipsmith inside a fresh user+network namespace. It refuses to run if any
interface besides `lo` (and the optional unaddressed `dummy` licence
interface) is visible. It reads the namespace's own link list; note that
`/sys/class/net` inside the namespace still shows the host's NICs. It needs
`CHIPSMITH_INSTALL_ROOT=/pool/dev/altera_pro`, and `CHIPSMITH` if the
`quartus-pro` chipsmith isn't on `PATH`.

`designs/blinky` (LED[0] at about 1.5 Hz from CLOCK1_50, KEY[0] resets)
**synthesises offline** [obs: 26 logic cells, 2 in, 1 out]. It stops at the
Fitter for lack of the licence. There is also a "use the Reset Release IP"
critical warning, which a real design should follow.

## Plan

0. **Licence** (user action): get the no-cost Agilex 5 licence, then
   compile blinky through to `.sof`/`.rbf` with `ORACLE_LICENSE_MAC` set.
   That is the first real oracle output.
1. **Container**: parse a real `.rbf` against the Security TRM (sections,
   descriptors, CMF, hashes). Split it into Altera-signed firmware and
   fabric sections. Check how stable the firmware is across designs and
   options.
2. **Reader**: teach cyclone2-re's `tools/ddb` (or a fork here) the v25
   header and raw-LZMA1 bodies, and check the result against the `.ddb.dmp`
   traces the way cyclone2-re did.
3. **Device model for sm4revb**: package/pad map (`pkg`, `pad`), then find
   the routing graph and the `asmm` configuration database. Decode
   `sof_meta.pb` for the sector and frame layout.
4. **bcm2bits**: find where Quartus decrypts `.bcm.xml` (REA on the loader
   library). If the plaintext is the block bit map, it seeds LAB/MLAB/M20K
   configuration.
5. **Fuzzing**: pair-diff the fabric sections per feature (LUT, FF,
   routing, I/O), as for Cyclone II. Expect per-sector hashes to change
   with any bit, so diff after stripping them.
6. **Writer and loader**: emit fabric sections with correct hashes, splice
   in the user's own Altera-signed CMF, and load with openFPGALoader after
   adding the A5EB013B IDCODE. On-board testing needs the user.
7. **Open flow**: yosys and nextpnr (himbaechel), fed from `db/`, the same
   way chipsmith `open` works for Cyclone II.

## License

MIT.
