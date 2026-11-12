# sm4revb routing graph

The routing graph `tools/rgdump/rgdump.sh` dumps from Quartus Prime Pro
26.1.1 (2026-10-31), plus `a5db usable`'s `usable.bin` (2026-11-01),
xz-compressed: 41 MB instead of 682 MB. Shipped as interoperability data, like
the database and keys (chipsmith docs/legal.md). The flake's `graph` package
decompresses it into the layout `specimens/rg/full` has, which a5pnr and a5db
(and chipsmith's `CHIPSMITH_AGILEX5_GRAPH`) read. Regenerate with rgdump.sh,
then `a5db usable`, and compress with `xz -6`.
