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
''
