# The Agilex 5 database on its own (db/: the A5E 013B's configuration bits,
# I/O, DSP and timing tables) plus keys/ (the keyed-hash and .sof HMAC keys
# every image needs). It's data that chipsmith reads (chipsmith ADR-0003),
# so taking it shouldn't mean building a5db.
{ lib, runCommand }:

runCommand "agilex5-db" {
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [ ../db ../keys ];
  };
} ''
  mkdir $out
  cp -r $src/db $src/keys $out/
  chmod -R u+w $out
  # The tables git keeps as .tsv.xz, decompressed once here: readers take a
  # table or its .xz, and io96's 300 MB layout costs chipsmith ~20 s per
  # build to decompress (pure-Rust LZMA, ~15 MB/s).
  find $out -name '*.xz' -exec xz -d {} +
''
