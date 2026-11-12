# What an image needs from Quartus Prime Pro 26.1.1

Just the files the image writer reads (chipsmith-agilex5's `rbf::install`),
in the install's own layout, so this directory stands in for an install:

- `devices/programmer/firmware/sm4revb/SHA_384/`: the nine SDM firmware
  modules a DE25-Nano image carries, unzipped from `agilex5e.zip`. Altera
  signs them; every Quartus-built bitstream for this die carries them, and
  checked against `specimens/hps-first/out/spl11/hpsf.core.rbf` all but one
  1 KiB block of each appears in it verbatim. Shipped as that bitstream
  content is, the way public FPGA projects ship `.sof` and `.rbf` files:
  tolerated practice, not something Quartus' licence grants (chipsmith
  docs/legal.md).
- `devices/programmer/sm4revb_npcf.ddb` (codebooks), the
  `fm_sector_bnxplv_rif_t*.ddb` flag lists, `devices/10nm/sm4revb/sof_meta.pb`
  and `programming_sequence.pb.enc`: data, like `db/`.

With these, chipsmith's writer gives the same bytes as from the install
(its `tests/image.rs`, 2026-11-12).
