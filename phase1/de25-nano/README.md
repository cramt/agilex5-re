# DE25-Nano phase 1

Phase 1 of the DE25-Nano's HPS-first boot, as the board runs it: the SDM
firmware (CMF), the HPS I/O and EMIF (HPIO) and the HPS section (U-Boot SPL
and its handoff data). It is in the board's QSPI since 2026-11-12, and every
core chipsmith builds for the board is phase 2 of it.

- `hps.rbf`: `quartus_pfg -c hpsf.sof hpsf.rbf -o hps_path=spl11.hex -o hps=1`
  on Quartus Prime Pro 26.1.1's build of `designs/hpsf` (Terasic's GHRD HPS
  and EMIF settings, `HPS_INITIALIZATION "HPS FIRST"`; README, "The
  HPS-first split"). Its SPL is Terasic's U-Boot (github.com/terasic/
  u-boot-socfpga, tag `de25_nano_revA_v1.0`, d0f8813f, GPL-2.0+), stock
  defconfig, built with Arm GNU Toolchain 11.2-2022.02.
- `shell.txt`: the shell lines (`tools/hps_shell.sh`) a core is built on.
- `hps_io_hash`: phase 1's HPS IO hash (`quartus_pfg -i`), which a core
  must carry.

chipsmith puts another SPL into it (`chipsmith agilex5-phase1`, the HPS
section rebuilt; everything else stays these bytes) and packs it as the QSPI
`.jic`. The SDM firmware in it is Altera's, shipped on the same terms as
`quartus/` (chipsmith's docs/legal.md).
