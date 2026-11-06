# openFPGALoader with the DE25-Nano's A5EB013B in its part table (IDCODE
# 0x4362c0dd as the board reports it (README (n): the image header holds
# 0xc362c0dd, the version nibble differs); IR length 10 as the A5EC008B's)
# and a "de25-nano" cable (09fb:6026, ADBUS4 enables JTAG). Upstream only knows
# the A5EC008B (src/part.hpp, checked against master on 2026-11-04; no
# upstream issue or PR filed, trabucayre/openFPGALoader). Remove
# this override once nixpkgs' openfpgaloader lists 0x4362c0dd and the DE25-Nano cable; until then the
# board test (tools/board/run.sh) builds it with
#   nix build --impure --expr '(builtins.getFlake "nixpkgs").legacyPackages.x86_64-linux.callPackage ./tools/board/openfpgaloader.nix {}'
{ openfpgaloader }:
openfpgaloader.overrideAttrs (old: {
  patches = (old.patches or [ ]) ++ [ ./openfpgaloader-a5eb013b.patch ];
})
